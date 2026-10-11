# Google sign-in for YouTube: developer set-up (owner, once)

Do this once as Jawjack's developer. Jawjack then ships with its own Google sign-in, and users only
press **Connect** → browser → **Allow**. They never see a set-up guide or need an API key.

Use the same Google Cloud project as your YouTube Data API v3 key (the API is already enabled
there; you can confirm under https://console.cloud.google.com/apis/dashboard). Make sure that project
is selected at the top of every page below.

1. **Branding**: https://console.cloud.google.com/auth/branding
   If you see **Get started**: App name `Jawjack`, User support email = yours, Audience
   **External**, Contact email = yours, agree → **Create**.
   (Optional now, needed for verification later: app logo, homepage, privacy policy link.)
2. **Audience**: https://console.cloud.google.com/auth/audience
   Publishing status stays **Testing**. Under **Test users** → **Add users** → the Google account(s) you
   use on YouTube → Save. Up to 100 accounts can sign in while testing.
3. **Data access (scopes)**: https://console.cloud.google.com/auth/scopes
   **Add or remove scopes** → filter `youtube.force-ssl` → tick
   `https://www.googleapis.com/auth/youtube.force-ssl` → **Update** → **Save**.
   This is the one permission Jawjack asks for: read and post YouTube live chat.
4. **Client**: https://console.cloud.google.com/auth/clients
   **Create client** → Application type **Desktop app** → Name `Jawjack` → **Create** →
   **Download JSON**.
5. **Give it to Jawjack**: save the downloaded file as
   `E:\git\chatterino7\recreation\google-client.json`. It's git-ignored and never committed. The next build
   bakes it in (`crates/desktop/build.rs`). For release builds you can instead set
   `JAWJACK_GOOGLE_CLIENT_ID` and `JAWJACK_GOOGLE_CLIENT_SECRET` in the environment.
6. **Try it**: Jawjack → Settings → Connections → YouTube → **Connect**. In the browser, pick your
   account. Google shows "Google hasn't verified this app" (expected while testing) → **Continue** →
   allow YouTube access. Then paste a YouTube live link into Add channel (Ctrl+K).

## Current state (October 11, 2026)

Done: Google Cloud project `jawjack` (number 334168989213) under the YouTube account; YouTube Data API v3
enabled; consent screen "Jawjack", External; scope `youtube.force-ssl`; Desktop client "Jawjack" (saved to
`recreation/google-client.json`, git-ignored); test user added; Branding filled in:
- home page https://zombie.digital/apps/jawjack
- privacy policy https://zombie.digital/apps/jawjack/privacy (source: the zombie-digital repo,
  `app/(site)/apps/[slug]/privacy/page.tsx`)
- terms https://zombie.digital/terms, authorized domain `zombie.digital`.

## Going live, in order

1. **Publish** (owner clicks): Audience → Publish app → Confirm. Anyone can then sign in, and the 7-day
   sign-in limit ends. Until verification, people see "Google hasn't verified this app" (Advanced → Go to
   Jawjack), and there is a 100-user lifetime cap.
2. **Prove the domain**: Google Search Console → add `zombie.digital` as a Domain property, verify with the
   DNS TXT record in Cloudflare, using the same Google account as the Cloud project.
3. **Verification** (Verification Center → Prepare for verification). Answers:
   - *Scope justification (youtube.force-ssl):* "Jawjack is a desktop live-chat client for streamers and
     moderators. When the user opens a YouTube live stream in Jawjack, it reads that stream's live chat
     (liveChatMessages.list / streamList) and looks up the video's title and live chat ID (videos.list).
     When the user types a message and presses Enter, Jawjack posts it to that live chat
     (liveChatMessages.insert). Posting chat requires youtube.force-ssl; no narrower scope allows
     liveChatMessages.insert. Jawjack does nothing else with the account."
   - *Demo video* (unlisted YouTube upload, 1-3 minutes, English UI): show the app name, press Connect in
     Settings → YouTube; the consent screen showing "Jawjack" and the YouTube permission, with the browser
     address bar visible (client ID in the URL); approve; paste a live stream link with Ctrl+K; chat
     arriving; type and send a message that appears on YouTube; then Settings → Sign out.
4. **More quota** (YouTube API Services audit and quota extension form,
   https://support.google.com/youtube/contact/yt_api_form). Owner submits (it asks for legal and contact
   details). Draft answers:
   - *Project number:* 334168989213. *API client:* Jawjack (Windows desktop), https://zombie.digital/apps/jawjack.
   - *What it does:* a native chat client for live streamers, moderators and viewers, combining Twitch
     and YouTube live chat; YouTube is used only to read a live stream's chat and post the user's own
     messages, with the user's OAuth consent. No data leaves the user's PC except requests to YouTube.
   - *Endpoints and why:* videos.list (1 unit) to find a stream's live chat; liveChatMessages.streamList
     (5 units per connection) to receive chat; liveChatMessages.list only as a fallback; liveChatMessages.insert
     (50 units) for messages the user sends.
   - *Quota-saving measures already in the app:* streaming instead of polling, adaptive slow polling as a
     fallback, no requests for closed tabs, a pause until the daily reset instead of retrying once quota
     runs out, and no background use.
   - *Estimate to request:* per active user per streaming hour ≈ 12 streamList reconnects (60 units) + 20
     sent messages (1,000 units) ≈ 1,000-1,100 units. Ask for 1,000,000 units/day (≈ 300 users streaming
     3 hours a day), and request more with real usage numbers later.
   - *Screenshots to attach:* the privacy policy page, the home page, the consent screen, and Jawjack's
     YouTube tab.

The daily quota resets at midnight Pacific time. Jawjack shows "back at <local time>" when it runs out.

## Why the secret can ship in the app

Google treats a Desktop-app client's secret as not confidential (it can't be hidden in software people
download). The sign-in is protected by PKCE and Google's consent screen instead. Keep the file out of
git anyway, so the repository stays clean.
