# Updates and versioning

Ported from Haiwire's app-side updater (`crates/desktop/src/updates.rs`).

- **Version:** `env!("CARGO_PKG_VERSION")` of the workspace (`recreation/Cargo.toml`,
  currently 0.1.0). Use Haiwire's scheme for releases: `0.1.0-preview.N`, then a
  higher base for beta; never reuse a published version.
- **Feed:** `https://apps.zombie.digital/downloads/jawjack/latest.json`, the same
  schema as Haiwire's (`version`, `channel`, `publishedAt`, `notes`, `setup {file, size, sha256}`).
  The download URL is derived, not taken from the feed:
  `/downloads/jawjack/archive/<v>/Jawjack-<v>-setup.exe`. A 404 shows "No releases
  published yet".
- **Checks:** 30 s after launch and every 6 hours while "Check automatically" is
  on (Appearance & memory → Updates; saved in the workspace file), plus the ⟳
  button. Nothing downloads or installs without a click.
- **Install:** download is size-capped and SHA-256 verified, then held
  deny-write. Install launches `Jawjack-<v>-setup.exe --app-pid <pid>`, saves the
  workspace and closes Jawjack. The installer must wait for that process to exit,
  replace files without touching `%LOCALAPPDATA%\ChatWorkbench`, and relaunch.
- **Release channels:** see [RELEASING.md](RELEASING.md).
- **UI:** version, status and ⟳ at the bottom of the workspaces sidebar (as in
  Haiwire); full controls in Appearance & memory. A status-bar notice announces a
  new version once.

Permissions follow the same idea: when a newer build requests Twitch scopes the
saved login lacks, the account button shows `•` and the status bar says so once
per launch; see [STREAMER_TOOLS.md](STREAMER_TOOLS.md).

## Not done yet

- A Jawjack installer: port `xhw_haiwire/apps/haiwire-setup` (payload format,
  WM_CLOSE wait, per-user install, Apps & Features entry, rollback).
- Release build/package/publish scripts (Haiwire keeps them in its ignored `.local-release/`).
- Code signing. The SHA-256 detects corruption but not a compromised server.

## Evidence (Windows, 2026-10-11)

The sidebar footer showed `v0.1.0 · No releases published yet` after the startup
check, because the feed doesn't exist yet. Download and install have not been tested.
