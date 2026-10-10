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

## Empty-state and account dialog follow-up

The browser distinguishes public provider loading/failure from an empty search,
and offers a bounded manual retry for failed public catalogs. Twitch's signed-out
state is explicit. Network failure does not imply an empty account inventory.

The account dialog now gives status text a full-width row above wrapping actions;
device authorization code/actions also stack to fit narrow dialogs. This fixes the
observed vertical text squeeze. Secure-storage errors remain visible; the app does
not fall back to plaintext credentials. The currently viewed preview was not replaced.

Final locked/offline Linux build passed (exit 0, 3.79s). Updated native dialog
appearance and retry interaction still require manual verification.
