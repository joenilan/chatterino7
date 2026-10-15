# AutoMod queue (MD09)

Messages AutoMod holds in your channel appear in an amber strip above that
channel's message box **and as inline held-message rows in chat**, with
Allow/Deny controls. The strip shows the chatter, the message, the reason ("AutoMod: swearing (level 2)" or "Blocked term")
and the time, with **Allow** (primary) and **Deny** buttons. The newest three are shown; the header
counts the rest.

- **Source:** EventSub `automod.message.hold` and `automod.message.update` (v2), condition
  broadcaster + moderator = you. Both are optional own-channel extras. The
  current six chat kinds, reserved mod queues and 18 own kinds yield the
  computed **135-channel** overall cap (see LIVE_CONNECTIONS.md).
- **Action:** Helix `POST /moderation/automod/message` (`moderator:manage:automod`). Buttons disable
  while the request runs. On success the item leaves at once and the status bar says "X's held
  message allowed by you". If it was already handled or expired, it is removed with an explanation.
- **Other moderators:** their Allow/Deny (and expiry) arrive through `automod.message.update` and
  remove the item, with "allowed by ModName" in the status bar.
- **Session only:** nothing is written to disk. Each channel keeps at most 50 held messages.
- **Diagnostics:** `inspect` → `workspace.own_extras_refused` lists any own-channel extras Twitch
  refused.

## Channels you moderate

Each EventSub socket reserves room for the AutoMod queue of **10 moderated
channels** (hold + update = 2 subscriptions each). Six chat kinds fit into
46 channels per shard after the reserve, and 18 own-channel extras reduce the
three-socket cap to **135 total**. Recompute from live.rs after changes.
These subscriptions are separate from a channel's required chat subscriptions: a refusal (not a mod
any more, missing scope) never affects chat and is retried after 10 minutes. On-screen channels are
first in line. When a channel closes, moves socket, or its session ends, its AutoMod subscriptions
are released or remade. `inspect` → `workspace.automod_channels` lists the covered channels.

Evidence (Windows, 2026-10-11): with 82 open channels on two sockets, 17 of 27 moderated channels
got AutoMod queues (3putt_, bessvibes, alphakep, …) and every chat stayed connected.

Inline held rows and their action buttons are implemented
(`automod::line`, `main.rs::automod_row`, `workspace.rs` delivery). **A real
live hold still has not been observed**, so its outcome and layout need native
acceptance. Rebalancing slots toward newly visible channels once a socket's 10
are used also remains open.

Evidence (Windows, 2026-10-11): Twitch accepted both subscriptions for #dreadedzombie
(`own_extras_refused: []`), and the parser/queue tests pass. No message has been held live yet,
because a broadcaster's own messages are never held.

## Dismiss and hide (owner request, 2026-10-11)

- **Dismiss (×)** on each held message removes it from your screen without allowing or denying. Other
  moderators can still handle it, and Twitch expires it.
- **Hide here** in the strip's header stops AutoMod for that channel: nothing is shown and the channel
  is no longer subscribed, which frees its slot for channels you actually moderate. For channels where you
  only hold the mod role as a friend. Settings → Chat lists hidden channels, and "Show all again" brings them back.


## Remaining parity after preview.14

The sender's own held-message lifecycle (held → approved/rejected/expired)
still needs a current official-contract review. The parity checklist proposes
researching `channel.chat.user_message_update`; do not subscribe or claim
support without verifying condition fields, permissions and what the event
actually exposes. Avoid displaying private held text to unauthorised viewers.
This is P1 in [the parity queue](PARITY_QUEUE_2026-10-14.md).
