# Streamer.bot integration

Owner request, 2026-10-11. **Implemented:** connection and pinned action buttons (below).
**Still planned:** events, aliases and globals. Streamer.bot runs a WebSocket server on the
streamer's own PC (Servers/Clients → WebSocket Server; usually `127.0.0.1:8080`,
configurable). Jawjack would connect to it as a client, locally and only after the owner
turns it on. Source: [Streamer.bot WebSocket docs](https://docs.streamer.bot/api/websocket).

## Protocol facts (from the docs, 2026-10-11)

- On connect the server sends `Hello` with `info` (instance ID, name, version) and,
  when a password is set, `authentication {salt, challenge}`.
  - Auth string: `b64(sha256(b64(sha256(password + salt)) + challenge))`, sent as
    `{"request":"Authenticate","id":…,"authentication":…}`.
  - Authentication is optional except for `SendMessage`.
- Every request carries `request` + `id`; responses echo `id` with `status: ok|error`.
- Useful requests:
  - `GetActions` and `DoAction {action:{id|name}, args:{…}}`
  - `Subscribe {events:{Category:[Event…]}}` / `UnSubscribe`, plus `GetEvents`
  - `GetCommands`, `GetCodeTriggers` and `ExecuteCodeTrigger {triggerName, args}`
  - `GetActiveViewers`, `GetBroadcaster`, `GetInfo`
  - `GetGlobals` and `GetGlobal`, `TwitchGetUserGlobal`
  - `SendMessage {platform: twitch|youtube|kick, bot, message}`
- Event categories include Twitch (139 events: chat, follows, subs, cheers,
  redemptions, raids, online/offline), **YouTube** (Super Chat, memberships, …),
  **Kick** (chat, follows, subs, Kicks), OBS, Streamlabs, Ko-fi, Fourthwall, Raw
  (action runs), Custom, Command and Misc (global variable updates).

## What Jawjack would offer

1. **Connection settings:** host, port, optional password (kept in the OS
   credential vault). The status shows Connected, Auth failed, Not running or Off. Nothing
   connects until the owner enables it.
2. **Action buttons:**
   - Pick Streamer.bot actions to show as buttons in the streamer Studio
     panel or channel header ("Start BRB", "Hydrate", "Clip that").
   - A buttons row per workspace, and a right-click entry on a message ("Run action with this
     user/message") that passes `user`, `userId`, `message`, `messageId` and
     `channel` as args.
3. **Chat aliases:** e.g. `!!brb` in the composer runs a chosen action instead of
   posting. These live alongside custom commands and are never sent to chat.
4. **Event feed:**
   - Subscribe to the events the owner picks and show them in the audience event
     feed and in event highlights.
   - This covers donations (Streamlabs, Ko-fi, Fourthwall) that Twitch never sends.
   - It also covers **YouTube and Kick events before Jawjack has native YouTube and Kick
     support**. Streamer.bot already holds those connections, so it is the fastest way to a
     multi-platform event view.
5. **Action results:** Raw `ActionCompleted` events show success or failure next to the button.
6. **Globals in Studio:** show chosen Streamer.bot global variables, such as death counter or
   queue length, and update them live through `GlobalVariableUpdated`.

## Rules

- Local only by default. A remote host needs an explicit warning, because the protocol is
  plain `ws://`.
- Never forward Twitch tokens to Streamer.bot. It has its own connections.
- Treat event payloads as untrusted display data, with bounded sizes and no markup.
- When Streamer.bot is closed, buttons show as unavailable. Reconnect with backoff.

## Order

After the Studio panel exists: connection + DoAction buttons first, then events,
then aliases and globals. OBS WebSocket (scene switching) uses a similar pattern.

## Implemented (2026-10-11)

- **Studio → Streamer.bot card.**
  - Shows a status dot (Connected, Connecting or Not connected) and a connection-settings gear. The defaults are
    127.0.0.1:8080. An optional password is stored in Windows Credential Manager and sent
    only if Streamer.bot's Hello asks for authentication.
  - Reconnects with 2–60 s backoff and is off until **Connect Streamer.bot**.
  - The setting persists as `streamer_bot` in the workspace file, without the password.
- **Pin actions:** pick up to 12 actions from `GetActions`, with a filter and group labels. They
  become Studio buttons. Renamed actions keep their pin, because pins are matched by ID.
- **Run:** `DoAction` by ID and name. The notification reports Streamer.bot's acknowledgement
  or its error. Right-clicking a chat message lists the pinned actions as
  "Streamer.bot · name" and passes `user`, `userId`, `userName`, `message`,
  `messageId`, `channel` and `source: "jawjack"` as arguments.

Evidence (Windows, 2026-10-11): connected to the owner's Streamer.bot 1.0.7 (no
password), listed the real actions with their groups, the filter worked, pinning "test" showed
a Studio button, and it reconnected automatically after an app restart. No action was run;
running one changes the owner's stream setup and needs their go-ahead.
