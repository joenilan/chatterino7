# YouTube API quota request (owner submits)

Form: https://support.google.com/youtube/contact/yt_api_form (sign in as the account that owns the
Google Cloud project `jawjack`, number 334168989213). Read on October 11, 2026. One form per app.
Not submitted yet; it can wait until Google's OAuth verification (submitted the same day) comes back.

## What only you can fill in

- Applying **as an individual** (or as Zombie Digital if it is a registered business).
- Full legal name; organization legal name ("self" if individual); primary website
  (https://zombie.digital); legal address (country, street, city, state, postal code).
- Category (pick the closest to "streaming / creator tools"), size: **Independent Developer/Sole Proprietor**.
- Primary contact name and email (technical and business contacts: "same as primary").
- How you first learned about the YouTube Data API (dropdown).
- Section 7: eight attestation checkboxes (ToS, privacy policy, developer policies, termination,
  demo-account waiver, truthfulness, data-processing consent, support recording). Read them; they are yours.

## Drafted answers

**Section 1:** Complete a compliance audit to request additional quota.

**Section 3, describe your work (max 5000 chars):**
Jawjack is a free native Windows chat client for live streamers, their moderators and viewers, in the
tradition of Chatterino. It shows Twitch and YouTube live chat side by side or merged into one feed, with
emotes, highlights, moderation tools and streamer tools. For YouTube it reads the live chat of streams the
user opens and posts the chat messages the user types, after the user signs in with Google
(youtube.force-ssl). It does nothing else with YouTube data: no uploads, edits, deletions, ratings,
comments or captions, no storage beyond the user's own PC, no servers, no analytics, no ads. The value
to the YouTube ecosystem: creators who stream to YouTube (often alongside Twitch) can run and moderate
their YouTube live chat in the same tool, which brings YouTube Live to streamers who otherwise only use
Twitch tools.

**Target audience:** Individual Content Creators; General Public (viewers and moderators).
**Monetization:** Free service (we do not charge users).
**Google representative:** No.
**Content Owner ID / Google Ads ID:** none.

**Section 4:** API Client Name: Jawjack (does not contain "YouTube").
Primary access URL: https://zombie.digital/apps/jawjack.
Privacy policy: https://zombie.digital/apps/jawjack/privacy. Terms: https://zombie.digital/terms.
Publicly accessible: Yes (preview builds; release data at zombie.digital/downloads/jawjack once published).

**Section 5 (appears after section 4):** see "Endpoints", "Quota-saving measures" and "Estimate" in
`GOOGLE_SIGN_IN_SETUP.md`: videos.list (1), liveChatMessages.streamList (5 per connection),
liveChatMessages.list (fallback), liveChatMessages.insert (50), liveBroadcasts.list (1, own live stream).
Request 1,000,000 units/day with the reasoning there.

**Section 6 (optional, one file under 10 MB, PNG/JPEG/PDF, at least 1280x720, address bar visible):**
- Architecture diagram: user's PC (Jawjack) talks directly to YouTube Data API, Twitch, 7TV/BTTV/FFZ; no
  Jawjack servers; tokens in Windows Credential Manager.
- User flow: Connect YouTube, Google consent, YouTube chat in Jawjack, sending, Sign out.
- Other: screenshots of the privacy policy page and the Jawjack YouTube tab. The OAuth demo video is
  https://youtu.be/iSrOtUL__Lw.
