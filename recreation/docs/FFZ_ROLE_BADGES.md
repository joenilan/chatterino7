# FFZ channel moderator and VIP badges

Jawjack reuses its existing anonymous FFZ room catalog request to read `room.mod_urls` and `room.vip_badge`. No new authentication or separate polling loop is added.

A replacement is eligible only when the Twitch message already carries that role's badge and the validated FFZ room Twitch ID matches the message's channel ID. FFZ metadata never grants moderation capabilities. Shared Chat source messages retain their original Twitch badges until source-specific provenance handling is implemented.

The badge keeps the original slot and 18×18 size. Moderator art uses the reference green background; VIP art has no additional background. The regular Twitch badge remains visible while custom art loads, or if it fails validation/download/decode. Existing 7TV badges remain after Twitch badges.

Image delivery accepts HTTPS `cdn.frankerfacez.com` role-specific `/room-badge/mod/` and `/room-badge/vip/` paths only. Protocol-relative provider URLs are normalized to HTTPS. Credentials, queries, fragments, unusual ports and unsafe paths are rejected. The existing bounded anonymous media cache handles fetching and decoding without redirects. Successful empty role maps clear previous replacements; transport failures keep the last successful catalog.

Channel-login identity changes invalidate the parent catalog binding immediately. Old queued results are rejected, and mismatched cached FFZ replacements are suppressed while the new channel catalog loads.

Supplemental global supporter and channel-grant badges are described in
[FFZ supporter badges](FFZ_SUPPORTER_BADGES.md). Arbitrary CSS and browser-extension
slot replacement rules remain unsupported.

References:
- https://api.frankerfacez.com/v1/swagger.json
- https://github.com/FrankerFaceZ/FrankerFaceZ/blob/master/src/modules/chat/room.js
- https://github.com/chatty/chatty/issues/410
- Chatterino7 `src/providers/ffz/FfzEmotes.cpp` and `src/messages/MessageElement.cpp`

Linux build verified. Current live FFZ payloads, native image decoding and visual acceptance remain unverified.
