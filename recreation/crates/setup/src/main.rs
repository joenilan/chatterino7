//! Jawjack per-user setup: install/upgrade and uninstall without admin rights.
//!
//! * Installs `Jawjack.exe` to `%LOCALAPPDATA%\Programs\Jawjack` from the
//!   SHA-256 verified payload appended to this executable, with a Start Menu
//!   (and optional Desktop) shortcut and an HKCU Apps & Features entry.
//! * Never ends a process: a running Jawjack is asked to close (WM_CLOSE) and
//!   setup stops if it stays open. The in-app updater starts setup with
//!   `--app-pid <pid> --quiet --relaunch --result-file <file>`: setup waits
//!   for that process (also a portable `chat-workbench.exe`) to exit, writes
//!   its outcome as JSON and starts the installed Jawjack again.
//! * One setup runs at a time (named mutex `Local\JawjackSetup`).
//! * Upgrades keep the user's settings in `%LOCALAPPDATA%\ChatWorkbench`.
//! * Uninstall removes the app, shortcuts and Apps & Features entry and keeps
//!   user data unless asked to remove it.
//!
//! `--test-root <dir>` (test only) redirects every location, including the
//! registry, to a disposable sandbox so tests never touch the real install.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod payload;
#[cfg(windows)]
mod setup;
#[cfg(windows)]
mod sha;
#[cfg(windows)]
mod sys;

#[cfg(windows)]
fn main() {
    std::process::exit(setup::run());
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Jawjack setup supports Windows only.");
    std::process::exit(1);
}
