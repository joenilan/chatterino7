//! First-party update checks with click-to-download and confirmed install
//! (product-neutral spec: Haiwire `docs/design/2026-10-12-update-flow.md`,
//! ported from Haiwire's `native_updates.rs`).
//!
//! Nothing downloads or installs without a click. With the Updates mode
//! **Notify me** (the default), the installed, default-profile copy reads the
//! release feed [`STARTUP_DELAY`] after startup and then every
//! [`POLL_INTERVAL`] (no backoff; a busy tick is skipped); **Off** reads it
//! only when asked ([`Updates::check_now`]). A newer version becomes
//! [`Phase::Available`] with its release notes and is announced once per
//! session ([`Updates::take_announcement`]). [`Updates::start_download`]
//! fetches and verifies the installer ([`Phase::Downloading`] then
//! [`Phase::Downloaded`]); [`Updates::request_install`] re-verifies it when the
//! user confirms, and [`Updates::launch_installer`] verifies once more and
//! starts setup. Quitting never installs. A failed download returns to
//! Available with a retry, a failed install start stays Downloaded with a
//! retry, and setup's own failure is reported on the next launch.
//!
//! Eligibility ([`Eligibility`]): only the running executable
//! `%LOCALAPPDATA%\Programs\Jawjack\Jawjack.exe` (case-insensitive) on the
//! default profile ([`crate::profile::name`] is `None`) checks by itself,
//! downloads or installs. Other copies (a `--profile dev` copy, a build output,
//! a portable copy) may check when asked but never replace the installed app.
//!
//! Feed (shared with Haiwire): the fixed
//! `https://apps.zombie.digital/downloads/jawjack/latest.json`, schema
//! `{version, channel, publishedAt, notes, setup: {file, size, sha256}}`.
//! `version` must be strict SemVer and `channel` one of preview/beta/stable.
//! Any URLs in the feed are ignored: the installer location is derived from
//! the fixed archive base and the validated version
//! (`/downloads/jawjack/archive/{version}/Jawjack-{version}-setup.exe`); the
//! feed supplies only the expected size and lowercase SHA-256. A missing feed
//! (HTTP 404) means no release is published and is not an error. Requests are
//! conditional (the last 200's `ETag` / `Last-Modified` are echoed, a 304
//! reuses the cached body); no `Cache-Control` is sent. Release notes come
//! from `notes.md` beside the feed only when its first heading names exactly
//! the version, otherwise from the feed's short `notes`; both are untrusted
//! plain text, bounded to 64 KiB, 80 lines and 400 characters per line.
//!
//! Transport: HTTPS to the fixed host only, OS certificate store (rustls with
//! native roots), no redirects, no cookie store, no automatic authentication;
//! every response is size- and time-bounded.
//!
//! Download: streamed into a fresh private `jawjack-update-*` temp folder with
//! the published size and SHA-256 enforced while streaming, renamed, reopened
//! deny-write and hashed again. The held file is hashed again on confirm and
//! right before launch; any mismatch fails closed and deletes the folder.
//!
//! Installer contract: setup is started as
//! `Jawjack-{version}-setup.exe --app-pid <pid> --quiet --relaunch
//! --result-file <data root>\update-result.json`. It waits for (or asks) this
//! process to exit, installs quietly keeping user settings, writes the result
//! file and relaunches Jawjack after success or failure. It is spawned with
//! `CREATE_BREAKAWAY_FROM_JOB`, falling back to plain flags when the job
//! forbids breakaway. Installing is Windows-only.
//!
//! Result file (written by setup, at most 8 KiB, UTF-8 JSON):
//! `{"version": "<semver being installed>", "ok": <bool>, "message": "<reason, empty on success>"}`.
//! The next launch reads it once and deletes it: a failure is reported as
//! "Update {version} was not installed" with the message; a success whose
//! version is this build is shown as "Updated to {version}"; a stale success
//! is ignored.
// Part of the API is for the update UI that is being rebuilt.

use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant, SystemTime},
};

pub const RELEASE_PAGE: &str = "https://apps.zombie.digital/apps/jawjack/releases";
const UPDATE_HOST: &str = "apps.zombie.digital";
const FEED_PATH: &str = "/downloads/jawjack/latest.json";
/// Release notes of the version `latest.json` names, beside it.
const NOTES_PATH: &str = "/downloads/jawjack/notes.md";
/// Installers live in the immutable per-version archive.
const ARCHIVE_PATH: &str = "/downloads/jawjack/archive";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_FEED_BYTES: usize = 64 * 1024;
const MAX_NOTES_BYTES: usize = 64 * 1024;
const MAX_NOTE_LINES: usize = 80;
const MAX_NOTE_CHARS: usize = 400;
const MAX_SUMMARY_CHARS: usize = 600;
/// ETag / Last-Modified values kept for conditional requests.
const MAX_VALIDATOR_LENGTH: usize = 256;
const MAX_VERSION_LENGTH: usize = 128;
const MAX_CHANNEL_LENGTH: usize = 32;
/// Hard cap on an installer download, independent of the feed's size.
const MAX_INSTALLER_BYTES: u64 = 256 * 1024 * 1024;
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// The first background check waits until startup work has settled.
pub const STARTUP_DELAY: Duration = Duration::from_secs(15);
/// Background checks repeat this often. A failed or skipped check is simply
/// tried again at the next tick: no backoff, no extra requests.
pub const POLL_INTERVAL: Duration = Duration::from_secs(4 * 60);
/// Setup's outcome (`--result-file`), in Jawjack's data folder.
const RESULT_FILE: &str = "update-result.json";
const MAX_RESULT_BYTES: u64 = 8 * 1024;
const MAX_RESULT_MESSAGE: usize = 600;
const DOWNLOAD_FOLDER_PREFIX: &str = "jawjack-update-";

// ------------------------------------------------------------------ mode

/// Settings › Updates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum UpdateMode {
    /// Check in the background and say when a new version is available.
    /// Downloading and installing always wait for a click.
    #[default]
    Notify,
    /// Check only when asked.
    Off,
}

impl UpdateMode {
    pub const ALL: [Self; 2] = [Self::Notify, Self::Off];

    /// Saved key ("notify" / "off").
    pub fn key(self) -> &'static str {
        match self {
            Self::Notify => "notify",
            Self::Off => "off",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Notify => "Notify me",
            Self::Off => "Off",
        }
    }

    /// Saved keys, including modes that once downloaded or installed by
    /// themselves ("automatic", "check_only"): they load as Notify me.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "notify" | "automatic" | "check_only" => Some(Self::Notify),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    /// The saved mode from Jawjack's settings: `update_mode` when valid,
    /// else the old `update_checks` switch (true → Notify me, false → Off),
    /// else Notify me.
    pub fn load(saved: &Value) -> Self {
        saved["update_mode"]
            .as_str()
            .and_then(Self::from_key)
            .or_else(|| {
                saved["update_checks"]
                    .as_bool()
                    .map(|on| if on { Self::Notify } else { Self::Off })
            })
            .unwrap_or_default()
    }

    fn checks_automatically(self) -> bool {
        self == Self::Notify
    }
}

// ----------------------------------------------------------- eligibility

/// Whether this running copy may check by itself, download and install.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Eligibility {
    /// The installed `Jawjack.exe` on the default profile.
    Installed,
    /// `--profile NAME` (for example the dev profile).
    SeparateProfile,
    /// Not the installed `Jawjack.exe` (a build output or portable copy).
    NotInstalled,
}

impl Eligibility {
    pub fn current() -> Self {
        let installed = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|base| base.is_absolute())
            .map(|base| base.join("Programs").join("Jawjack").join("Jawjack.exe"));
        let exe = std::env::current_exe().ok();
        Self::classify(
            crate::profile::name().is_some(),
            exe.as_deref(),
            installed.as_deref(),
        )
    }

    /// A separate profile always excludes updates; otherwise the running
    /// executable must be the installed one.
    fn classify(profile: bool, exe: Option<&Path>, installed: Option<&Path>) -> Self {
        if profile {
            return Self::SeparateProfile;
        }
        match (exe, installed) {
            (Some(exe), Some(installed)) if same_file_path(exe, installed) => Self::Installed,
            _ => Self::NotInstalled,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::SeparateProfile => "separate_profile",
            Self::NotInstalled => "not_installed",
        }
    }

    /// Why this copy never updates itself, or `None` when it does.
    pub fn reason(self) -> Option<&'static str> {
        match self {
            Self::Installed => None,
            Self::SeparateProfile => Some(
                "This copy uses a separate profile, so it never updates itself or the installed \
                 Jawjack.",
            ),
            Self::NotInstalled => Some("This is a development copy; install Jawjack to update it."),
        }
    }
}

fn same_file_path(a: &Path, b: &Path) -> bool {
    let normalize = |path: &Path| {
        let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let text = resolved.to_string_lossy().into_owned();
        text.strip_prefix(r"\\?\").unwrap_or(&text).to_lowercase()
    };
    normalize(a) == normalize(b)
}

// --------------------------------------------------------- install result

fn result_path() -> Option<PathBuf> {
    crate::profile::data_root().map(|root| root.join(RESULT_FILE))
}

/// The outcome setup reported for the previous launch's update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallResult {
    pub ok: bool,
    /// Empty when setup did not name a valid version.
    pub version: String,
    pub message: String,
}

/// Read and remove setup's bounded result file.
fn take_install_result(path: &Path) -> Option<InstallResult> {
    let metadata = std::fs::metadata(path).ok()?;
    let bytes = (metadata.len() <= MAX_RESULT_BYTES)
        .then(|| std::fs::read(path).ok())
        .flatten();
    let _ = std::fs::remove_file(path);
    parse_install_result(&bytes?)
}

fn parse_install_result(bytes: &[u8]) -> Option<InstallResult> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let ok = value.get("ok").and_then(Value::as_bool)?;
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| version.len() <= MAX_VERSION_LENGTH && Version::parse(version).is_ok())
        .unwrap_or_default()
        .to_owned();
    let message: String = value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_RESULT_MESSAGE)
        .collect();
    // A success that did not produce this build is stale; say nothing.
    if ok && version != CURRENT_VERSION {
        return None;
    }
    Some(InstallResult {
        ok,
        version,
        message,
    })
}

fn failure_text(result: &InstallResult) -> String {
    let update = if result.version.is_empty() {
        "The update".to_owned()
    } else {
        format!("Update {}", result.version)
    };
    let reason = if result.message.is_empty() {
        "Setup did not finish.".to_owned()
    } else {
        result.message.clone()
    };
    format!("{update} was not installed, so this version was kept. {reason}")
}

// ----------------------------------------------------------------- state

/// Where the update flow is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Phase {
    /// Nothing checked yet this session.
    #[default]
    Idle,
    Checking,
    UpToDate,
    /// The feed answered 404: no release is published.
    Unpublished,
    /// A newer version is known; nothing is downloaded. A failed download
    /// also lands here, with [`ErrorContext::Download`].
    Available,
    Downloading,
    /// A verified installer is held. A failed install start stays here with
    /// [`ErrorContext::Install`].
    Downloaded,
    /// Setup was started; Jawjack is exiting.
    Installing,
    /// The check failed ([`ErrorContext::Check`]); the next tick retries.
    Error,
}

impl Phase {
    pub fn key(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Checking => "checking",
            Self::UpToDate => "up-to-date",
            Self::Unpublished => "unpublished",
            Self::Available => "available",
            Self::Downloading => "downloading",
            Self::Downloaded => "downloaded",
            Self::Installing => "installing",
            Self::Error => "error",
        }
    }
}

/// Which step failed, for the retry action and its copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorContext {
    Check,
    Download,
    Install,
}

impl ErrorContext {
    pub fn key(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Download => "download",
            Self::Install => "install",
        }
    }
}

/// One line of release notes, as plain text (shown, never followed).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoteLine {
    Heading(String),
    Bullet(String),
    Text(String),
}

/// Something to announce once (toast or Windows notification).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Announcement {
    /// A check found a version not announced before in this session.
    Available(String),
    /// The installer for this version finished downloading and verified.
    Downloaded(String),
}

/// Snapshot of the update flow for the UI (T3 `DesktopUpdateState`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateView {
    pub phase: Phase,
    pub current_version: &'static str,
    /// The newer version the flow is about (installing, downloaded, else
    /// available).
    pub available_version: Option<String>,
    pub downloaded_version: Option<String>,
    /// 0–100 while downloading; 100 once downloaded or installing.
    pub download_percent: Option<u64>,
    pub notes: Vec<NoteLine>,
    /// Published installer size in bytes.
    pub size: Option<u64>,
    pub error: Option<(ErrorContext, String)>,
    /// The failed step can be retried with a click.
    pub can_retry: bool,
    /// This copy may download and install.
    pub can_download: bool,
    pub checked_at: Option<SystemTime>,
    /// The previous launch's failed install, until a new download starts.
    pub install_failure: Option<String>,
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
    /// latest.json's short `notes` summary, used when notes.md is missing.
    summary: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CheckResult {
    Release(Release),
    Missing,
}

/// The last 200 response of the feed and its validators.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FeedCache {
    etag: Option<String>,
    last_modified: Option<String>,
    body: Vec<u8>,
}

struct CheckOutcome {
    result: Result<CheckResult, String>,
    /// Replaces the feed cache when present.
    cache: Option<FeedCache>,
    /// Release notes for the newer version, when found.
    notes: Option<(String, Vec<NoteLine>)>,
}

#[derive(Clone, Debug)]
struct AvailableRelease {
    version: String,
    installer: Option<Installer>,
    notes: Vec<NoteLine>,
}

/// A verified installer, held open with a deny-write handle until launch.
struct Downloaded {
    version: String,
    installer: Installer,
    path: PathBuf,
    file: File,
}

enum DownloadEvent {
    Progress(u64),
    Done(Result<(PathBuf, File), String>),
}

/// UI-owned update state. Call [`Updates::poll`] from the UI loop.
pub struct Updates {
    phase: Phase,
    available: Option<AvailableRelease>,
    downloaded: Option<Downloaded>,
    installing_version: Option<String>,
    received: u64,
    total: u64,
    error: Option<(ErrorContext, String)>,
    check_rx: Option<Receiver<CheckOutcome>>,
    download_rx: Option<Receiver<DownloadEvent>>,
    feed_cache: Option<FeedCache>,
    mode: UpdateMode,
    eligibility: Eligibility,
    next_auto_check: Option<Instant>,
    checked_at: Option<SystemTime>,
    /// "Restart and install" was confirmed and verified.
    install_confirmed: bool,
    install_result: Option<InstallResult>,
    failure_reported: bool,
    /// The newest version already announced this session.
    announced: Option<String>,
    announcements: VecDeque<Announcement>,
    launched_args: Option<Vec<String>>,
    /// Tests only: a local `{status, body, notes_md}` response instead of
    /// the network.
    feed_fixture: Option<PathBuf>,
    /// Tests only: a local installer file instead of the network.
    installer_fixture: Option<PathBuf>,
}

impl Default for Updates {
    fn default() -> Self {
        Self::new()
    }
}

impl Updates {
    /// Inert state: no background checks, no in-app install.
    pub fn new() -> Self {
        Self::with_policy(UpdateMode::Off, Eligibility::NotInstalled, Instant::now())
    }

    /// The saved mode and this copy's eligibility; schedules the first
    /// background check and reads the previous update's result once.
    pub fn load(mode: UpdateMode) -> Self {
        let eligibility = Eligibility::current();
        let mut state = Self::with_policy(mode, eligibility, Instant::now());
        if eligibility == Eligibility::Installed {
            state.install_result = result_path().and_then(|path| take_install_result(&path));
        }
        state
    }

    fn with_policy(mode: UpdateMode, eligibility: Eligibility, now: Instant) -> Self {
        let mut state = Self {
            phase: Phase::Idle,
            available: None,
            downloaded: None,
            installing_version: None,
            received: 0,
            total: 0,
            error: None,
            check_rx: None,
            download_rx: None,
            feed_cache: None,
            mode,
            eligibility,
            next_auto_check: None,
            checked_at: None,
            install_confirmed: false,
            install_result: None,
            failure_reported: false,
            announced: None,
            announcements: VecDeque::new(),
            launched_args: None,
            feed_fixture: None,
            installer_fixture: None,
        };
        state.next_auto_check = state.auto_checks().then(|| now + STARTUP_DELAY);
        state
    }

    pub fn current_version(&self) -> &'static str {
        CURRENT_VERSION
    }

    pub fn mode(&self) -> UpdateMode {
        self.mode
    }

    /// Apply a new mode (the caller saves `mode.key()` as `update_mode`).
    /// Turning Notify me on checks now.
    pub fn set_mode(&mut self, mode: UpdateMode) {
        self.apply_mode(mode, Instant::now());
    }

    fn apply_mode(&mut self, mode: UpdateMode, now: Instant) {
        self.mode = mode;
        if !self.auto_checks() {
            self.next_auto_check = None;
        } else if self.next_auto_check.is_none() {
            self.next_auto_check = Some(now);
        }
    }

    pub fn eligibility(&self) -> Eligibility {
        self.eligibility
    }

    /// This copy may check by itself, download and install.
    pub fn eligible(&self) -> bool {
        self.eligibility == Eligibility::Installed
    }

    /// Why this copy never updates itself, for the UI.
    pub fn ineligibility_reason(&self) -> Option<&'static str> {
        self.eligibility.reason()
    }

    fn auto_checks(&self) -> bool {
        self.eligible() && self.mode.checks_automatically()
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn error_context(&self) -> Option<ErrorContext> {
        self.error.as_ref().map(|(context, _)| *context)
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error.as_ref().map(|(_, message)| message.as_str())
    }

    /// The failed step can be retried with a click.
    pub fn can_retry(&self) -> bool {
        match (self.phase, self.error_context()) {
            (Phase::Error, Some(ErrorContext::Check)) => true,
            (Phase::Available, Some(ErrorContext::Download)) => self.can_download(),
            (Phase::Downloaded, Some(ErrorContext::Install)) => self.install_ready(),
            _ => false,
        }
    }

    /// Snapshot for the UI.
    /// For the control channel's inspect call (QA).
    pub fn inspection(&self) -> serde_json::Value {
        let view = self.state();
        serde_json::json!({
            "phase": view.phase.key(),
            "label": self.status_label(),
            "current": view.current_version,
            "available": view.available_version,
            "downloaded": view.downloaded_version,
            "percent": view.download_percent,
            "size": view.size,
            "notes_lines": view.notes.len(),
            "error": view.error.map(|(context, message)| serde_json::json!({"context": context.key(), "message": message})),
            "error_message": self.error_message(),
            "can_retry": view.can_retry,
            "can_download": view.can_download,
            "checked": view.checked_at.is_some(),
            "install_failure": view.install_failure,
            "updated_to": self.updated_to(),
            "eligibility": self.eligibility().key(),
            "mode": self.mode().key(),
        })
    }
    pub fn state(&self) -> UpdateView {
        UpdateView {
            phase: self.phase,
            current_version: CURRENT_VERSION,
            available_version: self.available_version().map(str::to_owned),
            downloaded_version: self.downloaded_version().map(str::to_owned),
            download_percent: self.percent(),
            notes: self.notes().to_vec(),
            size: self.size(),
            error: self.error.clone(),
            can_retry: self.can_retry(),
            can_download: self.can_download(),
            checked_at: self.checked_at,
            install_failure: self.last_install_failure(),
        }
    }

    /// Start one check on a worker thread ("Check for updates"); works for
    /// every copy. Returns false while a check, download or install runs.
    pub fn check_now(&mut self) -> bool {
        self.begin_check(Instant::now())
    }

    fn begin_check(&mut self, now: Instant) -> bool {
        if self.check_rx.is_some()
            || self.install_confirmed
            || matches!(self.phase, Phase::Downloading | Phase::Installing)
        {
            return false;
        }
        // A known update stays on screen while it is checked again quietly.
        if !matches!(self.phase, Phase::Available | Phase::Downloaded) {
            self.phase = Phase::Checking;
            self.error = None;
        }
        self.next_auto_check = self.auto_checks().then(|| now + POLL_INTERVAL);
        let fixture = self.feed_fixture.clone();
        let cache = self.feed_cache.clone();
        let known_notes = self
            .available
            .as_ref()
            .filter(|release| !release.notes.is_empty())
            .map(|release| (release.version.clone(), release.notes.clone()));
        let (tx, rx) = mpsc::channel();
        self.check_rx = Some(rx);
        std::thread::spawn(move || {
            let outcome = match fixture {
                Some(path) => read_fixture(&path),
                None => fetch_release(cache, known_notes),
            };
            let _ = tx.send(outcome);
        });
        true
    }

    /// Collect worker results and start a due background check, without
    /// blocking. Returns true when visible state changed.
    pub fn poll(&mut self) -> bool {
        let checked = self.poll_check();
        let downloaded = self.poll_download();
        let scheduled = self.poll_schedule(Instant::now());
        checked || downloaded || scheduled
    }

    fn poll_schedule(&mut self, now: Instant) -> bool {
        let Some(due) = self.next_auto_check else {
            return false;
        };
        if now < due || !self.auto_checks() {
            return false;
        }
        if self.begin_check(now) {
            return true;
        }
        // Busy: skip this tick; nothing queues up.
        self.next_auto_check = Some(now + POLL_INTERVAL);
        false
    }

    fn poll_check(&mut self) -> bool {
        let Some(rx) = self.check_rx.as_ref() else {
            return false;
        };
        let outcome = match rx.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => CheckOutcome {
                result: Err("The update check stopped unexpectedly.".into()),
                cache: None,
                notes: None,
            },
        };
        self.check_rx = None;
        if let Some(cache) = outcome.cache {
            self.feed_cache = Some(cache);
        }
        self.apply_check(outcome.result, outcome.notes);
        true
    }

    fn apply_check(
        &mut self,
        result: Result<CheckResult, String>,
        notes: Option<(String, Vec<NoteLine>)>,
    ) {
        // A check finishing while the user's download or install runs never
        // changes what they chose.
        if self.install_confirmed || matches!(self.phase, Phase::Downloading | Phase::Installing) {
            return;
        }
        self.checked_at = Some(SystemTime::now());
        let holds_download = self.downloaded.is_some();
        match result {
            Ok(CheckResult::Release(release)) if release.newer => {
                if self
                    .downloaded
                    .as_ref()
                    .is_some_and(|held| held.version == release.version)
                {
                    self.phase = Phase::Downloaded;
                    return;
                }
                // A newer release replaces an older held download.
                if let Some(held) = self.downloaded.take() {
                    discard_download(held);
                }
                let same = self
                    .available
                    .as_ref()
                    .is_some_and(|known| known.version == release.version);
                let notes = notes
                    .filter(|(version, _)| *version == release.version)
                    .map(|(_, lines)| lines)
                    .or_else(|| {
                        release
                            .summary
                            .as_deref()
                            .map(|summary| vec![NoteLine::Text(summary.to_owned())])
                    })
                    .unwrap_or_default();
                // A failed download keeps its retry message until retried.
                if !(same && self.error_context() == Some(ErrorContext::Download)) {
                    self.error = None;
                }
                if self.announced.as_deref() != Some(release.version.as_str()) {
                    self.announced = Some(release.version.clone());
                    self.announcements
                        .push_back(Announcement::Available(release.version.clone()));
                }
                self.available = Some(AvailableRelease {
                    version: release.version,
                    installer: release.installer,
                    notes,
                });
                self.phase = Phase::Available;
            }
            Ok(CheckResult::Release(_)) => {
                if holds_download {
                    self.phase = Phase::Downloaded;
                } else {
                    self.available = None;
                    self.error = None;
                    self.phase = Phase::UpToDate;
                }
            }
            Ok(CheckResult::Missing) => {
                if holds_download {
                    self.phase = Phase::Downloaded;
                } else if self.available.is_some() {
                    self.phase = Phase::Available;
                } else {
                    self.error = None;
                    self.phase = Phase::Unpublished;
                }
            }
            Err(message) => {
                if holds_download {
                    self.phase = Phase::Downloaded;
                } else if self.available.is_some() {
                    // A failed check never hides a found update.
                    self.phase = Phase::Available;
                } else {
                    self.error = Some((ErrorContext::Check, message));
                    self.phase = Phase::Error;
                }
            }
        }
    }

    fn poll_download(&mut self) -> bool {
        let Some(rx) = self.download_rx.as_ref() else {
            return false;
        };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok(DownloadEvent::Progress(received)) => {
                    if self.phase == Phase::Downloading {
                        self.received = received;
                        changed = true;
                    }
                }
                Ok(DownloadEvent::Done(result)) => {
                    self.download_rx = None;
                    self.finish_download(result);
                    return true;
                }
                Err(TryRecvError::Empty) => return changed,
                Err(TryRecvError::Disconnected) => {
                    self.download_rx = None;
                    self.finish_download(
                        Err("The installer download stopped unexpectedly.".into()),
                    );
                    return true;
                }
            }
        }
    }

    fn finish_download(&mut self, result: Result<(PathBuf, File), String>) {
        let release = self.available.as_ref().and_then(|release| {
            release
                .installer
                .clone()
                .map(|installer| (release.version.clone(), installer))
        });
        match (result, release) {
            (Ok((path, file)), Some((version, installer))) => {
                self.received = installer.size;
                self.downloaded = Some(Downloaded {
                    version: version.clone(),
                    installer,
                    path,
                    file,
                });
                self.error = None;
                self.phase = Phase::Downloaded;
                self.announcements
                    .push_back(Announcement::Downloaded(version));
            }
            (Ok((path, file)), None) => {
                drop(file);
                remove_download_directory(&path);
                self.fail_download("The installer download finished without a release.".into());
            }
            (Err(error), _) => self.fail_download(error),
        }
    }

    /// A failed download returns to Available with a retry.
    fn fail_download(&mut self, message: String) {
        self.error = Some((ErrorContext::Download, message));
        self.phase = if self.available.is_some() {
            Phase::Available
        } else {
            Phase::Error
        };
    }

    /// The next once-per-session announcement (newly found version, finished
    /// download), oldest first.
    pub fn take_announcement(&mut self) -> Option<Announcement> {
        self.announcements.pop_front()
    }

    /// The previous launch's failed install, once ("Update {v} was not
    /// installed, …" with setup's reason), for a notification.
    pub fn take_last_install_failure(&mut self) -> Option<String> {
        if self.failure_reported {
            return None;
        }
        let text = self.last_install_failure()?;
        self.failure_reported = true;
        Some(text)
    }

    /// The previous launch's failed install, kept until a new download starts.
    pub fn last_install_failure(&self) -> Option<String> {
        self.install_result
            .as_ref()
            .filter(|result| !result.ok)
            .map(failure_text)
    }

    /// The version this build was just updated to by setup, if any.
    pub fn updated_to(&self) -> Option<&str> {
        self.install_result
            .as_ref()
            .filter(|result| result.ok)
            .map(|result| result.version.as_str())
    }

    pub fn checking(&self) -> bool {
        self.check_rx.is_some()
    }

    /// A check, download or install is in progress.
    pub fn busy(&self) -> bool {
        self.checking() || matches!(self.phase, Phase::Downloading | Phase::Installing)
    }

    /// A newer version is known (available, downloading, downloaded or
    /// installing).
    pub fn update_known(&self) -> bool {
        matches!(
            self.phase,
            Phase::Available | Phase::Downloading | Phase::Downloaded | Phase::Installing
        )
    }

    /// A verified installer is held.
    pub fn install_ready(&self) -> bool {
        self.phase == Phase::Downloaded && self.downloaded.is_some()
    }

    /// The newer version the flow is about (installing, downloaded, then
    /// available).
    pub fn available_version(&self) -> Option<&str> {
        self.installing_version
            .as_deref()
            .filter(|_| self.phase == Phase::Installing)
            .or(self.downloaded.as_ref().map(|held| held.version.as_str()))
            .or(self
                .available
                .as_ref()
                .map(|release| release.version.as_str()))
    }

    pub fn downloaded_version(&self) -> Option<&str> {
        self.downloaded.as_ref().map(|held| held.version.as_str())
    }

    fn version_or_default(&self) -> &str {
        self.available_version().unwrap_or("the latest release")
    }

    pub fn percent(&self) -> Option<u64> {
        match self.phase {
            Phase::Downloading => Some(percent(self.received, self.total)),
            Phase::Downloaded | Phase::Installing => Some(100),
            _ => None,
        }
    }

    /// Release notes of the available version (plain text lines).
    pub fn notes(&self) -> &[NoteLine] {
        self.available
            .as_ref()
            .filter(|release| self.available_version() == Some(release.version.as_str()))
            .map_or(&[], |release| release.notes.as_slice())
    }

    /// Published installer size in bytes.
    pub fn size(&self) -> Option<u64> {
        self.downloaded
            .as_ref()
            .map(|held| held.installer.size)
            .or_else(|| {
                self.available
                    .as_ref()
                    .and_then(|release| release.installer.as_ref())
                    .map(|installer| installer.size)
            })
    }

    /// Published installer size, e.g. "41.2 MB".
    pub fn size_label(&self) -> Option<String> {
        self.size().map(megabytes)
    }

    /// An installer is published for the available version and this copy
    /// may download it.
    pub fn can_download(&self) -> bool {
        self.eligible()
            && self
                .available
                .as_ref()
                .is_some_and(|release| release.installer.is_some())
    }

    /// Why Download is not offered for a known update, if it is not.
    pub fn download_unavailable_reason(&self) -> Option<&'static str> {
        if self.can_download() {
            return None;
        }
        self.ineligibility_reason().or(Some(
            "No installer was published with this version. See the release page.",
        ))
    }

    /// Short headline for Settings.
    pub fn status_label(&self) -> String {
        let version = self.version_or_default();
        match self.phase {
            Phase::Idle => match &self.install_result {
                Some(result) if result.ok => format!("Updated to {}", result.version),
                Some(result) if !result.version.is_empty() => {
                    format!("Update {} was not installed", result.version)
                }
                Some(_) => "Update was not installed".into(),
                None => "Not checked yet".into(),
            },
            Phase::Checking => "Checking for updates…".into(),
            Phase::UpToDate => "Up to date".into(),
            Phase::Unpublished => "No release published".into(),
            Phase::Available => match self.size_label().filter(|_| self.can_download()) {
                Some(size) => format!("Jawjack {version} is available · {size}"),
                None => format!("Jawjack {version} is available"),
            },
            Phase::Downloading => format!(
                "Downloading update ({}%)",
                percent(self.received, self.total)
            ),
            Phase::Downloaded => format!("Update {version} downloaded"),
            Phase::Installing => format!("Installing update {version}"),
            Phase::Error => "Update check failed".into(),
        }
    }

    /// Longer explanation for Settings.
    pub fn detail(&self) -> String {
        let version = self.version_or_default();
        match (self.phase, &self.error) {
            (Phase::Idle, _) => match &self.install_result {
                Some(result) if result.ok => format!(
                    "Jawjack was updated to {}. Your settings were kept.",
                    result.version
                ),
                Some(result) => failure_text(result),
                None => self.policy_detail(),
            },
            (Phase::Checking, _) => "Checking published releases…".into(),
            (Phase::UpToDate, _) => format!("v{CURRENT_VERSION} is the latest published release."),
            (Phase::Unpublished, _) => "No Jawjack release is currently published.".into(),
            (Phase::Available, Some((ErrorContext::Download, message))) => {
                format!("{message} Nothing was installed. Download it again when you're ready.")
            }
            (Phase::Available, _) if self.can_download() => format!(
                "Jawjack {version} is available. Nothing is downloaded until you choose \
                 Download; Jawjack then checks the installer against its published SHA-256."
            ),
            (Phase::Available, _) => match self.ineligibility_reason() {
                Some(reason) => format!("Jawjack {version} is available. {reason}"),
                None => format!(
                    "Jawjack {version} is available, but no installer was published with it. \
                     See the release page."
                ),
            },
            (Phase::Downloading, _) => format!(
                "Downloading {version}… {} of {}. Jawjack checks its SHA-256 when it finishes.",
                megabytes(self.received),
                megabytes(self.total)
            ),
            (Phase::Downloaded, Some((ErrorContext::Install, message))) => {
                format!("{message} The downloaded update is kept; try again when you're ready.")
            }
            (Phase::Downloaded, _) => format!(
                "Update {version} is downloaded and its SHA-256 verified. Restart and install \
                 closes Jawjack, installs it and starts Jawjack again. Quitting does not \
                 install it."
            ),
            (Phase::Installing, _) => format!(
                "Installing {version}. Jawjack closes, setup installs the update, and Jawjack \
                 starts again."
            ),
            (Phase::Error, Some((_, message))) => {
                format!("{message} Jawjack tries again at the next check.")
            }
            (Phase::Error, None) => "The update check failed.".into(),
        }
    }

    fn policy_detail(&self) -> String {
        if let Some(reason) = self.ineligibility_reason() {
            return format!("{reason} You can still check for new versions here.");
        }
        match self.mode {
            UpdateMode::Notify => "Jawjack checks for updates in the background and tells you \
                                   when one is available. Nothing downloads or installs until \
                                   you choose it."
                .into(),
            UpdateMode::Off => "Updates are checked only when you ask.".into(),
        }
    }

    /// Label for the primary update action, or `None` when this copy cannot
    /// install or no verified installer is published.
    pub fn install_action_label(&self) -> Option<String> {
        if !self.eligible() {
            return None;
        }
        match (self.phase, self.error_context()) {
            (Phase::Available, Some(ErrorContext::Download)) if self.can_download() => {
                Some("Retry download".into())
            }
            (Phase::Available, _) if self.can_download() => Some("Download".into()),
            (Phase::Downloading, _) => Some(format!(
                "Downloading… {}%",
                percent(self.received, self.total)
            )),
            (Phase::Downloaded, Some(ErrorContext::Install)) => Some("Retry install".into()),
            (Phase::Downloaded, _) => Some("Restart and install".into()),
            (Phase::Installing, _) => Some("Installing…".into()),
            _ => None,
        }
    }

    /// Start downloading the installer for the available version (only on
    /// an explicit click). Returns false when this copy cannot install, no
    /// verified installer is published, or nothing is available.
    pub fn start_download(&mut self) -> bool {
        if self.phase != Phase::Available || !self.can_download() {
            return false;
        }
        let Some((version, installer)) = self.available.as_ref().and_then(|release| {
            release
                .installer
                .clone()
                .map(|installer| (release.version.clone(), installer))
        }) else {
            return false;
        };
        self.phase = Phase::Downloading;
        self.error = None;
        self.install_result = None;
        self.received = 0;
        self.total = installer.size;
        let fixture = self.installer_fixture.clone();
        let (tx, rx) = mpsc::channel();
        self.download_rx = Some(rx);
        std::thread::spawn(move || {
            let progress = tx.clone();
            let result = download(&version, &installer, fixture, &mut |received| {
                let _ = progress.send(DownloadEvent::Progress(received));
            });
            let _ = tx.send(DownloadEvent::Done(result));
        });
        true
    }

    /// "Restart and install" was confirmed. Re-verifies the held installer
    /// so a changed file is caught before anything closes; a changed file
    /// is discarded and the download offered again. On `Ok` the caller saves
    /// and then calls [`Updates::launch_installer`].
    pub fn request_install(&mut self) -> Result<(), String> {
        if !self.eligible() {
            return Err(self
                .ineligibility_reason()
                .unwrap_or("This copy does not install updates.")
                .into());
        }
        if !self.install_ready() {
            return Err("No verified installer is ready.".into());
        }
        let verified = self
            .downloaded
            .as_ref()
            .map(|held| verify_held(&held.file, &held.installer));
        if let Some(Err(error)) = verified {
            self.reject_download(error.clone());
            return Err(error);
        }
        self.install_confirmed = true;
        Ok(())
    }

    /// The held installer no longer verifies: discard it and offer the
    /// download again.
    fn reject_download(&mut self, error: String) {
        if let Some(held) = self.downloaded.take() {
            discard_download(held);
        }
        self.install_confirmed = false;
        self.error = Some((
            ErrorContext::Download,
            format!("{error} Download it again."),
        ));
        self.phase = if self.available.is_some() {
            Phase::Available
        } else {
            Phase::Error
        };
    }

    /// The confirmed install was cancelled: the downloaded update waits.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn cancel_install(&mut self) {
        self.install_confirmed = false;
    }

    #[cfg_attr(not(test), allow(dead_code))]

    pub fn install_confirmed(&self) -> bool {
        self.install_confirmed
    }

    /// Arguments of the started installer, for inspection.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn launched_args(&self) -> Option<&[String]> {
        self.launched_args.as_deref()
    }

    /// After [`Updates::request_install`]: verify the held installer once
    /// more and start setup quietly with relaunch. Returns the installer
    /// path; the app should then exit. On failure Jawjack should stay open:
    /// the state shows "Install failed · Retry". Quitting without this call
    /// never installs.
    pub fn launch_installer(&mut self) -> Result<PathBuf, String> {
        self.launch(true)
    }

    fn launch(&mut self, spawn: bool) -> Result<PathBuf, String> {
        if !self.eligible() {
            return Err("This copy does not install updates.".into());
        }
        if !self.install_confirmed {
            return Err("Installing was not confirmed.".into());
        }
        let Some(held) = self.downloaded.as_ref() else {
            return Err("No verified installer is ready.".into());
        };
        let path = held.path.clone();
        if let Err(error) = verify_held(&held.file, &held.installer) {
            self.reject_download(error.clone());
            return Err(error);
        }
        let args = installer_args(std::process::id(), result_path().as_deref());
        if spawn && let Err(error) = spawn_installer(&path, &args) {
            self.install_confirmed = false;
            self.error = Some((ErrorContext::Install, error.clone()));
            self.phase = Phase::Downloaded;
            return Err(error);
        }
        self.launched_args = Some(
            args.iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
        );
        // Dropping the held handle after CreateProcess is safe: the running
        // installer's image is mapped and locked.
        if let Some(held) = self.downloaded.take() {
            self.installing_version = Some(held.version);
        }
        self.error = None;
        self.phase = Phase::Installing;
        self.install_confirmed = false;
        Ok(path)
    }

    #[cfg(test)]
    fn set_test_fixtures(&mut self, feed: PathBuf, installer: PathBuf) {
        self.feed_fixture = Some(feed);
        self.installer_fixture = Some(installer);
    }
}

impl Drop for Updates {
    /// A downloaded installer that was never started is removed with its
    /// private folder; a started one belongs to setup.
    fn drop(&mut self) {
        if let Some(held) = self.downloaded.take() {
            discard_download(held);
        }
    }
}

fn discard_download(held: Downloaded) {
    let Downloaded { path, file, .. } = held;
    drop(file);
    remove_download_directory(&path);
}

/// Remove an installer's private `jawjack-update-*` folder (never anything
/// else).
fn remove_download_directory(path: &Path) {
    if let Some(directory) = path.parent()
        && directory
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(DOWNLOAD_FOLDER_PREFIX))
    {
        let _ = std::fs::remove_dir_all(directory);
    }
}

fn percent(received: u64, total: u64) -> u64 {
    (received.saturating_mul(100) / total.max(1)).min(100)
}

/// Setup's quiet command line for a confirmed install.
fn installer_args(pid: u32, result: Option<&Path>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "--app-pid".into(),
        pid.to_string().into(),
        "--quiet".into(),
        "--relaunch".into(),
    ];
    if let Some(result) = result {
        args.push("--result-file".into());
        args.push(result.as_os_str().to_owned());
    }
    args
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

// ---------------------------------------------------------- release notes

fn clean_note(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .filter(|character| *character != '`')
        .collect::<String>()
        .replace("**", "")
        .trim()
        .chars()
        .take(MAX_NOTE_CHARS)
        .collect()
}

/// The version appears in `heading` as a whole token (so `1.0.0-preview.1`
/// does not match `1.0.0-preview.10`).
fn heading_names_version(heading: &str, version: &str) -> bool {
    heading.match_indices(version).any(|(index, _)| {
        let before = heading[..index].chars().next_back();
        let after = heading[index + version.len()..].chars().next();
        let token = |character: Option<char>| {
            character.is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
        };
        !token(before) && !token(after)
    })
}

/// Parse `notes.md` for `version` into bounded plain-text lines. The first
/// heading must name exactly that version; otherwise `None`.
fn parse_notes_md(text: &str, version: &str) -> Option<Vec<NoteLine>> {
    let mut lines = text
        .lines()
        .map(str::trim_end)
        .skip_while(|line| line.trim().is_empty());
    let first = lines.next()?.trim();
    if !first.starts_with('#') || !heading_names_version(first, version) {
        return None;
    }
    let mut notes: Vec<NoteLine> = Vec::new();
    let mut paragraph = String::new();
    let flush = |paragraph: &mut String, notes: &mut Vec<NoteLine>| {
        let text = clean_note(paragraph);
        if !text.is_empty() {
            notes.push(NoteLine::Text(text));
        }
        paragraph.clear();
    };
    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            flush(&mut paragraph, &mut notes);
            continue;
        }
        if let Some(heading) = line.strip_prefix('#') {
            flush(&mut paragraph, &mut notes);
            let text = clean_note(heading.trim_start_matches('#'));
            if !text.is_empty() {
                notes.push(NoteLine::Heading(text));
            }
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            flush(&mut paragraph, &mut notes);
            notes.push(NoteLine::Bullet(clean_note(item)));
        } else if raw.starts_with(' ')
            && paragraph.is_empty()
            && let Some(NoteLine::Bullet(last)) = notes.last_mut()
        {
            // A wrapped bullet continues on an indented line.
            let joined = format!("{last} {line}");
            *last = clean_note(&joined);
        } else {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(line);
        }
        if notes.len() >= MAX_NOTE_LINES {
            break;
        }
    }
    flush(&mut paragraph, &mut notes);
    notes.truncate(MAX_NOTE_LINES);
    Some(notes)
}

// ------------------------------------------------------------------- feed

fn release_result(body: &Value) -> Result<CheckResult, String> {
    let mut release = parse_release(body)?;
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
    let summary = value
        .get("notes")
        .and_then(Value::as_str)
        .map(|text| {
            clean_note(text)
                .chars()
                .take(MAX_SUMMARY_CHARS)
                .collect::<String>()
        })
        .filter(|text| !text.is_empty());
    Ok(Release {
        version: version.to_owned(),
        newer: false,
        installer: parse_installer(value.get("setup"), version),
        summary,
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

/// A validator is echoed back in a request header, so it must be short,
/// printable ASCII without CR/LF.
fn safe_validator(value: Option<String>) -> Option<String> {
    value.filter(|text| {
        !text.is_empty()
            && text.len() <= MAX_VALIDATOR_LENGTH
            && text.bytes().all(|byte| (0x20..0x7f).contains(&byte))
    })
}

/// Conditional request headers for the cached feed.
fn conditional_headers(cache: Option<&FeedCache>) -> Vec<(&'static str, String)> {
    let mut headers = Vec::new();
    if let Some(cache) = cache {
        if let Some(etag) = &cache.etag {
            headers.push(("If-None-Match", etag.clone()));
        }
        if let Some(modified) = &cache.last_modified {
            headers.push(("If-Modified-Since", modified.clone()));
        }
    }
    headers
}

/// One feed response: status, body (200 only) and validators.
struct FeedResponse {
    status: u16,
    body: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
}

/// Interpret a feed response against the cache: a 200 parses its body and
/// (with validators) becomes the new cache, a 304 reuses the cached body, a
/// 404 is "no release published".
fn interpret_feed(
    response: FeedResponse,
    cache: Option<FeedCache>,
) -> (Result<CheckResult, String>, Option<FeedCache>) {
    let (body, new_cache) = match response.status {
        200 => {
            if response.body.len() > MAX_FEED_BYTES {
                return (
                    Err("The update response exceeded the 64 KiB limit.".into()),
                    None,
                );
            }
            let etag = safe_validator(response.etag);
            let last_modified = safe_validator(response.last_modified);
            let new_cache = (etag.is_some() || last_modified.is_some()).then(|| FeedCache {
                etag,
                last_modified,
                body: response.body.clone(),
            });
            (response.body, new_cache)
        }
        304 => match cache {
            Some(cache) => (cache.body, None),
            None => {
                return (
                    Err("The update service answered 304 without a cached feed.".into()),
                    None,
                );
            }
        },
        404 => return (Ok(CheckResult::Missing), None),
        status => {
            return (
                Err(format!("The update service returned HTTP {status}.")),
                None,
            );
        }
    };
    let result = serde_json::from_slice::<Value>(&body)
        .map_err(|_| "The update response was not valid JSON.".to_owned())
        .and_then(|value| release_result(&value));
    (result, new_cache)
}

/// A test response fixture: `{status, body?, notes_md?}`.
fn read_fixture(path: &Path) -> CheckOutcome {
    let failed = |message: &str| CheckOutcome {
        result: Err(message.to_owned()),
        cache: None,
        notes: None,
    };
    let Ok(file) = File::open(path) else {
        return failed("Could not read the update-check fixture.");
    };
    let Ok(bytes) = read_bounded(file, MAX_FEED_BYTES + MAX_NOTES_BYTES, "") else {
        return failed("The update-check fixture exceeded its size limit.");
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return failed("The update response was not valid JSON.");
    };
    let status = value.get("status").and_then(Value::as_u64).unwrap_or(0) as u16;
    let body = value
        .get("body")
        .map(|body| serde_json::to_vec(body).unwrap_or_default())
        .unwrap_or_default();
    let (result, _) = interpret_feed(
        FeedResponse {
            status,
            body,
            etag: None,
            last_modified: None,
        },
        None,
    );
    let notes = match (&result, value.get("notes_md")) {
        (Ok(CheckResult::Release(release)), Some(Value::String(text))) if release.newer => {
            parse_notes_md(text, &release.version).map(|lines| (release.version.clone(), lines))
        }
        _ => None,
    };
    CheckOutcome {
        result,
        cache: None,
        notes,
    }
}

/// HTTPS client for the fixed host: OS certificate store, no redirects, no
/// cookie store (reqwest keeps none unless asked), no automatic auth, whole-
/// request time limit.
fn client(timeout: Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .tls_built_in_webpki_certs(false)
        .tls_built_in_native_certs(true)
        .https_only(true)
        .user_agent(concat!("Jawjack/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "Could not initialize update networking.".into())
}

fn get(
    timeout: Duration,
    path: &str,
    headers: &[(&'static str, String)],
) -> Result<reqwest::blocking::Response, String> {
    let mut request = client(timeout)?.get(format!("https://{UPDATE_HOST}{path}"));
    for (name, value) in headers {
        request = request.header(*name, value.as_str());
    }
    request.send().map_err(|error| {
        if error.is_timeout() {
            "The update service did not respond in time.".to_owned()
        } else {
            "Could not reach the update service.".to_owned()
        }
    })
}

/// Read at most `max` bytes; more is an error.
fn read_bounded(source: impl Read, max: usize, message: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(4096);
    source
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "The update response could not be read.".to_owned())?;
    if bytes.len() > max {
        return Err(message.to_owned());
    }
    Ok(bytes)
}

fn header(
    response: &reqwest::blocking::Response,
    name: reqwest::header::HeaderName,
) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// One feed check: a conditional GET of latest.json and, for a newer version
/// whose notes are not known yet, notes.md. Notes failures never fail the
/// check.
fn fetch_release(
    cache: Option<FeedCache>,
    known_notes: Option<(String, Vec<NoteLine>)>,
) -> CheckOutcome {
    let fetched = get(
        CHECK_TIMEOUT,
        FEED_PATH,
        &conditional_headers(cache.as_ref()),
    )
    .and_then(|response| {
        let status = response.status().as_u16();
        let etag = header(&response, reqwest::header::ETAG);
        let last_modified = header(&response, reqwest::header::LAST_MODIFIED);
        let body = if status == 200 {
            read_bounded(
                response,
                MAX_FEED_BYTES,
                "The update response exceeded the 64 KiB limit.",
            )?
        } else {
            Vec::new()
        };
        Ok(FeedResponse {
            status,
            body,
            etag,
            last_modified,
        })
    });
    let response = match fetched {
        Ok(response) => response,
        Err(error) => {
            return CheckOutcome {
                result: Err(error),
                cache: None,
                notes: None,
            };
        }
    };
    let (result, cache) = interpret_feed(response, cache);
    let notes = match &result {
        Ok(CheckResult::Release(release)) if release.newer => match known_notes {
            Some((version, lines)) if version == release.version => Some((version, lines)),
            _ => fetch_notes(&release.version).map(|lines| (release.version.clone(), lines)),
        },
        _ => None,
    };
    CheckOutcome {
        result,
        cache,
        notes,
    }
}

/// notes.md for `version`, bounded; `None` on any failure or mismatch.
fn fetch_notes(version: &str) -> Option<Vec<NoteLine>> {
    let response = get(CHECK_TIMEOUT, NOTES_PATH, &[]).ok()?;
    if response.status().as_u16() != 200 {
        return None;
    }
    let body = read_bounded(response, MAX_NOTES_BYTES, "").ok()?;
    parse_notes_md(std::str::from_utf8(&body).ok()?, version)
}

// ------------------------------------------------------------- installer

/// A fresh private download folder under the user's temp directory.
fn download_directory() -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let directory = std::env::temp_dir().join(format!(
        "{DOWNLOAD_FOLDER_PREFIX}{}-{:x}",
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
    fixture: Option<PathBuf>,
    progress: &mut dyn FnMut(u64),
) -> Result<(PathBuf, File), String> {
    if let Some(path) = fixture {
        let file =
            File::open(path).map_err(|_| "Could not read the installer fixture.".to_owned())?;
        return save_installer(file, installer, progress);
    }
    let response = get(
        DOWNLOAD_TIMEOUT,
        &installer_url_path(version, installer),
        &[],
    )?;
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
fn spawn_installer(path: &Path, args: &[OsString]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let directory = path
        .parent()
        .ok_or_else(|| "The installer could not be started.".to_owned())?;
    let spawn = |flags| {
        std::process::Command::new(path)
            .args(args)
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
fn spawn_installer(_path: &Path, _args: &[OsString]) -> Result<(), String> {
    Err(format!(
        "Installing updates in the app is available on Windows only. Download the release from {RELEASE_PAGE}."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn feed(status: u16, body: &str) -> FeedResponse {
        FeedResponse {
            status,
            body: body.as_bytes().to_vec(),
            etag: None,
            last_modified: None,
        }
    }

    fn parse(status: u16, body: &str) -> Result<CheckResult, String> {
        interpret_feed(feed(status, body), None).0
    }

    fn simple(version: &str, channel: &str) -> String {
        format!(r#"{{"version":"{version}","channel":"{channel}"}}"#)
    }

    fn release(value: CheckResult) -> Release {
        match value {
            CheckResult::Release(release) => release,
            CheckResult::Missing => panic!("expected a release"),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let directory = download_directory().unwrap().join(name);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn schedule_matches_the_update_flow_spec() {
        assert_eq!(STARTUP_DELAY, Duration::from_secs(15));
        assert_eq!(POLL_INTERVAL, Duration::from_secs(240));
        assert_eq!(CHECK_TIMEOUT, Duration::from_secs(30));
        assert_eq!(DOWNLOAD_TIMEOUT, Duration::from_secs(600));
        assert_eq!(MAX_FEED_BYTES, 64 * 1024);
        assert_eq!(MAX_NOTES_BYTES, 64 * 1024);
        assert_eq!((MAX_NOTE_LINES, MAX_NOTE_CHARS), (80, 400));
        assert_eq!(MAX_INSTALLER_BYTES, 256 * 1024 * 1024);
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
        assert!(release(parse(200, &simple("999.0.0", "stable")).unwrap()).newer);
        assert!(!release(parse(200, &simple(CURRENT_VERSION, "preview")).unwrap()).newer);
        for channel in ["preview", "beta", "stable"] {
            assert!(parse(200, &simple("0.1.1", channel)).is_ok(), "{channel}");
        }
        assert!(parse(200, &simple("0.2.0", "nightly")).is_err());
        assert!(parse(200, &simple("0.1.1", &"x".repeat(MAX_CHANNEL_LENGTH + 1))).is_err());
        assert!(parse(200, &simple("latest", "stable")).is_err());
        assert!(parse(200, "not json").is_err());
        assert!(parse(200, &" ".repeat(MAX_FEED_BYTES + 1)).is_err());
        assert!(read_bounded(&vec![b' '; MAX_FEED_BYTES + 1][..], MAX_FEED_BYTES, "big").is_err());
    }

    #[test]
    fn missing_feed_is_not_an_error() {
        assert_eq!(parse(404, "").unwrap(), CheckResult::Missing);
        assert!(parse(500, "").is_err());
        assert!(parse(302, "").is_err());
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
        };
        let ok =
            release(parse(200, &body("Jawjack-0.2.0-preview.3-setup.exe", 10, &hash)).unwrap());
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
            let parsed = release(parse(200, &body(file, size, sha)).unwrap());
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
    fn release_notes_are_bounded_plain_text_for_the_exact_version() {
        let text = "## Jawjack 0.3.0-preview.16\n\nEverything from preview.9, plus `fixes`.\n\
                    Second **line**.\n\n### Chat\n\n- Scrolling keeps the position\n  line.\n\
                    * Links\u{7} open\n";
        let lines = parse_notes_md(text, "0.3.0-preview.16").unwrap();
        assert_eq!(
            lines,
            [
                NoteLine::Text("Everything from preview.9, plus fixes. Second line.".into()),
                NoteLine::Heading("Chat".into()),
                NoteLine::Bullet("Scrolling keeps the position line.".into()),
                NoteLine::Bullet("Links open".into()),
            ]
        );
        // notes.md for another (or a prefix-matching) version is not used.
        assert!(parse_notes_md(text, "0.3.0-preview.1").is_none());
        assert!(parse_notes_md("No heading\n", "0.3.0-preview.16").is_none());
        let long = format!("# Jawjack 1.0.0\n{}", "- item\n".repeat(MAX_NOTE_LINES * 2));
        assert_eq!(
            parse_notes_md(&long, "1.0.0").unwrap().len(),
            MAX_NOTE_LINES
        );
        let wide = format!("# Jawjack 1.0.0\n- {}\n", "x".repeat(MAX_NOTE_CHARS * 2));
        assert_eq!(
            parse_notes_md(&wide, "1.0.0").unwrap(),
            [NoteLine::Bullet("x".repeat(MAX_NOTE_CHARS))]
        );
        // The latest.json summary is the fallback.
        let summary = release(
            parse(
                200,
                r#"{"version":"9.9.9","channel":"stable","notes":"Short `summary`."}"#,
            )
            .unwrap(),
        );
        assert_eq!(summary.summary.as_deref(), Some("Short summary."));
    }

    #[test]
    fn conditional_requests_echo_only_safe_validators_and_reuse_304() {
        assert!(conditional_headers(None).is_empty());
        let cache = FeedCache {
            etag: safe_validator(Some("\"abc\"".into())),
            last_modified: safe_validator(Some("Mon, 12 Oct 2026 10:00:00 GMT".into())),
            body: simple("999.0.0", "stable").into_bytes(),
        };
        assert_eq!(
            conditional_headers(Some(&cache)),
            [
                ("If-None-Match", "\"abc\"".to_owned()),
                (
                    "If-Modified-Since",
                    "Mon, 12 Oct 2026 10:00:00 GMT".to_owned()
                )
            ]
        );
        assert_eq!(safe_validator(Some("a\r\nX-Evil: 1".into())), None);
        assert_eq!(
            safe_validator(Some("x".repeat(MAX_VALIDATOR_LENGTH + 1))),
            None
        );
        assert_eq!(safe_validator(Some(String::new())), None);
        // A 304 reuses the cached body and keeps the cache.
        let (result, new_cache) = interpret_feed(feed(304, ""), Some(cache.clone()));
        assert!(release(result.unwrap()).newer);
        assert!(new_cache.is_none());
        assert!(interpret_feed(feed(304, ""), None).0.is_err());
        // A 200 with validators becomes the new cache; without, none.
        let mut fresh = feed(200, &simple("0.0.1", "stable"));
        fresh.etag = Some("\"v2\"".into());
        fresh.last_modified = Some("bad\nvalue".into());
        let (_, new_cache) = interpret_feed(fresh, Some(cache));
        let new_cache = new_cache.unwrap();
        assert_eq!(new_cache.etag.as_deref(), Some("\"v2\""));
        assert_eq!(new_cache.last_modified, None);
        assert!(
            interpret_feed(feed(200, &simple("0.0.1", "stable")), None)
                .1
                .is_none()
        );
    }

    #[test]
    fn saved_installer_verifies_size_and_hash() {
        let good = Installer {
            file_name: installer_file_name("9.9.9"),
            size: 3,
            sha256: ABC_SHA256.into(),
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
    fn update_mode_defaults_to_notify_and_migrates_the_old_switch() {
        assert_eq!(UpdateMode::load(&serde_json::json!({})), UpdateMode::Notify);
        for mode in UpdateMode::ALL {
            let saved = serde_json::json!({ "update_mode": mode.key() });
            assert_eq!(UpdateMode::load(&saved), mode);
        }
        let old_on = serde_json::json!({ "update_checks": true });
        let old_off = serde_json::json!({ "update_checks": false });
        assert_eq!(UpdateMode::load(&old_on), UpdateMode::Notify);
        assert_eq!(UpdateMode::load(&old_off), UpdateMode::Off);
        // The new key wins over the old switch.
        let both = serde_json::json!({ "update_mode": "notify", "update_checks": false });
        assert_eq!(UpdateMode::load(&both), UpdateMode::Notify);
        // Modes that once downloaded by themselves load as Notify me.
        for retired in ["automatic", "check_only"] {
            let saved = serde_json::json!({ "update_mode": retired });
            assert_eq!(UpdateMode::load(&saved), UpdateMode::Notify, "{retired}");
        }
        let unknown = serde_json::json!({ "update_mode": "sometimes", "update_checks": false });
        assert_eq!(UpdateMode::load(&unknown), UpdateMode::Off);
    }

    #[test]
    fn only_the_installed_default_profile_copy_updates_itself() {
        let installed = Path::new(r"C:\Users\A\AppData\Local\Programs\Jawjack\Jawjack.exe");
        let same = Path::new(r"c:\users\a\appdata\local\programs\jawjack\JAWJACK.EXE");
        let build = Path::new(r"E:\git\chatterino7\recreation\target\release\chat-workbench.exe");
        assert_eq!(
            Eligibility::classify(false, Some(same), Some(installed)),
            Eligibility::Installed
        );
        // A profile is excluded even when it runs the installed executable.
        assert_eq!(
            Eligibility::classify(true, Some(installed), Some(installed)),
            Eligibility::SeparateProfile
        );
        assert_eq!(
            Eligibility::classify(false, Some(build), Some(installed)),
            Eligibility::NotInstalled
        );
        assert_eq!(
            Eligibility::classify(false, None, Some(installed)),
            Eligibility::NotInstalled
        );
        assert_eq!(
            Eligibility::classify(false, Some(installed), None),
            Eligibility::NotInstalled
        );
        assert_eq!(Eligibility::Installed.reason(), None);
        assert!(
            Eligibility::NotInstalled
                .reason()
                .unwrap()
                .contains("install Jawjack")
        );
    }

    #[test]
    fn background_checks_run_only_for_an_eligible_copy_with_notify() {
        let now = Instant::now();
        let notify = Updates::with_policy(UpdateMode::Notify, Eligibility::Installed, now);
        assert_eq!(notify.next_auto_check, Some(now + STARTUP_DELAY));
        for (mode, eligibility) in [
            (UpdateMode::Off, Eligibility::Installed),
            (UpdateMode::Notify, Eligibility::SeparateProfile),
            (UpdateMode::Notify, Eligibility::NotInstalled),
        ] {
            let mut state = Updates::with_policy(mode, eligibility, now);
            assert_eq!(state.next_auto_check, None, "{mode:?} {eligibility:?}");
            assert!(!state.poll_schedule(now + POLL_INTERVAL * 100));
            assert!(!state.checking());
        }
        let mut state = notify;
        state.apply_mode(UpdateMode::Off, now);
        assert_eq!(state.next_auto_check, None);
        state.apply_mode(UpdateMode::Notify, now);
        assert_eq!(state.next_auto_check, Some(now), "enabling checks now");
        // Not yet due: nothing starts.
        let mut fresh = Updates::with_policy(UpdateMode::Notify, Eligibility::Installed, now);
        assert!(!fresh.poll_schedule(now));
        assert!(!fresh.checking());
        // A busy tick is skipped and rescheduled, with no backoff.
        fresh.phase = Phase::Downloading;
        let due = now + STARTUP_DELAY;
        assert!(!fresh.poll_schedule(due));
        assert_eq!(fresh.next_auto_check, Some(due + POLL_INTERVAL));
        fresh.phase = Phase::Idle;
        fresh.install_confirmed = true;
        assert!(!fresh.poll_schedule(due + POLL_INTERVAL));
        assert_eq!(fresh.next_auto_check, Some(due + POLL_INTERVAL * 2));
    }

    #[test]
    fn installer_arguments_are_quiet_and_always_relaunch() {
        let result = Path::new(r"C:\data\update-result.json");
        let args: Vec<String> = installer_args(42, Some(result))
            .into_iter()
            .map(|arg| arg.into_string().unwrap())
            .collect();
        assert_eq!(
            args,
            [
                "--app-pid",
                "42",
                "--quiet",
                "--relaunch",
                "--result-file",
                r"C:\data\update-result.json"
            ]
        );
        assert_eq!(
            installer_args(42, None),
            ["--app-pid", "42", "--quiet", "--relaunch"]
        );
    }

    #[test]
    fn install_results_report_failures_once_and_current_successes_only() {
        let ok = parse_install_result(
            format!(r#"{{"ok":true,"version":"{CURRENT_VERSION}","message":""}}"#).as_bytes(),
        )
        .unwrap();
        assert!(ok.ok);
        assert!(parse_install_result(br#"{"ok":true,"version":"0.0.1","message":""}"#).is_none());
        let failed = parse_install_result(
            format!(
                r#"{{"ok":false,"version":"0.9.0-preview.169","message":"Disk full\u0007 {}"}}"#,
                "x".repeat(2000)
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(!failed.ok);
        assert_eq!(failed.version, "0.9.0-preview.169");
        assert!(
            failed.message.starts_with("Disk full x"),
            "{}",
            failed.message
        );
        assert_eq!(failed.message.chars().count(), MAX_RESULT_MESSAGE);
        assert!(parse_install_result(b"not json").is_none());
        let mut state = Updates::new();
        state.install_result = Some(failed);
        assert_eq!(
            state.status_label(),
            "Update 0.9.0-preview.169 was not installed"
        );
        let text = state.take_last_install_failure().unwrap();
        assert!(text.starts_with("Update 0.9.0-preview.169 was not installed"));
        assert!(state.take_last_install_failure().is_none(), "reported once");
        assert!(state.state().install_failure.is_some(), "still listed");

        // The file is read once and removed.
        let directory = scratch("result");
        let path = directory.join(RESULT_FILE);
        std::fs::write(&path, br#"{"ok":false,"version":"","message":"x"}"#).unwrap();
        assert!(take_install_result(&path).is_some());
        assert!(!path.exists());
        assert!(take_install_result(&path).is_none());
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }

    fn newer(version: &str) -> Result<CheckResult, String> {
        Ok(CheckResult::Release(Release {
            version: version.into(),
            newer: true,
            installer: Some(Installer {
                file_name: installer_file_name(version),
                size: 2_500_000,
                sha256: "a".repeat(64),
            }),
            summary: Some("Summary.".into()),
        }))
    }

    #[test]
    fn check_results_follow_the_t3_transitions() {
        let now = Instant::now();
        let mut state = Updates::with_policy(UpdateMode::Notify, Eligibility::Installed, now);
        // A failed check with nothing known is a check error, retried later.
        state.apply_check(Err("offline".into()), None);
        assert_eq!(state.phase(), Phase::Error);
        assert_eq!(state.error_context(), Some(ErrorContext::Check));
        assert!(state.can_retry());
        // Found: available, announced once, notes from the summary.
        state.apply_check(newer("9.9.9"), None);
        assert_eq!(state.phase(), Phase::Available);
        assert_eq!(state.error_context(), None);
        assert_eq!(
            state.take_announcement(),
            Some(Announcement::Available("9.9.9".into()))
        );
        assert_eq!(state.take_announcement(), None);
        assert_eq!(state.notes(), [NoteLine::Text("Summary.".into())]);
        assert_eq!(state.status_label(), "Jawjack 9.9.9 is available · 2.5 MB");
        assert_eq!(state.install_action_label().as_deref(), Some("Download"));
        let view = state.state();
        assert_eq!(view.available_version.as_deref(), Some("9.9.9"));
        assert_eq!(view.size, Some(2_500_000));
        assert!(view.can_download);
        // Nothing downloads by itself: checks never start a download.
        assert!(state.download_rx.is_none());
        // The same version again is not announced again; a failed check or
        // a missing feed never hides it.
        state.apply_check(newer("9.9.9"), None);
        state.apply_check(Err("offline".into()), None);
        state.apply_check(Ok(CheckResult::Missing), None);
        assert_eq!(state.phase(), Phase::Available);
        assert_eq!(state.take_announcement(), None);
        // A failed download returns to Available with a retry.
        state.fail_download("The download failed.".into());
        assert_eq!(state.phase(), Phase::Available);
        assert_eq!(state.error_context(), Some(ErrorContext::Download));
        assert!(state.can_retry());
        assert_eq!(
            state.install_action_label().as_deref(),
            Some("Retry download")
        );
        // A background check keeps that retry visible.
        state.apply_check(newer("9.9.9"), None);
        assert_eq!(state.error_context(), Some(ErrorContext::Download));
        // A newer release replaces it and is announced.
        state.apply_check(newer("9.9.10"), None);
        assert_eq!(state.error_context(), None);
        assert_eq!(
            state.take_announcement(),
            Some(Announcement::Available("9.9.10".into()))
        );
        // A check result arriving during a download is ignored.
        state.phase = Phase::Downloading;
        state.apply_check(newer("9.9.11"), None);
        assert_eq!(state.phase(), Phase::Downloading);
        assert_eq!(state.available_version(), Some("9.9.10"));
        state.phase = Phase::Available;
        // Up to date clears the known update.
        state.apply_check(
            Ok(CheckResult::Release(Release {
                version: CURRENT_VERSION.into(),
                newer: false,
                installer: None,
                summary: None,
            })),
            None,
        );
        assert_eq!(state.phase(), Phase::UpToDate);
        assert!(!state.update_known());
        state.apply_check(Ok(CheckResult::Missing), None);
        assert_eq!(state.phase(), Phase::Unpublished);
    }

    #[test]
    fn ineligible_copies_see_updates_but_never_download() {
        let now = Instant::now();
        for eligibility in [Eligibility::SeparateProfile, Eligibility::NotInstalled] {
            let mut state = Updates::with_policy(UpdateMode::Notify, eligibility, now);
            assert!(!state.eligible());
            assert!(state.ineligibility_reason().is_some());
            state.apply_check(newer("9.9.9"), None);
            assert_eq!(state.phase(), Phase::Available);
            assert!(!state.can_download());
            assert!(!state.start_download());
            assert!(state.request_install().is_err());
            assert!(state.launch_installer().is_err());
            assert_eq!(state.install_action_label(), None);
            assert_eq!(
                state.download_unavailable_reason(),
                state.ineligibility_reason()
            );
        }
    }

    fn wait_for(state: &mut Updates, done: impl Fn(&Updates) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(state) {
            assert!(
                Instant::now() < deadline,
                "timed out in {}",
                state.phase().key()
            );
            state.poll_check();
            state.poll_download();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn fixture_feed(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let directory = scratch(name);
        let installer = directory.join("installer.exe");
        std::fs::write(&installer, b"abc").unwrap();
        let feed = directory.join("feed.json");
        std::fs::write(
            &feed,
            format!(
                r###"{{"status":200,"body":{{"version":"9.9.9","channel":"stable","setup":{{
                "file":"Jawjack-9.9.9-setup.exe","size":3,"sha256":"{ABC_SHA256}"}}}},
                "notes_md":"## Jawjack 9.9.9\n\n- Faster.\n"}}"###
            ),
        )
        .unwrap();
        (directory, feed, installer)
    }

    #[test]
    fn click_to_download_then_confirmed_install_and_quit_never_does() {
        let (directory, feed, installer) = fixture_feed("flow");
        let now = Instant::now();
        let mut state = Updates::with_policy(UpdateMode::Notify, Eligibility::Installed, now);
        state.set_test_fixtures(feed, installer);
        assert!(state.poll_schedule(now + STARTUP_DELAY), "due check starts");
        assert_eq!(
            state.next_auto_check,
            Some(now + STARTUP_DELAY + POLL_INTERVAL)
        );
        wait_for(&mut state, |state| !state.checking());
        assert_eq!(
            state.phase(),
            Phase::Available,
            "nothing downloads by itself"
        );
        assert_eq!(state.notes(), [NoteLine::Bullet("Faster.".into())]);
        assert_eq!(
            state.take_announcement(),
            Some(Announcement::Available("9.9.9".into()))
        );
        assert!(!state.install_confirmed(), "quitting never installs");
        assert!(state.launch_installer().is_err(), "nothing to install");
        // Click Download.
        assert!(state.start_download());
        assert!(state.busy());
        wait_for(&mut state, |state| state.install_ready());
        assert_eq!(
            state.take_announcement(),
            Some(Announcement::Downloaded("9.9.9".into()))
        );
        assert_eq!(state.percent(), Some(100));
        assert_eq!(
            state.install_action_label().as_deref(),
            Some("Restart and install")
        );
        // An unconfirmed launch is refused: only a confirmed click installs.
        assert!(state.launch_installer().is_err());
        assert!(state.install_ready());
        // A check for the same version keeps the download.
        assert!(state.check_now());
        wait_for(&mut state, |state| !state.checking());
        assert!(state.install_ready());
        assert_eq!(state.take_announcement(), None);
        // Confirm, cancel, confirm again, then install.
        state.request_install().unwrap();
        assert!(state.install_confirmed());
        assert!(!state.check_now(), "no checks once install is confirmed");
        state.cancel_install();
        assert!(!state.install_confirmed());
        state.request_install().unwrap();
        let path = state.launch(false).unwrap();
        assert_eq!(path.file_name().unwrap(), "Jawjack-9.9.9-setup.exe");
        let args = state.launched_args().unwrap();
        assert!(args.iter().any(|arg| arg == "--relaunch"), "{args:?}");
        assert!(args.iter().any(|arg| arg == "--quiet"), "{args:?}");
        assert_eq!(state.phase(), Phase::Installing);
        assert_eq!(state.available_version(), Some("9.9.9"));
        assert!(state.launch(false).is_err(), "never started twice");
        assert!(!state.check_now(), "no checks while installing");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }

    #[test]
    fn a_failed_install_start_stays_downloaded_with_a_retry() {
        let (directory, feed, installer) = fixture_feed("start");
        let mut state =
            Updates::with_policy(UpdateMode::Off, Eligibility::Installed, Instant::now());
        state.set_test_fixtures(feed, installer);
        assert!(state.check_now());
        wait_for(&mut state, |state| !state.checking());
        assert!(state.start_download());
        wait_for(&mut state, |state| state.install_ready());
        state.request_install().unwrap();
        // Simulate setup failing to start.
        state.install_confirmed = false;
        state.error = Some((ErrorContext::Install, "could not start".into()));
        assert_eq!(state.phase(), Phase::Downloaded);
        assert!(state.can_retry());
        assert_eq!(
            state.install_action_label().as_deref(),
            Some("Retry install")
        );
        drop(state);
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }

    #[test]
    fn a_changed_download_is_rejected_before_install() {
        let (directory, feed, installer) = fixture_feed("tamper");
        let mut state =
            Updates::with_policy(UpdateMode::Off, Eligibility::Installed, Instant::now());
        state.set_test_fixtures(feed, installer);
        assert!(state.check_now());
        wait_for(&mut state, |state| !state.checking());
        assert!(state.start_download());
        wait_for(&mut state, |state| state.install_ready());
        // Simulate a held file that no longer matches its published hash.
        if let Some(held) = state.downloaded.as_mut() {
            held.installer.sha256 = "0".repeat(64);
        }
        let folder = state
            .downloaded
            .as_ref()
            .unwrap()
            .path
            .parent()
            .unwrap()
            .to_owned();
        assert!(state.request_install().is_err());
        assert_eq!(state.phase(), Phase::Available);
        assert_eq!(state.error_context(), Some(ErrorContext::Download));
        assert!(state.can_retry());
        assert!(!folder.exists(), "the rejected download is removed");
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }
}
