# Chatterino7 recreation: daily-client replacement

Status: initial plan and foundation, 2026-10-09. This is development in Joe's
personal fork, not an upstream contribution or a released replacement.
Selected product name: **Jawjack**, by Zombie Digital. Internal crate names
remain stable while branding is rolled out.

## Master feature coverage (2026-10-10)

[FEATURE_COVERAGE.md](FEATURE_COVERAGE.md) is the current feature-by-feature
register, covering the audited Chatterino7 source and newer official Twitch
contracts. Its status definitions and delivery order supersede older phase lists
below. Current Twitch rich messages (GIFs, Cheers, reward highlights and typed
notices) follow acceptance of the published attention/font/status batch, reflecting
the owner's latest priority. Existing unfinished parity remains tracked.

## Goal and scope

Build a native Rust + GPUI Kit application Joe can use instead of Chatterino7,
with his Twitch account and full daily-client feature coverage. A new coat of
paint, anonymous viewer, or fixed-column mockup is not the final deliverable.
Ship useful checkpoints while retaining an explicit list of incomplete parity.
Do not change the installed Chatterino7 or migrate its credentials automatically.

Reference: `joenilan/chatterino7`, branch `chatterino7`, commit
`b3bf78c9d3894d9eb56d87a311ae2619c40aa904`. At the audit, SevenTV was seven
commits ahead (`919a6c1`), principally packaging/CI and Kick changes. Track that
upstream movement separately from acceptance of this rewrite.

## Current implementation batch: native workspace

The fixed two-pane replay preview has been replaced by sidebar workspaces,
channel tab groups, recursive resizable splits, workspace rename/reorder/close/reopen,
custom Workspace/View menus, a collapsible scrolling sidebar, soft-wrapping single-message drafts,
font controls and versioned local persistence. Empty channels are honest offline
states; synthetic sample messages and test-only controls are gone.

Linux and Windows builds have passed through the animated Twitch/7TV media batch.
Live traffic, saved-account restoration, message entrance motion and animated media
have been observed natively; the owner has also confirmed seeing animations.
Actual outbound delivery is still not accepted. Current implementation adds
Twitch badges, a searchable animated emote picker and source-text autocomplete;
see the newest VALIDATION.md entry for build/runtime evidence and limits.

## Confirmed visual direction (owner feedback, 2026-10-09)

Dark mode only. Follow Haiwire Studio palette and custom Windows title menu/frame,
not the default Windows caption. Plain-text selection must be dependable before
adding more chat features. Current reference design files:
https://github.com/joenilan/xhw_haiwire/blob/main/experiments/gpui-window/src/theme.rs
https://github.com/joenilan/xhw_haiwire/blob/main/docs/design/2026-10-04-studio-design-system.md

## Development workflow correction (owner, 2026-10-09)

Build and manually drive the actual app, using Haiwire-style inspection/control.
Remove test-only replay/timeout UI. Do not grow a test-harness project or demand
repeated owner checklists between tiny patches. Batch coherent product work.
Existing tests and old validation records are historical/supporting material,
not gates for every development change. No CI/Actions. A compiler pass establishes
compilation; runtime behavior is reported from actual interaction when performed.

The older phase descriptions below are feature inventory, not instructions to
implement every proposed harness, mock or benchmark before building the client.

## Product direction

- Dense, readable native desktop UI: restrained zinc surfaces, crisp separators,
  configurable fonts and scale, keyboard focus, minimal wasted vertical space.
- Channels remain the primary workspace. Tabs and arbitrarily nested resizable
  splits, multiple windows and popouts must eventually replace the fixed preview.
- Keep account, connection, permission and send-result states explicit.
- Emotes are first-class inline content, including multiple zero-width overlays.
  They must wrap, align, animate, select and copy correctly with text.
- No fake login, simulated successful sends, or hidden feature placeholders.
- Preserve drafts on failure and show uncertain send delivery without retrying it
  automatically. A native app should feel dependable before it looks elaborate.
- Accessibility: keyboard use, visible focus, screen-reader semantics, scalable
  text, high contrast, reduced motion, IME, and touch/DPI testing where relevant.

## Architecture

Dependencies point toward a framework-independent model:

1. `chat-core`: stable channel/user/message identifiers, fragments, timelines,
   redaction, replies, capabilities and normalized events. No GPUI or OAuth.
2. `twitch`: public-client authentication, token validation/refresh, Helix commands
   and EventSub receive/reconnect. An IRC adapter can cover compatibility gaps;
   it must not create a second message model.
3. `providers`: deterministic Twitch/7TV/BTTV/FFZ/emoji resolution, channel/global
   catalogs, personal entitlements, provider deltas and 7TV cosmetics.
4. `media`: bounded shared cache, fetch/decode/frame clocks and GPU upload.
   Reserve provider dimensions before loading. Decode limits and failure fallback
   apply to all remote media; never let each split download/decode its own copy.
5. `desktop`: GPUI Kit shell, dock/workspace tree, chat renderer, composer,
   usercards, menus and preferences. UI observes state instead of owning protocol.
6. `storage`: versioned non-secret workspace/settings, atomic replacement,
   recoverable backups and optional logs. OS credential vault is separate.
7. `replay/tests`: deterministic source events, media fixtures, fake clock/network,
   adverse lifecycle and benchmark workloads. No live moderation in automated tests.

Start with a small number of crates. Extract the remaining boundaries as real
features arrive; avoid generating empty abstraction crates in advance.

### Framework choice

Pin `gpui-kit = "=0.7.1"` and commit its resolved lockfile. The released Kit pins
matching `gpui-pre`/platform 0.3.8; do not combine arbitrary upstream GPUI versions.
Use `MessageScroller` for variable-height virtualization/tail following and
`SelectableText` for the initial plain-text replay. Override conversational row
spacing for dense chat. Keep stable identity separate from row index.

Evaluate `TextView` inline plugins for emote composition. They can host an
`InlineElement` with a baseline and copy fallback. Never interpret raw chat as
Markdown/HTML. If escaped application-controlled nodes cannot preserve exact
text/copy semantics or performance, build a focused custom text element on the
Kit's selection infrastructure. Test first; do not rewrite the renderer blindly.

GPUI's default animation behavior for inactive windows needs special attention:
Joe keeps chat beside games/streams. Decide and verify background animation policy,
while respecting reduced motion and minimizing hidden-window GPU work.

### Twitch account contract

Use a new, dedicated **public client** registration and Twitch's Device Code flow.
Do not copy Chatterino's client ID or ship a client secret. Browser Twitch login
alone does not authorize the new app. Registering an OAuth app/grant and persisting
credentials needs the owner's supported approval flow at that stage.

Receive via EventSub WebSocket and send via Helix using user:read:chat and
user:write:chat. Request moderation scopes only when implementing approved
capabilities. Validate token at startup and hourly; handle invalidation,
refresh rotation, denied scopes and user cancellation.

EventSub: subscribe within welcome timeout, process keepalives/reconnect/revocation,
resubscribe on ordinary disconnect, deduplicate envelope IDs, and explain missed
history (Twitch does not replay disconnect gaps). Helix HTTP 200 is not proof of
send: inspect is_sent and drop_reason. Never queue unbounded retries.

The reference's normal EventSub chat callbacks currently log rather than render;
its IRC path still drives visible chat. Copy behavior, not misleading class names.
Legacy PubSub is shut down and must not be ported.

## Delivery phases and completion gates

### F0: reproducible foundation (started)

- Separate Rust workspace under recreation; legacy C++ remains usable.
- Pure bounded timeline and event/redaction model with unit tests.
- Two independently scrollable native replay panes, stable row IDs, selectable
  Unicode text, deterministic bursts and timeout/redaction controls.
- Clear offline/synthetic labels. No claims of live Twitch or emote support.
- Linux core tests and compile checks; Windows MSVC run/screenshot required before
  calling the native UI accepted. Record every unrun stage.

### F1: prove the chat renderer

- Token model for text, badges, links, ordinary emotes and layered emote stacks.
- Static/animated WebP/GIF, missing images, malformed media and bounded cache.
- Selection/copy across rows including virtualized offscreen content; channel-scoped
  selection, combining marks, emoji, CJK, resizing and font changes.
- Maintain reading anchor during burst, prepend and retention pruning; tail resume.
- Measure four panes, 50,000 retained messages and 100 messages/second replay as a
  proposed workload, not an achieved claim. Record frame-time percentiles, memory,
  decode and interaction latency on the user's actual Windows hardware.
- Test visible-but-unfocused animation explicitly.

### F2: first usable authenticated client

- Approved public-client OAuth, OS-vault tokens, identity and connection display.
- Real receive/send in two editable/resizable channel splits, draft preservation,
  truthful failure/drop/uncertain-send status, reconnect without duplicate sends.
- Twitch and 7TV core emotes with live catalog updates and fallback text.
- Completion, input history, replies/cancel, timestamps, badges and safe links.
- Receive deletions/timeouts. Add capability-gated basic moderation via mocks first.
- Versioned saved channels/layout/preferences with corrupt-file recovery.
- This milestone is usable for basic daily chat, not full parity.

### F3: full 7TV and daily power features

- Personal emotes/entitlements, paints, badges, animated avatars and 4x media.
- BTTV/FFZ, provider toggles/precedence, emoji, favorites/picker/tooltips.
- Highlights, mentions, ignored users, filters, similarity suppression and nicknames.
- Search, reply threads, notifications, sound/mute, logs and streamer privacy.
- Nested split/tab/window management and keyboard-first navigation.

### F4: moderation and integration parity

- Usercards/history/notes, AutoMod, suspicious users, warnings, room modes, blocks,
  bans/timeouts/unbans, delete/clear, announcements and broadcaster commands.
- Full command/hotkey configuration; permission and missing-scope reasons.
- Recent-message history, shared-chat provenance, notifications/rewards/raids/etc.
- Overlay windows, Streamlink/tools, safe link previews/image upload, browser
  integration, backup import, updates/crash recovery and plugin compatibility plan.
- Generic IRC and experimental Kick retained on parity backlog. Decide their
  release placement explicitly instead of silently dropping them.

### F5: replace the daily client

Run a multi-day side-by-side trial with Joe. Verify Windows IME, DPI changes,
clipboard, multiple monitors, suspend/resume, revoked tokens, corrupt settings,
network outages, long sessions, accessibility and all enabled provider features.
Ship signed/reproducible Windows distribution when signing is configured. Keep
rollback and the old client until Joe accepts the replacement.

## Behavioral reference inventory

| Area | Reference paths |
| --- | --- |
| Accounts/network | src/controllers/accounts; src/providers/twitch/TwitchAccountManager.cpp; TwitchIrcServer.cpp; TwitchReadConnectionPool.cpp |
| Workspace | src/widgets/splits; src/widgets/Notebook.*; src/common/WindowDescriptors.*; src/singletons/WindowManager.cpp |
| Messages/selection | src/messages/MessageBuilder.cpp; MessageElement.*; Selection.hpp; layouts; src/widgets/helper/ChannelView.* |
| Providers | src/providers/{twitch,seventv,bttv,ffz,emoji}; src/controllers/emotes |
| 7TV layers/cosmetics | SeventvEmotes.cpp; SeventvPersonalEmotes.*; SeventvEventAPI.*; SeventvPaints.*; SeventvBadges.* |
| Input/commands | src/widgets/splits/SplitInput.*; src/controllers/{completion,commands,hotkeys} |
| Moderation | src/widgets/dialogs/UserInfoPopup.*; src/controllers/moderationactions; src/providers/twitch/{api,eventsub} |
| Daily tools | src/controllers/{highlights,filters,ignores,nicknames,notifications,logging}; src/messages/search |
| Settings/integrations | src/singletons/Settings.hpp; src/widgets/settingspages; src/controllers/plugins; docs/lua-meta |
| Regression scenarios | tests/src/{Selection,MessageLayout,InputCompletion,SplitInput,TwitchIrc,SeventvEventAPI,Image}.cpp; tests/snapshots/{IrcMessageHandler,EventSub} |

Named third-party precedence in the reference: 7TV personal, FFZ channel, BTTV
channel, 7TV channel, FFZ global, BTTV global, 7TV global. Preserve deliberately
rather than allowing request completion order to determine it.

## Safety, licensing and distribution

The reference code is MIT. Keep its copyright/permission notice with copied or
substantially translated portions. Audit per-file notices and media independently;
the notification sound has a special license. Do not blindly copy resource folders,
provider-hosted emote assets, logos, credentials or account settings.

GPUI/Kit software and examples are Apache-2.0. Documentation prose has separate
attribution requirements. This document synthesizes research and links sources;
it does not transplant their prose or illustrations. No upstream contribution,
asset purchase, account registration or production moderation is performed here.

## Primary references

- https://github.com/joenilan/chatterino7/tree/b3bf78c9d3894d9eb56d87a311ae2619c40aa904
- https://github.com/longbridge/gpui-kit/tree/v0.7.1
- https://gpui-kit.com/docs/installation/
- https://gpui-kit.com/component/message-scroller/
- https://gpui-kit.com/base/text-view/
- https://gpui-kit.com/base/text-selection/
- https://gpui-kit.com/docs/image/
- https://dev.twitch.tv/docs/chat/authenticating/
- https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#device-code-grant-flow
- https://dev.twitch.tv/docs/authentication/validate-tokens/
- https://dev.twitch.tv/docs/eventsub/handling-websocket-events/
- https://dev.twitch.tv/docs/chat/send-receive-messages/

## Jawjack multiplatform direction

Owner-approved future scope is recorded in [MULTIPLATFORM.md](MULTIPLATFORM.md):
Connections / Integrations, watching other creators as well as own streams, and
optional combined feeds. Twitch implementation remains first. Provider capability
research gates YouTube, Kick, TikTok and further integrations.

## Motion is part of feature completion

Owner direction: native UI polish includes animation when the feature is built,
not a later decoration pass. Keep motion quick and functional: sidebar reveal,
tab movement/selection, menu entry/exit, connection feedback, copy feedback and
new-message arrival without interfering with reading or selection. Never animate
every chat row gratuitously during busy streams. Support reversal and interrupted
transitions, respect OS reduced motion, and stop requesting frames when idle.
The sidebar now uses a target-based 180ms transition with a clipped sliding inner
panel; its persisted setting represents the target, not an intermediate width.
Actual Windows motion still needs direct inspection before claiming it polished.

## Composer and channel switching contract

Owner direction, 2026-10-09: one composer per visible chat pane, not an extra
composer for every channel. In a single-pane workspace, changing the selected
channel keeps the composer location and interaction stable. Explicit horizontal
or vertical splits each own their own composer and destination.

Each composer follows its active provider/account/channel context: restore that
channel's draft, select its emote inventory and autocomplete, recalculate sending
and moderation permissions, and clear or restore only a correctly scoped reply
target. Never send the previous channel's draft or reply to the newly selected one.

Cache emote metadata/assets by provider and channel. Show the correct cached set
immediately on return, then refresh stale entries asynchronously. Channel switches
invalidate request generations: late old-channel results cannot overwrite the
current picker, completion list or permissions. Loading should not remount or
steal focus from the composer, reset caret/selection, or jump the layout. These
are delivery requirements for the live chat/emote integration, not implemented
emote claims at this checkpoint.

Twitch composer correction: visual wrapping is not multiline message support.
Twitch messages are limited to 500 characters, Shift+Enter must not insert a newline,
and pasted line breaks become spaces without splitting or automatically sending
multiple messages. Over-limit pastes preserve the old draft and explain the limit.
Other providers may have different rules; limits belong to the active provider.

## Drag layout checkpoint

The layout toolbar toggle is replaced in source by native GPUI drag/drop. Drag a
channel header to workspace edges with a half-area placement preview; drag tabs
to reorder or append; drop a channel on another workspace tab to move its existing
entity. Draft/scroller state follows that entity, and close/save resolve its current
owner rather than a captured original tab. The current two-pane limit remains;
full/duplicate targets reject without deleting source state. Arbitrarily nested
VS Code-style docking is not claimed. The View menu retains a keyboard-accessible
layout alternative. Windows 7373edba verified a top-edge drop with visible preview, preserved draft,
and tab reordering with active-workspace identity intact. Other docking paths,
including full/duplicate rejection and cross-workspace moves, remain unverified.

## Next native batch: live text chat and closing polish

Implemented in source: shared Twitch EventSub receiving, message deletion/user
clear/channel clear handling, Helix sends with draft retention on failure, bounded
queues/deduplication, channel readiness, and reconnect/gap status. Real owner login
and channel readiness are verified; incoming traffic and delivered sends are not.
Emote rendering remains outstanding.

Middle mouse closes a tab only after a themed confirmation naming its workspace
and channels. Close icon and Ctrl+W use the same stable-ID confirmation. Cancel,
Escape, and default Enter leave the workspace intact; the explicit Close tab
button confirms. Duplicate requests cannot stack dialogs. Closed tabs can be
reopened during the session with Ctrl+Shift+T. These native close/cancel/reopen
flows were observed successfully on Windows at 7466e04b.

## Chat identity and composition batch — 2026-10-09

- Per-message Twitch badges, channel-first/global-fallback catalogs, 18px native
  badge icons and titles. Badge URLs are strictly allowlisted; no bearer token
  reaches the image CDN. Badge lookup never changes clipboard source text.
- A bounded metadata worker uses the existing Twitch grant, separately from the
  socket/subscription worker. Ten-minute metadata refresh; failed loads preserve
  valid data and retry after a minute. Account changes reject stale results.
- Channel-aware, searchable picker: 7TV channel/global aliases and Twitch global
  emotes, animated previews, 40-item pages, names, provider tooltips, keyboard
  dismissal and a reduced-motion-aware reveal. Search and insertion are separate
  from sending, retain the composer selection and honor the 500-character limit.
- Type :name for suggestions; arrows select, Enter/Tab inserts, Escape dismisses.
  Tab also completes a bare token. Matching is case-insensitive, exact source
  names are inserted, and changed cursor/text state cannot replace another word.
- Subscriber/personal Twitch emote ownership is not inferred from another user's
  messages. A user-owned catalog and its extra scope remain future work.
- Still pending: 7TV paints/personal cosmetics, FFZ/BTTV, reply threads, mentions,
  moderation menus, arbitrary nested splits, and the remaining parity inventory.

Sources: Twitch API global emotes/global+channel chat badges; EventSub chat
message badge fields. No account grant or scopes changed in this batch.

## Bounded chat memory — owner request, 2026-10-09

Appearance & memory offers 500 / 1k / 2k / 5k / 10k retained messages per channel.
The existing 10k default is preserved for old workspaces; the choice persists and
applies to new, open and recently closed panes. Lowering the cap trims oldest
in-memory rows and removes their dedup IDs; the scroller receives a front splice
instead of being reset or sent to the bottom. Selection is cleared only when its
endpoint was evicted. If the row being read is itself evicted, it cannot remain
available: the viewport uses the surviving history. Drafts are unaffected.

Channel headers show retained count / cap. Media has its separate bounded cache;
this is not a claim that total process memory is exactly 48 MiB. Chat history is
not written to disk by this feature, and no logging/export retention changed.

## Community emote parity — 2026-10-09

BTTV and FFZ anonymous global/channel catalogs now feed the existing bounded
image loader, inline renderer, picker and completion. Native Twitch fragments
remain authoritative. Community collisions match the original Chatterino7 order:
channel FFZ, BTTV, 7TV, then global FFZ, BTTV, 7TV. FFZ globals include only
`default_sets`; channel data selects `room.set`. BTTV combines channel and shared
emotes. Providers retain prior catalogs on HTTP failure and refresh every5minutes.

Static and animated assets have explicit strict CDN host/path allowlists. BTTV
uses animated GIF at2x and a real static WebP fallback to preserve the2MiB wire
budget; FFZ uses its returned animated/static scale URLs. Missing BTTV dimensions
start with a provisional square slot, then decoded image aspect ratio determines
inline width. Existing frame/pixel/decoded-memory limits are unchanged.

Limitations: BTTV prefix effects and legacy specially positioned overlay IDs are
not rendered by this provider yet. FFZ hidden/modifier entries are excluded from
this first standard-emote batch; its masks, transforms and centered independent
overlays need a dedicated implementation. Do not claim full modifier parity,
7TV personal cosmetics, paints or badge parity from these catalogs.

Primary references:
- https://betterttv.com/developers
- https://github.com/night/betterttv/blob/master/src/utils/cdn.js
- https://github.com/night/betterttv/blob/master/src/modules/emotes/style.css
- https://api.frankerfacez.com/v1/swagger.json
- https://www.frankerfacez.com/developers

## Screen-space docking and channel tabs — owner priority, 2026-10-09

The two-pane and sixteen-workspace product caps are removed. Workspace geometry
is a recursive horizontal/vertical tree, with resizable sibling proportions.
Drag a channel tab or header to a panel edge to split that panel; a center drop
combines channels into a tab group. Add Channel joins the first existing group.
Dragging an active tab out of a group splits against its remaining channels.
Removal collapses empty/singleton split containers. Old flat workspace layouts
migrate in memory without losing channels or drafts. Recursive geometry, selected
channel tabs and splitter sizes are included in atomic workspace persistence.

Each visible group has one active transcript/composer. Hidden channel entities
retain their per-channel draft, history and receive connection. Switching tabs
moves keyboard focus away from the old composer, so a hidden draft cannot be sent
by a subsequent Enter. Channel tabs support middle-click close confirmation.

View offers independent live-only workspace and channel-tab filters. This mirrors
Chatterino's workspace live-or-selected rule and extends it to channel groups.
The selected tab always remains visible. Unknown/stale status remains visible
rather than hiding a channel because an HTTP request failed. Hidden workspaces
remain reachable from the sidebar. Ctrl+Tab skips filtered workspace tabs.
Broadcast status comes from Helix Get Streams, not chat connection state: live,
offline and unknown have distinct indicators. Existing user authorization is used
without new scopes. Status refresh is batched up to 100 logins per request every
60 seconds; stale values expire after three minutes. No instant push guarantee.

A practical per-panel minimum protects readability; if a window is shrunk below
its retained layout, the dock can scroll instead of discarding panels. There is
no fixed pane count. This first recursive implementation still needs Windows
interaction verification, divider-reset polish and full Chatterino parity review.

Original source references: Notebook.cpp (live visibility and tab navigation),
SplitContainer.cpp/.hpp (recursive splits, drag and proportional sizing),
WindowDescriptors.cpp (recursive layout persistence).
Twitch endpoint: https://dev.twitch.tv/docs/api/reference/#get-streams

### Workspace navigation correction — 2026-10-09

Owner explicitly removed the top workspace-tab strip. Workspaces now live only
in the animated, persisted hamburger sidebar. Channel tabs are the only tab row
above each chat group. Sidebar entries retain creation, selection, drag reorder,
channel drop destinations, and confirmed close (button/middle-click). Keyboard
workspace navigation remains. Live-only workspace filtering has an explicit
Show all escape in the sidebar; channel filtering remains independent.

### Compact-window priority — owner screenshots, 2026-10-09

The reference Chatterino capture is539x469 pixels. A1050x640 hard minimum made
that use case impossible; the native minimum is now360x280. Title chrome is32px,
with hamburger, compact brand, account button and settings cog. Account controls
open on demand in a constrained dialog. Appearance is an overlay, not a dock-width
consumer. The42px channel/rename toolbar and persistent account band are removed.

Channel strips wrap and include a + which targets that specific group. Their
right-click menu offers add, live-only visibility and confirmed close. Channel
headers and composer/status chrome are denser, without removing copy feedback,
draft limits, emote access or send status. This is a compact interaction pass,
not a claim of complete Chatterino menu parity. Native540x470 verification remains
required before calling the compact layout accepted.

## Channel docking polish (2026-10-10)

Tab strips are dedicated merge/reorder targets, while leaf edges split and the
large center merges. Drag insertion respects wrapped tab rows. Tab context menus
include left/right ordering; Ctrl+PageUp/PageDown cycles the focused group and
retains each channel draft. Native/control drag ownership prevents cross-input
release from moving a panel. Native acceptance is recorded in VALIDATION.md.

## Retained-chat Find and channel actions (2026-10-10)

Per-channel Ctrl+F opens a transient compact search bar. Find matches visible
source text, usernames and emote labels within retained history only. Matching
UTF-8 source ranges are highlighted independently from copying/selection, including
inline media. Previous/next navigate matching messages, not individual occurrences;
F3, Shift+F3, Enter and Shift+Enter wrap results. Escape closes search at the found
location; Latest explicitly resumes live chat. Each pane owns its query and focus.
Queries are limited to 256 characters and highlights to 128 ranges per message.
Live appends update matches incrementally; redactions and history-limit changes
rebuild from current retained/redacted content. No server-history request or disk
archive is added. This is not yet Chatterino's advanced predicate/global search.

Channel-tab menus now offer Open stream in browser and Copy channel URL with
feedback, alongside grouping, live filtering, ordering and confirmed close.

## Message actions and local chatter cards (2026-10-10)

Right-clicking an unselected message opens actions for inspecting its author,
mention insertion, Twitch profile, copying text/username/user ID and detected
HTTP(S) links. A real text selection still uses terminal-style right-click copy,
verified clipboard feedback and selection clearing. Dialogs do not allow that
transcript-wide gesture to act on obscured chat underneath them.

Chatter cards show the server-provided login, badge labels and up to 20 current
retained messages in this channel. Counts are retained messages, not lifetime
statistics. Cards read current timeline data while open. Message action callbacks
resolve the message ID again, so deletion/retention cannot expose an old captured
body or URL. Logins are validated separately from localized display names.
Mention inserts at the current draft selection, respecting whitespace and the
500-character limit; it never sends.

Links are detected only in text fragments, excluding emote labels. HTTP(S) and
www URLs are normalized, user-info URLs rejected, punctuation trimmed on UTF-8
boundaries, and balanced parentheses retained. Ctrl+click-release opens a link or
inspects a username; ordinary clicks/drags retain normal text-selection behavior.
Pointer hit testing must be inside actual text, with matching press/release and
no drag. Underlines, hover hints and menu copy/open actions make links discoverable.
No link is fetched merely because a message is rendered. Profile/browser opening
uses an explicit action. No follower/account-age metadata or moderation controls
are claimed in this batch.


## Replies and retained conversations — 2026-10-10

Incoming EventSub replies retain parent/thread identifiers separately from message
text, so text selection, copy, search and emote offsets stay unchanged. Compact
reply context opens a live retained-conversation dialog with jump and reply actions.
The dialog uses at most the newest 100 matching rows from the existing bounded
channel timeline. Missing originals are labelled honestly; no remote history is
invented or fetched. Parent previews resolve current retained messages instead of
keeping copied text after moderation or eviction.

Reply is available from the ordinary message menu and conversation rows. A compact
composer target can be cancelled with × or Escape (after emote UI). Selecting or
cancelling it never submits or changes the draft. The selected message ID travels
with the pending send; the existing Helix request includes reply_parent_message_id
only for replies. Existing uncertainty handling and no-POST-retry behavior remain.
Shared-chat source messages are not offered as reply targets in the wrong channel.

Per-pane reply targets survive switching, docking and restart alongside drafts in
an optional `reply_drafts` workspace field. Only identifiers, name and known-deleted
state are stored, never quoted bodies. Old workspace files remain readable. A
known-deleted target blocks sending until explicitly cancelled/replaced. A restored
or evicted target is labelled outside retained history; Twitch remains authoritative
about whether that ID is still a valid send destination. Changed drafts/targets are
not cleared by acknowledgement of an older send.

Reference contracts: https://dev.twitch.tv/docs/api/reference/#send-chat-message
and https://dev.twitch.tv/docs/eventsub/eventsub-reference/#channel-chat-message-event.
No extra OAuth scopes, live moderation actions or synthetic chat were introduced.


## Ordered continuation plan — owner confirmed 2026-10-10

Deliver coherent native-verified batches, including useful QoL and visual polish.
Keep these states explicit rather than treating the original feature inventory as done:

1. **Implemented / native pass pending:** upward reply-context arrow; session-local
   unread/highlight tracking; channel/sidebar badges; deduplicated activity viewer;
   current-account mentions and replies; saved optional highlight words; Ctrl+wheel
   chat font sizing, Ctrl+plus/minus and Ctrl+0 reset. See VALIDATION.md for evidence.
2. **Next:** richer chatter cards, live channel information and chat restrictions,
   then moderation controls with appropriate confirmed permissions. Avoid presenting
   actions that the current account cannot actually perform.
3. **Queued:** 7TV paints/cosmetics, personal emotes and provider-specific behavior.
   Determine required contracts/permissions before promising exact capability.
4. **Queued:** remaining daily-client shortcuts/settings, multi-window/popouts,
   accessibility, and performance/animation refinement throughout.
5. **Later:** YouTube/Kick integrations; TikTok is low priority. Shared normalized
   message architecture should not force Twitch assumptions on future providers.

Full Chatterino feature parity is still an open program, not a completed claim.
Outgoing delivery and moderation remain distinct acceptance gaps.

### Attention behavior

Unread state is per pane because each view has its own scroll position. Only an
active-window, visible dock leaf following the tail can automatically acknowledge
rows, and only through an actually presented row inside the current content mask.
Inactive windows, hidden tabs/workspaces, offscreen splits, modal dialogs, search,
and settings do not automatically clear unread state. Channel menus and the activity
viewer provide explicit Mark read actions. Badge totals count unread pane views;
activity rows deduplicate by channel plus message ID.

Highlighting uses provider mention IDs, replies to the authenticated user's ID,
and whole-word case-insensitive text matching of that login or optional bounded
words/phrases. Emote labels and author labels are excluded; adjacent text fragments
are joined without joining across emotes. Own messages are excluded. Catalog
updates reconcile classification; moderation and retention remove stale references.
Account changes reset the attention epoch without replaying earlier messages for
the next user. Token refresh alone does not reset it.

The titlebar activity entry works with sidebar closed and live-only filters enabled.
It shows up to 200 newest matching retained messages, with Highlights / All unread,
Mark all read and Jump. No copied message log, sounds or OS notifications. Unread
counts and activity are session-local because transcript history is not persisted;
font size and highlight words are saved. Compact badges cap visually at 99+.


### Pastel stream status markers — owner request 2026-10-10

Channel tabs use mint filled circles for live broadcasts, lavender outlined circles
for offline broadcasts, and amber diamonds for unknown/unavailable status. Shapes
and tooltips distinguish states without relying on color alone. These represent
broadcast state, separately from chat connectivity and unread/highlight badges.
The markers reuse existing stream observations; no additional requests or polling.

## B1: typed Twitch rich messages (2026-10-10)

Implementation now preserves GIF, Cheermote and unknown fragments, message-type
classification, total Bits, reward IDs, Shared Chat source labels and typed notice
names/server text. GIF cards use unchanged supplied GIPHY URLs, anonymous downloads
and stable 160×112 slots; up to four render per message. Unsupported hosts or oversized
assets remain readable. GIF policy is separate from emotes: 12 MiB wire, 512px maximum
dimension, 120 frames, 24 MiB decoded per asset, within the existing 48 MiB total cache.
Failed animation can fall back to first-frame decoding of the same URL. No GIF sending.

Get Cheermotes metadata uses existing authorized Helix access, separate from anonymous
image loads. Prefix/tier mapping preserves source labels and visible Bits digits;
metadata refresh failures keep previous successful categories and retry after a minute.
Reward/intro/Power-up labels distinguish server meaning from local word highlights.
Gigantified messages use bounded 56px emotes. Message Effects receive a native pastel
accent and descriptive label, not an invented exact effect animation.

The existing user:read:chat connection also subscribes to channel.chat.notification.
Subscription/gift/raid/other notice text comes from Twitch's server fallback; unknown
notice names remain visible. No broader OAuth grant, paid action or private endpoint.
Existing arrival animation applies to these rows; visible GIFs share the media clock
and honor reduced motion. Deletion clears rich presentation and fragments.

Source/build support does not establish live acceptance of rare events. Record the
specific received types actually observed in VALIDATION.md; never generate paid events
or send test chat just to fill coverage. Full Shared Chat routing, exact Power-up
reproduction, personal emote entitlements and moderator pins remain separate work.

## B2: compact channel context (2026-10-10)

Channel details are available from a channel-tab context menu or Ctrl+I; metadata
is also summarized on hover. Live title, category, viewers, language and start time
reuse the existing batched Get Streams request and three-minute freshness policy.
No extra permanent toolbar or per-hover network request. Offline/unknown broadcast
state remains distinct from chat connectivity. The dialog body scrolls independently
with accessible dismissal at compact sizes; stream title copying has feedback.

Public room modes use Get Chat Settings without moderator_id plus
channel.chat_settings.update with the existing user:read:chat grant. REST/EventSub
name differences for follower duration and slow wait are normalized. A delayed
initial snapshot cannot overwrite a newer event; reconnect clears old room state.
Optional room-event subscription failure is visible but cannot prevent required
chat deletion/clear subscriptions. Compact composer hints explain restrictions;
the client does not infer subscription/role exemptions or falsely block sending.
Sources: https://dev.twitch.tv/docs/api/reference/#get-chat-settings and
https://dev.twitch.tv/docs/eventsub/eventsub-reference/#channel-chat-settings-update-event

## B2/B4: profile cards and everyday composer tools (2026-10-10)

Inspect chatter now loads public Twitch avatar, account creation date, broadcaster
role and bio on demand using the existing grant. It retains the clicked identity
when history rolls off. A bounded worker/cache separates metadata from chat; open
cards retry after backoff. JPEG/PNG/WebP avatars use the anonymous allowlisted
media loader and existing byte/decoded budgets. No email or follow/subscription
claims, added scope, or arbitrary image hosts.

Typing @ offers up to eight distinct recent speakers from that channel's retained
history; Enter/Tab inserts without sending, Escape dismisses. These are recent
chatters, not a viewer roster. Existing :emote and Tab completion stay available.
Alt+Up / Alt+Down recall up to 100 successful sends for the pane in this session,
restore the unsent draft on returning forward, and reset navigation after edits.
History clears on account change; delayed old-session results do not populate it.
No synthetic chat sends are needed or authorized for acceptance.
