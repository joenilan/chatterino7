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

Control-session launches skip automatic account restoration. Account buttons
still represent real actions; layout inspection does not authorize using them.
Ordinary builds and direct app use are the workflow; no CI or new test runner.
