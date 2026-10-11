//! First-party update checks and user-confirmed installer download.
//!
//! [`Updates::check`] reads the fixed release feed
//! `https://apps.zombie.digital/downloads/jawjack/latest.json` on a worker
//! thread; [`Updates::poll`] collects the result without blocking the UI.
//! The app may check automatically ([`Updates::auto_check_due`]: 30 s after
//! startup, then every 6 hours, unless [`Updates::enabled`] is off), but an
//! installer is only downloaded by [`Updates::start_download`] and only
//! started by [`Updates::install`], both of which the user must ask for.
//!
//! Feed schema (shared with Haiwire):
//! `{version, channel, publishedAt, notes, setup: {file, size, sha256}}`.
//! `version` must be strict SemVer and `channel` one of preview/beta/stable.
//! Any URLs in the feed are ignored: the installer location is derived from
//! the fixed archive base and the validated version
//! (`/downloads/jawjack/archive/{version}/Jawjack-{version}-setup.exe`). The
//! feed supplies only the expected size and lowercase SHA-256, which the
//! download must match while streaming, and again after it is reopened
//! deny-write and immediately before launch. A missing feed (HTTP 404) means
//! no release is published yet and is not an error. Redirects are disabled
//! and every response is size- and time-bounded.
//!
//! Installer contract (Jawjack's setup program does not exist yet): the
//! installer is started as `Jawjack-{version}-setup.exe --app-pid <pid>`,
//! where `<pid>` is this process. It must ask that process to close (or wait
//! for it to exit) before replacing files, keep user settings, and be able to
//! outlive the app: it is spawned with `CREATE_BREAKAWAY_FROM_JOB`, falling
//! back to plain flags when the job forbids breakaway. Installing is
//! Windows-only; other platforms get an explanatory error.
use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

pub const RELEASE_PAGE: &str = "https://apps.zombie.digital/jawjack/releases";
const UPDATE_HOST: &str = "apps.zombie.digital";
const FEED_PATH: &str = "/downloads/jawjack/latest.json";
/// Installers live in the immutable per-version archive.
const ARCHIVE_PATH: &str = "/downloads/jawjack/archive";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_FEED_BYTES: usize = 64 * 1024;
const MAX_VERSION_LENGTH: usize = 128;
const MAX_CHANNEL_LENGTH: usize = 32;
/// Hard cap on an installer download, independent of the feed's size.
const MAX_INSTALLER_BYTES: u64 = 256 * 1024 * 1024;
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Debug, Eq, PartialEq)]
enum Status {
    Idle,
    Checking,
    Current,
    Newer,
    Missing,
    Error(String),
}

/// Expected installer identity published beside a release.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Installer {
    file_name: String,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Release {
    version: String,
    newer: bool,
    installer: Option<Installer>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CheckResult {
    Release(Release),
    Missing,
}

enum Install {
    Idle,
    Downloading {
        received: u64,
        total: u64,
    },
    /// Verified installer, held open with a deny-write handle until launch.
    Ready {
        path: PathBuf,
        file: File,
    },
    Launched,
    Failed(String),
}

enum DownloadEvent {
    Progress(u64),
    Done(Result<(PathBuf, File), String>),
}

/// UI-owned update state. Call [`Updates::poll`] from the UI loop.
pub struct Updates {
    /// Automatic checks; persisted by the app. Manual checks always work.
    pub enabled: bool,
    status: Status,
    latest: Option<String>,
    installer: Option<Installer>,
    install: Install,
    started: Instant,
    last_check: Option<Instant>,
    check_rx: Option<Receiver<Result<CheckResult, String>>>,
    download_rx: Option<Receiver<DownloadEvent>>,
}

impl Default for Updates {
    fn default() -> Self {
        Self::new()
    }
}

impl Updates {
    pub fn new() -> Self {
        Self {
            enabled: true,
            status: Status::Idle,
            latest: None,
            installer: None,
            install: Install::Idle,
            started: Instant::now(),
            last_check: None,
            check_rx: None,
            download_rx: None,
        }
    }

    pub fn current_version(&self) -> &'static str {
        CURRENT_VERSION
    }

    /// True when the periodic policy wants a check now: 30 s after startup,
    /// then every 6 hours. Never while busy or once an installer is ready.
    pub fn auto_check_due(&self) -> bool {
        if !self.enabled
            || self.busy()
            || matches!(self.install, Install::Ready { .. } | Install::Launched)
        {
            return false;
        }
        match self.last_check {
            Some(at) => at.elapsed() >= CHECK_INTERVAL,
            None => self.started.elapsed() >= FIRST_CHECK_DELAY,
        }
    }

    /// Start one check on a worker thread. Returns false while a check or a
    /// download is active.
    pub fn check(&mut self) -> bool {
        if self.busy() {
            return false;
        }
        self.status = Status::Checking;
        self.latest = None;
        self.installer = None;
        self.install = Install::Idle;
        self.last_check = Some(Instant::now());
        let (tx, rx) = mpsc::channel();
        self.check_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(fetch_feed());
        });
        true
    }

    /// Collect worker results. Returns true when visible state changed.
    pub fn poll(&mut self) -> bool {
        let checked = self.poll_check();
        let downloaded = self.poll_download();
        checked || downloaded
    }

    fn poll_check(&mut self) -> bool {
        let Some(rx) = &self.check_rx else {
            return false;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err("The update check stopped unexpectedly.".into()),
        };
        self.check_rx = None;
        match result {
            Ok(CheckResult::Release(release)) => {
                self.status = if release.newer {
                    Status::Newer
                } else {
                    Status::Current
                };
                self.installer = release.installer.filter(|_| release.newer);
                self.latest = Some(release.version);
            }
            Ok(CheckResult::Missing) => {
                self.latest = None;
                self.status = Status::Missing;
            }
            Err(error) => {
                self.latest = None;
                self.status = Status::Error(error);
            }
        }
        true
    }

    fn poll_download(&mut self) -> bool {
        let Some(rx) = &self.download_rx else {
            return false;
        };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok(DownloadEvent::Progress(value)) => {
                    if let Install::Downloading { received, .. } = &mut self.install {
                        *received = value;
                        changed = true;
                    }
                }
                Ok(DownloadEvent::Done(result)) => {
                    self.install = match result {
                        Ok((path, file)) => Install::Ready { path, file },
                        Err(error) => Install::Failed(error),
                    };
                    self.download_rx = None;
                    return true;
                }
                Err(TryRecvError::Empty) => return changed,
                Err(TryRecvError::Disconnected) => {
                    self.install =
                        Install::Failed("The installer download stopped unexpectedly.".into());
                    self.download_rx = None;
                    return true;
                }
            }
        }
    }

    pub fn checking(&self) -> bool {
        self.status == Status::Checking
    }

    fn downloading(&self) -> bool {
        matches!(self.install, Install::Downloading { .. })
    }

    /// A check or a download is in flight.
    pub fn busy(&self) -> bool {
        self.checking() || self.downloading()
    }

    /// A verified installer is held and can be started with [`Updates::install`].
    pub fn ready(&self) -> bool {
        matches!(self.install, Install::Ready { .. })
    }

    /// The newer published version, if one is available.
    pub fn available(&self) -> Option<String> {
        (self.status == Status::Newer)
            .then(|| self.latest.clone())
            .flatten()
    }

    pub fn status_label(&self) -> String {
        match &self.status {
            Status::Idle => "Check for updates".into(),
            Status::Checking => "Checking for updates…".into(),
            Status::Current => "Up to date".into(),
            Status::Newer => "Update available".into(),
            Status::Missing => "No releases published yet".into(),
            Status::Error(_) => "Update check failed".into(),
        }
    }

    pub fn detail(&self) -> String {
        let version = self.latest.as_deref().unwrap_or("the latest release");
        match &self.status {
            Status::Idle if self.enabled => {
                "Jawjack checks for updates shortly after starting and every 6 hours.".into()
            }
            Status::Idle => "Automatic update checks are off.".into(),
            Status::Checking => "Checking published releases…".into(),
            Status::Current => format!("v{CURRENT_VERSION} is the latest published release."),
            Status::Newer => match &self.install {
                Install::Idle if self.installer.is_some() => format!(
                    "v{version} is available. Review its release notes, then download and \
                     install it here. Nothing is downloaded until you ask."
                ),
                Install::Idle => {
                    format!("v{version} is available. Get it from the release page.")
                }
                Install::Downloading { received, total } => format!(
                    "Downloading the v{version} installer… {} of {}.",
                    megabytes(*received),
                    megabytes(*total)
                ),
                Install::Ready { .. } => format!(
                    "The v{version} installer was downloaded and its SHA-256 verified. \
                     Installing asks Jawjack to close first. Your settings are kept."
                ),
                Install::Launched => {
                    "Setup started. Follow its prompts; it asks Jawjack to close before \
                     installing."
                        .into()
                }
                Install::Failed(error) => error.clone(),
            },
            Status::Missing => "No Jawjack release has been published yet.".into(),
            Status::Error(message) => message.clone(),
        }
    }

    /// Label for the install action, or `None` when no verified installer is
    /// published for an available version.
    pub fn install_label(&self) -> Option<String> {
        if self.status != Status::Newer || self.installer.is_none() {
            return None;
        }
        let version = self.latest.as_deref().unwrap_or("the latest release");
        Some(match &self.install {
            Install::Idle => format!("Download and install v{version}"),
            Install::Downloading { received, total } => {
                format!(
                    "Downloading… {}%",
                    received.saturating_mul(100) / (*total).max(1)
                )
            }
            Install::Ready { .. } => format!("Install v{version} now"),
            Install::Launched => "Setup started".into(),
            Install::Failed(_) => "Retry download".into(),
        })
    }

    /// Start downloading the installer for the available version. Returns
    /// false when none is published or one is already in progress or ready.
    pub fn start_download(&mut self) -> bool {
        if self.status != Status::Newer
            || matches!(
                self.install,
                Install::Downloading { .. } | Install::Ready { .. } | Install::Launched
            )
        {
            return false;
        }
        let (Some(installer), Some(version)) = (self.installer.clone(), self.latest.clone()) else {
            return false;
        };
        self.install = Install::Downloading {
            received: 0,
            total: installer.size,
        };
        let (tx, rx) = mpsc::channel();
        self.download_rx = Some(rx);
        std::thread::spawn(move || {
            let progress = tx.clone();
            let result = download(&version, &installer, &mut |received| {
                let _ = progress.send(DownloadEvent::Progress(received));
            });
            let _ = tx.send(DownloadEvent::Done(result));
        });
        true
    }

    /// Re-verify the held installer and start it. Returns the installer path.
    /// The app should then close (the installer also asks it to).
    pub fn install(&mut self) -> Result<PathBuf, String> {
        let (Install::Ready { path, file }, Some(installer)) = (&self.install, &self.installer)
        else {
            return Err("No verified installer is ready.".into());
        };
        let path = path.clone();
        let result = verify_held(file, installer).and_then(|()| spawn_installer(&path));
        match result {
            Ok(()) => {
                // Dropping the held handle after CreateProcess is safe: the
                // running installer's image is mapped and locked.
                self.install = Install::Launched;
                Ok(path)
            }
            Err(error) => {
                self.install = Install::Failed(error.clone());
                Err(error)
            }
        }
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

// ------------------------------------------------------------------- feed

fn client(timeout: Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Jawjack/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "Could not initialize update networking.".into())
}

fn get(timeout: Duration, path: &str) -> Result<reqwest::blocking::Response, String> {
    client(timeout)?
        .get(format!("https://{UPDATE_HOST}{path}"))
        .header("Cache-Control", "no-cache")
        .send()
        .map_err(|error| {
            if error.is_timeout() {
                "The update service did not respond in time.".to_owned()
            } else {
                "Could not reach the update service.".to_owned()
            }
        })
}

fn fetch_feed() -> Result<CheckResult, String> {
    let response = get(CHECK_TIMEOUT, FEED_PATH)?;
    let status = response.status().as_u16();
    let mut bytes = Vec::with_capacity(4096);
    if status == 200 {
        response
            .take(MAX_FEED_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "The update response could not be read.".to_owned())?;
    }
    parse_feed(status, &bytes)
}

fn parse_feed(status: u16, bytes: &[u8]) -> Result<CheckResult, String> {
    match status {
        200 => {}
        404 => return Ok(CheckResult::Missing),
        _ => return Err(format!("The update service returned HTTP {status}.")),
    }
    if bytes.len() > MAX_FEED_BYTES {
        return Err("The update response exceeded the 64 KiB limit.".into());
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "The update response was not valid JSON.".to_owned())?;
    let mut release = parse_release(&value)?;
    release.newer = version_is_newer(&release.version, CURRENT_VERSION)?;
    Ok(CheckResult::Release(release))
}

fn parse_release(value: &Value) -> Result<Release, String> {
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| "The release response did not contain a version.".to_owned())?;
    if version.is_empty() || version.len() > MAX_VERSION_LENGTH {
        return Err("The published version exceeded its size limit.".into());
    }
    let channel = value
        .get("channel")
        .and_then(Value::as_str)
        .ok_or_else(|| "The release response did not contain a channel.".to_owned())?;
    if channel.len() > MAX_CHANNEL_LENGTH || !matches!(channel, "preview" | "beta" | "stable") {
        return Err("The update service returned an unsupported release channel.".into());
    }
    Version::parse(version)
        .map_err(|_| "The published version was not valid SemVer.".to_owned())?;
    Ok(Release {
        version: version.to_owned(),
        newer: false,
        installer: parse_installer(value.get("setup"), version),
    })
}

fn installer_file_name(version: &str) -> String {
    format!("Jawjack-{version}-setup.exe")
}

/// Accept installer metadata only when it names exactly the derived file and
/// carries a bounded size and a lowercase SHA-256. The version must be safe in
/// a URL path (SemVer without build metadata). Anything else means "no in-app
/// install"; the release page remains available.
fn parse_installer(value: Option<&Value>, version: &str) -> Option<Installer> {
    let value = value?;
    if !version
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return None;
    }
    let file_name = installer_file_name(version);
    if value.get("file").and_then(Value::as_str) != Some(file_name.as_str()) {
        return None;
    }
    let size = value
        .get("size")
        .and_then(Value::as_u64)
        .filter(|size| (1..=MAX_INSTALLER_BYTES).contains(size))?;
    let sha256 = value
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })?
        .to_owned();
    Some(Installer {
        file_name,
        size,
        sha256,
    })
}

fn installer_url_path(version: &str, installer: &Installer) -> String {
    format!("{ARCHIVE_PATH}/{version}/{}", installer.file_name)
}

fn version_is_newer(candidate: &str, current: &str) -> Result<bool, String> {
    let current = Version::parse(current)
        .map_err(|_| "This build has an invalid version string.".to_owned())?;
    let candidate = Version::parse(candidate)
        .map_err(|_| "The published version was not valid SemVer.".to_owned())?;
    Ok(candidate.cmp_precedence(&current).is_gt())
}

// ------------------------------------------------------------- installer

/// A fresh private download folder under the user's temp directory.
fn download_directory() -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let directory = std::env::temp_dir().join(format!(
        "jawjack-update-{}-{:x}",
        std::process::id(),
        nanos & 0xffff_ffff_ffff
    ));
    std::fs::create_dir(&directory)
        .map_err(|_| "The installer could not be saved to the temporary folder.".to_owned())?;
    Ok(directory)
}

fn download(
    version: &str,
    installer: &Installer,
    progress: &mut dyn FnMut(u64),
) -> Result<(PathBuf, File), String> {
    let response = get(DOWNLOAD_TIMEOUT, &installer_url_path(version, installer))?;
    match response.status().as_u16() {
        200 => {}
        404 => {
            return Err(
                "The installer for this release is not published. Use the release page instead."
                    .into(),
            );
        }
        status => return Err(format!("The installer download failed (HTTP {status}).")),
    }
    save_installer(response, installer, progress)
}

/// Stream `source` to disk, verifying size and SHA-256 as it arrives, then
/// reopen the file deny-write and verify it again.
fn save_installer(
    source: impl Read,
    installer: &Installer,
    progress: &mut dyn FnMut(u64),
) -> Result<(PathBuf, File), String> {
    use std::io::Write;

    let directory = download_directory()?;
    let partial = directory.join(format!("{}.partial", installer.file_name));
    let target = directory.join(&installer.file_name);
    let result = (|| {
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .map_err(|_| "The installer could not be saved to the temporary folder.".to_owned())?;
        let mut source = source.take(installer.size.min(MAX_INSTALLER_BYTES) + 1);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut received = 0u64;
        loop {
            let read = source.read(&mut buffer).map_err(|_| {
                "The installer download was interrupted or exceeded its time limit.".to_owned()
            })?;
            if read == 0 {
                break;
            }
            received += read as u64;
            if received > installer.size {
                return Err(
                    "The installer download was larger than the published size and was discarded."
                        .into(),
                );
            }
            hasher.update(&buffer[..read]);
            output
                .write_all(&buffer[..read])
                .map_err(|_| "The installer could not be written to disk.".to_owned())?;
            progress(received);
        }
        output
            .sync_all()
            .map_err(|_| "The installer could not be written to disk.".to_owned())?;
        drop(output);
        if received != installer.size {
            return Err(format!(
                "The installer download was incomplete ({} of {}).",
                megabytes(received),
                megabytes(installer.size)
            ));
        }
        if hex(&hasher.finalize()) != installer.sha256 {
            return Err(
                "The downloaded installer did not match the published SHA-256 and was discarded."
                    .into(),
            );
        }
        std::fs::rename(&partial, &target)
            .map_err(|_| "The installer could not be saved to the temporary folder.".to_owned())?;
        let held = open_deny_write(&target)
            .map_err(|_| "The downloaded installer could not be reopened.".to_owned())?;
        verify_held(&held, installer)?;
        Ok((target.clone(), held))
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&directory);
    }
    result
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn open_deny_write(path: &Path) -> std::io::Result<File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x0000_0001) // FILE_SHARE_READ only: no writer can swap it.
            .open(path)
    }
    #[cfg(not(windows))]
    {
        File::open(path)
    }
}

fn verify_held(file: &File, installer: &Installer) -> Result<(), String> {
    use std::io::{Seek, SeekFrom};
    let mut file = file;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "The downloaded installer could not be verified.".to_owned())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| "The downloaded installer could not be verified.".to_owned())?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > installer.size {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    if total != installer.size || hex(&hasher.finalize()) != installer.sha256 {
        return Err(
            "The downloaded installer changed after verification and was not started.".into(),
        );
    }
    Ok(())
}

#[cfg(windows)]
fn spawn_installer(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let directory = path
        .parent()
        .ok_or_else(|| "The installer could not be started.".to_owned())?;
    let spawn = |flags| {
        std::process::Command::new(path)
            .arg("--app-pid")
            .arg(std::process::id().to_string())
            .current_dir(directory)
            .creation_flags(flags)
            .spawn()
    };
    // Setup must survive Jawjack closing, even inside a job.
    spawn(CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(0))
        .map(|_| ())
        .map_err(|error| format!("The installer could not be started ({error})."))
}

#[cfg(not(windows))]
fn spawn_installer(_path: &Path) -> Result<(), String> {
    Err(format!(
        "Installing updates in the app is available on Windows only. Download the release from {RELEASE_PAGE}."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(version: &str, channel: &str) -> Vec<u8> {
        format!(r#"{{"version":"{version}","channel":"{channel}"}}"#).into_bytes()
    }

    fn release(value: CheckResult) -> Release {
        match value {
            CheckResult::Release(release) => release,
            CheckResult::Missing => panic!("expected a release"),
        }
    }

    #[test]
    fn compares_semver_precedence() {
        assert!(version_is_newer("0.2.0", "0.1.0").unwrap());
        assert!(!version_is_newer("0.1.0", "0.1.0").unwrap());
        assert!(!version_is_newer("0.0.9", "0.1.0").unwrap());
        assert!(version_is_newer("1.0.0-preview.10", "1.0.0-preview.9").unwrap());
        assert!(version_is_newer("1.0.0", "1.0.0-preview.9").unwrap());
        assert!(!version_is_newer("0.1.0-beta.9", "0.1.0-preview.2").unwrap());
        assert!(!version_is_newer("1.0.0+feed.2", "1.0.0+feed.1").unwrap());
        assert!(version_is_newer("latest", "0.1.0").is_err());
        assert!(version_is_newer("v1.0.0", "0.1.0").is_err());
    }

    #[test]
    fn parses_feed_against_current_version() {
        let newer = release(parse_feed(200, &feed("999.0.0", "stable")).unwrap());
        assert!(newer.newer);
        let same = release(parse_feed(200, &feed(CURRENT_VERSION, "preview")).unwrap());
        assert!(!same.newer);
        assert!(parse_feed(200, &feed("0.2.0", "nightly")).is_err());
        assert!(parse_feed(200, &feed("latest", "stable")).is_err());
        assert!(parse_feed(200, b"not json").is_err());
        assert!(parse_feed(200, &vec![b' '; MAX_FEED_BYTES + 1]).is_err());
    }

    #[test]
    fn missing_feed_is_not_an_error() {
        assert_eq!(parse_feed(404, b"").unwrap(), CheckResult::Missing);
        assert!(parse_feed(500, b"").is_err());
        assert!(parse_feed(302, b"").is_err());
    }

    #[test]
    fn installer_metadata_must_match_the_derived_file_and_hash() {
        let hash = "a".repeat(64);
        let body = |file: &str, size: u64, sha: &str| {
            format!(
                r#"{{"version":"0.2.0-preview.3","channel":"preview","publishedAt":"2026-10-11",
                "notes":"x","url":"https://untrusted.invalid/x.exe",
                "setup":{{"file":"{file}","size":{size},"sha256":"{sha}","url":"https://untrusted.invalid/x.exe"}}}}"#
            )
            .into_bytes()
        };
        let ok = release(
            parse_feed(200, &body("Jawjack-0.2.0-preview.3-setup.exe", 10, &hash)).unwrap(),
        );
        let installer = ok.installer.expect("installer");
        assert_eq!(installer.size, 10);
        // Feed URLs are ignored: the path is derived from the fixed base.
        assert_eq!(
            installer_url_path("0.2.0-preview.3", &installer),
            "/downloads/jawjack/archive/0.2.0-preview.3/Jawjack-0.2.0-preview.3-setup.exe"
        );
        let upper = "A".repeat(64);
        for (file, size, sha) in [
            ("evil.exe", 10, hash.as_str()),
            ("Haiwire-0.2.0-preview.3-setup.exe", 10, hash.as_str()),
            ("Jawjack-0.2.0-preview.3-setup.exe", 0, hash.as_str()),
            (
                "Jawjack-0.2.0-preview.3-setup.exe",
                MAX_INSTALLER_BYTES + 1,
                hash.as_str(),
            ),
            ("Jawjack-0.2.0-preview.3-setup.exe", 10, "abc"),
            ("Jawjack-0.2.0-preview.3-setup.exe", 10, upper.as_str()),
        ] {
            let parsed = release(parse_feed(200, &body(file, size, sha)).unwrap());
            assert!(parsed.installer.is_none(), "{file} {size} {sha}");
        }
        // Build metadata cannot be placed in the derived path.
        let build = format!(
            r#"{{"version":"0.2.0+x","channel":"stable","setup":{{"file":"Jawjack-0.2.0+x-setup.exe","size":1,"sha256":"{hash}"}}}}"#
        );
        let parsed = parse_release(&serde_json::from_str(&build).unwrap()).unwrap();
        assert!(parsed.installer.is_none());
    }

    #[test]
    fn saved_installer_verifies_size_and_hash() {
        let good = Installer {
            file_name: installer_file_name("9.9.9"),
            size: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
        };
        let (path, file) = save_installer(&b"abc"[..], &good, &mut |_| {}).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"abc");
        verify_held(&file, &good).unwrap();
        #[cfg(windows)]
        assert!(std::fs::write(&path, b"xyz").is_err());
        drop(file);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());

        let wrong_hash = Installer {
            sha256: "0".repeat(64),
            ..good.clone()
        };
        assert!(save_installer(&b"abc"[..], &wrong_hash, &mut |_| {}).is_err());
        let too_small = Installer {
            size: 2,
            ..good.clone()
        };
        assert!(save_installer(&b"abc"[..], &too_small, &mut |_| {}).is_err());
        let too_large = Installer { size: 4, ..good };
        assert!(save_installer(&b"abc"[..], &too_large, &mut |_| {}).is_err());
    }

    #[test]
    fn auto_check_policy() {
        let mut updates = Updates::new();
        assert!(!updates.auto_check_due());
        updates.started = Instant::now() - FIRST_CHECK_DELAY;
        assert!(updates.auto_check_due());
        updates.enabled = false;
        assert!(!updates.auto_check_due());
        updates.enabled = true;
        updates.last_check = Some(Instant::now());
        assert!(!updates.auto_check_due());
        updates.last_check = Instant::now().checked_sub(CHECK_INTERVAL);
        if updates.last_check.is_some() {
            assert!(updates.auto_check_due());
        }
        updates.status = Status::Checking;
        assert!(!updates.auto_check_due());
    }

    #[test]
    fn labels_follow_state() {
        let mut updates = Updates::new();
        assert_eq!(updates.current_version(), CURRENT_VERSION);
        assert!(updates.available().is_none());
        assert!(updates.install_label().is_none());
        updates.status = Status::Newer;
        updates.latest = Some("9.0.0".into());
        assert_eq!(updates.available().as_deref(), Some("9.0.0"));
        assert!(updates.install_label().is_none());
        updates.installer = Some(Installer {
            file_name: installer_file_name("9.0.0"),
            size: 100,
            sha256: "a".repeat(64),
        });
        assert_eq!(
            updates.install_label().unwrap(),
            "Download and install v9.0.0"
        );
        updates.install = Install::Downloading {
            received: 50,
            total: 100,
        };
        assert!(updates.busy());
        assert_eq!(updates.install_label().unwrap(), "Downloading… 50%");
        assert!(updates.install().is_err());
        updates.status = Status::Missing;
        assert_eq!(updates.status_label(), "No releases published yet");
    }
}
