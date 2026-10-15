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

## P0 optional EventSub diagnostics — 2026-10-14 working tree

Official Twitch contracts reviewed 2026-10-14:
[EventSub subscription types](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/)
define `channel.channel_points_custom_reward_redemption.add` **v1** and
`channel.channel_points_automatic_reward_redemption.add` **v2**. Both
require broadcaster-scoped `channel:read:redemptions` or
`channel:manage:redemptions`, with `broadcaster_user_id` in the condition.
Jawjack's source already requests these versions/scopes; do not silently
request new grants. No live acceptance follows from a schema review.

`inspect` now exposes `workspace.own_eventsub` without launching another
server. `subscriptions` lists all 18 own-channel optional kind/version
pairs with independent `state` (`pending`, `subscribed`, `retrying`, or
`socket_gap`), last HTTP refusal code when applicable, and approximate seconds
to the next retry. A Helix `202 enabled` without a nonempty subscription ID
is **not** considered success. Only the refused optional kind is retried;
healthy channel chat subscriptions remain live.

`reward_notifications` separately counts custom/automatic events that reached
this app (`received`) and those that parsed into Activity items (`rendered`),
with a UTC last-seen timestamp; EventSub message IDs are deduplicated.
A delivered malformed reward event increments received but not rendered.
`unmapped_reward_notifications` independently counts reward events delivered
by Twitch but dropped because no broadcaster-to-channel mapping was available;
it keeps **only a count, never the payload or user identifiers**. This exposes
one previously silent loss path. `websocket_gaps` and `last_gap_utc_epoch`
record interruptions; missed events have no guaranteed
replay. These counters reset for a different signed-in broadcaster and remain
session-only. There are **no raw redemption payloads, viewer inputs, account IDs,
tokens, or persisted notification logs** in this diagnostic view.

Studio → Activity shows a concise amber warning if Twitch refuses either
redemption subscription. A green/connected chat alone is **never evidence** that
reward events are subscribed or delivered. This is observability and recovery,
not polling historical/automatic redemptions from the app-created reward queue.

Desktop tests on 2026-10-14: 107 passed, 5 ignored; locked Windows release
build passed. Final native signed-out QA control `inspect` verified the
`workspace.own_eventsub` report structure, including the new
`unmapped_reward_notifications: 0`, with no optional subscriptions or
notifications as expected without a signed-in account. A real
redemption, live subscription acceptance, socket revocation and
long-session recovery remain unverified; see `VALIDATION.md`.

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
