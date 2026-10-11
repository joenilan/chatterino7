# FFZ supporter and channel-grant badges

Jawjack reads the anonymous FFZ `/v1/badges/ids` catalog independently of chat
transport and login. Definitions and global assignments are replaced together;
revoked grants are not accumulated forever. Existing room requests also read
`room.user_badge_ids`. Both use Twitch IDs, including exact string IDs and numeric
IDs without a 32-bit narrowing conversion. Usernames are never an identity source.

Rendering follows native Chatterino ordering: Twitch badges (including supported
FFZ role replacements), FFZ global badges, FFZ channel badges, then 7TV badges.
Duplicate global/channel badge IDs share one slot. At most eight supplemental FFZ
badges are displayed per message. Each is an 18-pixel square with its supported
six-digit background color and a provider-labelled hover tooltip. Arbitrary CSS,
browser-extension `slot`/`replaces` semantics and custom-size artwork are not
implemented. These are cosmetics, never permission or moderation-role evidence.

Room grants require a verified room Twitch ID matching the message channel.
Shared Chat messages receive only global FFZ grants until source-specific room
provenance is implemented. Room snapshots replace previous grants, including
successful empty responses; malformed maps fail closed.

The separate global worker has one queued job, a five-second connection timeout,
a twenty-second total timeout and no redirects or credentials. Successful catalogs
refresh after fifteen minutes, failed fetches retry after sixty seconds. The
existing catalog refresh action can request an earlier retry. Last-good global
and room assignments expire after thirty minutes without successful refresh.

Bounds: sixteen MiB response body, 256 definition/map entries, one million total
assignment entries, sixteen MiB total user-ID text and 255 bytes per ID. Invalid
or oversized global maps reject the complete incoming snapshot rather than
silently retaining only some supporters. Maps keep sorted ID vectors and use
binary search during message rendering. These are engineering bounds; current
production payload size has not been measured. The control inspection reports
definition/grant counts, loading/failure state and last-good age, without user IDs.

Art must come from HTTPS `cdn.frankerfacez.com/badge/{matching badge ID}/`.
Credential-bearing URLs, queries, fragments, unexpected ports and unsafe paths
are rejected. Delivery uses the existing bounded anonymous media cache. Missing
or failed optional art does not replace the underlying Twitch badge or block chat.

References:
- https://api.frankerfacez.com/v1/swagger.json
- https://github.com/FrankerFaceZ/FrankerFaceZ/blob/master/src/modules/chat/badges.jsx
- Original Chatterino7 `src/providers/ffz/FfzBadges.cpp`, `FfzEmotes.cpp` and message builder

Linux compilation and focused parser checks are recorded separately from native
acceptance. Live supporter payloads, image decoding and Windows visual behavior
have not yet been verified for this slice.
