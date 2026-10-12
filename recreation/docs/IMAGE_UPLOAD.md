# Image upload (planned, not built)

Status: **parked**. A bonus feature, lower priority than core app work. This
records the agreed design so it can be picked up later.

## What it is

Twitch chat is text only. Chatterino lets you paste or drag an image into the
message box; it uploads the image to a host and puts the link in your message.
Jawjack's link hover preview would then show the image.

## Design

Use the owner's Supabase project (zombie-digital) with timed deletion.

1. **Jawjack:** an image pasted (Ctrl+V) or dropped into the message box asks
   first: "Upload publicly? Deleted after 24 hours." Nothing uploads without
   that confirmation.
2. **Upload route on zombie.digital** (e.g. `POST /api/jawjack/upload`).
   Jawjack sends the image with the user's Twitch access token. The route:
   - validates the token with `https://id.twitch.tv/oauth2/validate` and checks
     it was issued to Jawjack's client ID;
   - checks the Twitch user is allowed (see Access below);
   - accepts only PNG, JPEG, GIF or WebP, verified by file signature, up to
     about 10 MB;
   - rate-limits each Twitch user (e.g. 20 uploads an hour);
   - stores the file under a random name in a public `jawjack-uploads` bucket
     and returns its public URL.
3. **Timed deletion:** a scheduled Supabase function, copied from the existing
   `supabase/functions/delete-old-emote-uploads` (which clears `emote-uploads`
   after 30 minutes), deletes uploads older than the retention period.

The Supabase service-role key stays on the server. It must never be shipped
inside the app, because anyone could extract it from the program.

## Decisions still open

- **Access:** only the owner and an allowlist of Twitch accounts (recommended
  to start), or anyone signed in to Jawjack. Open access means strangers can
  host images on the owner's bucket and domain; timed deletion limits but does
  not remove that risk.
- **Retention:** 1 hour, 24 hours (suggested) or 7 days.
- **Cleanup trigger:** confirm that `delete-old-emote-uploads` is actually run
  by a Supabase cron schedule, and schedule the new one the same way.
- **Deploying:** pushing zombie-digital `main` deploys the live site; its
  pre-push tests fail on Windows, so a deploy needs the owner's go-ahead.
