# Chatterino7 recreation: daily-client replacement

Status: initial plan and foundation, 2026-10-09. This is development in Joe's
personal fork, not an upstream contribution or a released replacement.
Selected product name: **Jawjack**, by Zombie Digital. Internal crate names
remain stable while branding is rolled out.

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

The fixed two-pane replay preview has been replaced by actual workspace tabs,
channel entry, two resizable splits per tab, tab rename/reorder/close/reopen,
custom Workspace/View menus, a collapsible scrolling sidebar, soft-wrapping single-message drafts,
font controls and versioned local persistence. Empty channels are honest offline
states; synthetic sample messages and test-only controls are gone.

Linux and Windows release builds have passed. Native Windows inspection of
3edc6575 confirmed the corrected chat layout and restored workspaces. Remaining
interactions are being inspected. The owner registered the Twitch public client;
device-code sign-in and OS-vault handling are implemented but real authorization
has not been exercised. Live receiving/sending and emote rendering remain next.

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
