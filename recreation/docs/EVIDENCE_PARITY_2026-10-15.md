# Jawjack — evidence-based parity audit and origin routing fix
**Date:** 2026-10-15
**Branch:** `recreation/gpui-foundation`
**Starting checkpoint:** `93c72eba1` (preview.16 already published at `dfed4f403`).
**Status of this work:** local source/test batch only; no installation, push, new preview or live provider actions.

## Evidence standard

- **Implemented:** source exists, compiles and/or has deterministic tests; this is **not** real Twitch or native UX acceptance.
- **Broken (source-proven):** a concrete reachable code path behaves wrongly; fixed only when the code path and tests are updated.
- **Partially implemented:** important behavior exists but a defined portion is missing.
- **Awaiting live/native validation:** code exists but requires genuine provider events or visual/interactions on a representative build.
- **Genuinely missing:** verified absence of a feature/contract after source comparison.
- **Intentionally different / unsupported:** a product or official API boundary, not a missing implementation.

## Shared Chat delete correction — verified 2026-10-15

Official Twitch EventSub has both `message_id` for the channel receiving
a Shared Chat message and `source_message_id` for its originating room.
These can differ. A normal delete of the copy visible in a receiving room
must pair the receiving broadcaster with that copy's `message_id`, never
the source broadcaster ID with the receiving ID. See
[official Shared Chat example](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channelchatmessage)
and [Delete Chat Messages](https://dev.twitch.tv/docs/api/reference/#delete-chat-messages).

**Locally corrected:** `message_actions.rs` now sets `room_id: None`
for clicked-message delete; `combined.rs::moderation_destination`
fails closed for any cross-room `room_id` override. The dispatcher no longer
converts Shared Chat source broadcaster IDs into implicit deletion targets.
The old transient lookup in `catalog.rs` was removed. This does not attempt
to delete the original source-room message; that would need an explicit
separate operation using `source_message_id` and verified permissions.

**Validation:** Windows Rust 1.99 locked desktop suite: **115 passed,
0 failed, 5 ignored**, including receiving-room and wrong-origin regression
assertions. `cargo fmt --all` completed. No real moderation was performed.
Native signed-in visual/permission acceptance is still pending.

## P0 — combined-feed Twitch moderation (source-proven bug, LOCAL FIX)

**Previous defect, now corrected in local source:** `message_actions.rs` created clicked-message `PaneEvent::TwitchCommand` without the message source. `workspace.rs`'s generic command handler used `destination(pane,None,channel)`, which for a combined pane resolves `send_to` (e.g. alpha), even if a clicked message originated in beta. Consequently delete/timeout/ban/custom moderator buttons could check/send to alpha, where the user could hold *different* privileges. The ban confirmation also displayed the combined label, not the actual target.

**Local corrections:**

1. `main.rs` adds `TwitchMessageCommand { source, request }` so message actions are never dependent on the composer destination; typed `TwitchCommand`, send/reply destination and ordinary room controls remain unchanged.
2. `combined.rs::moderation_origin` rejects missing, ambiguous, out-of-combined-set and YouTube source identities. `moderation_destination` is used in the dispatcher and rejects mismatched Shared Chat cross-room delete overrides.
3. `message_actions.rs` resolves the *clicked message's* source for menu items, ban dialog, delete, timeout, ban and custom moderator button expansion. Ambiguous custom commands from a combined/special pane fail closed instead of pretending to execute in another room.
4. `workspace.rs` computes source-specific account moderator logins, propagates them into each pane, and routes `TwitchMessageCommand` through the tested destination helper; `run_twitch_command` then rechecks role/scopes against the resolved broadcaster.
5. Shared Chat copied-message deletion uses the receiving room's `message_id` with its receiving broadcaster; cross-room `room_id` overrides fail closed instead of pairing unrelated IDs. YouTube actions remain routed through `PaneEvent::YouTubeModerate`.
6. Combined/Mentions moderator buttons and message menus are visible only for source channels where the account is known to moderate; deleting the broadcaster's own messages is excluded from quick actions.

**Deterministic regressions:** `combined::tests::clicked_message_moderation_never_uses_send_destination`; `combined::tests::moderation_commands_target_clicked_channel_with_send_to_elsewhere`. Covered: regular room; alpha-send-to versus beta-message in combined; Mentions origin; DELETE, TIMEOUT, BAN, WARN request routing; missing/mismatched/unknown origin; YouTube separation; Shared Chat receiving-room ID and cross-room override refusal. The destination helper under test is called by the actual workspace event handler. No live moderation was attempted.

**External API check:** Twitch [moderation guide](https://dev.twitch.tv/docs/chat/moderation) explicitly requires the actual owning `broadcaster_id`, the signed-in `moderator_id` and `moderator:manage:chat_messages` to delete a message. Helix `run` already enforces this transport contract when given the correct source.

**Chatterino7 source comparison:** Original checkout `src/providers/twitch/TwitchChannel.cpp:2144–2152` and upstream SevenTV/chatterino7 at `cacc388` call `deleteChatMessages(this->roomId(), moderator->getUserId(), messageID)`, i.e. an explicit channel object owns the action. Original and upstream `src/controllers/commands/builtin/twitch/Ban.cpp:125–135` expose an explicit `--channel` argument via `parseChannelAction`; `src/providers/twitch/eventsub/MessageBuilder.cpp:291–315` reports Shared Chat source broadcaster on moderation notices. This corroborates source-aware routing but does **not** establish that Jawjack now achieves complete Chatterino7 UI parity.

**Remaining behavior not yet accepted:** natural authenticated action/permission variations; original source-room deletion (intentionally not implemented); user-card actions in combined/mentions panes remain conservative/partially available; source-specific non-moderation menus and room settings need separate review. Never exercise a live moderation action without owner's explicit approval.

## Evidence matrix — do not rebuild existing components

| Area | Classification on this checkpoint | Code and comparison | Needed acceptance / actual gap |
| --- | --- | --- | --- |
| Combined feeds (Twitch + YouTube), sending and message origin | **Implemented; specific moderation routing was broken and is locally fixed** | `combined.rs`, `workspace.rs::deliver_chat/destination`, `message_actions.rs`. Original `TwitchChannel.cpp` and upstream `Ban.cpp` above. | Source-specific user cards, room controls, Shared Chat/multi-source native acceptance; never call a pane's send destination the action's source. |
| Twitch account roles, per-source moderator actions | **Implemented/partial; locally hardened** | `capabilities.rs::check/role`, `workspace.rs::run_twitch_command`, `message_actions.rs`, `mod_buttons.rs`. | Role/scope transitions and alternate moderator account require authenticated read-only/native review. No live actions. |
| Preview.16 wrapping, metadata and deleted row | **Implemented and unit-tested; native acceptance outstanding** | `inline_chat.rs` visual line planner and source-byte hit testing, `main.rs` metadata overlay, `deleted_row_tests`. Original `Split.cpp` row behavior. | At 420–440px and normal widths: long raid lines, badges/emotes, deleted body-only fade/strike, exact copied bytes, link click/hover and selection. Historical 1020px signed-out screenshot is **not** this acceptance. |
| Emotes, images and rich attachments | **Implemented; some media/DPI interactions unverified** | `inline_chat.rs`, `rich_messages.rs`, `media.rs`, `twitch_assets.rs` and 7TV/FFZ/BTTV modules. | Real provider media, GIF, replies, zoom/stacking, high-DPI, motion preference, selection/copy; don't claim fully missing from old queue. |
| EventSub normal chat, typed notices, deletes, settings | **Implemented; optional kinds and typed fallback need live edge checks** | `live.rs::CHANNEL_KINDS` has 6 types including `channel.chat.notification` / `channel.chat_settings.update`; `live.rs::chat_message` retains `notice_type`; `rich_messages.rs::heading` interprets it. | Unknown/new typed-notice payload and elevated/hype chat visual acceptance remain partial/research. Existing subscribers should not be replaced. |
| Own-channel EventSub extras and channel point events | **Implemented and source-tested; live delivery unverified** | `live.rs::OWN_KINDS` has 18 extras, `activity_feed.rs`, `studio.rs`, `LIVE_CONNECTIONS.md`. Unit tests count custom/automatic, unknown and malformed notifications. | Natural custom/automatic redemption and 3-socket reconnect/revocation while signed in; no simulated claim of provider acceptance. |
| Moderator AutoMod queue | **Implemented; real holding/approval still unverified** | `automod.rs` parses `automod.message.hold/update`, `automod_actions.rs`, `live.rs::MOD_KINDS`. | Real moderator review and role transition require authorized conditions. |
| Sender's own held-message approval/rejection lifecycle | **Genuinely missing follow-up status** (distinct from moderator AutoMod) | Official [Twitch EventSub subscription types](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types) lists `channel.chat.user_message_hold` and `channel.chat.user_message_update` (scope `user:read:chat`); neither is in `CHANNEL_KINDS` or `OWN_KINDS`. | Research exact per-channel subscription cost, sender identity/conditions and user-visible status before adding, without silently broadening OAuth scopes. Avoid showing held message as approved solely because it disappeared. |
| Moderator pins versus Pinned Cheers, Kick, Lua, generic IRC | **Research / intentionally deferred**, not proof of absence of related UI | Historical parity list; current [Chatterino7 v2.5.6 release](https://github.com/SevenTV/chatterino7/releases/tag/v2.5.6) and upstream source must be checked per feature. | Separate official API, product and licensing decisions. Existing pinned/Streamlink/YouTube integrations should not be conflated with absent APIs. |

## Validation (2026-10-15)

- Windows Rust 1.99 command: `rustup run 1.99.0 cargo test -p chat-workbench --bin chat-workbench --locked`.
- After message origin fix, **120 total: 115 passed, 0 failed, 5 ignored**, including two added routing tests. Existing inline text geometry, deleted stroke, hit-testing, EventSub, reply and YouTube tests also passed.
- `cargo fmt --all` applied only new formatting; run `cargo fmt --all -- --check` after any remaining edits.
- Locked Rust 1.99 optimized release build **passed** after Shared Chat safety correction; formatting and `git diff --check` also passed (the existing `chat_text.rs` dead-code warning remains). Native visual acceptance is still outstanding; no local binary was installed or published.
- No real Twitch API mutations, messages, raids, advertisements or reward actions; no changed scopes, accounts, profiles, installed application or credential vault.

## Follow-up priorities (evidence-based)

1. Complete **native visual/interaction QA** on preview.16 at normal and 420–440px pane widths (rich media, selection, clicks, deleted rows); source tests are not substitutes for acceptance.
2. Verify **combined-feed source and Shared Chat permission behavior** using non-mutating read-only signed-in state and synthetic routing tests, never live moderation without approval. Audit combined user card, follow age and room controls separately before enabling them.
3. Scope sender `channel.chat.user_message_update` as a genuine independent AutoMod status gap, retaining moderator queue functionality and existing OAuth boundaries.
4. Accept actual incoming typed notices, EventSub rewards and provider media during ordinary use, logging only redacted outcome/diagnostic metadata; prioritize real regressions over an outdated checkbox count.

**References:** local `src/providers/twitch/TwitchChannel.cpp`, `src/controllers/commands/builtin/twitch/Ban.cpp`, `src/providers/twitch/eventsub/MessageBuilder.cpp`; [SevenTV/chatterino7](https://github.com/SevenTV/chatterino7/tree/cacc388); [official Twitch moderation](https://dev.twitch.tv/docs/chat/moderation); [official Twitch EventSub types](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types).
