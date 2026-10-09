# Jawjack

Jawjack by Zombie Digital is a native Rust + GPUI Kit Twitch workspace, built as Joe's Chatterino7
replacement. Dark Studio styling, a custom Windows frame, and a channel-first
layout. The original C++ application remains untouched.

## Workspace features

- Named workspace tabs: create, switch, rename, drag to reorder, close, and reopen the
  recently closed workspace during the session.
- Add Twitch channel names or channel URLs. Each workspace supports two
  independently resizable channel splits. Drag a channel header toward the chat
  area’s left/right/top/bottom edge to rearrange them, or onto a workspace tab to
  move it there. Drop previews show placement; full or duplicate targets reject
  the move without losing the source pane. Nested layouts beyond two panes remain
  future work. A Windows run verified a top-edge drop and tab reordering while
  preserving the active workspace and channel draft.
- Optional workspace sidebar, closed by default, with a hamburger toggle that
  remembers its state. Opening/closing slides smoothly, including mid-motion
  reversal, and respects OS reduced-motion preferences. Tabs and channel controls
  work without it.
- Soft-wrapping Twitch composers with a 500-character message limit, ordinary
  cut/copy/paste, undo/redo and retained drafts. Pasted line breaks become spaces.
- Local saved tabs, active workspace, split sizes/orientation, sidebar, font size
  and separate drafts for the same channel in different workspaces.
- Dark-only appearance controls with 12–24px transcript sizing.
- Custom Workspace and View menus; keyboard shortcuts for frequent operations.
- Model-backed transcript selection with right-click/Ctrl+C copying and verified
  clipboard feedback, ready for live chat data.
- Optional session-only native app control, documented in [APP_CONTROL.md](docs/APP_CONTROL.md).

## Current connection status

Twitch device-code sign-in is implemented using Jawjack’s public Client ID and the
OS credential vault. It includes saved-account validation, cancellation, expiry,
refresh-token rotation and local sign-out. The real authorization flow has not yet
been exercised. No client secret is required or embedded.

Live chat transport is **not connected yet**. Send stays disabled and Enter retains
the draft. No synthetic chat is seeded. Live receiving/sending, emotes, moderation
and Chatterino7 parity remain on [the roadmap](docs/PLAN.md). Planned multiplatform
viewer and broadcaster workflows are in [MULTIPLATFORM.md](docs/MULTIPLATFORM.md).

## Build and run

Use Rust 1.99.0 with Windows MSVC, Visual Studio 2022 C++ tools, Windows SDK and
CMake. GPUI Kit is pinned to 0.7.1. From this directory:

```sh
cargo run -p chat-workbench --release --locked
```

The built executable is `target/release/chat-workbench.exe` under this directory.
Use ordinary builds and direct interaction with the real app for development.
No CI/Actions or separate test-script workflow is required. Commits use `[skip ci]`.
The native workspace compiles and links on Linux and Windows. Windows inspection
confirmed the corrected transcript/composer layout and workspace restoration.
The account flow and full daily-client behavior remain unverified.

## Shortcuts

- Ctrl+T: new workspace
- Ctrl+W: close workspace
- Ctrl+Shift+T: reopen recently closed workspace
- Ctrl+Tab / Ctrl+Shift+Tab: next / previous workspace
- Ctrl+K: add channel
- Transcript: Ctrl+A selects retained messages, Ctrl+C or Ctrl+Insert copies,
  Escape clears selection; right-click copies too.
- Composer: ordinary editing shortcuts; text wraps visually without inserting
  message line breaks. Shift+Enter does not create a multiline Twitch message.

## Local storage

Windows settings are stored in `%LOCALAPPDATA%\ChatWorkbench\workspace.json`.
A previous-file backup is kept before replacement. Drafts are plaintext local
application data, not credentials. Invalid/unsupported settings files are left
untouched and saving is disabled for that run, with an explanatory status.
Closing a split preserves its draft for reopening in the same workspace.
Recently closed workspace reopening is session-local; active tabs restore after
relaunch. Twitch tokens use a separate OS-vault entry and never enter workspace.json.

Minimum window size is 1050×640. Automatic selection edge scrolling, full Unicode
grapheme semantics, accessibility/IME review and full daily-client parity remain
unfinished. State inspection is not proof of visual correctness or a native run.
