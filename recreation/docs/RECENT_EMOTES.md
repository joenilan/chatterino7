# Recently used emotes

The emote browser now includes a Recently used collection, alongside Favorites.
It lists up to 48 distinct emotes from successfully sent messages, newest first.
The collection is shared across channel panes during the current account session.

- Sending must receive Twitch's positive `is_sent` acknowledgement. Inserting an emote into a draft, a failed or uncertain send, and an old session's late result do not add anything.
- Only exact emote tokens known to the active channel/account are recorded. Text, mentions and command names are not saved as emotes.
- The collection stores provider, asset identity and alias, without message bodies or media URLs. Repeated use promotes the existing item rather than duplicating it.
- Every display and insertion still checks current channel availability and alias precedence. Revoked, unavailable or shadowed aliases are omitted; their old media is not reconstructed.
- Search and service filters work without replacing recency order with alphabetical sorting. Mixed providers share one compact grid.
- Clear removes the recent list across open pickers. Sign-out/account changes and app exit clear it as well. Favorites remain independently saved.

Build validation is Linux-only for this batch. Real send acknowledgement, account switching and picker interaction still need native acceptance.
