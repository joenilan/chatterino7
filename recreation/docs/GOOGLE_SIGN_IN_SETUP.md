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

## While in Testing

- Only the test users from step 2 can sign in. Google ends their sign-in after **7 days**, and pressing
  Connect again is one click.
- Quota is shared by every user of this project: about 5 units per chat refresh and 50 per message
  sent, out of 10,000 a day by default. You can request more in Google Cloud once people use it.

## Before a public release: verification

Google must verify the app before anyone can sign in without being a test user. `youtube.force-ssl` is a
"sensitive" scope: verification is free (no paid security audit). It needs:
- a homepage and a privacy-policy page on a domain you own (e.g. `apps.zombie.digital/jawjack`),
  with the domain verified in Google Search Console and added under Branding → Authorized domains;
- a short screen recording showing the sign-in and how Jawjack uses YouTube chat;
- a one-line reason for the scope ("read and post the user's YouTube live chat in a chat client").

Submit from https://console.cloud.google.com/auth/verification (Audience → Publish app first). It usually
takes a few days to a few weeks. After that the 7-day limit and the 100-user cap go away.

## Why the secret can ship in the app

Google treats a Desktop-app client's secret as not confidential (it can't be hidden in software people
download). The sign-in is protected by PKCE and Google's consent screen instead. Keep the file out of
git anyway, so the repository stays clean.
