# Discord ↔ Twitch chat bridge (idea, owner request 2026-10-11)

Not implemented. This is a future possibility: link a channel in the streamer's Discord server with their Twitch
chat, so people in each place can talk to each other. Jawjack could also show that Discord
channel as an ordinary pane.

## How it would work
- **The streamer's own Discord bot** (created in the Discord developer portal, invited to their
  server). Its token is stored in Windows Credential Manager like the Twitch login, never in workspace files.
- **Discord → Jawjack:** Discord Gateway WebSocket with the `GUILD_MESSAGES` and
  `MESSAGE_CONTENT` intents, for the one linked channel only. This also gives Jawjack a Discord pane
  (read and reply) that can sit in splits like any channel, sharing the provider-neutral feed model
  in [MULTIPLATFORM.md](MULTIPLATFORM.md).
- **Twitch → Discord:** a Discord **webhook** per linked channel. Webhooks can post with a
  per-message name and avatar, so Twitch chatters appear under their own name and Twitch avatar,
  with emotes as text or images.
- **Discord → Twitch:** sent by the broadcaster account or a separate bot account as
  `[Discord] name: message`. Twitch rate limits apply (20 messages per 30 s, or 100 for a bot with mod/VIP),
  so busy Discord channels need batching or a per-user rate.

## Rules to get right
- **No loops:** never relay the bridge's own messages (webhook ID / bot user ID / the `[Discord]` prefix).
- **Moderation follows both ways:** a Twitch delete, timeout or ban removes the mirrored Discord
  message and pauses relaying for that user; Discord deletes remove the Twitch copy where possible.
  Twitch's AutoMod and blocked terms still apply to relayed text, and messages Twitch refuses are reported back.
- **Opt-in and visible:** the bridge is off by default and shows a clear "bridged" marker in both places. The
  streamer chooses the direction (both, Twitch → Discord only, Discord → Twitch only) and
  can filter it (subs only, or no links, mentions or @everyone; mentions are always disabled on relay).
- **Mentions:** Discord pings are stripped (`allowed_mentions: none`), so Twitch can't ping the server.
- **Uptime:** a desktop app only bridges while it's running. The options are (a) relay only while
  Jawjack is open, which is simple and the default, (b) let Streamer.bot do the relaying (it already has a
  Discord integration and runs all stream), or (c) a hosted relay later. Per ROLE_EXPERIENCES §9, that would
  be a separate service with its own account isolation, not part of ordinary viewing.

## Order
After Kick/YouTube adapters establish the provider model, start with a read-only Discord pane, then
Twitch → Discord via webhook, and finally Discord → Twitch with the moderation rules above.
