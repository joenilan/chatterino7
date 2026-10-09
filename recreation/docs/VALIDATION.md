# Foundation validation

2026-10-09 cloud Linux, Rust 1.99.0.

- `cargo test -p chat-core`: passed, 8 tests.
- Independent `rustc --edition 2024 --test crates/chat-core/src/lib.rs` replay of
  the tests after formatting: passed, 8 tests.
- `cargo fmt --all --check`: passed.
- Direct `clippy-driver --edition 2024 --crate-type lib -D warnings` on chat-core:
  passed after correcting one collapsible-if warning. This is not full workspace Clippy.
- GPUI `cargo check -p chat-workbench --locked`: attempted; dependency download
  reported a CONNECT proxy HTTP 403 before application compilation. Not passed.
- Full workspace Clippy, native GUI build/run, Windows rendering, selection,
  scroll anchoring, screenshots and performance benchmarks: not yet verified.

The Windows workflow is scoped to recreation branches and has read-only repository
permissions. A workflow file is not evidence that a run was enabled, started or
passed. Its result must be checked for the published commit.

These tests cover the current pure model only. No network credentials, live
messages, moderation actions, provider assets or installed applications were used.
