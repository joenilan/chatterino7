# YouTube live chat (read-only, first step)

Paste a YouTube live link into Add channel (Ctrl+K): `youtube.com/watch?v=…`, `youtu.be/…` or
`youtube.com/live/…`. It opens as a tab ("▶ Channel"), with "Channel · Stream title" in the header.

- **API:** official YouTube Data API v3 with your own API key (Settings → Connections → YouTube; stored in
  Windows Credential Manager, masked, with a reveal toggle). `videos.list` (1 unit) resolves the live
  chat, then `liveChatMessages.list` (5 units) is polled at YouTube's `pollingIntervalMillis`, never faster
  than 3 s. The default key quota (10,000 units a day) covers roughly 2–3 hours of live chat a day.
- **Messages:** text, Super Chats/Stickers ("[Super Chat $5.00] …") and membership messages. Owner,
  moderator and member status map to the broadcaster, moderator and subscriber badges, so search and filters
  (`is:mod`, `is:sub`) work. Each author gets a stable name colour.
- **Read-only:** sending is refused with an explanation (it needs Google OAuth, which comes later).
- **Isolation:** pane names are `yt:<videoId>`. They are filtered out of every Twitch path (subscriptions,
  stream status, 7TV, recent messages), so a YouTube tab can't affect Twitch.
- Not live, or chat off: it checks again every minute. Ended chat: the status says so. Quota used up:
  shown in the status.

Evidence (Windows, 2026-10-11): unit tests for URL parsing and message mapping. Live: a YouTube tab
without a key shows "Add a YouTube API key in Settings > Connections", and all Twitch channels still connected.
**Not yet tested with a real key or a live stream.**
