# Twitch chat GIF previews and bounded animation

GIF fragments received through Twitch's existing chat transport have larger hover
cards, sharing the already decoded attachment. The card can animate and identifies
Twitch/GIPHY as the source. Reduced motion keeps both thumbnail and hover static.
There are no hover-time downloads, purchases or send-permission changes.

## Fixed fallback path

Previously a GIF used full-resolution frames and the ordinary 120-frame limit.
A larger or longer animation could exceed either that limit or the 24 MiB decoded
budget, causing the attachment to silently display its static fallback.

Rich GIFs now have their own bounded collector:

- Input remains limited to the existing HTTPS GIPHY host/path allowlist, 12 MiB
  wire size, 1,024-pixel source dimensions, decoder allocation limits and at most
  600 source frames.
- Each retained frame is fitted within 192×128 without upscaling.
- At most 240 frames are retained. Longer accepted loops are temporally sampled;
  skipped-frame duration is merged into retained frames, preserving the complete
  loop's duration rather than truncating it.
- Retained pixel memory is at most 240×192×128×4 bytes (22.5 MiB), within the
  existing 24 MiB per-GIF budget. Global cache budgets remain unchanged.
- Ordinary emote limits remain unchanged.
- The hover card explains static fallback reasons, reduced motion or a source
  containing only one decoded frame. Unsupported/download-limited GIFs retain
  readable feedback.

A thumbnail can still be static when the source itself is static, reduced motion
is enabled, a source exceeds the bounded limits, or global animation capacity is
unavailable. Hovering does not bypass these protections.

## Evidence boundary

Linux locked development compilation passed. Temporary collector checks covered
loop duration, frame/byte accounting, sampling transitions, the 600-source-frame
cap and thumbnail dimensions. These checks did not inspect the owner's current
Charborg message. Its exact media response and native Windows animation still
need observation; the old fallback path is a verified code issue, not a confirmed
diagnosis of that particular live GIF.
