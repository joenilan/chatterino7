# Jawjack: Viewer, Moderator and Streamer experiences

Status: product and architecture plan, 2026-10-10. These experiences are planned,
not implemented or accepted. This extends the full Chatterino7 replacement and
[MULTIPLATFORM.md](MULTIPLATFORM.md); it does not replace or reduce any parity work
in [FEATURE_COVERAGE.md](FEATURE_COVERAGE.md).

## 1. Product direction

One fast chat application, three useful starting arrangements:

- **Viewer:** watch channels, talk, follow conversations and enjoy rich emotes.
- **Moderator:** keep chat readable and act on the right message/person quickly.
- **Streamer:** keep the broadcaster's own chat anchored, manage the broadcast and
  notice important audience events without abandoning chat.

A person can stream on their own channel, moderate another, and watch a third in
one window. A setup choice must never trap them in a separate edition of Jawjack.
Full viewer functionality remains available in every experience.

The selected experience configures presentation and defaults. Platform authority
comes from the authenticated account, validated scopes and actual channel role.
Choosing Moderator must never manufacture moderator privileges. Choosing Streamer
must never make someone else's channel editable.

## 2. Non-negotiable continuity

- Continue every Chatterino7 parity item. Keep its existing coverage ID; record
  expanded behavior beside that item instead of removing it from the backlog.
- Keep dark Studio styling, GPUI Kit, the custom titlebar, restrained pastel state
  markers, polished motion and reduced-motion support.
- Workspace navigation stays in the collapsible sidebar. Do not reintroduce a
  second workspace tab bar or permanent dashboard above chat.
- Channel tabs remain the main navigation; + adds channels, right-click provides
  context actions, middle-click close retains confirmation for ordinary tabs.
- Keep unrestricted recursive splits subject to screen space, drag-to-combine,
  one composer per visible group, and channel/account-specific saved drafts.
- Preserve terminal-style selection and right-click copy. Moderation must not
  hijack right-clicking a selected transcript.
- Support the existing compact window target without forcing the startup size.
- New controls need complete hover, focus, loading, failure and confirmation states,
  not only a working click handler.

## 3. Onboarding: useful in under a minute

### First launch

1. **Choose how you use chat most:** Viewer, Moderator or Streamer. Each card has
   a short description and a small accurate layout illustration. “Change anytime”
   stays visible. Skip selects the minimal Viewer presentation.
2. **Connect the platform you need now.** Start with supported Twitch sign-in.
   Explain secure credential storage and requested permissions in ordinary words.
   Do not display future platforms as functioning login buttons.
3. **Choose channels.** Viewer can paste channel URLs. Moderator can add channels
   and verify privileges after connection. Streamer confirms the account's own
   channel as the primary broadcast chat. A display name is not proof of ownership.
4. **Preview the arrangement and enter chat.** Optional text-size and reduced-motion
   choices; defer advanced settings. No long feature tour before the app is usable.

Onboarding never requests every eventual moderator/broadcaster scope. Enable an
optional feature when the owner wants it and explain its actual added permission.
A declined optional grant leaves ordinary chat usable.

### Existing installations

Do not interrupt launch with a forced wizard or rewrite the current workspace.
Offer “Choose your setup” once from the titlebar/account menu, then leave it in
Settings. Switching experience offers a preview and preserves existing channels,
drafts, pane sizes, filters and credentials. Presets should configure defaults,
not overwrite carefully customized layouts on every launch.

### Interrupted setup

Save non-secret progress locally. Closing the wizard or losing the network must
not leave a half-created duplicate account or discard channels already added.
Separate “account signed in,” “chat connected,” “broadcast live,” and “tools
permitted” in both state and wording. Secure-storage failure is actionable text,
not an endlessly spinning sign-in screen or a plaintext credential fallback.

## 4. Shared layout: chat gets the space

Use the same shell for all three experiences:

- **Titlebar:** hamburger/workspaces, Jawjack, compact connection/activity state,
  account and settings, native-style window controls.
- **Channel tab row:** avatar/platform/live state, channel label, unread signal,
  plus button; no duplicate navigation tier.
- **Focused channel header:** concise channel status and context-relevant tools.
  An experience badge or shield opens the tools for that channel.
- **Main area:** chat, with optional user-requested splits or utility drawers.
- **Bottom:** the current group's composer and clear send destination.

Wide windows may show a few labeled actions. Narrow windows collapse them into
well-labeled icon buttons/menus; the underlying function must remain reachable by
keyboard. Do not solve density by shrinking text until it becomes unreadable.

A tools drawer opens beside chat when space permits, and overlays temporarily in
compact windows. Remember the user's open/closed choice per workspace and tool,
but never reopen a disruptive overlay merely because the window became smaller.
Closing returns focus to the opener. Reversing a slide animation must work while
it is still moving; resize/drag operations must not leave invisible hit targets.

## 5. Viewer experience

Default: clean channel tabs, chat and composer. Sidebar closed unless preferred.
Priority features remain emotes, user cards, replies, mentions, search, unread
navigation, follow/visit convenience, quick switching and reliable copy/paste.

Streamer/moderator controls do not occupy permanent space here. A user with actual
moderator privileges can switch tools on in that channel's header or context menu.
Viewing someone else's livestream remains a first-class use case on every future
provider where the official API supports it.

## 6. Streamer experience: the own-chat anchor

### Recommended pin rule

The first-tab protection is approved. The per-workspace interpretation below is a
planning recommendation to validate before implementation.

The selected primary broadcast channel is always the **first tab in the primary
chat group of the active workspace**, visibly pinned. It cannot be reordered,
dragged away, closed with middle-click, hidden by “show only live,” or displaced
by newly added channels while Streamer experience is enabled.

This is an own-chat anchor, not a second tab strip. Other tabs and split groups
remain freely rearrangeable. Additional views of the same feed may be opened in
other panes/windows later; they do not replace or unlock the anchor.

To make the invariant predictable:

- Each workspace identifies one primary group. Creating/switching workspaces
  ensures the anchor exists there before rendering or restoring focus.
- Splitting a normal tab never moves the anchor. Dragging a group containing it
  must preserve the anchored view in the primary group and clearly preview this.
- Closing/restructuring the primary group promotes a surviving group and restores
  the anchor as its first tab. Never silently lose its draft or reply target.
- Closing the last workspace creates a minimal workspace with the anchor.
- The anchor is visible when offline, disconnected, filtered out or signed out;
  its status explains which of those applies. It must not pretend the stream is live.
- Context menu says why Close/Move is unavailable, and links to experience settings.
  No repeated confirmation dialog for an action that is deliberately forbidden.
- Switching to Viewer explicitly ends this protection; it does not delete the
  channel. Remember ordinary tab state so changing experience is reversible.
- Account changes rebind only after verifying the new account's stable channel ID.
  Preserve the previous account's drafts separately; never send them as the new user.

Use one canonical own-channel binding, with a separate anchored ViewId in each
workspace under this proposed all-workspace behavior; never move one physical view between
workspaces or create duplicate network subscriptions. An alternative worth reviewing
is one protected Streamer/Home workspace with freely arranged other workspaces.
The migration must not silently duplicate an owner's existing layout before this
cross-workspace behavior is chosen.

Multiple broadcast accounts/providers: choose a primary anchor explicitly. Other
owned feeds can appear after it as secondary pinned tabs or in a Broadcast workspace.
Do not silently assume the most recently authorized account is the primary stream.

### Streamer tools, within reach

A compact **Studio** action in the own-channel header opens a broadcast drawer:

1. **Stream details:** current title, category/game, tags/language where supported;
   Edit opens a small focused form with explicit Save and Cancel.
2. **Chat controls:** supported room modes with actual current values and role-aware
   actions; show which channel/account will be affected.
3. **Audience activity:** meaningful subscriptions, gifts, Cheers, rewards and other
   supported events, using verified provider events and preserving chat attribution.
4. **Broadcast actions:** raid flow, polls and predictions when authorized and
   eligible. These are separate deliberate actions, not accidental menu shortcuts.
5. **Connection health:** chat/event connection, reconnect state and stale data.
   Do not call API connectivity “stream health” or claim encoder metrics without a
   real supported data source.

Optional compact status items: live/offline, elapsed stream time, viewers, category.
Keep viewer count hideable for streamers who prefer not to see it. Never display
zero when the real state is unknown, delayed or inaccessible.

### Editing titles and categories correctly

Fetch current metadata before opening the form. Category selection resolves a
provider category ID; free text alone must not become an invalid game ID. Keep the
original snapshot and an independent edit buffer. Detect external updates while
editing and offer review instead of silently overwriting changes made elsewhere.

Save is explicit; no write on every keystroke. Disable duplicate submission,
retain edits on failure, and read back uncertain outcomes before offering a retry.
Do not infer API success from a dismissed dialog. If a provider lacks an action,
show a clear supported link to its dashboard instead of a fake control.

## 7. Moderator experience

The channel header exposes a clear **Moderation / Viewing** toggle. It controls
that channel's tool presentation, persists per account/channel, and does not change
server privileges. Global setup selects the default for verified moderated channels.

### Immediate tools

- Contextual actions on a selected message/user: reply, copy, inspect user, delete,
  timeout presets, ban/unban, and relevant provider actions.
- A compact moderation strip or drawer with held messages/AutoMod, room modes,
  recent actions and queue counts. Use a single status icon in tiny windows.
- User history combines only accessible retained/provider history and labels gaps;
  do not imply a complete historical record from the current session's messages.
- Commands and hotkeys invoke the same validated action pipeline as buttons.
  Existing copy/selection and text-editor shortcuts take precedence in their contexts.

Hover action affordances should be subtle, with keyboard equivalents. Avoid layout
jumps when they appear. A fast-moving message must not move a destructive target
under the pointer: bind actions to the original stable message/user ID, keep the
selected target card steady and show channel/name/reason before consequential work.

### Safe speed

Fast moderation should remove unnecessary navigation without making actions vague:

- Delete and timeout use an explicit target and a configured quick-action policy.
- Ban, clear-chat, raid, ending a prediction and other high-impact actions receive
  a review step appropriate to their consequence; no accidental action on hover.
- Bulk actions are a later deliberate workflow with a visible target count/scope,
  not a byproduct of transcript text selection.
- Live permission loss disables actions immediately and invalidates queued work.
- If another moderator already acted, reconcile to current state instead of replaying
  the same action or treating it as an unexplained error.
- Report pending/succeeded/failed/uncertain separately. A toast cannot substitute
  for provider acknowledgement. Retry rules must be action-specific.

“Undo” appears only where a real inverse is supported and meaningful. Deleting a
message is not universally reversible; unbanning someone does not restore history.

### Mixed-role usage

A moderator watching an unrelated channel sees Viewer tools there. A broadcaster
moderating another channel gets only that channel's verified moderator capability.
The account identity and target channel remain visible in any action confirmation.
Never reuse the primary streamer's token authority for a different connected account.

## 8. Capabilities and permission model

Proposed model (not current implementation):

- `ExperiencePreference`: Viewer / Moderator / Streamer.
- `ToolPresentation`: Viewing / Moderating, scoped to account and channel.
- `FeedKey`: provider + stable channel/live-chat identity.
- `AccountKey`: provider + stable authenticated account identity.
- `ViewId`: unique pane/view instance, separate from the feed it watches.
- `Capabilities`: independent read, send, delete, timeout, ban, manage-room,
  manage-stream-info and provider-specific feature states.
- `CapabilityState`: available, authorization-required, role-required, unsupported,
  unknown/checking, rate-limited or temporarily unavailable, with a useful reason.

Gate actions both where they are displayed and immediately before dispatch. UI
state can become stale. Scopes alone do not establish channel authority; role alone
does not establish a usable token grant. Moderation/broadcaster endpoints must use
the intended account, not whichever login was last used by the application.

New grants are incremental and user-initiated. Do not silently broaden existing
login permissions because a preset was selected. Login retries, scope upgrades,
account disconnect and provider feed reconnect are distinct operations with distinct
labels. Tokens remain in the OS vault and never enter workspace/preferences files.

## 9. Multiservice integration

Keep the provider feasibility and delivery order in MULTIPLATFORM.md. Twitch is the
first complete experience; YouTube/Kick follow supported official contracts, and
TikTok remains lower priority and conditional on legitimate API availability.

A role is per connected account/channel. A viewer can add someone else's YouTube
or Kick feed where supported; own-channel control is not a prerequisite for reading.

For a combined feed:

- Preserve platform, channel, sender and original message ID on every row.
- One explicit send destination; a reply locks to its source. Cross-posting would
  be a separate opt-in workflow with a visible destination list and per-destination
  results, never the default composer behavior.
- Preserve original and receiving channel IDs for Twitch Shared Chat. Its provider-
  specific moderation semantics must be evaluated; source attribution alone does not
  confer authority over that source.
- Moderation actions apply only to the selected message's valid provider/channel target.
  No cross-platform identity assumption based on matching usernames.
- Queues can be viewed together, but each item retains its provider-specific action
  vocabulary, account requirement and outcome.
- Broadcast metadata edits default to one destination. Future multi-platform edits
  preview each platform's supported fields and report partial success honestly.
- A hosted webhook relay, if needed by a provider, is separate from local UI state.
  Its access and account isolation require explicit design; do not embed app secrets
  in a native binary or add a server dependency to ordinary Twitch viewing.

## 10. Persistence and migrations

Version the new schema. Persist experience defaults, per-channel presentation,
primary broadcaster binding, primary-group identity, drawer preferences and role
layout choices. Keep authorization evidence transient or refreshable; do not restore
permission merely because an old preferences file says “moderator.”

Current name-keyed dock entries and workspace/channel draft keys need stable-ID
migration before multi-account/multi-provider correctness can be claimed. Rename,
account switching and duplicate views must not merge unrelated drafts or move a
reply target to another account. Preserve existing files and recovery backups; if
migration is incomplete, use a readable recovery path rather than resetting layout.

Shared feeds may share receive/network work. Each view still owns scroll position,
selection and focused composer interaction. Define draft sharing explicitly: default
independent drafts per account/feed/view. Moving preserves the same ViewId and its
account-scoped draft/reply state; account/feed-only recovery must never select another
view's draft. Moving is not copying, and a mirror must not silently consume its draft.

## 11. Performance, accessibility and polish

- Retain bounded transcripts, virtualized rows/media and coalesced event updates.
  Offscreen moderation queues and inactive drawers should not animate continuously.
- Prioritize sending/receiving and user input above enrichment/API refresh work.
  Respect provider rate-limit/reset information and backoff independently per service.
- Keep original item IDs under animations, filters and virtualized list recycling.
- Readable scalable type, keyboard operation, visible focus and text labels/tooltips
  accompany icons. Do not communicate role, risk or network status through color alone.
- Reduced motion affects drawers, badges and incoming-message animation consistently.
- Screen-reader labels name target and action, including platform and channel in
  merged views. Icon-only compact UI must remain understandable.
- Layout transitions preserve focus, selection and drafts; no forced scrolling to the
  bottom when toggling tools, changing role or updating stream information.

## 12. Delivery sequence and source ownership

This roadmap is additive. Continue parity work while landing coherent experience
batches; do not block all chat development on every future provider or broadcaster API.

1. **State foundation and reversible setup.** Stable account/feed/view IDs, validated
   optional capability propagation, experience preference, non-destructive migration.
   Owners: auth.rs, live.rs, workspace.rs, storage.rs, dock.rs; introduce focused
   capability/preferences modules instead of enlarging workspace.rs indefinitely.
2. **Streamer anchor and Viewer/Moderator presentation.** Pin invariants, role toggle,
   compact headers/drawer, context menu explanations, restoration and account-change
   behavior. Actual write tools remain gated until their provider implementation exists.
3. **Complete first broadcaster utility.** Title/category editor with fetched state,
   permission explanation, explicit save, conflict/failure handling and readback.
   Owners: new broadcaster metadata service, existing stream_status.rs and profiles.rs.
4. **First complete moderation utility.** Stable user/message target, delete/timeout/
   ban where supported, capability checks, action-result feedback and room-mode controls.
   Owners: new moderation action service, message_actions.rs, live.rs, room_settings.rs.
5. **Moderation queues and broadcaster events.** AutoMod/held-message workflows,
   action history, supported subscription/reward events, bounded event subscriptions.
6. **Expanded broadcast tools.** Raids, polls, predictions and rewards after official
   eligibility/permission contracts and deliberate UX are implemented.
7. **Connections and second provider.** Generalized account UI and official adapter,
   followed by merged feeds and multi-destination broadcast utilities where permitted.

UI arrangement, animations, error states and accessibility ship with each utility.
No separate fake-data demo app, permanent testing toolbar or giant harness is required.

## 13. Manual acceptance in the real app

Build the exact changed tree, then drive meaningful scenarios directly:

- First launch/skip/revisit onboarding; repeated or interrupted login.
- Change experience without losing channels, draft, font, filters or pane sizes.
- Streamer pin under offline-only filtering, tab reorder, middle-click, split/merge,
  workspace close/reopen, app restart, account logout/switch and duplicate views.
- Viewer/moderator toggle in mixed-role channels; permission revoked while a tool is open.
- Compact and wide layouts, text scaling, keyboard focus, motion interruptions.
- Stream form cancel, concurrent external edit, failure and uncertain readback.
- Targeted moderation during rapidly arriving chat, provider rejection and duplicate action.
- Cross-provider send/reply/moderation attribution once multiple adapters actually exist.

Read-only UI checks do not authorize real moderation, raids or broadcast changes.
Use the owner's specifically authorized targets for live write verification. Capture
actual-app screenshots at meaningful UI checkpoints for remote/mobile review; label
fresh/signed-out/empty data states and never pass a mockup off as running behavior.

## 14. Decisions already made and open research

Approved direction: three setup choices; own chat first and protected in Streamer
mode; readily accessible streamer/moderator tools; Viewer/Moderator switchability;
continued full parity and multi-service plans; compact chat-first design.

Planning recommendation: anchor each workspace's primary group and choose one
primary broadcast account. This resolves the otherwise ambiguous meaning of “first”
with multiple split groups. Validate that interaction before locking the storage model.

Open research does not block the initial role presentation: provider eligibility,
moderator-versus-broadcaster endpoint differences, rate limits, optional grant UX,
which live events third-party clients can actually receive, and future provider review
requirements. Record unsupported capabilities explicitly instead of silently removing
features from parity tracking.

## 15. Current source prerequisites, before role implementation

Inspected source on the current development branch:

- `dock.rs` uses channel names as Leaf/Deck keys; existing remove/insert/tabify/
  reorder operations have no pin policy. Centralize policy across every mutation,
  restoration and future multi-window route, not only the visible tab widget.
- `workspace.rs` currently forbids duplicate channel names within a workspace and
  uses workspace/channel draft keys. Add stable group/view IDs and account/provider
  context before own-chat mirrors or multiple account binding.
- Pane send callbacks capture their channel. Do not rename/rebind a pane in place
  while leaving an old captured destination active.
- `auth.rs` has one account/vault slot. It validates but does not propagate the
  optional granted scope list. Preserve validated capabilities without changing
  requested scopes until the user chooses the feature.
- `live.rs::Identity` combines user ID and token; configuration changes share an
  epoch. Distinguish stable identity, credential version, capability version and
  subscription revision so token refresh does not become a user/layout switch.
- Existing transport channel deduplication is useful. Preserve it as view identity
  becomes separate from feed identity.
- Existing pane minimum dimensions and overflow are real usability constraints.
  Add equalize/focused-pane zoom/recovery, not an artificial count cap or silent
  layout destruction on small screens.

These are migration prerequisites, not claims that every current single-account
interaction is broken. Historical two-pane language in old checkpoints must never
supersede the current unlimited-splits product decision.

## 16. Twitch capability contract snapshot

Research date: 2026-10-10. Verify endpoint details again when implementing. No
permissions were granted and no live broadcaster/moderation action was performed
for this plan.

| Capability | Required grant family | Important boundary |
| --- | --- | --- |
| Discover moderated channels | `user:read:moderated_channels` | Authenticated user's own moderator list |
| Edit title/category/tags | `channel:manage:broadcast` | Broadcaster token; moderator/editor website access is not equivalent |
| Timeout/ban/unban | `moderator:manage:banned_users` | Appropriate channel authority and token identity |
| Delete message / clear chat | `moderator:manage:chat_messages` | Distinct targets and consequences |
| Room settings | `moderator:manage:chat_settings` | Read actual state before mutation |
| AutoMod settings | `moderator:read:automod_settings` / `moderator:manage:automod_settings` | Observation and configuration differ |
| Held AutoMod queue | `moderator:manage:automod` | Receiving held-message events itself needs this management-level grant |
| Blocked terms | `moderator:read:blocked_terms` / `moderator:manage:blocked_terms` | Respect private-term visibility |
| Unban requests | `moderator:read:unban_requests` / `moderator:manage:unban_requests` | Decision workflow, not a generic chat action |
| Shield Mode | `moderator:read:shield_mode` / `moderator:manage:shield_mode` | Clear active-state and scope feedback |
| Warnings | `moderator:manage:warnings` | Explicit target and acknowledgement state |
| Announcements / shoutouts | `moderator:manage:announcements` / `moderator:manage:shoutouts` | Separate deliberate actions |
| Polls / predictions | `channel:read:polls`, `channel:read:predictions`; corresponding `channel:manage:*` | Broadcaster-owned API operations |
| Raids | `channel:manage:raids` | Pending raid is not completed viewer transfer |
| Custom rewards | `channel:read:redemptions` / `channel:manage:redemptions` | Mutation/fulfillment restricted to rewards created by the same app |
| Account emote inventory | `user:read:emotes` | Optional separate permission; globals do not prove subscriber entitlement |

Sources: [Twitch scope catalog](https://dev.twitch.tv/docs/authentication/scopes/),
[API reference](https://dev.twitch.tv/docs/api/reference/),
[moderation guide](https://dev.twitch.tv/docs/chat/moderation/).

Event contracts matter separately from buttons. AutoMod hold/update subscriptions
need the documented management scope; do not advertise them as a read-only grant.
Stream online/offline events require no broadcaster authorization, but still need
working event delivery and snapshot reconciliation. See
[EventSub subscription types](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/).

A poll begins when creation succeeds; prediction resolution allocates points; a raid
has a pending phase and cancellation window. Design their review screens and result
states around those consequences. References:
[polls](https://dev.twitch.tv/docs/api/polls/),
[predictions](https://dev.twitch.tv/docs/api/predictions/),
[raids](https://dev.twitch.tv/docs/api/raids/).

## 17. Cross-reference to the parity backlog

- WS01–WS04, WS08, WS12: workspaces, dock/view identities, pin policy, windows and recovery.
- WS05–WS06: broadcast status and stream-information read/edit utility.
- WS10–WS11: special queues, combined feeds and preserved source context.
- IN05 and later command work: shared capability-aware dispatch, no pretend slash commands.
- EM02/EM14: account emote inventory and the service/streamer browser remain active work.
- All MD moderation rows: keep existing scope while introducing role-aware prominence.
- Account/authentication and connection rows: incremental grants, role refresh and
  provider/account isolation must support every experience.

The experience plan organizes the same roadmap into daily workflows. No existing
parity row becomes complete merely because a preset, drawer or permission prompt
has been added.
