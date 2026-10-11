# Installer and release packaging

Ported from Haiwire's `apps/haiwire-setup` and `.local-release/` scripts.
Code: `crates/setup` (package and binary `jawjack-setup`), `scripts/`.

## What the installer does

`Jawjack-<v>-setup.exe` is a per-user installer and uninstaller. It needs no
administrator rights and uses no external tools.

- **Payload:** the packager appends the app folder to `jawjack-setup.exe`. The
  folder holds `Jawjack.exe` (the release `chat-workbench.exe`, renamed),
  `README.txt`, `notes.md` and `THIRD_PARTY_NOTICES.txt`. The payload is zlib
  compressed. A 64-byte `JJSETUP1` trailer carries the SHA-256 of the
  compressed stream, and every file has its own SHA-256. Setup verifies all of
  them before it writes anything, and checks each file again after writing it.
  A plain `cargo build` has no payload, so it refuses to install (exit 4). The
  format allows for an Authenticode signature added later.
- **Install location:** `%LOCALAPPDATA%\Programs\Jawjack\`, containing
  `Jawjack.exe`, the files above, and `uninstall.exe` (setup without its payload).
- **Shortcuts:** Start Menu `Jawjack.lnk`. A Desktop shortcut is optional: a
  checkbox, on by default for a first install and kept on upgrade if one exists.
- **Apps & Features:** `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Jawjack`
  with DisplayName, DisplayVersion, Publisher "Zombie Digital", icon, install
  location, uninstall and quiet-uninstall commands, and EstimatedSize.
- **Atomic replace:** files are written to a hidden staging folder next to the
  install folder, then swapped in by rename. If anything fails, the previous
  install is restored.
- **Safety checks:** an existing install folder is replaced only if it is empty
  or already holds a Jawjack install. A folder that is a link (reparse point)
  is refused.
- **Running app:** setup finds a Jawjack running from the install folder. With
  `--app-pid`, it also finds that process (`Jawjack.exe` or a portable
  `chat-workbench.exe`). It asks the app to close with WM_CLOSE and waits up to
  90 s. It never force-closes the app; if Jawjack stays open, nothing is changed
  (exit 3). Quiet mode refuses straight away.
- **In-app updater:** `crates/desktop/src/updates.rs` starts
  `Jawjack-<v>-setup.exe --app-pid <pid>`, then saves and closes Jawjack. The
  final dialog offers "Launch Jawjack".
- **User data:** `%LOCALAPPDATA%\ChatWorkbench` (settings, workspace) is never
  touched by install or upgrade. Twitch sign-ins live in Windows Credential
  Manager, and setup never touches them.

Dropped from Haiwire: the browser connector, the worker/connector host
handling, the start-at-login Run entry, and the bundled ConPTY files.

Jawjack has no `.ico` yet, so no icon resource is embedded and dialogs show no
main icon. The application manifest (`asInvoker`, Common Controls 6 for task
dialogs, PerMonitorV2 DPI) is still embedded. `build.rs` passes it through the
MSVC linker (`/MANIFESTINPUT`) instead of `embed-resource`.

### Uninstall

Use Apps & Features → Jawjack → Uninstall, or run
`%LOCALAPPDATA%\Programs\Jawjack\uninstall.exe --uninstall`. The uninstaller
copies itself to `%TEMP%\jawjack-uninstall-*` and runs from there, so it can
delete the install folder. It closes Jawjack the same way the installer does,
then removes the app folder, both shortcuts and the registry entry.
`%LOCALAPPDATA%\ChatWorkbench` is kept unless the user ticks "Also remove my
Jawjack settings and data" (or passes `--remove-data`). The temporary copy
deletes itself afterwards.

### Command line

| Option | Meaning |
|---|---|
| `--quiet` / `/S` | No dialogs; print status to stdout |
| `--desktop-shortcut` / `--no-desktop-shortcut` | Override the desktop-shortcut default |
| `--launch` | Quiet install: start Jawjack afterwards |
| `--uninstall [--remove-data]` | Uninstall (optionally delete settings) |
| `--app-pid <pid>` | Also wait for this Jawjack process to close (used by the updater) |
| `--payload-info` | Print the payload's version and file hashes as JSON; changes nothing |
| `--test-root <dir>` | Test only: every path and registry key goes into a sandbox (see below) |

Exit codes: 0 OK, 1 failed, 2 cancelled, 3 Jawjack still running, 4 payload missing/damaged.

## Packaging

From `recreation/` (Python 3.11+ and the pinned toolchain via rustup):

```powershell
# Release build of committed HEAD only (recommended for anything you publish):
.\scripts\Build-Release.ps1 -NotesFile path\to\notes.md

# Or package the working tree (builds chat-workbench + jawjack-setup first):
python scripts/Package-Release.py --notes-file path\to\notes.md
python scripts/Package-Release.py --notes-file notes.md --no-build   # reuse target/release
```

- `Build-Release.ps1` exports `HEAD:recreation` to `target/release-src`, builds
  there into the normal `target/` directory, then runs `Package-Release.py --no-build`.
  This replaces `target/release/chat-workbench.exe` with the HEAD build.
- `Package-Release.py` warns, and records in `PROVENANCE.txt`, when the working
  tree has uncommitted changes.
- The version is `[workspace.package].version` in `Cargo.toml`. It must be
  `X.Y.Z`, `X.Y.Z-preview.N` or `X.Y.Z-beta.N`. The channel comes from the version.
- Optional signing: `--sign-thumbprint <sha1>` or `JAWJACK_SIGN_THUMBPRINT`
  (signtool from the Windows SDK, or `JAWJACK_SIGNTOOL`). Without one, signing is skipped.

Output in `release-out/<version>/` (git-ignored; an existing folder is never overwritten):

| File | Purpose |
|---|---|
| `Jawjack-<v>-setup.exe` + `.sha256` | Installer (primary download) |
| `Jawjack-<v>-x64-portable.zip` + `.sha256` | Portable app folder |
| `latest.json` | Updater feed: `version`, `channel`, `publishedAt`, `notes`, `setup {file, size, sha256}`, plus `files`/`notesFile` |
| `manifest.json`, `notes.md`, `PROVENANCE.txt` | Release record |
| `app/Jawjack/` | The unpacked app folder that was packaged |

After assembling the setup, the packager runs `--payload-info` on it (read-only)
and checks that the version, file names, sizes and hashes match.

### Sandbox test

```powershell
python scripts/Test-InstallerSandbox.py            # add --with-app to also launch the app
```

The test installs into a temp folder through `--test-root`. That folder must
contain `.jawjack-setup-test-root`. Registry writes go below
`HKCU\Software\JawjackTest\Setup-<hash>` and are removed at the end. The test
covers:

- damaged-payload refusal
- install
- file hashes
- the registry entry
- upgrade, keeping user data
- uninstall through the registered uninstaller, keeping data
- uninstall with `--remove-data`

It snapshots the real install, the shortcuts, the Apps & Features key and the
data-folder presence, and fails if any of them changed.

## Publishing (owner only)

No publish script is ported. Uploading to `apps.zombie.digital` is the owner's
step. The updater expects:

- `https://apps.zombie.digital/downloads/jawjack/latest.json`, the feed
- `https://apps.zombie.digital/downloads/jawjack/archive/<v>/Jawjack-<v>-setup.exe`.
  The app builds this path itself and ignores any URL in the feed.
- the portable zip, `.sha256` files and `notes.md` beside it in `archive/<v>/`,
  linked from the release page `https://apps.zombie.digital/jawjack/releases`

Order, following Haiwire's publisher:

1. Upload everything except `latest.json` to `archive/<v>/`.
2. Check the remote SHA-256 of the setup file against `latest.json`.
3. Upload `latest.json` last, because it is the switch that announces the release.

Never reuse or downgrade a published version. The download page should explain
SmartScreen ("More info → Run anyway") and Smart App Control; see
[RELEASING.md](RELEASING.md).

## Verified vs not verified (2026-10-11)

### Verified

- **Build:** `cargo build -p jawjack-setup --release` gives no warnings, and
  `cargo clippy -p jawjack-setup` is clean.
- **Unit tests (6):**
  - payload parsing and rejection: path traversal, missing app file, bad hash, bad version
  - SHA-256 known vector
  - path containment
  - the `--test-root` marker and layout
- **Manifest:** the application manifest is embedded in `jawjack-setup.exe`.
- **No payload:** the plain build refuses to install (exit 4).
- **Packaging:** `Package-Release.py` ran locally and produced
  `Jawjack-0.1.0-setup.exe` (about 14.4 MB) and `Jawjack-0.1.0-x64-portable.zip`
  (about 14.0 MB). The setup's `--payload-info` matched the packaged files.
- **Feed:** `latest.json` passes the rules in `updates.rs`:
  - file name `Jawjack-<v>-setup.exe`
  - size within 1 byte–256 MiB
  - lowercase 64-hex SHA-256 that matches the file
  - a valid channel
- **Sandbox test:** `Test-InstallerSandbox.py` passed (without `--with-app`):
  - damaged payload refused
  - install, file hashes, uninstall image and registry values checked
  - upgrade kept data and shortcuts
  - the registered uninstaller ran from `%TEMP%` and kept data
  - `--remove-data` removed data
  - the real profile was unchanged, and the test registry key and temp folders were removed

### Not verified

- A real install into the owner's profile and Apps & Features.
- The interactive task dialogs, and the WM_CLOSE flow against a running Jawjack
  (`--with-app` was not run).
- The in-app updater → setup handoff end to end.
- Signing.
- A second, clean machine (MSVC runtime, SmartScreen).
- `Build-Release.ps1`: it was written but not run. The local package above came
  from the working tree, which had another developer's uncommitted changes.
