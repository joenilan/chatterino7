# Live connections (EventSub sharding and optional activity)

Each connected Twitch chat requires six EventSub kinds (message, delete, user
clear, clear, chat notification, and chat settings). Current source also reserves
ten moderated-channel AutoMod queues **per socket** (two kinds each) and
18 optional own-channel subscriptions for follows, both channel-point redemption
kinds, AutoMod, moderation and Studio status.

Twitch's budget in this architecture is 300 subscriptions per WebSocket and
three sockets per user token. Source at reviewed commit `3ec84d69f` computes:

- `SOCKET_BUDGET = 300 - 10 * 2 = 280`
- `CHANNELS_PER_SOCKET = floor(280 / 6) = 46`
- `MAX_CHANNELS = 46 * 3 - ceil(18 / 6) = 135`

This **replaces the stale historical 150/50 limit**, and may change with the
actual `CHANNEL_KINDS`, `MOD_KINDS`, `OWN_KINDS` constants. Recalculate
instead of copying a number into future docs. Own channel is prioritized even
if its chat tab is not open, so optional broadcaster activity is not lost merely
because of a closed/hidden own tab.

- The manager assigns channels in priority order, opens up to three shards,
  and shows a reason for channels beyond the active cap.
- Adding or removing a channel updates just that channel's subscriptions.
  A changed signed-in **user** resets the connection; token refresh/new scopes
  do not reset healthy subscriptions.
- Helix requests are paced and back off for low rate budget/HTTP 429. A fresh
  socket with no enabled subscriptions is serviced first.
- `session_reconnect` hands the socket over; a fresh connection re-subscribes.
  A revocation can retry its kind/channel. Since preview.13, revoked/refused
  optional own-channel extras (including reward kinds) can retry separately
  without tearing down healthy chat subscriptions. Optional refusals are
  exposed by `inspect` → `workspace.own_extras_refused`.
- `inspect` also lists disconnected channel statuses. **Connected chat is not
  confirmation that optional reward subscriptions succeeded.** A WebSocket
  interruption can leave an unrecoverable event gap; no guaranteed replay is
  claimed.
- The Helix **app-manageable custom-reward queue** is different from EventSub
  redemption notifications. Automatic rewards do not live in that queue.

## Evidence

Windows 2026-10-11: 13 channels connected around four seconds after launch.
Adding a channel took about two seconds while #xqc kept receiving chat.
86 channels connected across two sockets in roughly 80 seconds during a cold
start. These are historical observations, not a test of the current 135 cap.

Preview.13–14 source implements optional own-channel retries; focused unit
tests cover custom + automatic redemption classification, own-channel
priority and duplicate notification IDs. Dedicated live retry/three-socket
acceptance remains pending. **No actual live redemption was observed** by the end
of preview.14. Long sessions, three sockets, revocation, suspend/resume and
rate-limit behavior remain unverified in the field.

Next validation and observability plan:
[PARITY_QUEUE_2026-10-14.md](PARITY_QUEUE_2026-10-14.md), P0. Preserve the
installed/live Twitch session and never simulate paid activity just for QA.
