# Live connections (EventSub sharding)

Each connected channel uses six EventSub subscriptions (message, delete, clear
user, clear, notification, room settings). Twitch allows 300 enabled
subscriptions per WebSocket and three WebSockets per user token, so Jawjack
connects at most **150 channels** at once (50 per socket).

- A manager thread assigns channels to up to three socket shards, opening and
  closing sockets as needed. Channels arrive in priority order: visible panes,
  the rest of the active workspace, then other workspaces by most recent use.
  Channels beyond the cap show why they are not connected.
- Adding or closing a channel subscribes or deletes only that channel; other
  chats are not interrupted. Only a change of signed-in user resets every
  connection; token refresh and new scopes reuse existing subscriptions.
- Requests are paced (110 ms) inside Helix's shared budget and back off on low
  `Ratelimit-Remaining` or HTTP 429. A fresh session with no subscription is
  served first, because Twitch closes it after ten seconds.
- `session_reconnect` hands subscriptions over to the new socket; a fresh
  connection resubscribes its channels. A revocation affects only that channel
  (retried after a minute). Logins that no longer exist retry after 30 minutes.
- Control inspection lists `disconnected` channels with their status text.

## Evidence (Windows, 2026-10-11)

Visible panes connected ~4 s after launch with 13 channels. Adding a channel
connected it in ~2 s while #xqc kept receiving (114 → 129 messages, no gap);
closing channels kept the rest connected. With 86 channels (two sockets), every
existing account connected within ~80 s of a cold start.
Not yet exercised: three sockets (>100 channels), the 150 cap, revocation and
rate-limit back-off under real pressure, long sessions across reconnect handoffs.
