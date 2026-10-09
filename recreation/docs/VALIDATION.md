## BTTV / FFZ implementation checkpoint — 2026-10-09

Linux offline locked build passed against the new catalog/renderer integration.
Windows build and native BTTV/FFZ images are not yet verified. Existing native
preview remains ba9bb3b until the connected Windows tooling can safely update it.
No synthetic messages or new test harness were added. No OAuth scopes changed.

The remaining history-settings check encountered intermittent Deadlink envelope
errors even on a standalone read-only inspect, without a terminalID or app exit.
Independent process/log/screenshot checks found the app responsive and receiving
chat, with no crash evidence and no outstanding control request/response files.
History was still10k and original drafts49/0. Root has not changed the limit.

## Native keyboard correction accepted — 2026-10-09 21:00 UTC

Windows ba9bb3b675a9bb8a9e63163a1db33777c47416a8 built with Rust1.99 locked
release, exit0 in12.08s. In a verified disconnected channel-not-found pane:
- Arrows selected DansGame; Enter inserted DansGame once.
- Tab completed :Di to DinoDance once.
- Neither set send_pending or showed Sending/Not sent.
The temporary pane was removed; original three tabs and both exact drafts were
restored (49 and0 characters). No send was repeated. App left running.

History controls are compiled, but saving another cap is not yet accepted.
Deadlink returned `terminal request failed: Failed` on the Appearance click;
two read-only reconciliation attempts failed. Click outcome is unknown. Last
confirmed history limit10,000; counts0 and265. Do not claim settings persistence
passed until the actual saved workspace and UI are inspected.

## Completion action-routing correction — 2026-10-09

Native b5ad20e found a real defect: Enter completion also triggered a send attempt
in HutchMF; Twitch rejected it as followers-only. No delivery or retry claimed.
Both saved drafts were restored exactly. Tab navigated focus instead of completing,
and arrows did not retain the selected candidate. Picker search/paging and Escape
worked. Populated-cache inspection (15,590 bytes) and subsequent commands worked.

Correction: composer Enter has a single semantic action owner. The independent
InputEvent::PressEnter submit handler and raw-key completion handling are removed.
Toolkit MoveUp/MoveDown/IndentInline actions and composer-scoped Tab now route to
completion before editor/default focus actions. Picker Enter uses the same owner.
Linux build passed; native corrected behavior is pending. Do not use public chat
as a safe fallback when checking whether a key might send.

## Configurable history cap — 2026-10-09

Linux offline locked build passed (5.87s). Added persisted per-channel history
cap controls while preserving the existing10k default. Native settings/persistence
interaction pending; no large-traffic performance measurement claimed.

## Native badge/picker result and control fix — 2026-10-09

Windows 9fae62f build exit 0 (1.04s confirming incremental build; initial build
terminal result was lost during a Haiwire restart). Real incoming messages show
Twitch badges. HutchMF picker loaded 1,065 choices. DinoDance search returned one
result without changing drafts; paired native snapshots visibly show its animated
preview, closing the earlier exact-emote visual gap. Clicking inserted unsent text
and closed the picker. Both original drafts were restored by local comparison.

Inspection then failed because the media-rich response exceeded the old 16 KiB
shared request/response limit, leaving that response unresolved. This correction
separates the bounded response budget (256 KiB) from the unchanged small request
budget and ensures server oversize errors retain response identity for cleanup.
Native paging/completion/channel switching/tooltips still await the corrected
preview. No chat messages were sent.

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

### 3f18e079 native recovery and provider catalogs — 2026-10-09

Windows locked Rust 1.99 release build passed in 30.47 seconds. Existing account,
tabs and exact saved drafts restored. History 5k saved, then 10k restored/saved;
both pane headers displayed /10000. Catalog counts: BTTV/FFZ globals54/9,
dreadedzombie19/18, HutchMF33/0. Natural Twitch/7TV images observed in busy chat.
BTTV/FFZ-specific image/picker acceptance remains open. Native input stopped on
apparent owner activity. A later exited preview was relaunched successfully;
account, drafts and 10k state remained intact. No chat messages sent in this run.

### Recursive docking batch — implementation, not native acceptance

Linux offline locked build covers recursive split geometry, grouped channel tabs,
separate persisted live filters, broadcast metadata worker and updated inspection.
Required native checks: old layout restore; three-plus panes; nested horizontal
and vertical edge drops; center regrouping; resize persistence; canceled drag;
channel switch focus and exact drafts; middle-click cancel/close; live-only filters
and selected-offline visibility; reconnect and restart. Do not infer these passes
from compilation or the preceding 3f18e079 runtime.

### dc404246 native docking findings

Windows locked release build passed16.55s. Three visible panes with nested axes
were proven using a top-edge drop. A bottom-edge drop showed Place below over the
right panel but regrouped into the left group on release: confirmed defect.
The compact batch resolves drop target/edge from current painted panel bounds and
release pointer rather than retaining the drag-hover state/callback's pane name.
This correction is compiled, not yet verified natively. Control inspection now
records the last drop's source, target, edge and pointer coordinates for diagnosis.
Owner layout and exact drafts were restored; temporary QA workspace9 remains.

### e7d4f409 compact native acceptance — 2026-10-09

Windows Rust1.99 locked release build passed (exit0,14.04s). Native inspection at
540x470 verified account-dialog open/close without auth changes, gear/settings
overlay without changing pane widths, sidebar toggle, group + add-form/cancel,
and channel-tab right-click Add/live-filter/Close menu. Default launch size was
not changed; original1280x820 client size was restored after the temporary check.

No unexpected drops occurred during those controls. A controlled bottom-edge
move then passed: QA B at release(958,700), local-control input, target QA C,
Place below; dock and screenshot confirmed A left / C above B right. Captured
MouseUpEvent position avoids relying on mutable cursor state. This proves that
case; it does not establish the cause of earlier unexplained native changes.

QA workspace9 was removed through normal confirmed close. Owner workspace4,
layout, sidebar state and every preexisting draft matched their saved baseline;
history remained10000 and live messages arrived in both owner channels. Preview
left running with overlays closed. No messages sent or account grants changed.

Still not claimed: full Chatterino parity, every divider/reset/cancel permutation,
all provider-specific image variants, and comprehensive live-status transition
coverage. The old two-pane cap is removed; three-pane nested layouts were observed.
