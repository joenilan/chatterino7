# Jawjack recent implementation progress

Updated 2026-10-11. This is a concise source-and-build progress supplement to
FEATURE_COVERAGE.md. Older coverage-table states predate these batches; preserve
those feature IDs and outstanding parity rather than treating their old status
cells as proof that the newer code is absent. Native acceptance remains separate.

## Recent delivered slices

- **RD01 / RD09 / AT01–02:** Manual, hover and Alt-hover chat pause; explicit resume;
  last-read divider; first-unread and previous/next-highlight navigation.
  [Reading behavior](CHAT_READING.md)
- **IN04:** Searchable, bounded successful-send history with draft restoration.
  Included in the reading behavior document above.
- **RD03 / MD01–02:** Provider-authored Twitch mention identities open chatter cards
  from precise text targets. [Typed mentions](TWITCH_MENTIONS.md)
- **EM14:** Session-only Recently used emote collection, ordered by confirmed sends,
  with current availability checks. [Recent emotes](RECENT_EMOTES.md)
- **MD03:** Private local-profile chatter notes, deliberate save/delete, unsaved-edit
  protection and bounded storage. [Private notes](PRIVATE_CHATTER_NOTES.md)
- **EM07:** Supported static 7TV paints render over the original shaped username
  glyphs, preserving text geometry and explicit local color priority. Unsupported
  image/shadow/layered paint behavior still falls back. [Paints](SEVENTV_PAINTS.md)
- **EM11:** FFZ custom moderator/VIP room badge replacement with Twitch fallback.
  Global supporter and channel-grant badge snapshots are also implemented, with
  bounded ID-based membership and expiry. [Role badges](FFZ_ROLE_BADGES.md),
  [supporter badges](FFZ_SUPPORTER_BADGES.md)
- **AT03:** Local bounded literal/sender/channel highlight-rule editor and colors.
  Regex and full provider event policies remain open. [Rules](HIGHLIGHT_RULES.md)
- **AT10:** Local nicknames and color overrides preserve canonical Twitch identity.
  [Appearance](CHATTER_APPEARANCE.md)
- **Twitch rich media:** GIF preview cards and bounded duration-preserving animation
  sampling. Live provider examples still require acceptance. [GIFs](TWITCH_GIF_PREVIEWS.md)

## Evidence boundary

These recent batches compile in the Linux development environment. Focused source
reviews and narrow parser checks support specific implementation claims; they do
not establish Windows interaction, live sending, moderation, visual fidelity,
long-session performance, or complete Chatterino parity. Each feature document
records remaining native acceptance. Avoid replacing these distinctions with a
blanket “all tests passed” or “complete” label.

## Priorities that remain

1. Native acceptance of the combined batch largely done on Windows 2026-10-11 with
   eleven observed defects fixed (VALIDATION.md). Remaining: hover/Alt-hover pause,
   sent-history browser, FFZ room-role art, note reload after restart.
2. Live acceptance of FFZ supporter/channel grants and remaining 7TV rendering/visibility parity.
3. Account and channel capability model before moderator write actions; permission
   changes must be deliberate, and viewer chat must keep working when declined.
4. Streamer/Moderator/Viewer experiences, including the account-level own-chat
   anchor in every workspace, as specified in [ROLE_EXPERIENCES.md](ROLE_EXPERIENCES.md).
5. Continue the full register: multi-window/popouts, provider lifecycle behavior,
   moderation, filters and future multi-platform feeds. No feature is silently
   removed by this progress supplement.
