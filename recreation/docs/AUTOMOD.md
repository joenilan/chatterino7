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

Not yet: moderated channels other than your own, which need a share of the subscription budget per
moderated channel. Also not yet: inline held rows in the chat timeline (Chatterino style).

Evidence (Windows, 2026-10-11): Twitch accepted both subscriptions for #dreadedzombie
(`own_extras_refused: []`), and the parser/queue tests pass. No message has been held live yet,
because a broadcaster's own messages are never held.
