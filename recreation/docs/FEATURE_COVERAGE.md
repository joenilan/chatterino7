# Jawjack feature coverage and delivery plan

This is the master coverage register for replacing Chatterino7 with Jawjack and
supporting newer Twitch chat features. Every row is a deliverable or an explicit
research item. A feature being in Chatterino does not prove the current public API
supports reproducing it; a feature appearing in Twitch's current interface does
not prove a third-party client can initiate it.

Jawjack keeps its own dark, compact GPUI Kit design. Behavioral depth should match
or exceed the reference without reproducing old chrome, permanent toolbars, or
outdated networking. This register supersedes the older broad phase lists as the
feature-tracking source; PLAN.md remains the architecture and decision overview.

## Evidence and status rules

Audit date: 2026-10-10. Original C++ baseline: joenilan/chatterino7 branch
chatterino7, b3bf78c9d3894d9eb56d87a311ae2619c40aa904. Latest inspected upstream
SevenTV/chatterino7 branch chatterino7: 919a6c1fd2b4750e8fc718c164ed3d5d4d688dde.
Original local src matches the baseline. Rust implementation baseline for this
register is recreation/gpui-foundation at c12a498d82e48fc79fbb446b0d46d7ec2639200e.

- **V**: named native behavior verified. Does not imply every edge or full parity.
- **P**: source implemented and built; native acceptance remains pending.
- **Part**: useful subset exists; remaining behavior is listed explicitly.
- **Next**: prioritized work after accepting the currently published batch.
- **Backlog**: planned, not implemented.
- **Research**: contract, permission, upstream behavior or distribution needs proof.
- **Unavailable**: only use after verifying the relevant capability is unavailable;
  otherwise use Research with the unresolved question. Never silently delete a row.

Latest accepted Windows code is 0cfe8325, including the attention/font/pastel
batch and Settings pointer/compact-layout fixes. Named native checks passed for
custom-word highlights, All unread, Mark all read, visible-tail clearing, hidden
channel badges, font controls and settings persistence. Activity Jump, inactive/
offscreen acknowledgment and natural own-mention/reply matching remain unverified.
Actual delivered chat sends and live moderation are still unverified. Detailed
measured evidence stays in VALIDATION.md, not in a blanket “everything works” label.

## Product decisions that every batch must preserve

- Dark-only Studio palette, native custom titlebar, GPUI Kit controls and motion.
- Workspace navigation in the collapsible sidebar; channel tabs own the top strip.
- Unlimited recursive splits subject to practical screen space, drag placement and
  regrouping, compact minimum 360×280 without forcing that as the startup size.
- One composer per visible tab group, independent per-channel drafts/reply targets,
  no silent cross-posting or tab-switch draft loss.
- Terminal-style selection/right-click copy with visible feedback; normal editing
  shortcuts, middle-click close confirmation, helpful context menus and tooltips.
- Animation accompanies features: reversible transitions, bounded message entrance,
  visible-only animated media and reduced-motion alternatives. No flashing badges.
- Root owns GitHub changes; actual native builds and app interaction provide Windows
  evidence. No unsolicited CI/Actions or PRs. Use [skip ci] in commit messages.
- Do not add test-only buttons, fake traffic or a growing separate harness project.
  Compile, manually drive real behavior, fix observed defects and record limits.
- Preserve current user layout/account/drafts/settings before replacing a preview.
- Read/display support, sending, paid actions and moderation are separate capabilities.
  Scope/credential expansion requires its own approval when implementation needs it.
## Chatterino7 coverage register

Reference paths below are relative to the audited original repository; Rust
implementation lives under `recreation/crates`. “Part” always leaves work open.
The next-action column is intentionally behavioral rather than a list of buttons.

### Accounts and message transport

| ID | Feature | State | Remaining work and evidence boundary | Reference |
| --- | --- | --- | --- | --- |
| AC01 | Device login and saved account | Part | Native sign-in/restore observed; interrupted login, refresh/revocation and secure removal need broader real-use evidence. | controllers/accounts; providers/twitch/TwitchAccountManager.cpp |
| AC02 | Multiple accounts and switching | Backlog | Account manager, per-account grants/identity isolation, deliberate composer/account changes. | controllers/accounts |
| AC03 | Anonymous reading | Research | Preserve as a compatibility target; evaluate current supported transport and auth constraints. | providers/twitch/TwitchIrcServer.cpp |
| AC04 | EventSub lifecycle | Part | Existing reconnect/dedup/bounded queue; long-session, revocation, suspend and subscription-capacity recovery remain. | providers/twitch/eventsub; TwitchReadConnectionPool.cpp |
| AC05 | Text send and acknowledgements | Part | Source inspects send/drop/uncertainty; actual delivered send unverified. Add permission/room-aware feedback. | providers/twitch/TwitchChannel.cpp |
| AC06 | Incoming delete/user clear/channel clear | Part | Redaction implemented, including reply references; real moderation-driven acceptance remains. | messages/MessageFlag.hpp; providers/twitch/eventsub |
| AC07 | Server timestamps and message flags | Next | Extend normalized Message, time display/copy/logging, known and unknown event metadata. | messages/MessageBuilder.cpp; singletons/Settings.hpp |
| AC08 | System notices and typed events | Next | Sub/gift/raid/announcement/reward/Bits/streak/etc. rows; distinct from connection status. | providers/twitch/IrcMessageHandler.cpp |
| AC09 | Shared Chat provenance | Part | Cross-source reply guard only; origin IDs, badges, source routing/dedup/moderation still missing. | providers/twitch/TwitchChannel.cpp; messages/MessageBuilder.cpp |
| AC10 | Recent history/backfill | Research | Retained history exists; optional recent-message service, trust/privacy/source choice and prepend anchors do not. | providers/recentmessages |
| AC11 | Bounded retention | Part | Native history cap/settings observed; day-long memory, selected/pruned rows and all duplicate-view edges remain. | common/Channel.cpp; widgets/helper/ChannelView.cpp |
| AC12 | Room settings and capability state | Part | Public initial snapshot/live updates and compact hints implemented; native pending. Account role/exemption model remains. | providers/twitch/api; widgets/splits/SplitHeader.cpp |

### Reading and navigation

| ID | Feature | State | Remaining work and evidence boundary | Reference |
| --- | --- | --- | --- | --- |
| RD01 | Virtualized transcript and tail following | Part | Native scrolling/Latest works; complete anchor behavior and pause-on-hover/modifier options. | widgets/helper/ChannelView.cpp |
| RD02 | Stable selection and clipboard | Part | Native text/right-copy accepted; grapheme/bidi/CJK/IME/DPI, edge auto-scroll and layered-emote cases remain. | messages/Selection.hpp; widgets/helper/ChannelView.cpp |
| RD03 | Message actions | Part | Copy/name/ID/mention/card/reply actions accepted in named cases; configurable clicks and richer menus remain. | widgets/helper/ChannelView.cpp |
| RD04 | Safe links | Part | Source-byte targeting and copy/open implemented; browser opens not exercised in QA. Previews/unshortening/custom browser remain. | providers/links |
| RD05 | Retained Find | V | Native per-pane search, matches/navigation/live arrivals observed; exact occurrence visibility and prior pixel restoration still separate gaps. | messages/search; widgets/helper/SearchPopup.cpp |
| RD06 | Advanced and global search | Backlog | Cross-channel queries, regex, author/channel/badge/subtier/link/flag predicates. | messages/search |
| RD07 | Replies and retained conversations | Part | Native target/cancel/persistence/dialog/jump accepted; actual send and retained-root/deletion edge acceptance remain. | messages/MessageThread; widgets/dialogs/ReplyThreadPopup.cpp |
| RD08 | Followed or detached threads | Backlog | Subscribe/unsubscribe policies, thread notifications, persistent/pinned thread views. | messages/MessageThread; ReplyThreadPopup.cpp |
| RD09 | Read markers and scrollbar annotations | Backlog | Last-read line, mention/search markers and clear unread history navigation. | widgets/Scrollbar.cpp; ChannelView.cpp |
| RD10 | Moderated content policy | Part | Existing views resolve current data; future logs/previews/alerts must not resurrect deleted/private content. | messages/MessageFlag.hpp; ChannelView.cpp |

### Workspaces and windows

| ID | Feature | State | Remaining work and evidence boundary | Reference |
| --- | --- | --- | --- | --- |
| WS01 | Workspace create/rename/reorder/close/reopen | V | Native ordinary paths accepted; sidebar-only navigation is intentional Jawjack design. | widgets/Notebook; singletons/WindowManager.cpp |
| WS02 | Recursive split layout and channel decks | Part | Native edge splitting/center regrouping/tab reorder accepted; all divider/cancel/device permutations remain. No artificial two-pane cap. | widgets/splits |
| WS03 | Per-view composer and draft ownership | V | Native tab/group changes and reply-target persistence accepted; future account/provider changes must preserve it. | widgets/splits/SplitInput.cpp |
| WS04 | Sidebar and live-only filters | Part | Existing user workflow; hidden unread discovery in new activity batch awaits native acceptance. | widgets/splits/SplitHeader.cpp |
| WS05 | Broadcast status | Part | Native live/offline pastel markers observed; unknown/stale edges remain. Broadcast state must remain distinct from chat connection. | widgets/splits/SplitHeader.cpp |
| WS06 | Stream title/game/viewers/uptime | Part | Ctrl+I/context-menu details, title/category/viewers/language/start timestamp implemented; native pending. Live elapsed-uptime formatting remains. | widgets/splits/SplitHeader.cpp |
| WS07 | Quick switcher and visit history | Backlog | Keyboard-first workspace/channel switching, recent navigation and neighbor-pane focus. | widgets/dialogs/switcher; Notebook |
| WS08 | Multiple windows and popouts | Backlog | Independent windows, split/workspace popout, cross-window drag and persisted geometry. | widgets/Window.cpp; common/WindowDescriptors |
| WS09 | Overlay and attached windows | Backlog | Always-on-top, transparency/click-through and deliberate interaction modes; browser attachment later. | widgets/OverlayWindow; AttachedWindow; FramelessEmbedWindow |
| WS10 | Special channels | Part | New local activity dialog only; mentions, whispers, watching/live and AutoMod feeds need explicit models. | widgets/dialogs/SelectChannelDialog.cpp |
| WS11 | Combined feeds | Backlog | Preserve per-message origin and an explicit destination composer; never broadcast by accident. | util/MultiChannel; SelectChannelDialog.cpp |
| WS12 | Split recovery and reset conveniences | Backlog | Neighbor focus, equalize/reset divider, split close/reopen history and robust drag cancellation. | widgets/splits |

### Composer and commands

| ID | Feature | State | Remaining work and evidence boundary | Reference |
| --- | --- | --- | --- | --- |
| IN01 | Single-message editor and saved drafts | Part | Native editing/500-character/newline behavior observed; full IME/accessibility and send-wait states remain. | widgets/splits/SplitInput.cpp |
| IN02 | Emote completion | Part | Native keyboard selection/insertion accepted; configurable prefixes, ranking/favorites and account inventory remain. | controllers/completion |
| IN03 | Username completion | P | @ recent-speaker completion (eight, canonical login, retained channel history), Enter/Tab insert and Escape dismiss; native pending; full roster/broadcaster ranking remains. | controllers/completion/sources; widgets/ChatterListWidget |
| IN04 | Sent-message input history | P | Alt+Up/Down bounded session-local successful sends with draft restoration and account clearing; native pending, persistence/search remains. | widgets/splits/SplitInput.cpp |
| IN05 | Command routing | Next | Prevent unsupported slash commands being mistaken for working moderator commands; deliberate literal-text behavior. | controllers/commands/CommandController.cpp |
| IN06 | Custom command editor | Backlog | Aliases, parameters/context variables, multiword expansion and validation. | controllers/commands/CommandModel.cpp; settingspages/CommandPage.cpp |
| IN07 | Command completion | Backlog | Discover supported built-in/custom actions with permission-aware hints. | controllers/completion; controllers/commands |
| IN08 | Utility commands | Backlog | /me, usercard/chatters/mods/VIPs, copy/clear, reply/whisper, stream/popout and related utilities. | controllers/commands/builtin/Misc.cpp; builtin/twitch |
| IN09 | Broadcaster commands | Backlog | Clip/marker, title/game/color, announcements and engagement commands as supported by current APIs. | controllers/commands/builtin/twitch |
| IN10 | Configurable hotkeys | Part | Fixed shortcuts exist; action registry/editor, conflicts, contexts, arguments and import/export remain. | controllers/hotkeys; settingspages/KeyboardSettingsPage.cpp |
| IN11 | Spellcheck | Backlog | Optional dictionaries/languages/suggestions with editable-input integration. | controllers/spellcheck |
| IN12 | Font and input QoL | Part | Native font controls/reset and saved-setting persistence observed. Continue feedback/focus/cancellation polish. | widgets/splits/SplitInput.cpp; Jawjack workspace.rs |

### Emotes and Chatterino7 identity

| ID | Feature | State | Remaining work and evidence boundary | Reference |
| --- | --- | --- | --- | --- |
| EM01 | Twitch static/animated inline emotes | Part | Native Twitch and animation observed; newer rich fragments and every format/scale still separate. | providers/twitch/TwitchEmotes.cpp |
| EM02 | Available account emote inventory | Backlog | Subscriber/follower/Bits/reward/temporary entitlements; catalog presence alone is insufficient. | providers/twitch/TwitchEmotes.cpp |
| EM03 | Twitch badges | Part | Native global/channel badge media observed; category settings and all metadata/flair variants remain. | providers/twitch/TwitchBadges.cpp |
| EM04 | 7TV channel/global aliases | Part | Public catalogs and basic zero-width support exist; special/unlisted sets and provider controls remain. | providers/seventv/SeventvEmotes.cpp |
| EM05 | 7TV personal emotes | Backlog | Per-user entitlements, multiple linked-platform accounts, set changes and highest-priority resolution. | providers/seventv/SeventvPersonalEmotes |
| EM06 | 7TV live updates and presence | Backlog | EventAPI lifecycle/deltas, cosmetic changes, set migration and optional presence/activity. Current implementation polls. | providers/seventv/SeventvEventAPI; eventapi |
| EM07 | 7TV paints | Backlog | Linear/radial/image paints, animated textures, shadows, DPI and mentions; user-selectable effect limits. | providers/seventv/SeventvPaints; paints |
| EM08 | 7TV badges | Backlog | Entitlements, animation and visibility options. | providers/seventv/SeventvBadges |
| EM09 | Twitch and 7TV avatars | Part | On-demand Twitch avatar with allowlisted bounded JPEG/PNG/WebP; native pending; 7TV identity remains. | widgets/dialogs/UserInfoPopup.cpp |
| EM10 | BTTV and FFZ core | Part | Catalog counts observed; source media support exists, provider-specific visual/picker checks remain. | providers/bttv; providers/ffz |
| EM11 | BTTV live updates and FFZ badges | Backlog | Activity/update lifecycle, custom mod/VIP/supporter badges and provider options. | providers/bttv; providers/ffz |
| EM12 | Provider modifiers and overlays | Part | 7TV overlay foundation exists; BTTV/FFZ modifiers currently skipped; multiple-overlay geometry/copy/wrap still needs coverage. | messages/layouts; providers/bttv; providers/ffz |
| EM13 | Emoji | Part | Unicode text only; shortcodes, picker styles, favorites and boundary correctness remain. | providers/emoji |
| EM14 | Picker and favorites | Part | Native searchable animated picker/paging; source/creator details, tabs, tags, favorites and rich menus remain. | widgets/dialogs/EmotePopup.cpp |
| EM15 | Precedence and consistency | Part | Deterministic channel/global order exists; personal 7TV first and agreement across picker/completion/rendering remain. | messages/MessageBuilder.cpp |
| EM16 | Formats, scale and motion | Part | GIF/WebP emote decode exists; 4×/AVIF and full DPI/background/hidden/minimized policy need explicit decisions and evidence. | messages/Image; layouts; CHANGELOG.c7.md |

### Attention and filtering

| ID | Feature | State | Remaining work and evidence boundary | Reference |
| --- | --- | --- | --- | --- |
| AT01 | Unread and retained activity | Part | Native unread/tail clearing, activity filters and Mark all read observed; Jump and inactive/offscreen edges unverified. | Jawjack attention.rs; activity.rs |
| AT02 | Basic mentions and highlight words | Part | Native custom-word highlighting observed; natural own-mention/reply matching unverified. | controllers/highlights; settingspages/HighlightingPage.cpp |
| AT03 | Advanced highlight rules | Backlog | Regex/case/user/badge rules, colors, priority/exclusions and bounded evaluation. | controllers/highlights |
| AT04 | Special-event highlight policies | Backlog | Whisper/sub/reward/first-message/announcement/AutoMod/thread rules. | singletons/Settings.hpp |
| AT05 | Sounds, taskbar and attention preferences | Backlog | Per-rule/per-channel sound/mute, foreground behavior, duration and privacy controls. | controllers/pings; sound; highlights |
| AT06 | Local ignores and replacements | Backlog | Phrase/regex replacement, exclusions and safe display semantics. | controllers/ignores; settingspages/IgnoresPage.cpp |
| AT07 | Twitch blocks | Backlog | Approved account-backed block sync/actions, local visibility policy and clear distinction from local ignore. | controllers/ignores |
| AT08 | Message filter language | Backlog | Named typed expressions, validation, per-view selection and regex limits. Live-only tab filtering is unrelated. | controllers/filters/lang; FilterSet.cpp |
| AT09 | Similarity suppression | Backlog | Bounded threshold/window/user rules, dim/hide and highlight interaction. | messages/MessageBuilder.cpp; Settings.hpp |
| AT10 | Local nicknames and colors | Backlog | Canonical identity remains intact under display aliases and per-user color overrides. | controllers/nicknames; userdata |
| AT11 | Pronouns | Research | Provider contract/cache, opt-in display and privacy/preferences. | providers/pronouns |

### Usercards and moderation

| ID | Feature | State | Remaining work and evidence boundary | Reference |
| --- | --- | --- | --- | --- |
| MD01 | Local chatter cards | V | Native retained-message card/copy/mention accepted; richer account data remains below. | widgets/dialogs/UserInfoPopup.cpp |
| MD02 | Full profile context | Part | Public Twitch created-at/avatar/bio/broadcaster role in compact card; native pending. Follow/subscription data and moderation remain. | UserInfoPopup.cpp |
| MD03 | Persistent notes | Backlog | Local notes editor/storage and streamer-mode redaction. | controllers/userdata; EditUserNotesDialog.cpp |
| MD04 | Chatter list and roles | Backlog | Filtering/fetch limits, actual account permissions, distinguish chatters from viewers. | widgets/ChatterListWidget.cpp |
| MD05 | Delete, timeout, ban and undo-family actions | Backlog | Target/reason/duration, channel role/scopes, result handling; receiving redaction is not action support. | controllers/moderationactions; builtin/twitch/Ban.cpp; Unban.cpp; DeleteMessages.cpp |
| MD06 | Warnings and blocks | Backlog | Explicit warnings/block/unblock semantics and proper permissions. | builtin/twitch/Warn.cpp; Block.cpp |
| MD07 | Moderator mode and custom buttons | Backlog | Compact optional actions, editable presets, keyboard equivalents and clear unavailable reasons. | settingspages/ModerationPage.cpp |
| MD08 | Room mode management | Backlog | Slow/follower/subscriber/emote-only/unique-chat display and authorized changes. | builtin/twitch/ChatSettings.cpp |
| MD09 | AutoMod and blocked terms | Backlog | Held queue, approve/deny, reasons/categories, state updates and privacy of blocked terms. | providers/twitch/eventsub; messages/MessageBuilder.cpp |
| MD10 | Suspicious users | Backlog | Monitored/restricted state, annotations and capability-gated management. | builtin/twitch/LowTrust.cpp |
| MD11 | Roles and Shield Mode | Backlog | Mod/VIP lists/changes, Shield status/control and exact permission errors. | builtin/twitch/AddModerator.cpp; AddVIP.cpp; ShieldMode.cpp |
| MD12 | Announcements and shoutouts | Backlog | Authenticated per-channel actions, display variants and results. | builtin/twitch/Announce.cpp; Shoutout.cpp |
| MD13 | Raids, ads, polls and predictions | Research | Verify each current endpoint, role and consequential-action flow; do not blindly port old commands. | builtin/twitch/Raid.cpp; StartCommercial.cpp; Poll.cpp; Prediction.cpp |
| MD14 | Pins | Research | Current privileged APIs and ordinary-viewer limitations are documented in TW10/11. | builtin/twitch/Pin.cpp |

### Preferences and integrations

| ID | Feature | State | Remaining work and evidence boundary | Reference |
| --- | --- | --- | --- | --- |
| PF01 | Appearance and density | Part | Dark Studio retained; font family/weight, UI scale, timestamps/line/badge/emote options and collapse controls remain. | settingspages/GeneralPage.cpp |
| PF02 | Accessibility | Part | Toolkit/fixed shortcuts only; screen reader semantics, keyboard-only paths, focus, high contrast, IME/bidi/DPI need acceptance. | Jawjack PLAN.md; native controls |
| PF03 | Streamer privacy | Backlog | Manual/automatic policy for notes, whispers, thumbnails, restricted users, viewer data and alerts. | singletons/StreamerMode; Settings.hpp |
| PF04 | Optional logs | Backlog | Destination/allowlist/per-stream files, privacy and deletion/retention policy; distinct from transcript memory. | controllers/logging |
| PF05 | Live notifications | Backlog | Channel rules, toast/sound/taskbar, initial-live suppression and deliberate navigation. | controllers/notifications; settingspages/NotificationPage.cpp |
| PF06 | External playback/tools | Part | Browser URLs exist; Streamlink/custom player/quality/options/mod-view/clip/marker integration remain. | settingspages/ExternalToolsPage.cpp; util/StreamLink |
| PF07 | Link previews and image uploads | Research | Safe bounded previews, explicit upload destination/approval, progress/error and deletion URL; no automatic screenshot upload. | providers/links; util/ImageUploader |
| PF08 | Browser integration | Research | Native messaging, approved extension IDs, watching feed and attached-window behavior. | singletons/NativeMessaging; widgets/AttachedWindow |
| PF09 | Plugins | Research | Decide Lua compatibility or replacement; bounded commands/events/network/files and migration strategy. | controllers/plugins; docs/lua-meta; widgets/PluginRepl.cpp |
| PF10 | Settings backup/recovery | Part | Atomic versioned JSON/prior backup exists; user-facing restore/import/export/profiles/portable paths and migration UX remain. | RestoreBackupsDialog.cpp; Jawjack storage.rs |
| PF11 | Packaging, update and rollback | Backlog | Installer/signing/update strategy, crash recovery, licensing and ownership-safe rollout. | singletons/Updates; LastRunCrashDialog.cpp |
| PF12 | Generic IRC | Research | Explicit legacy parity item; select supported scope and release placement rather than silently dropping it. | providers/irc |
| PF13 | Kick | Research | Original experimental transport/features need a current official-provider audit; no parity claim from source alone. | providers/kick; builtin/kick; KickLoginPage.cpp |
| PF14 | YouTube and other providers | Backlog | New scope, separately tracked in MULTIPLATFORM.md; viewer feeds and explicit combined-feed destinations. TikTok later. | MULTIPLATFORM.md |

### Upstream cases to preserve

The Chatterino7 changelog is also an acceptance source: multiple same-platform
connections, special emote sets, paint/shadow behavior, high-DPI painted mentions,
clickable mentions during personal-emote updates, AVIF selection and correct avatar
URLs. These should become concrete checks within EM05–16, not disappear beneath a
single “7TV supported” checkbox. [Fork changelog](https://github.com/joenilan/chatterino7/blob/b3bf78c9d3894d9eb56d87a311ae2619c40aa904/CHANGELOG.c7.md)
## Current Twitch coverage beyond the legacy reference

These rows are required coverage, even when the old Chatterino client does not
implement them. **Receive/render and send/act have independent statuses.**

| ID | Feature | Current Jawjack | Planned treatment and boundary |
| --- | --- | --- | --- |
| TW01 | GIF Keyboard messages | P | A dedicated GIF fragment and renderer; never silently discard a GIF-only row. Paid Tier 2/3 eligibility applies to sending in enabled channels, not to viewing received GIFs. Public GIF sending remains Research. |
| TW02 | Cheers and Cheermotes | P | Preserve amounts and tier, resolve the appropriate media, show readable fallback and accessible amount. Generic emote rendering alone is not Cheer support. |
| TW03 | Bits Message Effects | Part | Respect the ordinary message classification and offer a restrained native effect/fallback. Exact effect reproduction needs a documented mapping and sufficient metadata; do not invent it from a color or badge. |
| TW04 | Gigantify an Emote | P | Use a distinct bounded presentation rather than treating it as an ordinary-size emote. Preserve identity and readable selection/copy behavior. |
| TW05 | On-Screen Celebration and custom Power-ups | Research | Distinguish broadcaster-authorized detail from ordinary chat. A desktop chat client should not claim to reproduce a stream-video celebration from absent data. |
| TW06 | Channel Points highlighted messages | P | Separate reward styling from local keyword highlights and Bits effects; retain reward provenance. |
| TW07 | Sub-only reward messages and introductions | P | Typed message presentation, compact labeling and graceful handling when detail is absent. |
| TW08 | Subscription and community notices | Part | Rich system rows for subscriptions, resubs, gifts, upgrades, raids, announcements, streaks and newer notices. Respect anonymity and server fallback text. |
| TW09 | Shared Chat | Part | Current cross-channel reply guard exists; full provenance, source badges, origin links, deduplication and explicit send semantics remain missing. |
| TW10 | Moderator-pinned messages | Research | Public pin APIs now exist, but they are privileged. Provide read/manage affordances only with verified scope and channel role; ordinary-viewer pin parity is not established. |
| TW11 | Pinned Cheers | Research | Separate product feature from moderator pins. Do not route it through the mod-pin API merely because both use the word “pinned.” |
| TW12 | Personal Twitch emote entitlements | Backlog | Paginated Get User Emotes and channel context for follower emotes. Distinguish catalog availability from permission to send. Requires an additional approved scope. |
| TW13 | Current badge metadata | Part | Existing global/channel badge rendering; preserve newer metadata, tier/flair distinctions and unknown badge types rather than dropping them. |
| TW14 | Initial and changing room restrictions | P | Fetch initial settings, then reconcile live updates. Composer explains slow/follower/subscriber/emote-only or verification restrictions without erasing drafts. |
| TW15 | Held, rejected and uncertain messages | Part | Existing drop/uncertainty feedback; add held-message lifecycle and better structured reasons. Never blindly resend an ambiguous POST. |
| TW16 | New event and fragment types | Part | Preserve unknown identifiers and display a useful fallback. Schema changes must not make messages disappear. |

B1 implementation now covers these basic receive/render paths; native rare-event
acceptance is pending. Message Effects use semantic accent/label, and notifications
use typed names plus server fallback text. These do not claim full effect reproduction
or every notice-specific action/detail. See PLAN.md for concrete limits.

### Verified contracts and gaps

Current EventSub identifiers to preserve include fragment types `text`, `emote`,
`cheermote`, `mention`, `gif`; `gif.id/url`; `cheer.bits`; `message_type` values
`channel_points_highlighted`, `channel_points_sub_only`, `user_intro`,
`power_ups_message_effect`, `power_ups_gigantified_emote`; reward IDs; reply parent
and thread IDs; Shared Chat source IDs/badges; and notification `notice_type` with
`system_message` fallback. The ordinary chat payload does not provide a detailed
message-effect ID. [EventSub schema](https://dev.twitch.tv/docs/eventsub/eventsub-reference/#channel-chat-message-event)

Broadcaster-authorized `channel.bits.use` carries richer Power-up details,
including `power_up.message_effect_id`, and needs `bits:read`. This is not an
ordinary viewer's event feed for every watched channel. [Bits event authorization](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channelbitsuse)

Twitch documents the GIF Keyboard for paid Tier 2/3 subscribers where enabled.
The supplied GIF URL must remain unchanged; IRC also has a `gifs` positional tag.
Support receiving first. No documented public API input for sending a GIF was found
in this audit. The same gap applies to viewer-side Bits spending, Power-up purchases
and Channel Points redemption. Do not emulate private web endpoints to manufacture
support. [GIF eligibility](https://help.twitch.tv/s/article/chat-basics),
[IRC GIF tags](https://dev.twitch.tv/docs/chat/irc/#privmsg-tags),
[public send contract](https://dev.twitch.tv/docs/api/reference/#send-chat-message)

Public chat sending supports text/emotes and replies; `is_sent` and `drop_reason`
are authoritative. User-token Shared Chat sends have different routing constraints
from app-token `for_source_only`. Send-and-pin is privileged and has incompatible
parameter combinations. These must be modeled as capabilities, not a generic
“send anything” function. [Sending and replies](https://dev.twitch.tv/docs/chat/send-receive-messages/),
[send API](https://dev.twitch.tv/docs/api/reference/#send-chat-message)

Mod-pin APIs are documented as GET/PUT/PATCH/DELETE `/helix/chat/pins`. The read
path itself requires moderator/broadcaster authorization; no pin EventSub event was
found in this audit. Plan a bounded authorized refresh path rather than promise
instant ordinary-viewer pin sync. Pinned Cheers are separate.
[Pin APIs](https://dev.twitch.tv/docs/api/reference/#get-pinned-chat-message),
[API changelog](https://dev.twitch.tv/docs/change-log/),
[Cheering settings](https://help.twitch.tv/s/article/cheering-for-partners-affiliates)

Get User Emotes uses `user:read:emotes` and pagination; a channel/global list is
not a user's send entitlement list. Room-setting events need an initial snapshot.
No public custom written-rule acceptance endpoint was found. Link to Twitch when a
required action cannot be performed through the supported interface.
[Emote and room migration guide](https://dev.twitch.tv/docs/chat/irc-migration/)

### Permission-aware delivery

| Capability | Scope family to verify before implementation | UI rule |
| --- | --- | --- |
| Receive and ordinary send | Existing `user:read:chat`, `user:write:chat` | No new grant for current baseline. |
| Personal picker | `user:read:emotes` | Explain missing entitlement access without hiding received emotes. |
| Own subscription eligibility | `user:read:subscriptions` where needed | Do not infer tier from account identity or unrelated channel badges. |
| Pin/delete | `moderator:read:chat_messages`, `moderator:manage:chat_messages` | Role plus scope, explicit read/manage distinction. |
| Announcements and room modes | `moderator:manage:announcements`, `moderator:manage:chat_settings` | Per-channel capabilities and clear target. |
| Timeout/ban and warnings | `moderator:manage:banned_users`, `moderator:manage:warnings` | Deliberate target/duration/reason; show actual result. |
| AutoMod, Shield, suspicious users | Corresponding AutoMod/Shield/suspicious-user scopes | Implement the relevant panel before requesting access. |
| Broadcaster event detail | `bits:read`, `channel:read:redemptions`, `channel:read:subscriptions` | Owner-authorized integration; not a requirement for a viewer client. |
| Optional IRC compatibility | `chat:read`, `chat:edit` | Add only for a demonstrated compatibility gap. |

Scopes and channel roles can change; recheck their precise current contracts at
implementation time. This plan authorizes neither broader grants nor paid actions.
[Scope reference](https://dev.twitch.tv/docs/authentication/scopes/),
[chat authentication](https://dev.twitch.tv/docs/chat/authenticating/),
[moderation guide](https://dev.twitch.tv/docs/chat/moderation/)

### Rich media architecture

Extend the normalized model before adding visual effects. Preserve typed payloads,
source identifiers and unknown metadata. Keep text/copy/search ranges stable rather
than baking decoration into the message string. Notifications are typed events;
subscriber gifts and announcements must not be disguised as ordinary user text.

GIFs need a dedicated media path rather than silently raising every emote limit.
Set explicit per-item wire/decode/dimension/frame budgets, total cache limits,
bounded workers, cancellation and decode-error fallbacks. Check actual sample
assets before selecting those limits. Validate supported provider URLs while keeping
the supplied URL intact; do not log credentials or sensitive query material. No
arbitrary remote HTML, scripts or cookies in the native chat renderer.

Render visible content only, share decoding and animation clocks between duplicate
views, stop hidden animation work, and retain static/reduced-motion alternatives.
Reserve dimensions to avoid jumps. Deletion must remove content from rows, replies,
activity, dialogs, cached previews and in-flight completion callbacks. A missing GIF
must remain a readable placeholder, including when its source text is empty.

Prefer restrained native effects: a compact reward label, readable accent, bounded
one-shot motion and optional enlarged media. Preserve the user's narrow-window use
case. Exact Twitch visual imitation is secondary to accurate meaning, permissions,
readability and smooth performance.
## Delivery order and complete batch outcomes

The order can change for an observed regression or an owner priority. Changes must
be recorded here; adding a task must not silently remove an earlier item.

| Batch | Outcome | Dependencies and evidence |
| --- | --- | --- |
| B0 Current native acceptance | Named Windows checks passed; Settings defects fixed and owner state restored. | 0cfe8325 running; remaining edge cases stay explicit above and in VALIDATION.md. |
| B1 Current Twitch rich messages | Preserve typed rich fragments and notices; render GIFs, Cheers, reward highlights, introductory/system messages and power-up fallbacks. | TW01–08, TW16; current schemas; bounded media design. Verify actual received events where available; clearly mark unavailable cases. |
| B2 Channel and chatter context | Channel title/game/live state, room restrictions, richer usercards, chat history navigation and safe user actions. | Initial metadata snapshots plus live updates; explicit rate/cache budgets and role visibility. |
| B3 Personal emotes and 7TV depth | Account-aware picker entitlements, personal sets, cosmetics/paints/badges/avatars, event-driven provider updates and precedence. | Additional scope approval where required; current provider contracts; independent cosmetic rendering budget. |
| B4 Daily chat power tools | Input history, commands, timestamps, advanced highlight/filter/ignore rules, nicknames, notification preferences and configurable shortcuts. | Preserve single-submit ownership, per-channel drafts, bounded regex/filter work and privacy defaults. |
| B5 Moderator desk | Capability-gated delete/timeouts/bans, room controls, warnings, AutoMod, suspicious users, announcements and authorized pins. | Actual role and incremental scopes. Read-only display first; real moderation is separately authorized. |
| B6 Workspace completion | Popouts/multiple windows, cross-window docking, restore/import/export, per-view settings, remaining navigation conveniences. | Shared feed identities, independent view state, stable geometry/DPI and no credential copying. |
| B7 Integrations and distribution | Optional logs/privacy, external tools, browser integration, update/packaging strategy, plugin compatibility decision. | Explicit storage/external-process boundaries; no automatic installs or arbitrary script execution. |
| B8 Daily replacement acceptance | Side-by-side use by Joe, close remaining feature rows, resilience/accessibility/performance polish. | Owner acceptance, actual delivered send evidence, long-session observations; keep rollback. |
| B9 Additional platforms | YouTube/Kick plus optional combined feeds; TikTok later. | See MULTIPLATFORM.md. Verify each platform's current official receive/send/moderation limits independently. |

QoL belongs in every batch: shortcuts, sensible focus, clear copying/saving/sending
feedback, hover affordances, compact placement, cancellation and motion. A batch is
not “done” merely because a button exists or the Rust compiler succeeds.

## Native evidence without process overhead

Build the exact tree before publication/runtime handoff. Use the real application
and existing control interface for manual verification; no fake chat, test-only UI
or separate harness expansion. The evidence record should be short and practical:
commit, build result, what was actually driven, observed problems, exact untested
cases, and whether the owner's latest state was restored.

For new media: observe genuine messages if available, including loading failure,
static/reduced-motion presentation, narrow layouts, selection/copy and deletion
when an authorized real event is available. Do not purchase GIF eligibility, Bits,
subscriptions or rewards merely to get a sample. If a sample is unavailable, mark
that rendering branch unverified and ask only when it actually blocks a decision.

For actions: show account/channel/permission context and never infer success from
an HTTP status alone. Preserve drafts on failure or uncertainty. Read-only research
and app driving do not authorize sending chat, making purchases or moderating people.

For performance: measure actual frame responsiveness, process memory, cache use,
long sessions and multi-channel bursts when encountered. Record hardware/build and
conditions with numbers. Avoid invented FPS or “optimized” claims based only on code.
Prioritize fixes to visible lag, scroll anchors, UI-thread decoding or unbounded work.

## Keeping coverage current

1. Every implementation batch names the row IDs it advances and updates their state.
2. Every new feature found in upstream code, settings or current provider docs gets a
   row with a source and owner decision, rather than being silently omitted.
3. Split broad rows when a subset ships. “Emotes work” must not imply personal emotes,
   zero-width stacks, cosmetics, GIF messages and entitlement-aware sending all work.
4. Put permissions and receive/send limitations beside the feature, not in a hidden
   footnote. Revalidate changing APIs at implementation time.
5. A superseded or deliberately excluded feature remains visible with rationale.
   Legacy PubSub is replaced by supported transports; private endpoint scraping is
   not a planned workaround. Plugin compatibility may need a deliberate replacement
   design rather than blindly executing old scripts.
6. Keep README, PLAN, this register and VALIDATION consistent. Consumer README should
   not claim obsolete pane/window limits or label verified live traffic as untested.
7. No finite audit guarantees future Twitch/upstream changes are already covered.
   This is the comprehensive audited baseline and ongoing register, not a claim that
   all source branches, platforms or current implementation have been accepted.

## Source index

- [Audited Chatterino7 source](https://github.com/joenilan/chatterino7/tree/b3bf78c9d3894d9eb56d87a311ae2619c40aa904/src)
- [Inspected SevenTV upstream](https://github.com/SevenTV/chatterino7/tree/919a6c1fd2b4750e8fc718c164ed3d5d4d688dde)
- [Jawjack source baseline](https://github.com/joenilan/chatterino7/tree/c12a498d82e48fc79fbb446b0d46d7ec2639200e/recreation)
- [Architecture and decisions](PLAN.md)
- [Native evidence and limits](VALIDATION.md)
- [Supported app control](APP_CONTROL.md)
- [Twitch account contract](TWITCH_ACCOUNT.md)
- [Additional platforms](MULTIPLATFORM.md)
- [GPUI Kit](https://gpui-kit.com/) — implementation UI toolkit; existing dependency remains pinned, not upgraded by this plan.
- [Twitch chat docs](https://dev.twitch.tv/docs/chat/)
- [Twitch WebSocket lifecycle](https://dev.twitch.tv/docs/eventsub/handling-websocket-events/)
- [Twitch API changelog](https://dev.twitch.tv/docs/change-log/)

Per-file software and asset licenses still apply. Preserve required notices for any
copied/adapted implementation; do not assume provider-hosted images or old bundled
sounds can be redistributed with the application.

### B3 live catalog implementation checkpoint

Source now includes anonymous 7TV global/channel-set and owner-update subscriptions
with ACK/reconnect reconciliation, coalesced REST refresh and polling fallback.
Windows acceptance and real remote set changes remain pending. Personal emotes,
entitlement badges and paints are still separate unfinished features.
