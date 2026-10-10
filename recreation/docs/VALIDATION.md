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

## 2026-10-10: channel regrouping and input ownership repair

Owner reported that panels could not merge, then observed an apparently held tab
when merely hovering. The Windows reproduction did leave a synthetic drag held
between a move call and screenshot capture. Release inspection reported a native
input at a different coordinate, so the prior center-drop result is not valid
center-merge acceptance. No owner click was observed. The test stopped and the
owner's workspace and size were restored; temporary QA workspace 10 remains.

Source changes: dedicated tab-strip merge targets, larger center merge area,
shared hover/release resolver, drag tab ordering, context-menu left/right ordering,
and Ctrl+PageUp/PageDown cycling within the focused channel group. Added atomic
control drag and source-matched pointer releases, plus hover cancellation for
stale drags. Linux build passed; Windows build and manual acceptance pending.

### Native acceptance: 7c1d662c

Windows locked release build passed with Rust 1.99.0, exit 0, 18.80 seconds.
Using only atomic drag calls in temporary QA workspace 10:
- Center B (70,430) to A (640,210): merged [A,B].
- B (210,48) to right edge (1170,400): A left / B right.
- B (710,48) to A tab strip (75,48): merged [B,A].
- B (75,48) to end of strip (270,48): reordered [A,B].
- Ctrl+PageUp selected A; Ctrl+PageDown selected B.

All drag responses reported released:true, pointer_owner:null and local-control
input. Screenshots showed no drag left active; no routing failures or unexpected
layout changes. QA workspace 10 was closed through normal confirmation. Owner
workspace 4, exact saved layout/drafts/sidebar, history 10000, original 1280x820
size and window position were preserved. No chat sends or auth changes. Running
PID 23160, control session jj-manual-7c1d662c-atomic, owned terminal hw-2-52.
Evidence: isolated-profile screenshots 104–109. This establishes the exercised
regrouping paths, not complete Chatterino feature parity or all device input cases.

## 2026-10-10: retained-history Find implementation

Linux locked offline build passed after adding per-pane search, independent exact
source highlights in plain and inline-media rows, keyboard navigation, incremental
arrival matching, and channel URL actions. Compiler evidence only so far; native
Windows behavior and keyboard/no-send acceptance remain pending. The existing
virtualized list is retained. Its scroll-to-item positions a matching message's
start and may resume tail following when bottom is reached; exact occurrence
visibility and restoration to a pre-search pixel offset are not claimed.

### Native acceptance: 75e7725c

Windows locked release build passed with Rust 1.99.0, exit 0, 17.98 seconds.
Deadlink briefly stopped replying; the uncertain add-channel operation was
reconciled before more mutations. Access recovered and native checks finished.
On actual HutchMF traffic, two StreamElements matches visibly highlighted.
Enter/F3 navigated row 11 to 16, Shift+Enter/Shift+F3 returned to 11. Ctrl+F
worked from composer, transcript and selected-tab/workspace focus; header Find
worked. Escape/reopen retained query, no-match search reported 0/null, and an
unchanged query acquired additional matches from real incoming messages.
Search state remained independent across two panes. Enter in Find preserved a
14-character disconnected draft with no send pending. Selection endpoints
survived query/navigation changes and copying returned clipboard_verified.
Latest closed Find and resumed following. Browser/URL menu entries were visible;
browser opening and emote-label highlighting were not separately exercised.

Temporary QA workspace 11 was removed through normal confirmation. Owner layout,
all preexisting drafts, sidebar and history 10000 matched exactly; original
1280x820 client geometry at (1072,306) was restored, overlays closed,
pointer_owner:null and active_drag:false. No sends, auth changes or source edits.
Running PID 47220, session jj-manual-75e7725c-find2, owned terminal hw-1-3.
Last successful inspection hw-1-29; isolated-profile screenshots 114–119.

## 2026-10-10: message action implementation

Linux locked offline build passed (7.77 seconds) after message menus, retained
chatter cards, validated provider logins and source-byte link interactions were
added. Existing selection and search rendering remain separate from activation.
Windows compilation and native interaction evidence are pending. No new tests,
CI, external profile requests, chat sends or OAuth scopes were added.

### Native acceptance: 8b1bcc6d

Windows Rust 1.99.0 locked release build passed, exit 0, 18.38 seconds; checkout
remained clean. On actual HutchMF traffic, no-selection message menu, chatter
inspection, Ctrl-click username and Escape dismissal passed. The card fit the
360x280 minimum window. Username copy, selected-text right-click copy, and exact
real-link copy produced visible feedback. Mention insertion replaced only the
selected composer span, sent nothing, and its temporary draft was cleared.

QA workspace 12 was closed through normal confirmation. Owner tab, all preexisting
drafts, sidebar, history limit and original 1280x820 geometry matched baseline.
Both owner channels were connected, overlays closed, active_drag:false and
pointer_owner:null. Running PID 44632, session jj-manual-8b1bcc6d-actions,
owned terminal hw-1-35. Native evidence includes isolated-profile screenshots
122–127 (menus/cards), 131 (unsent mention), 133 (link copy feedback), and
135 (restored owner). No browser opens, sends or account authorization changes.
Browser activation, remaining individual copy variants, and live deletion while
an action/card remains open were not exercised; code guards are not runtime proof.


## Replies implementation checkpoint — 2026-10-10

Linux offline locked build passed for the initial reply integration. Native Windows
build and runtime verification are pending. The next native pass must exercise
actual message Reply / View conversation, cancellation without draft loss, target
replacement, channel/workspace isolation, saved target after relaunch, compact
360×280 layout, retained jump and naturally arriving reply previews where available.
Use existing real traffic only; never send a message to validate the controls.
Outgoing delivery, live moderation redaction and unavailable-parent edge cases
remain unverified until directly observed. Restore owner geometry/layout/drafts.

## Native replies accepted — 2026-10-10 01:43 UTC

Windows exact da39e26b2c7767646a53d333adfbe3e2da157c96 fast-forwarded cleanly.
Rust 1.99 locked release build exited 0 in 16.91s. Actual native interactions:
- Selecting/replacing reply targets preserved draft text.
- × and Escape cancelled targets without erasing text.
- Channel/workspace switching kept draft and target isolated.
- Naturally arriving replies showed context; conversation, Jump and Reply worked.
- Reply target and conversation dialog fit 360×280.
- Find navigated reply rows; atomic selection/right-copy returned clipboard_verified.
- Controlled restart restored exact target and draft with outside-history label.
QA13 text/target cleared and workspace removed through normal confirmation.
Owner drafts, layout, sidebar, history cap and original bounds were restored;
both owner channels connected, overlays closed, no active drag/pointer owner.
Running session jj-manual-da39e26b-replies-restart, PID 45920, owned terminal hw-1-99.
No messages sent, URLs opened or authentication actions performed.
Outgoing delivery, real moderation/deletion and a thread with its original parent
retained remain untested. Build/runtime evidence is not full Chatterino parity.


## Attention and font QoL implementation checkpoint — 2026-10-10

Code adds upward reply arrow, per-pane unread/highlight references, visible-row
acknowledgement, compact badges, global retained activity, optional highlight words,
and Ctrl+wheel / keyboard font controls. Initial Linux build passed (10.00s).
Native acceptance is pending. Required bounded real-app checks: hidden channel and
workspace unread, inactive/background window if safely possible, returning to tail,
scrolled-back retention, explicit mark read, live-filter-hidden activity access,
activity jump, keyword highlights using naturally arriving traffic, compact layout,
Ctrl+wheel versus plain scrolling, reset and saved font/keyword settings. Preserve
owner's newly edited five-channel grouped workspace. No outgoing QA messages.
Unavailable real self-mentions/account switching/moderation must remain unverified.


### Stream marker polish — 2026-10-10 02:30 UTC

Pastel state markers and explanatory tooltips added to the pending attention/font
batch. Native validation remains blocked by Deadlink's no-project-selected error
and the desktop executor setup-refresh failure. No Windows pull or relaunch has
occurred; da39e26b is still the last-confirmed preview. The error does not establish
that selecting a Haiwire project is an intended connector requirement.

## 2026-10-10 — Windows recovery and Settings hit testing

- Deadlink recovered; checkout 535e6a7 was clean and built on Windows with Rust
  1.99.0, locked release, exit 0 in 20.24 seconds. Owned preview PID 60088,
  session `jj-manual-535e6a7c-attention`.
- Genuine HutchMF messages and upward reply-context glyph observed. Settings
  suppressed read acknowledgement; unread cleared after returning to visible tail.
- Native defect: clicking the highlight input could open a reply conversation
  underneath Settings. Added GPUI hitbox occlusion to the Settings card, including
  scroll isolation. Child controls remain interactive; background rows should no
  longer receive pointer input through the card. Native retest pending.
- No sends, authentication changes or moderation performed. Remaining font/activity
  checks and owner-state restoration are tracked by the existing native task.

### ec327c2 native follow-up and compact close correction

Windows locked release build passed (20.69s). Actual Settings typing/editing,
right-click and scrolling no longer activated underlying chat. Temporary highlight
word matched genuine HutchMF rows in transcript/activity. Activity shortcut,
All unread, Mark all read, dialog read suppression, tail clearing, hidden-channel
and sidebar badges, font shortcuts/reset/plain scrolling, pastel stream markers,
upward reply glyph and compact activity were observed. Restart and final cleanup
were still pending at this report.

At 360×280 the scrolling Settings Done label overlapped the footer and did not
close when clicked. Moved Done into a non-scrolling header; only the settings body
scrolls, and dismissal returns focus to the workspace. Linux build passed (3.88s);
native compact retest remains pending. No outbound chat or auth changes.

### 0cfe832 final native acceptance

Exact Windows revision 0cfe83251850745e10db5f02e7ab0a474826d0ed built with
Rust 1.99.0, locked release, exit 0 in 18.76 seconds; checkout clean.
At normal size and 360×280, Settings Done remained visible and dismissed before
and after scrolling. Input accepted typing without touching the chat draft; no
footer overlap observed. Earlier checks also verified font controls, custom-word
highlighting, All unread, Mark all read and saved-settings persistence.

QA14 was removed. Saved owner tabs, preexisting drafts, preferences and highlight
terms matched the original snapshot exactly; original geometry/sidebar restored,
jennybunnybean selected. Isolated preview PID 65088, session
`jj-manual-0cfe8325-settings`, left running. No sends, auth or moderation actions.
Activity Jump, inactive/offscreen acknowledgment and natural own-mention/reply
matching remain unverified. This is acceptance of named behaviors, not full parity.

## 2026-10-10 — Jenny purple-heart first-line alignment

Owner reported username movement beside a heart. Native read-only capture on
0cfe8325 showed JennyBunnyBean remained on the same line as Twitch static purple
heart 555555584, with ample width, but lower than the badge strip. Font14,
window1280×820, five retained messages, no active entrance effects. Evidence in
isolated profile: 167-jenny-heart-original.png and inspection JSON. Raw message
IDs/fragments/dimensions are not exposed by current inspection; no claims on them.

Cause: inline media text uses a minimum30px line box; badges used a fixed22px
box. Badge slots now share the actual first-line height and center their18px
images, for both plain and media rows and changed font sizes. Inline layout is
created once and shares its line-height helper with badge placement. No wrapping
or decoded-media sizing changes are claimed in this narrow fix. Native retest pending.
