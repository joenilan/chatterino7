# YouTube live chat

Paste a YouTube live link into Add channel (Ctrl+K): `youtube.com/watch?v=…`, `youtu.be/…` or
`youtube.com/live/…`. It opens as a tab ("▶ Channel"), with "Channel · Stream title" in the header.

- **API:** official YouTube Data API v3 with your own API key (Settings → Connections → YouTube; stored in
  Windows Credential Manager, masked, with a reveal toggle). `videos.list` (1 unit) resolves the live
  chat, then `liveChatMessages.list` (5 units) is polled at YouTube's `pollingIntervalMillis`, never faster
  than 3 s. The default key quota (10,000 units a day) covers roughly 2–3 hours of live chat a day.
- **Messages:** text, Super Chats/Stickers ("[Super Chat $5.00] …") and membership messages. Owner,
  moderator and member status map to the broadcaster, moderator and subscriber badges, so search and filters
  (`is:mod`, `is:sub`) work. Each author gets a stable name colour.
- **Sending (Sign in with Google):** Settings → Connections → Chat on YouTube → Set up… opens a guide.
  You create a "Desktop app" OAuth client in the same Google Cloud project (with links to the exact pages),
  add yourself as a test user, paste the client ID and secret (or the downloaded JSON), and press
  **Sign in with Google**. The browser shows Google's consent page, and Jawjack receives the code on a
  temporary 127.0.0.1 port (PKCE, scope `youtube.force-ssl`). The client and refresh token live in
  Windows Credential Manager. Messages go out through `liveChatMessages.insert` (50 quota units each).
  Sending in a YouTube tab before signing in opens the guide. In Testing mode Google ends sign-ins
  after 7 days, and the guide says so. Public release needs Google's verification for this scope.
- **Onboarding:** the first YouTube tab without a key shows **Set up YouTube…**. Every guide links the
  exact console pages and tests the key with YouTube before saving it.
- **Isolation:** pane names are `yt:<videoId>`. They are filtered out of every Twitch path (subscriptions,
  stream status, 7TV, recent messages), so a YouTube tab can't affect Twitch.
- Not live, or chat off: it checks again every minute. Ended chat: the status says so. Quota used up:
  shown in the status.

Evidence (Windows, 2026-10-11): unit tests for URL parsing and message mapping. Live: a YouTube tab
without a key shows "Add a YouTube API key in Settings > Connections", and all Twitch channels still connected.
**Not yet tested with a real key or a live stream.**

## One sign-in reads and sends (owner feedback, 2026-10-11)

A Google sign-in (OAuth) can read chat as well as send it, so **Connect YouTube** (Settings → Connections,
or the Connect YouTube… button in a YouTube tab) is the main path. It's a six-step guide: create or pick a
project, enable the YouTube Data API v3, the one-time consent form, add your own account as a test
user, create a Desktop-app client, then paste and Sign in with Google. Reading uses the sign-in's
access token (refreshed automatically) and falls back to an API key. "API key instead…" remains for
read-only use without signing in. The guide explains Google's terms ("test users" means the accounts
allowed to sign in while an app is unverified).

