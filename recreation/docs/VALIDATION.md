# Foundation validation

2026-10-09 cloud Linux, Rust 1.99.0.

- `cargo test -p chat-core --locked --offline`: passed, 13 tests.
- `cargo clippy -p chat-core --locked --offline --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check`: passed.
- GPUI `cargo check -p chat-workbench --locked`: dependency download was blocked
  by a CONNECT proxy HTTP 403 before application compilation. Not passed.

The initial Windows Actions run for 76f433a passed formatting and all eight
then-existing core tests. It reached the desktop crate and failed on exactly one
reported diagnostic: unused `ButtonVariants` import with warnings denied. That
import has been removed. The revised desktop has not yet been rebuilt on Windows.

Owner requested no CI/Actions. The recreation workflow was removed in f8e36f4.
No rerun is planned; future commits use `[skip ci]`. The owner will build/run locally
and report errors. Release linking, GUI runtime and performance remain unverified.

Source review fixes:
- Disjoint, monotonic selection-order ranges prevent equal-index rows in separate
  panes from being silently included in same-pane selection. True per-pane scope
  isolation and virtualized offscreen copy remain future acceptance gates.
- Redaction uses item-anchor-preserving remeasurement and skips unchanged results.
- Overlay grouping preserves source copy text, limits layers per stack, and handles
  orphan overlays, intervening text, Unicode and newlines. No emote images rendered.

No live Twitch session, moderation actions, credentials or installed applications
were used. No Windows, browser, accessibility or owner acceptance is claimed.

## Dark frame and selection follow-up

Owner ran the prior preview on Windows and supplied a screenshot: native app
opened, but default light appearance/OS titlebar were rejected and text selection
was reported barely usable. The screenshot alone does not prove replay/timeout
results; the owner reported clicking those controls.

New patch (not yet owner-run):
- Dark-only Studio palette, Segoe UI 14px body, 40px custom caption, 46px Windows
  hit targets and a 6px top resize strip, based on Haiwire's documented design.
- Model-owned independent per-pane UTF-8 byte selections; no ephemeral toolkit
  selection registration. Copy reads current retained messages, including rows
  outside the viewport. Selection clears on endpoint pruning or redaction.
- Drag, reverse drag, Shift extension, double-click word, triple-click message,
  Ctrl+C/copy button; wheel during drag reprojects against the newly painted rows.
- 21 core tests pass, core Clippy with denied warnings passes, rustfmt passes.

Exact new native UI compile and runtime checks must be performed on Windows.
Check dragging/resizing/maximizing, title controls, pane-isolated copying,
selection while scrolling/bursting, and redaction. Automatic edge auto-scroll,
full grapheme-aware word semantics, touch and screen-reader verification remain.
