# Jawjack patch to gpui-pre 0.3.8

Vendored from the official crates.io package with its Apache-2.0 license.
Pinned upstream revision: 279fe070bb389b79652e52065b2f001edcc0b11b.

Changes are limited to three source files:
- src/text_system.rs exposes bounded grayscale coverage for already-shaped glyphs.
- src/elements/text.rs exposes an original shaped prefix and foreground painting
  that can omit its glyphs without reshaping or changing wrap/selection geometry.
- src/text_system/line.rs implements prefix glyph omission, preserving normal
  decorations and all existing callers' behavior.

Raster callers must use actual returned dimensions, account for platform padding,
retain original geometry, and fall back for unsupported glyphs or bounds. Prefix
callers must paint a valid replacement before suppressing ordinary foreground.
The application supplies bounded caches and frame-safe image retirement.

Shared Cargo registry files and platform backends are unchanged. Review these
three source diffs when upgrading. Retain the upstream license and provenance.
