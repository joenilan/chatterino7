# Manual app control

The app can expose an explicitly launched, session-only local control path, modeled
on Haiwire's native control approach. This is for driving the actual window.
Normal startup exposes no endpoint. There is no network server, token, shell
execution, account access, or general file tool.

## Launch and use

After building, launch the normal executable with a fresh session name:

```powershell
.\target\release\chat-workbench.exe --control-session joe-chat-01
```

The same executable can issue one manual action against that running window:

```powershell
.\target\release\chat-workbench.exe --control-call joe-chat-01 '{"method":"inspect"}'
.\target\release\chat-workbench.exe --control-call joe-chat-01 '{"method":"pointer","kind":"down","button":"left","x":100,"y":140}'
.\target\release\chat-workbench.exe --control-call joe-chat-01 '{"method":"pointer","kind":"up","button":"left","x":100,"y":140}'
.\target\release\chat-workbench.exe --control-call joe-chat-01 '{"method":"key","key":"ctrl-a"}'
```

Coordinates are window-local logical pixels. Inspect returns the actual transcript
bounds; use current layout and a native screenshot, rather than guessing after a
resize. Send down/move/up separately for drags. `held:true` on move keeps the left
button held. `button:right` follows the same copy handler as a physical right click.
`shift:true` supports extending selection.

Methods:
- inspect: active workspace, tabs, visible channel counts, scrolling state, transcript bounds, selection endpoints,
  draft character counts and last copy verification outcome. No draft or clipboard
  text is exposed in the state response.
- focus: target workspace, channel_input, composer or transcript; pane is a visible pane index.
- pointer: kind down/move/up, x/y, optional left/right button, held, shift.
- key: a GPUI keystroke name; key-down and key-up are both dispatched.
- text: single-line committed text, at most 2048 bytes, to the focused editor.
- scroll: x/y and signed lines from -120 to 120.
- resize: width/height within 1050x640 through 8192x8192.

No fixture injection or test controls are part of the protocol. A successful
`dispatched` response means input was delivered, not that a UI outcome is correct;
inspect afterward and observe the real window. State inspection does not substitute
for screenshot review.

## Lifecycle and access

The named directory is created under the current user's
LOCALAPPDATA\ChatWorkbench\control. It must be a new session name, beneath the
user's private application-data directory. The existing OS directory permissions
are the local access boundary; this is not remote-client authentication. Do not
share that folder or use it on a machine with an untrusted account that can write
the owner's application data. No persistent enablement is saved.

One command may be outstanding. Frames are size-limited, published by atomic
rename, tied to session/request IDs, and expire before UI execution. Filesystem
polling runs on a background thread; actions are dispatched on the GUI thread.
An old session name is deliberately not reused. Closing the process ends control.
The app never deletes unrelated files or automatically purges old session folders.

Timeout is an unknown outcome. Do not blindly repeat a mutation. If the prior
request cannot be reconciled, close the controlled instance and start a fresh
session. Current Deadlink exposes command creation/output but no stdin writes;
the one-shot command mode supports successive calls without a separate script.

## Current evidence

The complete native workspace builds on Linux and Windows. A real Windows
control-session run on 3edc6575 confirmed positive transcript height (503 px),
visible composer, correct title-bar hamburger and restored workspaces. Earlier
b92a50a inspection found the zero-height layout defect that prompted that fix.
No real OAuth, token persistence or live chat result is claimed. Further manual
interaction checks are in progress. Native screenshots remain local to the
isolated profile. Cloud visual inspection remains blocked by display sockets.

Control-session launches restore the saved Twitch account through the same OS
vault and token-validation path as ordinary launches. Account buttons and chat
sends still represent real actions; layout inspection does not authorize them.
Ordinary builds and direct app use are the workflow; no CI or new test runner.

Windows 7373edba locked release build passed (7.38s). Native interaction verified:
Shift+Enter left a 34-character draft unchanged; a visible “Place above” preview
and drop reordered two channels into a stacked layout while preserving the draft;
tab dragging changed tab order while retaining the active workspace and its panes.
These are bounded observations, not complete docking, clipboard, Unicode, account
or live-chat acceptance. The isolated controlled app remained open afterward.

## Live-chat and middle-button inspection

The native pointer command also accepts `button: "middle"` for actual tab-close
interaction. Inspection reports `close_tab_pending` by stable tab ID, per-pane
connection status and send-pending booleans. The `offline` value reflects actual
pane connection state, not a hardcoded preview label. No access/refresh tokens,
message bodies or draft contents are included. Account restoration remains off
for controlled launches; signing in or sending is a real account action.

## Windows observation — 2026-10-09, 7466e04b

The locked Rust 1.99.0 Windows release build passed. One responding native window
was confirmed after the transient terminal-tool failures cleared. In the isolated
signed-out instance, middle-click named the intended inactive workspace and its
channel. Enter, Escape and Cancel kept the workspace; explicit Close removed only
that workspace; Ctrl+Shift+T restored its stable ID and channel. The app was left
open with Sign in with Twitch visible, connected=false and send_pending=false.
No OAuth grant, credential restoration or chat send was performed in this pass.
Live authentication, receiving/sending and emote rendering are still unverified.

Windows b8379a6 (2026-10-09): locked release build passed in 9.03s. A new control
session automatically restored the saved owner account without another grant.
HutchMF receiving progressed 1 → 6 → 29 → 48 → 60 with visible real messages,
independently observed by the owner. The earlier uncertain send was not retried.

Message entrance polish uses arrival timestamps instead of row-mount animations:
180ms, 6px horizontal slide, cubic ease-out and a restrained fade. At most eight
entrances per pane are retained; overload clears effects and cools down for 300ms.
Only rendered live-tail rows request animation frames. History reading, selection,
inactive windows and OS reduced motion suppress effects. This is implementation
behavior, not a measured frame-rate or CPU performance claim. Native animation
appearance and copy behavior must be observed on the updated Windows build.

Windows fcf43ae: locked release build passed in 8.64s, saved login restored, both
channels connected and HutchMF reached 240 messages. The global status visibly
read Live / 2 connected panes. History rows stayed anchored while traffic arrived;
drag selection and right-click copy returned clipboard_verified. Entrance motion
was not visually verified: initial capture issues and an existing selection
suppressed it. A collapsed selection anchor after Latest was found to suppress
entrances unnecessarily; the condition now checks an actual selection or drag.
Windows 8b0b54f then passed its locked release build in 8.49s, restored the saved
account and received HutchMF messages (9 → 11). Colored, heavier usernames and
the corrected live indicator were visibly confirmed. Entrance motion remains
unverified; no extended recording was performed. Tabs and the 49-character owner
draft were preserved, with no chat send.

## Entrance visibility correction

The owner reported messages still appearing without visible motion. Earlier
code suppressed all effects in an unfocused window, cleared every effect on a
burst, and spent its 180ms timer before first presentation. The updated path
starts its 260ms clock when a row is rendered, uses a more visible 12px/6px
slide and 25%-to-full opacity, and retains at most the newest 12 effects.
Pending effects expire after one second if never presented. It respects history,
nonempty selection/drag and reduced motion, without requiring window focus.
These are identified implementation causes; the exact cause on the owner's
previous screen was not established. Windows 610577f passed its locked release build in 7.77s. A bounded 2.874-second
capture verified one real HutchMF row dim/offset, then sliding and brightening
over successive frames. A separate inspection saw an effect start and clear.
The owner subsequently confirmed the motion looked good.

### Emote composition inspection

Pane inspection includes `emote_picker_open`, `emote_picker_matches`,
`emote_picker_page`, and bounded suggestion labels. Shared catalog inspection
includes Twitch global emote/badge counts and per-channel badge counts. These
are public catalog names/counts, not draft contents or credentials. Existing
pointer/key/text operations drive the real controls; insertion does not send.

### Response budget correction

Commands remain limited to 16 KiB frames (8 KiB JSON input). Inspection responses
have a separate 256 KiB cap for the bounded 128-entry media cache. The server
turns an oversized result into a small identity-matched error response, which the
client consumes normally. This prevents the old 16 KiB shared limit from leaving
a successful inspection response unresolved and blocking later calls. Timeouts
and identity mismatches still require reconciliation; mutations are never replayed.

### Atomic pointer gestures (2026-10-10)

Use `{"method":"drag","from":{"x":100,"y":48},"to":{"x":600,"y":48}}`
for drag-and-drop. This dispatches down, interpolated moves, and up in one call,
refreshing hit testing between events. It returns `released:true` and current
workspace state. Do not leave a synthetic down held while taking screenshots or
waiting for another tool call; that can overlap the owner's physical mouse.
Native/control pointer ownership is recorded at down and checked at release.
A move with no left button, or input from the other source, cancels the active
layout drag. Hover alone must not commit a layout operation.

`focus` now accepts target `search` for a visible pane. It opens the actual Find
bar and focuses its input. Inspection exposes search_open, search_matches and
search_current_row without returning query text, and reports active_drag at the
window level. Search input must never route Enter into the chat composer.

Pointer requests accept boolean `control` and `alt` modifiers alongside `shift`.
For Ctrl+click username inspection, dispatch a matching down/up pair immediately
with `control:true`, then a neutral move to clear synthetic modifiers. Do not
leave a held button between screenshots or tool calls. Browser link activation
must not be exercised against an unreviewed URL from incoming chat.
