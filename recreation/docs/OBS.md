# OBS control (Studio → Stream)

Jawjack connects to **obs-websocket 5** (built into OBS 28+) on the same machine or another PC on
your network. That covers dual-PC setups: enter the streaming PC's address, e.g. `192.168.1.50:4455`.

**Card:** shows a LIVE pill with the uptime ticking, a REC pill, and the mic state (first Mic/Aux input; click to
mute or unmute). While live, a red banner warns if the mic is muted. Stream health shows kbps, % dropped frames
(amber at 1% or more), CPU and fps. Scenes are a two-column grid, ordered as in OBS's dock; the scene on air is
lilac, and a click switches to that scene. You can also open Settings… or Disconnect.

**Settings:** address, port and password. The password is stored in Windows Credential Manager and masked, with a
reveal toggle. **Find OBS on my network** scans this PC's /24 private network (plus localhost) on the
entered port, 4455 and 4444, and confirms each hit by its greeting. Results are buttons that fill in the
address and port. A legacy obs-websocket 4.x server (usually 4444) is shown as "old obs-websocket 4.x",
because Jawjack speaks only the v5 protocol, so it asks you to update OBS. obs-websocket doesn't announce
itself on the network, so scanning is the only way to auto-detect it. Custom ports work: type the port first.

**Streamer mode:** in Automatic mode, "OBS is streaming" now also turns streamer mode on, which covers OBS
running on another PC where the process check can't see it.

**On the OBS PC:** Tools → WebSocket Server Settings → Enable, then note the port and password (Show
Connect Info). Allow the port through Windows Firewall on the private network.

Protocol: Hello (op 0), then Identify (op 1) with `rpcVersion` 1, auth `b64(sha256(b64(sha256(pw+salt))+challenge))`
and Scenes|Inputs|Outputs events. Jawjack polls GetStreamStatus and GetStats every 2 s, and reacts to the events
CurrentProgramSceneChanged, SceneListChanged, StreamStateChanged, RecordStateChanged and InputMuteStateChanged.
A close code of 4009 means the password was refused.

Evidence (Windows, 2026-10-11): with OBS closed, the card shows its not-set-up and "isn't running" states,
and the network scan completes with "None found". Untested so far: a live connection, scenes, mic and health
(the owner's OBS WebSocket server is off).
