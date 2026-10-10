# Emote picker loading

The picker retains its scrolling, virtualized grid and service/streamer tabs.
Loading is scoped to the viewport and nearby rows, not the whole account catalog.

- Hovering the emote button warms a bounded set near the last scroll position.
- Opening and rendering prioritize the current grid before streamer avatars.
- Only laid-out visible avatars request low-priority images. Offscreen tabs no
  longer fill the download queue ahead of the grid.
- Nearby-row warming uses the actual scroll offset and laid-out viewport, with
  two rows behind and four ahead. The list's measurement callback does not warm
  unrelated rows at the top.
- Loading cells reserve the same dimensions and have a subtle pulse. Reduced
  motion displays a steady placeholder.
- A decoded image fades over 240 ms starting at its first clipped, visible
  prepaint in this picker. The fade wraps its canvas in a Div: GPUI 0.3.8 does not
  apply Canvas style opacity to its low-level image paint call.
- Scrolling back to an already revealed cached image does not replay the fade.
  Reveal bookkeeping is bounded to 1,024 image IDs per picker.
- Reduced motion displays decoded images immediately.

The shared media cache still uses four workers, a bounded request queue, a
48 MiB decoded budget and 512 cache entries. Low-priority warming stops at twelve
pending requests. This is bounded preloading and progressive presentation, not a
promise to fetch every subscription emote before the browser opens.

The prior implementation had ineffective canvas opacity and a cache-age fade
that could expire before an image became visible. Those source defects are
corrected here. Linux compilation and native visual verification are separate
checks; do not infer that a still screenshot proves a temporal fade.

## Decode feedback and layout invalidation

The cache inspection exposes aggregate failure categories (HTTP status, timeout,
transport, wire limit, unsupported format, decoder failure, dimensions, frame
limit, decoded-memory limit and cache budget). These are fixed labels; request
URLs, credentials and raw network errors are not included. A failed count alone
is not evidence of a network outage. Existing fallback and cooldown behavior is
unchanged.

Only base inline emote/cheer assets register transcript geometry dependencies.
Picker-only images, avatars, fixed-size badges and GIF cards still repaint when
ready, but do not independently trigger a full transcript remeasurement. A known
inline asset completing, failing, expiring its failed cooldown or being evicted
still invalidates geometry conservatively across panes. Evictions outside the
completion pump are drained on the next tick. Dependency keys also cover queue-rejected inline requests, are removed with
cache entries and are capped at 1024. Overflow switches to conservative full
invalidation for the rest of the cache session, preserving correctness. This is a source-level reduction in
unnecessary work; no frame-time speedup is claimed without measurement.
