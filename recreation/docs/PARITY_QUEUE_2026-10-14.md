# Jawjack — next Chatterino parity work (2026-10-14)

**Historical queue.** Preview.16 has shipped the first-line chat layout and
deleted-message styling. For today's **updated** priorities and unresolved
native acceptance, read
[HANDOFF_2026-10-15_PREVIEW16.md](HANDOFF_2026-10-15_PREVIEW16.md)
before working from the older P0 EventSub instructions below.

**2026-10-15 priority update:** This remains the historical parity backlog, but the next execution handoff is now [HANDOFF_2026-10-15.md](HANDOFF_2026-10-15.md). The owner requested **full-width continuation-line chat wrapping** as the new P0. Preview.15 already shipped the EventSub observability *source* slice described below; its real provider-event validation remains open and is now P1. Do not repeat the completed source batch.

This is the **current prioritized continuation queue**, not a replacement for
[FEATURE_COVERAGE.md](FEATURE_COVERAGE.md) or the detailed
[CHATTERINO_PARITY.md](CHATTERINO_PARITY.md). The older tables contain historically
accurate snapshots and some stale status cells: check current Rust source, newer
feature docs, and named native evidence before declaring work missing or complete.

## Reviewed baseline and evidence

- Windows checkout: `E:\git\chatterino7`; branch `recreation/gpui-foundation`.
- Reviewed HEAD: `3ec84d69f` (`0.1.0-preview.14`), clean at the start
  of this documentation-only review. These are **review-time observations**, not
  permission to reset the tree on a later date.
- In recent commits, channel points gained automatic and custom redemption
  EventSub handling, own-channel priority and optional-subscription retry;
  tab-strip wheel and Ctrl+Tab navigation were added; compact sidebar, follower
  middle-click, and a mentions-only bell were implemented.
- Preview.14 previously passed 102 desktop unit tests (5 ignored), packaging and
  sandboxed installer tests; the public release was checked. A *real redemption*
  has still not been observed in the new implementation. A successful unit test
  is not proof Twitch delivered a real event.
- `VALIDATION.md` records a real successfully delivered owner-channel send
  (2026-10-11). Older claims that **no send was ever verified** are superseded.
- Current source in `live.rs` declares six per-channel subscriptions, reserves
  ten moderated-channel AutoMod queues per socket (two kinds each), and includes
  18 own-channel extras. Its `MAX_CHANNELS` formula is currently **135**,
  not the old 150-channel figure in some historical docs. Recheck after edits.
- Light/Dark/System theme, Streamer/Moderator/Viewer presentation, own-channel
  anchor, inline AutoMod lines, and workspace windows already have implementation
  slices; do not rebuild them from older `Part` statuses.

**Evidence labels:** *implemented* = source+build; *native checked* = observed
UI/connection on a named build; *live checked* = a genuine provider event or
authorized action; *research* = current official contract/permissions unresolved.
Every completed batch should record its level and the specific unverified cases.

## Priority queue

| Order | Register IDs | Next useful work | Done when / evidence |
| --- | --- | --- | --- |
| **P0 — start here** | AC04, AC08, AT04, TW06, TW16, WS10 | **Own-channel EventSub / channel-point observability and recovery.** Verify optional custom/automatic redemption subscriptions, clear refusal/retry reporting and real-event path into Studio. Distinguish a missing notification from the Helix queue and from a post-disconnect gap. | Subscriptions and refusal states are inspectable without token leakage; incoming event is parsed once and rendered; reconnect/revocation never drops healthy chat. Unit tests + real native inspection. Real redemption remains pending until naturally observed. |
| **P1** | MD09, AT04, AC08, TW08, TW16 | **Remaining AutoMod and notices.** Research `channel.chat.user_message_update` for the sender's held-message approval/rejection lifecycle; handle elevated/hype chat notices and unknown typed events with readable fallbacks. | Current official EventSub conditions/scopes verified first; a held message is not marked approved just because it vanished; typed notice is retained without dropping body or leaking private content. |
| **P2** | WS11, AC09, TW09, MD04–05 | **Combined-feed origin parity.** Per-origin room settings, chatter/context menus, moderation actions, account-role and send destination; preserve Shared Chat source identity. | A command or moderation action acts on the message's real broadcaster, not the combined display name; drafts, replies, deletes and copies remain correct across two+ sources. |
| **P3** | TW01–08, EM07–16, RD02 | **Rich rendering / media edge parity.** Native acceptance for Cheers, GIFs, highlighted rewards, subscriptions, badges, modifier overlays, painted names, static/reduced motion and selection/copy at compact sizes. | Actual sampled media where available and bounded fallback where not; do not infer full fidelity from a parser test. |
| **P4** | RD01–04, PF02, WS02/08 | **Daily-use robustness.** DPI, CJK/IME/bidi, transcript selection/copy, hover pause, compact split/popup layouts and long-session reconnect/memory. | Named native checks with actual screen sizes, stable drafts/focus, no uncontrolled media growth or false unread badges. |
| **Research / later** | TW10–11, WS09, PF07–09, PF12–14 | Moderator pins versus Pinned Cheers; image upload; Lua-plugin compatibility; browser extension; generic IRC; Kick and additional platforms. | Separate current API, security, privacy and licensing decision before writing implementation. Do not treat historical Chatterino support as permission to use private endpoints. |

The unchecked items in `CHATTERINO_PARITY.md` remain open even when a portion
has code. Checkboxes are **presence**, not exhaustive live-event acceptance.

## P0 — first fresh-chat delivery slice

**Goal:** make it possible to explain *why* a live channel-point redemption was
absent, without breaking a working stream, expanding permissions silently, or
pretending an app-owned queue contains every Twitch reward.

1. **Audit before editing.** Read `crates/desktop/src/live.rs` (kind lists,
   assignment/retry, dedup, socket reconnection, delivery), `activity_feed.rs`,
   `rewards.rs`, `studio.rs`, `workspace.rs`, `capabilities.rs`,
   `docs/LIVE_CONNECTIONS.md`, `docs/STUDIO.md`, and `docs/AUTOMOD.md`.
   Check the actual `OWN_KINDS` types and versions against current *official*
   Twitch EventSub docs rather than relying on a copied historical schema.
2. **Enumerate the four distinct outcomes.** (a) kind subscribed but Twitch has
   not sent an event; (b) refused/pending/retrying optional kind; (c) event
   delivered but ignored/duplicated/hidden by UI; (d) socket interrupted and
   some events during the gap unavailable. There is no guaranteed replay of a
   missed websocket notification. Never invent an event or make an availability
   claim from the custom-reward queue.
3. **Useful diagnostics, not a new tool panel.** Reuse existing bounded,
   session-only `inspect`/status reporting (such as
   `workspace.own_extras_refused`). If necessary, add concise per-kind status
   and last-received metadata, with no bearer tokens, raw user input, user IDs,
   persistent sensitive event logs or new always-on server. A connection saying
   "Connected" must not imply *optional* reward subscriptions succeeded.
4. **Parser and routing correctness.** Cover custom and automatic reward types,
   malformed/unknown event values, duplicate event message IDs, hidden own tabs,
   account switching, retained source-channel identity and safe UI fallback.
   A custom reward made elsewhere may be **read-only** to this app; `rewards.rs`
   fetches the `UNFULFILLED` queue for app-manageable rewards. The automatic
   redemptions are **not** entries in that custom queue.
5. **Real-world acceptance, only when available.** Start a separate
   `--profile dev` application with the existing native `--control-session`
   facility, preserving the installed Jawjack and its credentials. Inspect
   runtime subscription state and Studio without clicking live channel actions.
   If a genuine reward naturally arrives, note exact version, kind, whether
   Studio showed it, and whether a visible tab was open. Otherwise label the
   live case **unverified**. Do not ask the user to purchase/redeem rewards merely
   to manufacture a test.
6. **Regression checks.** Add focused tests for duplicate notification IDs,
   custom vs automatic mapping, optional-refusal/revocation behavior and
   reconnect status where feasible. Run locked Rust 1.99 test/build commands;
   inspect the native app before claiming UI acceptance. Keep the original
   user chat session and dirty files intact.

**Finish P0 before broadening scope.** Only then consider P1's narrowly scoped
held-message and elevated/hype-chat parity, after verifying current Twitch
subscription permissions. Never autonomously moderate, run ads/raids, post chat,
fulfil rewards, change OAuth scopes, or purchase Bits during parity testing.

## 2026-10-14 implementation update (unreleased)

P0 was implemented through an initial **source- and unit-test-verified**
slice, not closed as a fully proven live integration. The working tree now
provides:

- `workspace.own_eventsub.subscriptions` for all optional kinds: subscribed,
  pending, refused/retrying, or disconnected gap; safe HTTP code/retry delay.
- Custom and automatic reward notifications counted before vs after Activity
  parsing; unmapped broadcaster-channel notifications also counted (not
  rendered), without retaining message or viewer data. Duplicate IDs suppressed.
- Reward-specific Studio refusal warning, missing-subscription-ID rejection,
  and independent retry without restarting healthy chat.
- Account reset and WebSocket gap counters. WebSocket events missed during
  downtime are not recoverable from the custom-reward queue.
- A single Enter keydown send path and an in-flight guard for live slash
  actions; successful chat send no longer emits DraftChanged twice.
- After `/raid`, a local estimated countdown and an explicit Cancel
  action; the browser-only "Raid Now on Twitch" shortcut was removed
  because it did not execute immediate completion. No fictitious Helix
  instant-raid endpoint is claimed.

Validation: 107 unit tests passed, 5 ignored; locked Rust 1.99 release
build passed. After initial ephemeral-control failures, a **single owned
QA launcher** kept the native `--profile` window alive through successful
session inspection. A separate signed-out QA profile reproduced first-Enter
inline completion (7 → 8), and the corrected build verified first Enter
executes local `/uptime` (7 → 0), repeated Enter stays empty, Tab and
arrow-selected completion still work. Additional source audit corrected
repeated sends: the original text is retained for success acknowledgements
even when Twitch requires an invisible suffix on the wire; the regression
test passed. Installed Jawjack remained running untouched. No actual live
redemption, real raid, real Twitch send, or live optional-subscription
acceptance is claimed; countdown UI appearance remains unverified.
Continue with signed-in authorized-native and reconnect checks
before marking AC04/TW06 complete or publishing. Queued raid does not
confirm raid delivery.

## Project invariants and QA

- Jawjack uses native Rust + GPUI Kit, not a re-skinned webview; reuse current
  source and scripts. Keep the dense channel-first UI, custom title bar, compact
  screen support, distinct split/tab drafts and source-appropriate moderation.
- Do not disturb the installed program, accounts, stream, retained user state,
  Chatterino7 code, or unrelated dirty files. Git inspection first; no
  `git reset --hard`, blanket `clean`, force-push or mass process shutdown.
- Use isolated dev profiles for testing. Do not copy the default Twitch credential
  vault: parallel use of a refresh token can revoke the live session.
- Real sends, moderation, reward fulfilment and account-permission changes are
  consequential and need specific authorization, independent of permission to
  inspect source. Never confuse test dispatch with verified provider acceptance.
- Tests and checks: `cargo +1.99.0 test -p chat-workbench --bin chat-workbench --locked`;
  `cargo +1.99.0 fmt --all -- --check`; targeted locked release build with
  the installed 1.99 MSVC toolchain. Rust 1.88 on the system PATH is too old.
  Build from `recreation/`. Do not add unsolicited CI/PRs.
- Version increments and publication are a **separate** release step. Follow
  `docs/INSTALLER.md`, `docs/UPDATES.md` and existing
  `.local-release/` tooling if the owner asks. Never overwrite an existing
  published preview or update the running installed app as a side effect.
- After a batch, update the affected feature IDs here and in
  `FEATURE_COVERAGE.md` / `CHATTERINO_PARITY.md` only with appropriate
  evidence; append native observations to `VALIDATION.md`.
  `HANDOFF_2026-10-14.md` contains the next-chat startup procedure.
