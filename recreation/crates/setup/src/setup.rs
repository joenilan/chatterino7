//! Install, upgrade and uninstall flows.

use crate::{payload, sha, sys};
use std::{
    fs,
    io::Write,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const EXIT_OK: i32 = 0;
const EXIT_FAILED: i32 = 1;
const EXIT_CANCELLED: i32 = 2;
const EXIT_APP_RUNNING: i32 = 3;
const EXIT_DAMAGED: i32 = 4;
const EXIT_BUSY: i32 = 5;

const APP_EXE: &str = "Jawjack.exe";
/// The release binary's own name, as run from a portable copy.
const PORTABLE_EXE: &str = "chat-workbench.exe";
const UNINSTALLER: &str = "uninstall.exe";
const LINK_NAME: &str = "Jawjack.lnk";
const UNINSTALL_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Jawjack";
const TEST_KEY_ROOT: &str = r"Software\JawjackTest";
const TEST_MARKER: &str = ".jawjack-setup-test-root";
const RELEASE_PAGE: &str = "https://apps.zombie.digital/jawjack/releases";
/// Jawjack keeps its settings here (crates/desktop/src/storage.rs).
const DATA_DIR_NAME: &str = "ChatWorkbench";
const DESCRIPTION: &str = "Jawjack chat client for Twitch";
const CLOSE_WAIT: Duration = Duration::from_secs(90);
/// Named mutex: one setup (install or uninstall) at a time per user session.
const LOCK_NAME: &str = r"Local\JawjackSetup";
/// Upper bound for the result file's `message` (characters).
const RESULT_MESSAGE_MAX: usize = 1000;

const BUTTON_PRIMARY: i32 = 100;
const BUTTON_SECONDARY: i32 = 101;
const BUTTON_CANCEL: i32 = 2; // IDCANCEL, also returned by Escape/close.

#[derive(Default)]
struct Options {
    uninstall: bool,
    quiet: bool,
    desktop_shortcut: Option<bool>,
    remove_data: bool,
    launch: bool,
    from_temp: bool,
    payload_info: bool,
    test_root: Option<PathBuf>,
    app_pid: Option<u32>,
    /// Start the installed Jawjack after an update started by the in-app
    /// updater (also after a failed install, so the kept version returns;
    /// not when Jawjack never closed or another setup is running).
    relaunch: bool,
    /// Write the outcome here for the in-app updater to read.
    result_file: Option<PathBuf>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy().to_ascii_lowercase();
        match text.as_str() {
            "--uninstall" | "/uninstall" => options.uninstall = true,
            "--quiet" | "/quiet" | "/s" => options.quiet = true,
            "--desktop-shortcut" => options.desktop_shortcut = Some(true),
            "--no-desktop-shortcut" => options.desktop_shortcut = Some(false),
            "--remove-data" => options.remove_data = true,
            "--launch" => options.launch = true,
            "--relaunch" => options.relaunch = true,
            "--result-file" => {
                let value = PathBuf::from(args.next().ok_or("--result-file needs a file path")?);
                options.result_file = Some(value);
            }
            "--from-temp" => options.from_temp = true,
            "--payload-info" => options.payload_info = true,
            "--test-root" => {
                let value = args.next().ok_or("--test-root needs a directory")?;
                options.test_root = Some(PathBuf::from(value));
            }
            "--app-pid" => {
                let value = args.next().ok_or("--app-pid needs a process id")?;
                options.app_pid = Some(
                    value
                        .to_string_lossy()
                        .parse()
                        .map_err(|_| "--app-pid needs a process id")?,
                );
            }
            _ => return Err(format!("Unknown option {}", arg.to_string_lossy())),
        }
    }
    Ok(options)
}

/// Every location setup reads or writes.
struct Layout {
    install_dir: PathBuf,
    data_dir: PathBuf,
    start_menu_link: PathBuf,
    desktop_link: PathBuf,
    uninstall_key: String,
    test: Option<(PathBuf, String)>,
}

impl Layout {
    fn resolve(test_root: Option<&Path>) -> Result<Self, String> {
        match test_root {
            Some(root) => Self::test(root),
            None => Self::user(),
        }
    }

    fn user() -> Result<Self, String> {
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute() && path.is_dir())
            .ok_or("LOCALAPPDATA is unavailable")?;
        let programs = sys::known_folder(sys::KnownFolder::Programs)
            .map_err(|_| "The Start Menu folder is unavailable")?;
        let desktop = sys::known_folder(sys::KnownFolder::Desktop)
            .map_err(|_| "The Desktop folder is unavailable")?;
        Ok(Self {
            install_dir: local_app_data.join("Programs").join("Jawjack"),
            data_dir: local_app_data.join(DATA_DIR_NAME),
            start_menu_link: programs.join(LINK_NAME),
            desktop_link: desktop.join(LINK_NAME),
            uninstall_key: UNINSTALL_SUBKEY.to_owned(),
            test: None,
        })
    }

    /// Test-only sandbox: a marked directory; registry below JawjackTest.
    fn test(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .ok()
            .map(sys::plain_path)
            .filter(|root| root.is_dir() && root.join(TEST_MARKER).is_file())
            .ok_or(
                "--test-root must be an existing directory containing .jawjack-setup-test-root",
            )?;
        let mut hasher = sha::Sha256::new().map_err(|_| "SHA-256 is unavailable")?;
        hasher.update(root.to_string_lossy().to_lowercase().as_bytes());
        let digest = sha::hex(&hasher.finish().map_err(|_| "SHA-256 is unavailable")?);
        let key = format!(r"{TEST_KEY_ROOT}\Setup-{}", &digest[..16]);
        let local_app_data = root.join("LocalAppData");
        let start_menu = root.join("StartMenu").join("Programs");
        let desktop = root.join("Desktop");
        for directory in [&local_app_data, &start_menu, &desktop] {
            fs::create_dir_all(directory).map_err(|_| "Could not prepare the test root")?;
        }
        Ok(Self {
            install_dir: local_app_data.join("Programs").join("Jawjack"),
            data_dir: local_app_data.join(DATA_DIR_NAME),
            start_menu_link: start_menu.join(LINK_NAME),
            desktop_link: desktop.join(LINK_NAME),
            uninstall_key: format!(r"{key}\Uninstall\Jawjack"),
            test: Some((root, key)),
        })
    }

    fn app_exe(&self) -> PathBuf {
        self.install_dir.join(APP_EXE)
    }
}

struct Ui {
    quiet: bool,
    /// The payload version being installed, once known.
    version: std::cell::RefCell<String>,
    /// The last error shown, for `--result-file`.
    last_error: std::cell::RefCell<Option<String>>,
}

impl Ui {
    fn new(quiet: bool) -> Self {
        Self {
            quiet,
            version: std::cell::RefCell::default(),
            last_error: std::cell::RefCell::default(),
        }
    }

    fn info(&self, main: &str, content: &str) {
        println!("jawjack-setup: {main} {content}");
        if !self.quiet {
            sys::message(main, content, false);
        }
    }

    fn error(&self, main: &str, content: &str) {
        println!("jawjack-setup: ERROR {main} {content}");
        *self.last_error.borrow_mut() = Some(format!("{main} {content}"));
        if !self.quiet {
            sys::message(main, content, true);
        }
    }
}

pub fn run() -> i32 {
    let options = match parse_options() {
        Ok(options) => options,
        Err(error) => {
            println!("jawjack-setup: ERROR {error}");
            let quiet = std::env::args_os().skip(1).any(|arg| {
                matches!(
                    arg.to_string_lossy().to_ascii_lowercase().as_str(),
                    "--quiet" | "/quiet" | "/s"
                )
            });
            if !quiet {
                sys::message("Jawjack Setup", &error, true);
            }
            return EXIT_FAILED;
        }
    };
    let ui = Ui::new(options.quiet || options.payload_info);
    // Only a checked path is ever written. An unusable one stops the install
    // (nothing is changed) but Jawjack is still started again below.
    let (result_file, result_error) = match options.result_file.clone().map(checked_result_path) {
        Some(Ok(path)) => (Some(path), None),
        Some(Err(error)) => (None, Some(error)),
        None => (None, None),
    };
    let layout = match Layout::resolve(options.test_root.as_deref()) {
        Ok(layout) => layout,
        Err(error) => {
            ui.error("Setup could not start.", &error);
            if let Some(path) = &result_file {
                write_result(path, EXIT_FAILED, &ui);
            }
            return EXIT_FAILED;
        }
    };
    if options.uninstall {
        return uninstall(&options, &layout, &ui);
    }
    let code = match result_error.filter(|_| !options.payload_info) {
        Some(error) => {
            ui.error(
                "Setup could not start.",
                &format!("{error}. Nothing was changed."),
            );
            EXIT_FAILED
        }
        None => install(&options, &layout, &ui),
    };
    if options.payload_info {
        return code;
    }
    if let Some(path) = &result_file {
        write_result(path, code, &ui);
    }
    // Jawjack closed itself for this update: give it back, updated or not.
    // When it never closed, or another setup is running, there is nothing to
    // start. A failure before the close wait (a damaged payload) must not
    // start a second Jawjack beside the one still exiting.
    if options.relaunch
        && !matches!(code, EXIT_APP_RUNNING | EXIT_BUSY)
        && layout.app_exe().is_file()
        && options
            .app_pid
            .is_none_or(|pid| sys::wait_for_exit(&[pid], CLOSE_WAIT))
    {
        launch_app(&layout);
    }
    code
}

/// `--result-file` must be an absolute path whose folder exists, and must not
/// name a folder or a link.
fn checked_result_path(path: PathBuf) -> Result<PathBuf, String> {
    let parent_ok = path.parent().is_some_and(|parent| {
        !parent.as_os_str().is_empty() && parent.is_dir() && !is_reparse_point(parent)
    });
    if !path.is_absolute() || path.file_name().is_none() || !parent_ok {
        return Err("--result-file needs an absolute file path in an existing folder".into());
    }
    if path.is_dir() || is_reparse_point(&path) {
        return Err("--result-file must name a plain file".into());
    }
    Ok(path)
}

/// The outcome the in-app updater reads:
/// `{"version":"<v>","ok":true|false,"message":"<reason, empty on success>"}`.
fn result_json(version: &str, ok: bool, message: &str) -> String {
    let message: String = message.chars().take(RESULT_MESSAGE_MAX).collect();
    format!(
        "{{\"version\":{},\"ok\":{ok},\"message\":{}}}",
        json_string(version),
        json_string(if ok { "" } else { &message })
    )
}

/// Write the result beside its final name, then rename it over that name, so
/// a reader never sees a half-written file.
fn write_result(path: &Path, code: i32, ui: &Ui) {
    let ok = code == EXIT_OK;
    let message = ui
        .last_error
        .borrow()
        .clone()
        .unwrap_or_else(|| format!("Setup stopped (exit code {code})."));
    let text = result_json(&ui.version.borrow(), ok, &message);
    let Some(name) = path.file_name() else {
        return;
    };
    let mut temp_name = name.to_os_string();
    temp_name.push(format!(".{}.tmp", nonce()));
    let temp = path.with_file_name(temp_name);
    let write = || -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    };
    if let Err(error) = write() {
        let _ = fs::remove_file(&temp);
        println!("jawjack-setup: the result file could not be written ({error})");
    }
}

fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push('"');
    out
}

/// Hold the one-setup lock, or report that another setup is running.
fn acquire_lock(layout: &Layout, ui: &Ui) -> Result<sys::SetupLock, i32> {
    let name = match &layout.test {
        Some((_, key)) => format!("{LOCK_NAME}-{}", key.replace('\\', "-")),
        None => LOCK_NAME.to_owned(),
    };
    sys::setup_lock(&name).ok_or_else(|| {
        ui.error(
            "Another Jawjack setup is running.",
            "Let it finish; this one changed nothing.",
        );
        EXIT_BUSY
    })
}

fn nonce() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("{:x}-{:x}", std::process::id(), nanos & 0xffff_ffff_ffff)
}

fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|_| "Setup could not locate its own file".to_owned())
}

// ------------------------------------------------------------ running app

fn running_app(layout: &Layout, app_pid: Option<u32>) -> Vec<sys::RunningProcess> {
    let install = layout.install_dir.canonicalize().ok();
    sys::processes()
        .into_iter()
        .filter(|process| {
            let installed = install
                .as_deref()
                .is_some_and(|install| sys::path_within(&process.image, install));
            // The updating window may run from another (portable) location.
            let requested = app_pid == Some(process.pid)
                && process
                    .image
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.eq_ignore_ascii_case(APP_EXE)
                            || name.eq_ignore_ascii_case(PORTABLE_EXE)
                    });
            installed || requested
        })
        .collect()
}

/// Ask a running Jawjack to close; never terminate it.
fn ensure_closed(options: &Options, layout: &Layout, ui: &Ui, action: &str) -> Result<(), i32> {
    let running = running_app(layout, options.app_pid);
    if running.is_empty() {
        return Ok(());
    }
    // Started by the in-app updater: Jawjack already saved and is closing
    // itself, so wait for it (and anything else running from the install
    // folder) to exit. Never ask it to close, never end it.
    if options.quiet && options.app_pid.is_some() {
        let pids: Vec<u32> = running.iter().map(|process| process.pid).collect();
        if sys::wait_for_exit(&pids, CLOSE_WAIT) && running_app(layout, None).is_empty() {
            return Ok(());
        }
        ui.error(
            "Jawjack is still running.",
            &format!("Close Jawjack, then {action} again. Nothing was changed."),
        );
        return Err(EXIT_APP_RUNNING);
    }
    if options.quiet {
        ui.error(
            "Jawjack is running.",
            &format!("Close Jawjack, then {action} again. Nothing was changed."),
        );
        return Err(EXIT_APP_RUNNING);
    }
    let choice = sys::task_dialog(
        "Close Jawjack to continue",
        &format!(
            "Jawjack is running. Setup will ask it to close before it can {action}; \
             Jawjack saves your workspace as it closes. Setup never force-closes it."
        ),
        &[
            (BUTTON_PRIMARY, "Close Jawjack and continue"),
            (BUTTON_CANCEL, "Cancel"),
        ],
        None,
    );
    if choice.button != BUTTON_PRIMARY {
        return Err(EXIT_CANCELLED);
    }
    let pids: Vec<u32> = running.iter().map(|process| process.pid).collect();
    sys::request_close(&pids);
    let closed = sys::wait_for_exit(&pids, CLOSE_WAIT) && running_app(layout, None).is_empty();
    if !closed {
        ui.error(
            "Jawjack is still running.",
            &format!(
                "It may be waiting for you to answer a prompt. \
                 Nothing was changed. Close Jawjack, then {action} again."
            ),
        );
        return Err(EXIT_APP_RUNNING);
    }
    Ok(())
}

// ---------------------------------------------------------------- install

fn install(options: &Options, layout: &Layout, ui: &Ui) -> i32 {
    let executable = match current_exe() {
        Ok(path) => path,
        Err(error) => {
            ui.error("Setup could not start.", &error);
            return EXIT_FAILED;
        }
    };
    let payload = match payload::load(&executable) {
        Ok(payload) => payload,
        Err(error) => {
            ui.error(
                "Jawjack could not be installed.",
                &format!("{error}. Download the setup file again from {RELEASE_PAGE}."),
            );
            return EXIT_DAMAGED;
        }
    };
    *ui.version.borrow_mut() = payload.version.clone();
    if options.payload_info {
        let files: Vec<String> = payload
            .files
            .iter()
            .map(|file| {
                format!(
                    r#"{{"name":"{}","size":{},"sha256":"{}"}}"#,
                    file.name,
                    file.bytes.len(),
                    sha::hex(&file.sha256)
                )
            })
            .collect();
        println!(
            r#"{{"version":"{}","image_len":{},"files":[{}]}}"#,
            payload.version,
            payload.image_len,
            files.join(",")
        );
        return EXIT_OK;
    }

    // One setup at a time: a quiet update started by the in-app updater never
    // races a setup the user started.
    let _lock = match acquire_lock(layout, ui) {
        Ok(lock) => lock,
        Err(code) => return code,
    };

    if layout.install_dir.exists() && !owned_install_dir(&layout.install_dir) {
        ui.error(
            "Jawjack could not be installed.",
            &format!(
                "{} already exists and does not look like a Jawjack installation. \
                 Nothing was changed.",
                layout.install_dir.display()
            ),
        );
        return EXIT_FAILED;
    }
    let installed_version = sys::reg_get_string(&layout.uninstall_key, "DisplayVersion")
        .filter(|_| layout.app_exe().is_file());
    let desktop_default = options
        .desktop_shortcut
        .unwrap_or(installed_version.is_none() || layout.desktop_link.is_file());
    let desktop = if options.quiet {
        desktop_default
    } else {
        let main = match installed_version.as_deref() {
            Some(old) if old == payload.version => format!("Reinstall Jawjack {}", payload.version),
            Some(old) => format!("Update Jawjack {old} to {}", payload.version),
            None => format!("Install Jawjack {}", payload.version),
        };
        let content = format!(
            "Installs for this Windows user only, in {}. No administrator rights are \
             needed.\n\nYour settings in {} and your saved sign-ins are kept. \
             This build is unsigned; check the SHA-256 published with the release.",
            layout.install_dir.display(),
            layout.data_dir.display()
        );
        let choice = sys::task_dialog(
            &main,
            &content,
            &[
                (
                    BUTTON_PRIMARY,
                    if installed_version.is_some() {
                        "Update"
                    } else {
                        "Install"
                    },
                ),
                (BUTTON_CANCEL, "Cancel"),
            ],
            Some(("Create a desktop shortcut", desktop_default)),
        );
        if choice.button != BUTTON_PRIMARY {
            return EXIT_CANCELLED;
        }
        choice.verified
    };

    if let Err(code) = ensure_closed(options, layout, ui, "install the update") {
        return code;
    }

    if let Err(error) = place_files(layout, &payload, &executable) {
        ui.error(
            "Jawjack could not be installed.",
            &format!("{error}. Any previous installation was kept."),
        );
        return EXIT_FAILED;
    }
    let mut warnings = Vec::new();
    if let Err(error) = sys::create_shortcut(
        &layout.start_menu_link,
        &layout.app_exe(),
        &layout.install_dir,
        DESCRIPTION,
    ) {
        warnings.push(format!(
            "The Start Menu shortcut could not be created ({error})."
        ));
    }
    if desktop {
        if let Err(error) = sys::create_shortcut(
            &layout.desktop_link,
            &layout.app_exe(),
            &layout.install_dir,
            DESCRIPTION,
        ) {
            warnings.push(format!(
                "The desktop shortcut could not be created ({error})."
            ));
        }
    } else if layout.desktop_link.is_file() {
        let _ = fs::remove_file(&layout.desktop_link);
    }
    if let Err(error) = register(layout, &payload) {
        warnings.push(format!("Apps & Features registration failed ({error})."));
    }
    let notes = warnings.join(" ");

    let main = format!("Jawjack {} is installed.", payload.version);
    if options.quiet {
        println!(
            "jawjack-setup: {main}{}",
            if notes.is_empty() {
                String::new()
            } else {
                format!(" {}", notes.trim())
            }
        );
        if options.launch && !options.relaunch {
            launch_app(layout);
        }
        return EXIT_OK;
    }
    let choice = sys::task_dialog(
        &main,
        &format!(
            "Start it from the Start Menu{}.{}",
            if desktop { " or the desktop" } else { "" },
            if notes.is_empty() {
                String::new()
            } else {
                format!("\n\n{}", notes.trim())
            }
        ),
        &[
            (BUTTON_PRIMARY, "Launch Jawjack"),
            (BUTTON_SECONDARY, "Close"),
        ],
        None,
    );
    if choice.button == BUTTON_PRIMARY {
        launch_app(layout);
    }
    EXIT_OK
}

/// An existing install folder is replaced only when it is empty or holds a
/// previous Jawjack installation.
fn owned_install_dir(directory: &Path) -> bool {
    if is_reparse_point(directory) || !directory.is_dir() {
        return false;
    }
    let empty = fs::read_dir(directory).is_ok_and(|mut entries| entries.next().is_none());
    empty || directory.join(UNINSTALLER).is_file() || directory.join(APP_EXE).is_file()
}

fn is_reparse_point(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    })
}

/// Write and verify every file in a sibling staging folder, then swap it into
/// place with renames, restoring the previous installation on failure.
fn place_files(
    layout: &Layout,
    payload: &payload::Payload,
    executable: &Path,
) -> Result<(), String> {
    let parent = layout
        .install_dir
        .parent()
        .ok_or("invalid install location")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("{} could not be created ({error})", parent.display()))?;
    if is_reparse_point(parent) {
        return Err("the install location is a link".into());
    }
    let id = nonce();
    let staging = parent.join(format!(".Jawjack-setup-{id}"));
    fs::create_dir(&staging)
        .map_err(|error| format!("a staging folder could not be created ({error})"))?;
    let result = (|| -> Result<(), String> {
        for file in &payload.files {
            write_verified(&staging.join(&file.name), &file.bytes, &file.sha256)?;
        }
        // The uninstaller is this setup program without its payload.
        let image = read_prefix(executable, payload.image_len)?;
        let mut hasher = sha::Sha256::new().map_err(|_| "SHA-256 is unavailable")?;
        hasher.update(&image);
        let digest = hasher.finish().map_err(|_| "SHA-256 is unavailable")?;
        write_verified(&staging.join(UNINSTALLER), &image, &digest)
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let previous = parent.join(format!(".Jawjack-old-{id}"));
    let had_previous = layout.install_dir.exists();
    if had_previous && let Err(error) = rename_with_retry(&layout.install_dir, &previous) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!(
            "the existing installation is in use and could not be replaced ({error})"
        ));
    }
    if let Err(error) = fs::rename(&staging, &layout.install_dir) {
        if had_previous {
            let _ = fs::rename(&previous, &layout.install_dir);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(format!(
            "the new files could not be moved into place ({error})"
        ));
    }
    if had_previous {
        // Best effort; a leftover hidden folder does not affect the app.
        let _ = fs::remove_dir_all(&previous);
    }
    Ok(())
}

fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if Instant::now() >= deadline => return Err(error),
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    }
}

fn read_prefix(path: &Path, len: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::with_capacity(len as usize);
    fs::File::open(path)
        .and_then(|file| file.take(len).read_to_end(&mut bytes))
        .map_err(|_| "setup could not read its own program file".to_owned())?;
    if bytes.len() as u64 != len {
        return Err("setup could not read its own program file".into());
    }
    Ok(bytes)
}

fn write_verified(path: &Path, bytes: &[u8], expected: &[u8; 32]) -> Result<(), String> {
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let write = || -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    };
    write().map_err(|error| format!("{name} could not be written ({error})"))?;
    let written =
        fs::read(path).map_err(|error| format!("{name} could not be read back ({error})"))?;
    let mut hasher = sha::Sha256::new().map_err(|_| "SHA-256 is unavailable")?;
    hasher.update(&written);
    if &hasher.finish().map_err(|_| "SHA-256 is unavailable")? != expected {
        return Err(format!("{name} failed SHA-256 verification after writing"));
    }
    Ok(())
}

fn quoted(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn register(layout: &Layout, payload: &payload::Payload) -> std::io::Result<()> {
    let uninstaller = layout.install_dir.join(UNINSTALLER);
    let mut uninstall = format!("{} --uninstall", quoted(&uninstaller));
    if let Some((root, _)) = &layout.test {
        uninstall.push_str(&format!(" --test-root {}", quoted(root)));
    }
    let quiet = format!("{uninstall} --quiet");
    let icon = format!("{},0", layout.app_exe().display());
    let location = layout.install_dir.display().to_string();
    let size_kb = u32::try_from((payload.total_size() / 1024).max(1)).unwrap_or(u32::MAX);
    sys::reg_set_values(
        &layout.uninstall_key,
        &[
            ("DisplayName", "Jawjack"),
            ("DisplayVersion", &payload.version),
            ("Publisher", "Zombie Digital"),
            ("DisplayIcon", &icon),
            ("InstallLocation", &location),
            ("UninstallString", &uninstall),
            ("QuietUninstallString", &quiet),
            ("URLInfoAbout", RELEASE_PAGE),
        ],
        &[("NoModify", 1), ("NoRepair", 1), ("EstimatedSize", size_kb)],
    )
}

fn launch_app(layout: &Layout) {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let home = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| layout.install_dir.clone());
    let spawn = |flags| {
        std::process::Command::new(layout.app_exe())
            .current_dir(&home)
            .creation_flags(flags)
            .spawn()
    };
    if spawn(DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB).is_err() {
        let _ = spawn(DETACHED_PROCESS);
    }
}

// -------------------------------------------------------------- uninstall

fn uninstall(options: &Options, layout: &Layout, ui: &Ui) -> i32 {
    let executable = match current_exe() {
        Ok(path) => path,
        Err(error) => {
            ui.error("Setup could not start.", &error);
            return EXIT_FAILED;
        }
    };
    // The uninstaller cannot delete its own folder while running from it.
    let install = layout.install_dir.canonicalize().ok();
    if !options.from_temp
        && install
            .as_deref()
            .is_some_and(|install| sys::path_within(&executable, install))
    {
        return relaunch_from_temp(&executable, ui);
    }
    let _lock = match acquire_lock(layout, ui) {
        Ok(lock) => lock,
        Err(code) => return code,
    };

    let version = sys::reg_get_string(&layout.uninstall_key, "DisplayVersion");
    let remove_data = if options.quiet {
        options.remove_data
    } else {
        let choice = sys::task_dialog(
            &format!(
                "Uninstall Jawjack{}?",
                version
                    .as_deref()
                    .map(|v| format!(" {v}"))
                    .unwrap_or_default()
            ),
            &format!(
                "Removes the app from {} and its shortcuts.\n\nYour settings and \
                 workspace in {} are kept unless you tick the box below. Saved Twitch \
                 sign-ins stay in Windows Credential Manager either way.",
                layout.install_dir.display(),
                layout.data_dir.display()
            ),
            &[(BUTTON_PRIMARY, "Uninstall"), (BUTTON_CANCEL, "Cancel")],
            Some(("Also remove my Jawjack settings and data", false)),
        );
        if choice.button != BUTTON_PRIMARY {
            return EXIT_CANCELLED;
        }
        choice.verified
    };

    if let Err(code) = ensure_closed(options, layout, ui, "uninstall") {
        return code;
    }

    let mut notes = Vec::new();
    for link in [&layout.start_menu_link, &layout.desktop_link] {
        if link.is_file() && fs::remove_file(link).is_err() {
            notes.push(format!("{} could not be removed.", link.display()));
        }
    }

    let mut failed = false;
    if layout.install_dir.exists() {
        if is_reparse_point(&layout.install_dir) || !owned_install_dir(&layout.install_dir) {
            notes.push(format!(
                "{} was not removed because it does not look like a Jawjack installation.",
                layout.install_dir.display()
            ));
            failed = true;
        } else if let Err(error) = remove_dir_with_retry(&layout.install_dir) {
            notes.push(format!(
                "{} could not be fully removed ({error}).",
                layout.install_dir.display()
            ));
            failed = true;
        }
    }
    if !failed && let Err(error) = sys::reg_delete_tree(&layout.uninstall_key) {
        notes.push(format!(
            "The Apps & Features entry could not be removed ({error})."
        ));
    }

    if remove_data && layout.data_dir.exists() {
        if is_reparse_point(&layout.data_dir) {
            notes.push("Your data folder is a link and was not removed.".into());
        } else {
            let left = remove_tree_best_effort(&layout.data_dir);
            if left > 0 {
                notes.push(format!(
                    "{left} item(s) in {} are still in use and were kept.",
                    layout.data_dir.display()
                ));
            }
        }
    }

    if let Some((_, key)) = &layout.test {
        let _ = sys::reg_delete_tree(key);
        sys::reg_delete_if_empty(TEST_KEY_ROOT);
    }

    let data_note = if remove_data {
        "Your Jawjack settings and data were removed."
    } else {
        "Your Jawjack settings and data were kept."
    };
    let summary = format!("{data_note} {}", notes.join(" "));
    if failed {
        ui.error("Jawjack was only partly uninstalled.", summary.trim());
    } else {
        ui.info("Jawjack was uninstalled.", summary.trim());
    }
    if options.from_temp {
        schedule_self_delete(&executable);
    }
    if failed { EXIT_FAILED } else { EXIT_OK }
}

fn relaunch_from_temp(executable: &Path, ui: &Ui) -> i32 {
    let directory = std::env::temp_dir().join(format!("jawjack-uninstall-{}", nonce()));
    let copy = directory.join("jawjack-uninstall.exe");
    let prepared =
        fs::create_dir(&directory).and_then(|()| fs::copy(executable, &copy).map(|_| ()));
    if let Err(error) = prepared {
        ui.error(
            "Jawjack could not be uninstalled.",
            &format!("A temporary copy could not be made ({error})."),
        );
        return EXIT_FAILED;
    }
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    args.push("--from-temp".into());
    match std::process::Command::new(&copy)
        .args(args)
        .current_dir(&directory)
        .spawn()
    {
        Ok(_) => EXIT_OK,
        Err(error) => {
            ui.error(
                "Jawjack could not be uninstalled.",
                &format!("The uninstaller could not start ({error})."),
            );
            EXIT_FAILED
        }
    }
}

fn remove_dir_with_retry(directory: &Path) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match fs::remove_dir_all(directory) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) if Instant::now() >= deadline => return Err(error),
            Err(_) => std::thread::sleep(Duration::from_millis(300)),
        }
    }
}

/// Delete everything that is not in use; return how many entries remain.
/// Links are removed without following them.
fn remove_tree_best_effort(directory: &Path) -> usize {
    let mut left = 0;
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_reparse_point(&path) {
                let removed = fs::remove_dir(&path).or_else(|_| fs::remove_file(&path));
                left += usize::from(removed.is_err());
            } else if path.is_dir() {
                left += remove_tree_best_effort(&path);
            } else if fs::remove_file(&path).is_err() {
                left += 1;
            }
        }
    }
    if left == 0 && fs::remove_dir(directory).is_err() {
        left += 1;
    }
    left
}

fn schedule_self_delete(executable: &Path) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Some(directory) = executable.parent() else {
        return;
    };
    let name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !name.starts_with("jawjack-uninstall-") {
        return;
    }
    let command = format!(
        "ping -n 3 127.0.0.1 >nul & rmdir /s /q \"{}\"",
        directory.display()
    );
    let _ = std::process::Command::new("cmd.exe")
        .args(["/d", "/c"])
        .raw_arg(command)
        .current_dir(std::env::temp_dir())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_within_are_case_insensitive_and_bounded() {
        let base = Path::new(r"C:\Users\A\AppData\Local\Programs\Jawjack");
        assert!(sys::path_within(
            Path::new(r"c:\users\a\appdata\local\programs\jawjack\Jawjack.exe"),
            base
        ));
        assert!(!sys::path_within(
            Path::new(r"C:\Users\A\AppData\Local\Programs\Jawjack2\x.exe"),
            base
        ));
    }

    #[test]
    fn result_strings_are_valid_json() {
        let text = json_string("a \"b\" c\\d\ne");
        assert_eq!(text, r#""a \"b\" c\\d\u000ae""#);
    }

    #[test]
    fn result_json_has_the_agreed_shape() {
        assert_eq!(
            result_json("0.1.0", true, "ignored"),
            r#"{"version":"0.1.0","ok":true,"message":""}"#
        );
        assert_eq!(
            result_json("0.1.0", false, "Jawjack is still running."),
            r#"{"version":"0.1.0","ok":false,"message":"Jawjack is still running."}"#
        );
        let long = "x".repeat(5000);
        assert!(result_json("", false, &long).len() < RESULT_MESSAGE_MAX + 64);
    }

    #[test]
    fn result_path_must_be_absolute_in_an_existing_folder() {
        assert!(checked_result_path(PathBuf::from("result.json")).is_err());
        let missing = std::env::temp_dir().join(format!("jawjack-no-such-{}", nonce()));
        assert!(checked_result_path(missing.join("result.json")).is_err());
        assert!(checked_result_path(std::env::temp_dir()).is_err());
        assert!(checked_result_path(std::env::temp_dir().join("jawjack-result.json")).is_ok());
    }

    #[test]
    fn result_file_is_written_atomically() {
        let directory = std::env::temp_dir().join(format!("jawjack-result-unit-{}", nonce()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("result.json");
        fs::write(&path, b"old").unwrap();
        let ui = Ui::new(true);
        *ui.version.borrow_mut() = "0.2.0".into();
        ui.error("Jawjack could not be installed.", "disk \"full\"");
        write_result(&path, EXIT_FAILED, &ui);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{"version":"0.2.0","ok":false,"message":"Jawjack could not be installed. disk \"full\""}"#
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn second_lock_is_refused() {
        let name = format!(r"Local\JawjackSetupUnit-{}", nonce());
        let first = sys::setup_lock(&name).expect("first lock");
        assert!(sys::setup_lock(&name).is_none());
        drop(first);
        assert!(sys::setup_lock(&name).is_some());
    }

    #[test]
    fn test_root_requires_marker() {
        let root = std::env::temp_dir().join(format!("jawjack-setup-unit-{}", nonce()));
        fs::create_dir_all(&root).unwrap();
        assert!(Layout::test(&root).is_err());
        fs::write(root.join(TEST_MARKER), b"").unwrap();
        let layout = Layout::test(&root).unwrap();
        assert!(
            layout
                .uninstall_key
                .starts_with(r"Software\JawjackTest\Setup-")
        );
        let canonical = root.canonicalize().unwrap();
        assert!(sys::path_within(&layout.install_dir, &canonical));
        assert!(!layout.install_dir.to_string_lossy().starts_with(r"\\?\"));
        assert!(layout.app_exe().ends_with(r"Programs\Jawjack\Jawjack.exe"));
        assert!(layout.data_dir.ends_with(DATA_DIR_NAME));
        let _ = fs::remove_dir_all(&root);
    }
}
