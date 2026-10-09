# Chat workbench

A Rust + GPUI Kit foundation for Joe's full Chatterino7 replacement.
This directory is an **offline development preview**, not the finished client.
The existing C++ application is unchanged. No Twitch credentials are used.

Read [the full plan](docs/PLAN.md) for the feature inventory, architecture,
release phases, authenticated Twitch flow and daily-client acceptance criteria.

## What is here

- Framework-independent bounded chat timelines and normalized events.
- Duplicate suppression for retained message IDs, channel isolation and redaction.
- Unicode-safe text/emote copy representation and bounded zero-width overlay grouping.
- Dark-only Haiwire-inspired surfaces and a custom Windows caption/frame.
- Pane-owned, model-backed text selection with Ctrl+C, reverse dragging and word/line selection.
- A two-pane GPUI replay harness with 250 synthetic messages per pane, +100-message
  bursts and a mock timeout control. Each pane retains up to 10,000 messages.
- Pinned GPUI Kit 0.7.1 and a lockfile; local build commands.

## Build

Use Rust 1.99.0 (the toolchain used to resolve this lockfile).
On Windows, use MSVC with Visual Studio 2022 C++ tools, Windows SDK and CMake.
See [GPUI Kit's platform prerequisites](https://gpui-kit.com/docs/installation/).
From this directory:

```sh
cargo test -p chat-core --locked
cargo fmt --all --check
cargo clippy --workspace --locked -- -D warnings
cargo run -p chat-workbench --release --locked
```

The Windows executable, after a successful release build, is
`recreation/target/release/chat-workbench.exe` from the repository root.
Linux also needs X11/Wayland development libraries and a graphical Vulkan session
for execution. A headless unit-test pass is not a native UI acceptance pass.

## Current limitations

No live Twitch login, receiving, sending, emote images, cosmetics, persisted
workspace, custom docking, moderation API actions, installer or updater yet.
The preview is intentionally labeled OFFLINE REPLAY.

Timeline duplicate detection covers retained records, not an unlimited history.
Out-of-order moderation tombstones, transport envelope deduplication, per-message
size limits and source validation are required before accepting network input.
Selection now uses independent pane state and copies offscreen intermediate rows from
the model. Native drag behavior, auto-scroll at viewport edges, complex Unicode
grapheme selection, DPI/IME and accessibility still require hands-on validation.
Automatic edge scrolling is not implemented yet; use the wheel while dragging.

## Verification

See [VALIDATION.md](docs/VALIDATION.md) for exact checks and current limitations.
No browser, Windows or owner acceptance is implied by this source checkpoint.

CI/Actions are intentionally not used for this recreation. Commits use `[skip ci]`.
The owner runs Windows builds and reports errors for follow-up fixes.

## Interaction preview 02

Select transcript text and right-click, press Ctrl+C/Ctrl+Insert, or use Copy
selection. A verified clipboard copy clears the highlight and shows Copied.
Ctrl+A selects retained messages in the focused transcript; Escape clears it.
A local-only draft provides editable cut/copy/paste and undo/redo. Nothing is sent
and draft text is not saved on close. Each pane shows its scrolling state and
has a Latest control. Native validation status is recorded in VALIDATION.md.
