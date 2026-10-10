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

Existing chat login requests are unchanged. **Enable subscription emotes** in the
account popup starts an explicit same-account permission upgrade. The existing
session remains available during the device flow; cancellation or an unsuccessful
upgrade preserves it subject to its original expiry. Authorizing a different account
is rejected before replacing stored credentials. The picker has an Account shortcut
and always shows inventory status in All/Twitch views, including when other catalogs
already contain emotes. The agent must not approve a grant on the owner's behalf.

Inventory work is isolated from chat, cancels stale account generations, and is
bounded to 32 pages, 20,000 entries, 90 seconds and 4 MiB per response. Cursor loops
are rejected. Reaching a budget is visibly **partial**, never reported as complete.
The inventory refreshes periodically; failed retrieval clears uncertain availability.
Profiles are fetched through the existing bounded metadata owner. No account token
or inventory is written into ordinary workspace settings.

Linux compilation is verified. Real subscription loading, owner permission approval,
and populated owner-tab behavior still require authenticated native verification.
