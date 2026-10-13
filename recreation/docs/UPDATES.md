# Updates, previews and the dev profile

Jawjack follows Haiwire's update flow (itself adapted from T3 Code's desktop
updater): **notify → click to download → confirm to install**. The spec is
`E:\git\xhw_haiwire\docs\design\2026-10-12-update-flow.md`; the logic is
`crates/desktop/src/updates.rs`, the UI `crates/desktop/src/workspace/update_ui.rs`.

## Versions

`[workspace.package] version` in `recreation/Cargo.toml` is the product version:
`0.1.0-preview.N` for rolling previews, then a higher base for beta
(`0.2.0-beta.1`; SemVer sorts `beta` before `preview`). Never republish a
version with different bytes.

## Feed

`https://apps.zombie.digital/downloads/jawjack/latest.json` (`version`,
`channel` preview|beta|stable, `notes`, `setup {file, size, sha256}`), with
`notes.md` beside it. Feed URLs are ignored: the installer is always
`/downloads/jawjack/archive/<v>/Jawjack-<v>-setup.exe`. Release history:
`https://apps.zombie.digital/apps/jawjack/releases`.

## Behaviour

- **Settings › General › Updates:** Notify me (default) or Off. Notify me checks
  15 s after launch and then every 4 minutes, with conditional requests
  (ETag / Last-Modified); Off checks only when asked.
- **Only the installed copy** (`%LOCALAPPDATA%\Programs\Jawjack\Jawjack.exe` on
  the default profile) checks by itself, downloads or installs. Dev builds,
  portable copies and other profiles can check when asked but never download.
- A found update shows "Jawjack {v} is available" on the sidebar update button
  and a notification (also a Windows one while Jawjack is in the background),
  once per version per session. Clicking opens the release notes with
  **Download**. The download is size-capped and SHA-256 checked while
  streaming and again after reopening it deny-write.
- **Restart and install** always asks first. Jawjack saves, starts
  `Jawjack-<v>-setup.exe --app-pid <pid> --quiet --relaunch --result-file <data>\update-result.json`
  and closes. Setup waits for it to exit, installs, keeps settings and
  sign-ins, writes its result and starts Jawjack again (also after a failure).
  The next launch reports a failed install once. **Quitting never installs.**
- `inspect` on the control channel includes `updates` (phase, versions,
  eligibility, mode) and `profile`.

## Dev profile

`--profile dev` (or `JAWJACK_PROFILE=dev`) runs a development copy with its own
settings (`%LOCALAPPDATA%\Jawjack-Dev`) and its own Credential Manager entries
(`https://jawjack.zombie.digital/dev/...`), shown as a "Dev" chip and "Jawjack
Dev" title. It never shares a sign-in with the installed app (two copies racing
for one-use Twitch refresh tokens sign each other out). One window runs per
profile; a second launch brings the first forward.

## Releasing

See [RELEASING.md](RELEASING.md) and [INSTALLER.md](INSTALLER.md). Building and
publishing tooling lives in the git-ignored `recreation/.local-release/`.
