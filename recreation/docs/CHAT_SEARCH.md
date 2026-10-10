# Retained chat search

Ctrl+F opens Find in the current channel. Search never sends a message, downloads
history or changes the composer draft. It covers only messages still retained in
that pane. Enter/F3 and Shift+F3 navigate matches; Escape closes Find, and Latest
returns to the live tail.

## Queries

Plain text remains a case-insensitive phrase, including spaces. Adding filters,
quoted phrases or exclusions creates an AND query:

- `from:username` or `from:@username`: exact login/display-name match.
- `from-id:1234`: exact Twitch user ID, used by the chatter-history action.
- `badge:moderator`, `badge:subscriber`: Twitch badge set IDs on the message.
  This does not search independently rendered 7TV cosmetic badges.
- `has:link`, `has:emote`, `has:reply`, `has:gif`, `has:bits`: content-presence filters.
- `is:notice`, `is:deleted`: normalized notice/deleted state.
- `"great stream" -spoiler`: required phrase plus excluded text.
- `from:username has:link -example.com`: combined author/content/text filters.
- `-badge:moderator`: excludes messages carrying that Twitch badge.
- `"has:link"`: searches the literal text rather than applying the filter.

The small Links/Emotes/Replies/Bits controls add/remove filters without rewriting
quoted phrases. The help affordance explains syntax. Unknown values for recognized
filters produce an inline error. Unknown prefixes are ordinary searchable text.
The query is bounded to 256 characters and 32 structured terms; no regular-expression
engine or external query execution is involved.

## Chatter history and highlighting

Right-click a message and choose Search this chatter's messages, or open its chatter
card and choose Search messages. The search uses the immutable user ID, so display
name changes do not split that retained history. Empty/system identities do not gain
an author-search action. The card closes before Find takes focus.

Matches map Unicode case-folded bytes back to the source text before highlighting.
Overlapping ranges merge; filter-only results mark the author. New messages join an
open search, pruning follows retention, and catalog enrichment invalidates emote
filters. Deleted body text is never recovered: presence filters reject redacted
content, while explicit deleted/author/badge queries can find the remaining record.

## Coverage and verification

Advances Chatterino parity RD06 (structured retained search), RD03 (message actions)
and B2 chatter context. Cross-channel/global search, regex, saved searches and
server-side backfill remain separate future work. Exact Linux build is required
before publication; native search/menu/compact-window acceptance remains pending
for this revision. No synthetic chat or test-only application controls are added.
