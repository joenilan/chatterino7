# Jawjack

Jawjack by Zombie Digital is a native Rust + GPUI Kit Twitch client being built as
Joe's Chatterino7 replacement. It combines dark Studio styling, a custom Windows
frame and a compact channel-first layout. The original C++ application is untouched.

## Chat and workspaces

- Channel tabs with context menus, drag reordering and middle-click close confirmation.
- Recursive resizable splits and tab groups: drag to an edge to split or to the
  center/tab strip to combine. No artificial two-pane limit.
- Named workspaces managed in a collapsible, animated sidebar. The titlebar
  hamburger remembers whether it is open; there is no duplicate workspace tab bar.
- Independent channel drafts and reply targets, including across workspaces.
- Soft-wrapping composers with Twitch's 500-character message limit, ordinary
  editing shortcuts and saved drafts. Pasted line breaks become spaces.
- Live incoming chat with username colors, badges, animated Twitch/7TV emotes,
  BTTV/FFZ catalogs, a searchable emote picker and keyboard emote completion.
- Bounded slide-and-fade message entrances, visible-media animation and
  reduced-motion behavior. Retained chat history is bounded.
- Transcript selection, right-click copying with feedback, retained-message Find,
  message actions, local chatter cards, replies and retained conversation views.
- Live-only channel filtering and compact layouts down to 360×280. Normal startup
  size is 1280×820; compact support does not force a small window.
- Latest source adds unread/activity views, mention/word highlighting, font
  shortcuts and pastel live/offline/unknown indicators. Windows verification of
  this newest batch remains pending.

This is still a development client. Full Chatterino7 parity, modern Twitch rich
messages, moderation and additional providers are tracked in the
[master feature plan](docs/FEATURE_COVERAGE.md). A listed plan is not a shipped feature.

## Twitch connection

Device-code sign-in uses Jawjack's public Client ID and the OS credential vault.
No client secret is embedded. Native Windows sign-in, saved-login restoration,
authenticated incoming chat and animated Twitch/7TV media have been observed.
DinoDance animation has also been observed in the picker.

Sending is implemented with draft-preserving failure/uncertainty handling, but
actual delivered sends remain unverified. Broader refresh/revocation recovery and
moderation behavior still need real-use evidence. BTTV/FFZ catalog loading is
observed; individual provider rendering coverage remains incomplete.

See [account details](docs/TWITCH_ACCOUNT.md) and the dated
[native evidence record](docs/VALIDATION.md) for precise limits.

## Build and run

Use Rust 1.99.0 with Windows MSVC, Visual Studio 2022 C++ tools, Windows SDK and
CMake. GPUI Kit is pinned to 0.7.1. From this directory:

```sh
cargo run -p chat-workbench --release --locked
```

The Windows executable is `target/release/chat-workbench.exe` under this directory.
The project has built on Linux and Windows; platform acceptance is recorded per
revision. Build and directly interact with the actual app for development.
Commits use `[skip ci]`; no CI/Actions or separate test-script workflow is required.
Optional session-only native app control is described in
[APP_CONTROL.md](docs/APP_CONTROL.md).

## Shortcuts

- Ctrl+T: new workspace
- Ctrl+W: close workspace
- Ctrl+Shift+T: reopen recently closed workspace
- Ctrl+Tab / Ctrl+Shift+Tab: next / previous workspace
- Ctrl+PageDown / Ctrl+PageUp: next / previous channel tab
- Ctrl+K: add channel
- Ctrl+F: find retained messages; F3 / Shift+F3: next / previous match
- Ctrl+Shift+M: activity; Ctrl+Shift+R: mark all read (latest source)
- Ctrl+wheel or Ctrl+= / Ctrl+-: transcript font size; Ctrl+0: reset (latest source)
- Transcript Ctrl+A selects retained messages; Ctrl+C, Ctrl+Insert or right-click
  copies. Escape clears selection or dismisses the active interaction.
- Composer uses ordinary editing shortcuts. Text wraps visually; Shift+Enter
  does not create a multiline Twitch message.

## Local storage

Windows settings are stored in `%LOCALAPPDATA%\ChatWorkbench\workspace.json`.
A previous-file backup is kept before replacement. Drafts and reply-target metadata
are plaintext local application data, not credentials. Invalid or unsupported
settings files are preserved and saving is disabled for that run with an explanation.
Twitch tokens use a separate OS-vault entry and never enter workspace.json.

Workspaces, layouts, preferences and drafts restore after relaunch. Recently closed
workspace reopening and unread counters are session-local. Closing a split preserves
its draft for reopening in that workspace. Native QA may use an isolated profile.

## Roadmap

- [Complete feature coverage and delivery order](docs/FEATURE_COVERAGE.md)
- [Architecture and design decisions](docs/PLAN.md)
- [Multiplatform chat direction](docs/MULTIPLATFORM.md)

Accessibility/IME, full Unicode selection, richer media and provider behavior,
moderation, multiwindow workflows and daily-client parity remain active work.
