# 7TV paint pipeline checkpoint

This slice adds paint entitlement ingestion and inspectable cosmetic metadata.
**It does not yet paint transcript usernames.** The ordinary readable name remains
in use until the glyph-mask renderer is integrated and visually verified.

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

## Remaining renderer integration

Use actual shaped author glyphs from the existing text layout. Preserve advances,
baseline, fallback fonts, hit-test/copy offsets, selection and search backgrounds.
Only author foreground changes; colon and following space remain ordinary text.

The pinned gpui-pre 0.3.8 has private raster_bounds/rasterize_glyph methods.
A narrow vendored API is required for the planned production grayscale-mask
route. Do not modify the shared Cargo registry or substitute a rectangular mask.

The wrapper must return raster origin, actual bitmap dimensions and coverage,
reject emoji/subpixel-color output, enforce pixel budgets before and after
rasterization, and account for macOS's possible extra subpixel-phase pixel.
Windows returns one grayscale coverage byte per pixel in this mode.

InlineChat should retain Span.author on each painted Piece and replace only the
matching original shaped foreground. ChatText needs a narrow foreground-exclusion
path so glyph overhang is neither clipped nor painted twice. Cache keys must
include glyph/font identity, positions, size, scale/phase, paint and base color.
Draw precomposited images at exact device/logical correspondence.

Native Windows review must cover accents, combining marks, fallback fonts,
fractional/2× DPI, wrap, selection, author actions, clipping and busy-feed cost.
Image/animated paints and shadows require their own bounded follow-on rendering
work; they are not included in static-preview support.

## Evidence

The locked Linux development build passed for this checkpoint. Focused temporary
checks covered 13 parsing/sampling cases: gradient endpoints/midpoint, angle
normalization, signed color, alpha, duplicate stops, invalid repeat span, radial
center, unsupported effects, invalid bounds and solid colors. No live paint
entitlement or native inspector acceptance is claimed.

## Sources

- [7TV cosmetic models](https://github.com/SevenTV/SevenTV/blob/main/shared/src/old_types/cosmetic.rs)
- Chatterino reference: src/providers/seventv/SeventvPaints.cpp and paints/.
- Pinned gpui-pre 0.3.8: text_system.rs, elements/text.rs and window.rs.
