# AutoMod queue (MD09)

Messages AutoMod holds in your channel appear in an amber strip above that channel's message box.
Each shows the chatter, the message, the reason ("AutoMod: swearing (level 2)" or "Blocked term")
and the time, with **Allow** (primary) and **Deny** buttons. The newest three are shown; the header
counts the rest.

- **Source:** EventSub `automod.message.hold` and `automod.message.update` (v2), condition
  broadcaster + moderator = you. Both are own-channel extras, so the per-socket budget still allows
  49 channels per socket.
- **Action:** Helix `POST /moderation/automod/message` (`moderator:manage:automod`). Buttons disable
  while the request runs. On success the item leaves at once and the status bar says "X's held
  message allowed by you". If it was already handled or expired, it is removed with an explanation.
- **Other moderators:** their Allow/Deny (and expiry) arrive through `automod.message.update` and
  remove the item, with "allowed by ModName" in the status bar.
- **Session only:** nothing is written to disk. Each channel keeps at most 50 held messages.
- **Diagnostics:** `inspect` → `workspace.own_extras_refused` lists any own-channel extras Twitch
  refused.

## Channels you moderate

Each EventSub socket reserves room for the AutoMod queue of **10 moderated channels** (hold +
update = 2 subscriptions each), which (with five own-channel extras) sets the per-socket channel cap to 45 (135 total).
These subscriptions are separate from a channel's required chat subscriptions: a refusal (not a mod
any more, missing scope) never affects chat and is retried after 10 minutes. On-screen channels are
first in line. When a channel closes, moves socket, or its session ends, its AutoMod subscriptions
are released or remade. `inspect` → `workspace.automod_channels` lists the covered channels.

Evidence (Windows, 2026-10-11): with 82 open channels on two sockets, 17 of 27 moderated channels
got AutoMod queues (3putt_, bessvibes, alphakep, …) and every chat stayed connected.

Not yet: inline held rows in the chat timeline (Chatterino style), and rebalancing slots toward
newly visible channels once a socket's 10 are taken.

Evidence (Windows, 2026-10-11): Twitch accepted both subscriptions for #dreadedzombie
(`own_extras_refused: []`), and the parser/queue tests pass. No message has been held live yet,
because a broadcaster's own messages are never held.
