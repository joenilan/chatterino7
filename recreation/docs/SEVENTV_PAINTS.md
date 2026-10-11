# 7TV paint pipeline checkpoint

This slice adds observed paint entitlements, cosmetic inspection, and bounded
static gradient rendering on transcript usernames. Native visual acceptance
is still pending; Linux compilation alone is not visual verification.

## Implemented

- Reuses the existing anonymous EventAPI connection and channel subscriptions.
  There are no added per-sender requests or presence publications.
- Recognizes PAINT cosmetic definitions and entitlement create/delete messages.
  A definition alone never equips a paint.
- Keys grants by numeric Twitch connection ID. Either definition/grant arrival
  order resolves. Replacing a grant and then receiving an old paint's deletion
  does not remove the replacement.
- Repeated definitions replace old values, including transitions from a supported
  to an unsupported paint. Session reset clears definitions, grants and previews.
- Retains at most 256 definitions and 2,048 sender grants. Definitions in use are
  protected from eviction. Grants expire after 30 minutes without observation;
  expiry is checked at most once every ten seconds.
- Parses packed signed/unsigned RGBA without losing the high bit, stable duplicate
  stops, finite positions, repeat intervals, and normalized angles.
- Samples bounded static linear and radial gradients, including circle/ellipse
  shapes, stop alpha over a chosen base and hard edges at duplicate stops.
- Unsupported image, shadow, layered and styled-text paints stay explicitly marked
  unsupported rather than pretending a flat-color substitution is paint support.
- The chatter inspector displays observed badge artwork and paint name, plus a
  cached 192×32 color preview for supported static definitions. The preview uses
  the dark canvas as its base; it is not a glyph rendering or an account editor.
- Preview allocation happens only on definition changes, not every UI frame.
  At the definition cap, preview pixels total at most 6 MiB before renderer overhead.
- Inspection reports definition, grant, unresolved and unsupported counts.

Passive events do not provide a historical roster. A paint or badge may be absent
until the current connection observes the corresponding grant. Cosmetic grants
are distinct from saved local nicknames and color overrides.

## Static username rendering

Actual shaped author glyphs supply grayscale masks. Plain rows retain ChatText's
original wrapping, link colors, hit testing and selection; inline-media rows retain
InlineChat's existing geometry. The author foreground changes without recoloring
its colon or following space. Explicit saved local name colors take priority.

The vendored gpui-pre 0.3.8 patch exposes bounded grayscale rasterization and an
original-layout prefix foreground replacement. It does not edit the shared Cargo
registry or platform backends. Rasterization uses actual returned dimensions,
including possible macOS phase padding, and preserves glyph vertical offsets.

Composition limits: 64 glyphs, scale up to 4×, 128K pixels per raster, 2048×512
combined bounds, 512K pixels per composite, two new composites per 16ms interval.
Cached entries are capped at 256 and 16 MiB; pending image retirement is included
in a 32 MiB transient cap. Image retirement is deferred and forces fresh scenes
so retained GPUI paint ranges cannot replay freed atlas tiles.

Unsupported paints, emoji glyphs, negative origins, unsafe cluster boundaries,
names spanning visual wraps, or exhausted budgets retain ordinary readable text.
Cache identity includes paint, font/glyph layout, metrics, device phase and scale.
Paints use the currently observed numeric Twitch-ID grant; no historical roster
or extra per-sender fetch is implied.

Native Windows review remains: accents and combining marks, fallback fonts,
fractional/2× DPI, wrapping, selection, author actions, clipping and busy-feed
cost. Image/animated paints, layered effects and shadows remain unsupported.

## Evidence

The locked Linux development build passed for this checkpoint. Focused temporary
checks covered 13 parsing/sampling cases: gradient endpoints/midpoint, angle
normalization, signed color, alpha, duplicate stops, invalid repeat span, radial
center, unsupported effects, invalid bounds and solid colors. No live paint
entitlement or native rendering acceptance is claimed. The static username
renderer also passed the locked Linux development build; its GPU output still
needs native review.

## Sources

- [7TV cosmetic models](https://github.com/SevenTV/SevenTV/blob/main/shared/src/old_types/cosmetic.rs)
- Chatterino reference: src/providers/seventv/SeventvPaints.cpp and paints/.
- Pinned gpui-pre 0.3.8: text_system.rs, elements/text.rs and window.rs.

## Approximate rendering (2026-10-11)

Live traffic showed nearly every current paint carries drop shadows, layers or
text styling, which previously rejected the whole paint. The supported linear or
radial brush is now drawn and the omitted parts are recorded as
`approximation` ("Approximate: shadows not drawn"), shown on the chatter card.
Layered paints without a top-level brush use their first layer. Image (URL)
paints and unknown functions still fall back to the name color.

Native Windows, #xqc: 6 of 7 observed definitions rendered approximately and 1
fell back; painted names kept their original glyph geometry (no gap before the
colon). Shadows, multi-layer composition and image paints remain open parity work.

## Live connection fix (2026-10-11)

With many channels open, every change to the channel list (each newly resolved
channel ID) made the 7TV EventAPI connection reconnect and wipe all observed
paint and badge grants, so none survived. The channel routes were also sorted by
ID before 7TV's subscription limit was applied, so visible channels could fall outside it.
Now channel changes subscribe and unsubscribe in place, and channels are taken in the same
priority order as Twitch connections (visible panes first). After the fix, with
82 channels open: 0 grants → 4 badges and 5 paints from 9 entitled senders within two minutes.

## 7TV activity (2026-10-11)

After a successful send, Jawjack posts a channel presence for the signed-in user's 7TV
account (`POST 7tv.io/v3/users/{7tv id}/presences`, kind 1), at most once a minute
per channel. That's what Chatterino7's "send activity" setting does. 7TV then broadcasts the user's
entitlements, so other 7TV users, and Jawjack itself, see the paint, badge and
personal emotes. It can be turned off under Appearance & memory → "Share 7TV activity".
The 7TV account is looked up once from the Twitch ID. That response embeds emote sets and can
exceed 1 MB, so it is read with a 4 MB cap. Control inspection reports `seventv_presence`.

Evidence (Windows, 2026-10-11): after a send in #dreadedzombie, the result was "HTTP 200 for
channel 32612328". After a fresh restart, the owner's own messages showed the 7TV badge and gold paint.

## Full paint rendering (2026-10-11)

Of 7TV's 1,043 paints (catalog query, 2026-10-11), 929 have shadows and 132 are
image paints. Jawjack now draws all three parts:

- **Layers:** the top-level brush and every `gradients` layer, composited first-on-top over the
  name colour (linear/radial, repeating, per-stop alpha).
- **Shadows:** CSS `drop-shadow` semantics. They are chained in order, and each shadow is cast by
  everything drawn so far, using a three-pass box blur that approximates a Gaussian with σ = radius/2 and the
  given offset and colour. The canvas grows up to 32 px around the name to fit the glow.
- **Image paints:** URL layers from `cdn.7tv.app/paint/` load through the media cache.
  They are sized like `background-size: cover`. Animated WebP paints advance frame by frame
  (reduced motion shows frame 0), and each frame is a cached composite.
- Still approximate: `text` styling (stroke, weight, transform). Its paints are labelled
  "Approximate" on the user card.

Evidence: the owner's "North Star" paint (repeating gold stripes plus two orange glows, radius
0.1 and 4) renders with a visible orange halo. Unit tests cover the North Star definition and image-layer sampling.
No image paint has been observed live in chat yet.
