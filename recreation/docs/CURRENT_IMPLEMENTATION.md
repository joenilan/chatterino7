# Jawjack recent implementation progress

Updated 2026-10-14. This is the current implementation supplement to
FEATURE_COVERAGE.md. Older coverage-table states predate these batches; preserve
their feature IDs and remaining edges without reimplementing features that
already exist. Source, native UI and live Twitch acceptance are distinct.
The next priorities are in [the parity queue](PARITY_QUEUE_2026-10-14.md).

## Latest published previews (2026-10-14)

- **Preview.14** (commit `3ec84d69f`) adds a mentions-only titlebar bell with
  a plain number, independent from regular unread messages. `Ctrl+Shift+M`
  retains broader Highlights & Unread activity.
- **Preview.13** adds automatic and custom own-channel redemption EventSub
  handling, optional subscription retry, paginated queue for **app-manageable**
  custom rewards, wheel/Ctrl+Tab channel tab navigation, independent sidebar
  scrolling and a visible compact update footer. A genuine live redemption is
  **not yet verified**.
- Existing source also has guest/anonymous chat, Dark/Light/System themes,
  role-based tools, own-channel anchors, workspaces in separate windows,
  ten-language spellcheck and inline AutoMod. See
  [CHATTERINO_PARITY.md](CHATTERINO_PARITY.md) before filing duplicates.
- Preview.14 had desktop tests and sandboxed installer validation; those tests
  are not blanket native Twitch/visual acceptance. A genuine owner-channel
  send **was** observed in Windows validation on 2026-10-11
  ([VALIDATION.md](VALIDATION.md)).
- Current `live.rs` calculates **135** channel slots from six per-channel
  kinds, ten reserved moderated AutoMod queues per socket, and 18 own extras.
  Older 150-channel claims are stale. Recalculate from source after edits.

## Earlier implementation slices (2026-10-11)

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

- **Import from Chatterino:** read-only preview and import of layout, highlights,
  nicknames, ignored phrases, commands and display preferences. [Import](CHATTERINO_IMPORT.md)
- **AC04 / AT06 / AT03:** sharded EventSub connections with current
  `MAX_CHANNELS = 135` after optional AutoMod/own-event reserves,
  incremental changes ([connections](LIVE_CONNECTIONS.md)); local ignores
  ([ignores](IGNORES.md)); regex highlight rules.

## Evidence boundary

The earlier slices above initially had Linux build evidence; subsequent Windows
native checks and source changes are documented in `VALIDATION.md` and feature
docs. Treat each acceptance claim as specific to the **observed revision**.
Preview.14 has unit and packaging evidence, but real redemption and some
rare-event UI behavior remain unverified. No complete parity or guaranteed
backfill across Twitch WebSocket gaps is claimed.

## Active priorities (2026-10-14)

1. **P0:** channel-point/EventSub optional-kind visibility, refusal/retry and
   reconnect diagnostics; real redemption acceptance when one occurs naturally.
2. **P1:** sender AutoMod held-message outcome and elevated/hype-chat notices,
   after verifying official contracts, role and scope limits.
3. **P2:** combined feeds' per-origin room modes, chatter context and actions.
4. **P3/P4:** rare rich-media/badge/FFZ/7TV cases, compact DPI/IME/bidi,
   and native long-session/concurrency checks.
5. Moderator pins, Lua, image upload, browser extension, Kick and generic IRC
   remain recorded research/backlog, not silent feature removals.

Read the [next parity queue](PARITY_QUEUE_2026-10-14.md) and
[continuation handoff](HANDOFF_2026-10-14.md) for bounded scope, acceptance,
non-disruption rules and concrete source entry points.
