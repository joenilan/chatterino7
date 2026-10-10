# 7TV personal emotes and cosmetics implementation contract

## Current boundary (2026-10-10)

- Global/channel catalogs and anonymous live object subscriptions are implemented.
  Native Windows subscription ACKs and ordinary emote rendering were observed.
- The signed-in user's first owned Personal set has source support, with public
  identity/flag checks, sender-scoped expansion and own picker entries. Native
  evidence is tracked separately in VALIDATION.md.
- Passive sender-scoped Personal/Commercial set grants now have source support,
  with bounded anonymous channel subscriptions and complete-set refetches. This
  is awaiting Windows verification. Passive badge source support is also present;
  paints remain unfinished. A public
  style ID alone is not proof of an active entitlement.
- No automatic presence publishing. This would disclose user/channel activity.

## Next data slice: passive sender-scoped entitlements

Use one anonymous EventAPI connection and reuse existing bounded media decoding.
Track Twitch user ID → entitled set IDs, set ID → parsed immutable set, cosmetic
ID → definition, and Twitch user ID → active cosmetic IDs. Never promote personal
aliases into a channel-wide catalog. Keep native Twitch fragments authoritative;
personal aliases precede community aliases only for the matching sender.

Per-channel public event conditions use ctx=channel, platform=TWITCH and the
resolved Twitch channel ID. Relevant types are cosmetic.create,
entitlement.create/delete and emote_set.*. Object subscriptions continue to watch
actual set IDs and owner user IDs. Channel subscriptions additionally consume
server-side presence-topic capacity: leave headroom under HELLO's announced limit.

Entitlement events identify their user through connections with platform TWITCH;
match numeric connection IDs, never display names. Handle deletes for EMOTE_SET,
not just badges/paints. Buffer references whose definition has not arrived yet.
A set-create followed by a pushed snapshot replaces the prior contents rather than
adding forever. Alias rename/removal is identity-aware. Unknown kinds are ignored.

Reconnect has no supported resume replay. Resubscribe and reconcile; clear or
expire unresolved event-derived assignments so revoked entries cannot persist
forever. Passive subscriptions are not a historical roster snapshot. Owned-set
REST data is also not a complete list of special sets granted through entitlements.
Document those coverage boundaries instead of promising universal personal emotes.

Avoid one HTTP request per chat sender. Prioritize observed senders and coalesce
missing set requests. Use bounded workers/queues, generations, last-good catalogs,
negative caching and Retry-After/backoff. Candidate budgets: 2,048 sender mappings,
256 sets, 10,000 total active entries, ref-counted subscriptions and LRU eviction.
These are implementation budgets, not provider guarantees or pane-count limits.

## Paint rendering: actual GPUI constraints

Verified against the installed gpui-pre 0.3.8 implementation:

- TextRun.color and Window::paint_glyph accept a solid Hsla.
- with_content_mask is rectangular clipping, not a glyph-alpha mask.
- paint_layer batches geometry; it is not an offscreen compositing target.
- TextSystem::raster_bounds/rasterize_glyph are crate-private.
- ShapedLine exposes font metrics, runs, font IDs and positioned glyphs.
- StyledText.layout().line_layout_for_index(0) provides the actual shaped layout.
- RenderImage and Window::paint_image can display a precomposited mask result.

Do not substitute a flat color or independently shaped image element and claim
paint parity. A repeated clipped-glyph prototype is possible with public APIs,
but general two-dimensional gradients multiply glyph primitives and are not the
production default for a busy chat feed.

## Bounded production first slice

Use a narrow vendored GPUI grayscale raster API, with licenses intact, rather than
editing the shared Cargo registry. Return raster bounds and coverage for existing
RenderGlyphParams. Preserve the platform rasterizer, font fallback and shaped
identities. Keep the patch small enough to review when upgrading GPUI.

Keep current text layout as the sole geometry owner. InlineChat should preserve
its author-span identity into painted pieces; ChatText should use the actual first
line's glyphs. Replace only author foreground when a mask is available. Keep colon
and space outside the paint; keep selection/search backgrounds beneath it.

Baseline comes from the original full line:

    origin.y + (line_height - ascent - descent) / 2 + ascent

Include glyph.position.y and actual shaped x positions. Do not independently
reshape the username: fallback fonts, kerning and baseline can change. Preserve
advance geometry and overhang bounds separately.

Raster at device scale with grayscale coverage. Match GPUI's origin quantization
and glyph offsets, and include scale/subpixel phase in cache keys. Composite a
bounded username mask, sample the paint into it, then produce the same pixel
convention as media.rs (red/blue conversion before RenderImage). Draw at exact
pixel/logical correspondence, not Contain scaling. Respect viewport clipping.

Cache masks and composites with hard dimension/pixel/LRU budgets. Repeated messages
from the same name/paint should reuse images. Do not create a new RenderImage ID
for every frame. First support static linear/radial paints, one visual-line name,
monochrome glyphs, no shadows or URL paints. Unsupported names/effects deliberately
fall back to readable ordinary text without being counted as supported paints.

## Original semantics to preserve

- Linear endpoints depend on angle and username rectangle. Repeats remap the
  interval between first and last stops.
- Radial paints use a centered circle of radius max(width,height)/2, not an ellipse.
- Blend stop alpha over the user's base name color before glyph coverage.
- URL paints stretch over the name rectangle and composite over user color.
- Interpret packed RGBA through its unsigned bit pattern; preserve duplicate stops.
- Reject nonfinite values and empty/zero repeating spans without division by zero.

Later image paints can reuse masks and the bounded media decoder, with their own
URL allowlist, current-frame cache, reduced-motion behavior and cancellation.
Avoid precompositing all animation frames for every name. A brush-aware GPU glyph
shader is the longer-term route, but affects several renderer backends.

## Native acceptance

Use real authorized app data and source-level evidence without manufacturing chat
traffic. Confirm unchanged line height, wrapping, hit indices and copied text;
author Ctrl-click, selection, colon exclusion, clipping, accents/combining marks,
fallback fonts and fractional/2× DPI. Measure visible-feed cost and cache bounds.
Grayscale masks may look different from ClearType, so Windows review is essential.

## References

- Original C++: src/providers/seventv/SeventvEmotes.cpp, SeventvPersonalEmotes,
  SeventvEventAPI.cpp, eventapi/, SeventvPaints.cpp and paints/.
- https://github.com/SevenTV/SevenTV/blob/main/apps/event-api/src/http/v3/mod.rs
- https://github.com/SevenTV/SevenTV/blob/main/shared/src/event_api/payload.rs
- https://github.com/SevenTV/SevenTV/blob/main/shared/src/old_types/cosmetic.rs
- https://github.com/SevenTV/SevenTV/blob/main/apps/api/src/http/v3/rest/users.rs
- https://github.com/SevenTV/EventAPI#close-codes

## Implemented passive entitlement slice (2026-10-10)

One anonymous socket now subscribes to channel entitlement create/delete/reset
and emote-set events. It never publishes presence. Grants use numeric Twitch
connection IDs; aliases resolve only for their sender. The picker exposes only
the signed-in user's grants. Owned-set bootstrap remains independent.

Dispatches carry no channel provenance, so grants are connection-session-scoped.
Changing the watched channel list recreates the connection and clears grants,
also releasing the server's hidden presence topics. Disconnect, reset and queue
overflow clear event-derived state. Each individual grant expires after 30 minutes
without another observation; a different grant cannot extend it. Passive coverage
is not a historical roster or a claim that every sender's personal set is known.

Create/update events invalidate cached set contents and trigger a coalesced full
REST fetch rather than guessing sparse rename/removal semantics. Set deletion
removes grants. Per-set request versions reject old results after newer changes;
empty responses replace old contents. Only Personal/Commercial sets and listed,
PERSONAL-eligible, Twitch-allowed emotes pass through the existing media policy.

Limits: 2,048 sender mappings, eight grants per sender, 256 referenced sets,
10,000 cached aliases, 1,024 normalized event changes, existing 32-job/2-result
HTTP queues. Half the announced subscription budget is reserved for object
updates; channel bundles use the remaining half including their hidden presence
topic. This bounds live coverage, not the number of chat panes. HTTP refreshes
retain the five-second minimum, 60-second failure cooldown and five-minute poll.

No source-level or native live event acceptance is implied by successful compilation.
The Windows verification queue remains blocked on the reported Deadlink action cap.

## Passive 7TV badges — source implemented

Cosmetic definitions and sender grants stay separate; registration alone never
equips a badge. Either arrival order resolves. One active badge per sender is
replaced on a newer grant; a delayed delete for an old ID cannot remove the new
badge. Repeated definitions replace metadata/artwork. Session resets clear both.
Each grant expires conservatively after 30 minutes without re-observation, which
can temporarily omit a still-equipped badge. Definitions remain until bounded
capacity eviction/session reset because the server may not resend known artwork.

The channel bundle now costs six slots, including cosmetic.create and the hidden
presence topic. Definitions are capped at 256 and grants at 2,048 senders. Expiry
scans run at most every ten seconds. WEBP artwork near 36px uses a separate strict
HTTPS cdn.7tv.app/badge path validator and existing anonymous media/cache budgets.

The UI uses a fixed 18px slot beside Twitch badges, aligned to the actual first
text line, with a 7TV tooltip, animation and reduced-motion support. Existing text
selection/copy/author targeting remain untouched. Native Windows rendering is
still awaiting verification.
