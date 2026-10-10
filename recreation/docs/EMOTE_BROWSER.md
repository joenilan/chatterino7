# Emote browser

The picker uses a continuous virtualized grid rather than pages. Origin tabs select
Available here, Global, Personal (when known), or a known open-channel collection.
Service filters and search compose with that choice. Real channel avatars are used
when available; initial tiles are the fallback. Service marks are typographic.

Visible rows request media. Group headings retain service and origin identity.
Other-channel and shadowed aliases are dimmed, and current-channel availability
is checked again immediately before draft insertion. Inserting keeps the picker
open and never sends a message. Twitch globals are currently loaded; subscription,
follower, Bits and temporary account inventory remains separate unfinished work.

The popup is a deferred window-anchored overlay with adaptive columns and compact
combined navigation on short windows. It snaps inside window margins. Raw transcript
right-click-copy handlers exclude popup bounds, including neighboring panes.

- Search arrows retain text editing.
- Up/Down move across actual grid rows.
- Alt+Left/Right select adjacent cells.
- Enter inserts into the draft; Escape closes.

## Evidence and remaining review

Final locked/offline Linux build passed (exit 0). Static review covered grouping,
selection identity, live availability checks, virtualization and popup placement.
Native visual acceptance is pending. A cloud preview opened, but its graphics
backend returned EGL_BAD_SURFACE during X11 shared-memory presentation and the
window remained black. This is a preview-environment failure, not visual acceptance.

Manual review still required: avatars and fallback tiles, service/search composition,
scrolling, normal/short-window placement, repeated insertion without sending,
keyboard navigation, resizing, and interaction with channel tabs and composer.
