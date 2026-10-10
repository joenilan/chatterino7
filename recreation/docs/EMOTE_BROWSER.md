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

## Saved favorites

Favorites are a dedicated star collection alongside the source/avatar tabs. Right-
click a tile to add/remove it, or use the selected-emote star beside search. A
pastel star marks saved tiles and an in-app notice confirms the change. Insertion
still uses the ordinary left click/Enter flow and rechecks current-channel availability.

Save up to 256 provider/asset-ID/alias keys through the existing workspace persistence
owner. Favorites are shared across local workspaces; search and service filtering
still apply. No bearer tokens or arbitrary media URLs are stored with favorites.
If an emote disappears or its catalog is unloaded, an unavailable placeholder remains
removable rather than silently consuming capacity forever. Its asset is not fetched
and it cannot be inserted. Restore validates bounded keys. Open and closed workspace
picker entities refresh together after changes.

Source builds successfully on Linux. Populated-catalog add/remove and restart
persistence still require native acceptance; empty-state UI inspection alone does not
prove those behaviors.

## Twitch account inventory

The optional account inventory uses Twitch's authenticated
[Get User Emotes](https://dev.twitch.tv/docs/api/reference/#get-user-emotes) endpoint
with `user:read:emotes`. It follows pagination independently of open channel tabs.
Returned emotes are grouped by owning broadcaster, using profile names/avatars
where loaded, and merge into an existing source tab for that broadcaster. Account
emotes participate in completion, favorites and current-channel insertion checks.
The base request has no broadcaster context; subscriber-unlocked follower emotes
returned by Twitch are retained. Additional follower-only channel-context queries
remain future work and must not grant global availability.

Normal Twitch sign-in requests chat and `user:read:emotes` together, then loads the
account inventory automatically. There is no separate enable button or feature toggle.
An older saved chat-only login remains usable for chat; the account popup explains
that the owner should sign out and sign in once to update its permissions. The picker
has an Account shortcut and shows inventory status even when other catalogs already
contain emotes. Permission consent remains with the owner during ordinary Twitch
sign-in. No grant is initiated automatically on application launch.

Inventory work is isolated from chat, cancels stale account generations, and is
processed in 16-page / 90-second batches, continuing from the last cursor rather
than repeatedly restarting the first pages. Intermediate results are usable while
loading continues. A complete pass is bounded to 512 pages, 20,000 entries and
4 MiB per response. Cursor loops are rejected. Reaching a total budget is visibly
**partial**, never reported as complete.
The inventory refreshes periodically. Transient failures retain same-account results
only until their original ten-minute expiry; authorization failures clear them.
Profiles are fetched through the existing bounded metadata owner. No account token
or inventory is written into ordinary workspace settings.

Windows build 5067379 verified normal-login account inventory loading (1,199 emotes),
with the owner completing consent. That run exposed a partial-inventory cap; the
continuation change requires a fresh native verification.


## Smooth media loading

The virtualized picker requests visible images first and preloads at most two rows
ahead, only when fewer than twelve media requests are pending. Four bounded download
workers feed a shared 512-entry, 48 MiB decoded cache. A full download queue never
evicts already decoded images. Pending requests are not duplicated just because
they have waited in the queue.

Unloaded tiles keep a fixed-size neutral placeholder, then fade in over 160 ms.
Reduced-motion mode skips the fade. Already cached images appear immediately.
Failed images retain their readable label and tooltip. The picker remains usable
throughout loading; the full account inventory is never downloaded into image
memory at once. Native visual verification is required for this loading revision.
