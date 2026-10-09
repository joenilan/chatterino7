# Current checkpoint: badges and emote composition — 2026-10-09

Linux offline locked build passed against the implementation tree (5.46s).
Windows build and real-app interaction for this batch are pending; no runtime
acceptance is implied by the compiler. No test harness or synthetic traffic added.

Previous b7b744d native result: animated Twitch and 7TV media visibly changed poses;
owner subsequently confirmed animations. DinoDance was identified independently
as emotesv2_dcd06b30a5c24f6eb871e8f5edbd44f7 with 25 decoded frames and frame clock
2→19 over 487ms; that exact emote was not in the captured visible rows.

This batch requires ordinary native inspection of badge rows, picker search,
paging and insertion, completion keyboard handling, Escape/focus restoration,
long-draft protection and retained owner draft. Never send a chat message merely
to inspect these controls. The earlier ambiguous send remains unrepeated.

---

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

Patch 7cb9e984 (owner subsequently reported running it and liking the design):
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

## Terminal-style copy follow-up

The owner requested highlight then right-click copying, alongside normal editing
shortcuts. Right-click inside a transcript now invokes that pane's existing
model-backed copy operation without changing the selection. Empty selection leaves
the clipboard unchanged. Ctrl+C remains supported. Cut/paste belongs to editable
fields; a live composer is still future work. Native right-click behavior remains
pending Windows verification; the owner's design feedback is not full interaction
acceptance.

## Interaction polish batch (Preview 02)

Owner feedback after 76412f7: right-click copying appeared ineffective, and there
was no highlight clearing or confirmation. No successful native right-click
acceptance is claimed for that revision. Source inspection found the toolkit
scrollbar consumes right mouse-down in its bubble handler; ordinary text event
interception was not proven.

This batch includes:
- Viewport-gated capture-phase right-click handling, before scrollbar bubbling.
- One copy path for right-click, keyboard and button; clipboard read-back must
  match before clearing selection and showing a deduplicated in-app Copied toast.
  Failed verification retains selection and explains retry; empty selection
  preserves existing clipboard contents and gives an instruction.
- Transcript-only Ctrl+C/Ctrl+Insert, Ctrl+A and Escape contexts, separate from
  editable draft input. The draft uses toolkit cut/copy/paste and undo/redo.
  It is explicitly local, unsent and not persisted after closing.
- 22px line boxes instead of dead inter-row padding; wider clipped text hit areas
  excluding the scrollbar gutter.
- Per-pane Latest buttons, Following latest / Reading history status, tooltips
  describing synthetic replay/moderation, wrapping action rows, and shortcut help.
- Visible INTERACTION PREVIEW 02 label to identify the running UI.

25 core tests, core Clippy with denied warnings, and rustfmt pass. The offline
native check stops because accesskit 0.24.1 is not cached. Desktop catalog remains
offline at the latest check. This is not a native compilation or UX pass.

Windows acceptance for this batch, once native execution is available:
1. Select forward/backward in each pane; right-click text, padding and scrollbar.
   Paste into the local draft: exact content, selection gone, one success toast.
2. Repeat via Ctrl+C, Ctrl+Insert and Copy selection. Try empty selection with
   existing clipboard contents: clipboard unchanged; no success claim.
3. Select across offscreen rows, use Ctrl+A, then Escape; verify pane isolation.
4. Focus local draft: Ctrl+A/X/C/V/Z work only there; right-click exposes editing
   behavior rather than copying chat. Draft text never sends to a network.
5. Scroll up, replay a burst, verify reading position/status, then pane Latest.
6. Resize to minimum window size; toolbar wraps without obscuring transcript or
   draft. Check repeated copies replace the in-app notice instead of piling up.
7. Redaction clears old selection and changes copy source; app close/reopen
   respects the documented nonpersistent draft behavior.

Still pending: automatic selection edge scrolling, grapheme-aware selection,
native accessibility/IME/DPI review, and live Twitch/network features.

## Owner workflow correction

The owner requested removal of test-only UI and repeated test-script/checklist
workflows. Replay/timeout controls and their development-only handlers are removed.
Future development prioritizes ordinary builds and manually driving the real app.
See APP_CONTROL.md for the new explicit-session local driver; its native execution
is unverified. Existing historical test results above are not new release gates.
