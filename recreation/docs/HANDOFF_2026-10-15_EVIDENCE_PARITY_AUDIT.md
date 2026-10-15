# Jawjack — Evidence-first Chatterino parity audit and continuation handoff
**Date:** 2026-10-15
**Repository:** `E:\git\chatterino7`
**Branch:** `recreation/gpui-foundation`
**App workspace:** `E:\git\chatterino7\recreation`
**Latest inspected HEAD:** `6efc76403` (GitHub and local aligned; clean before this documentation-only handoff)
**Published preview:** `0.1.0-preview.16` from release source commit `dfed4f403`

## Mandate

Lead the next Jawjack development session by **auditing actual remaining behavioral gaps**, not by treating old unchecked Chatterino parity rows as missing implementations.

The owner challenged the earlier recommendations because most suggested features are **already built**. They are right: distinguish **implemented**, **implemented but failing/partial**, **not implemented**, **blocked by public API**, **not yet validated live**, and **intentionally different product design**. No feature is “missing” merely because a historical checklist says `[ ]`, `Part`, or `Next`.

**Do genuine research:** inspect current Jawjack Rust and original Chatterino7 C++ implementations, current upstream Chatterino7 where relevant, and official Twitch API/EventSub contracts. Read the old docs to locate questions and past evidence, **not** as conclusive proof of current behavior. Cross-check named source paths and executable/runtime results. Date and link external findings. Do not claim a live API event was validated by a parser/unit test.

## Verified source baseline — do not reimplement these

Checked from current Rust source on 2026-10-15, not inferred solely from the docs:

| Area | Present now (source evidence) | Exact remaining question |
| --- | --- | --- |
| EventSub / channel points | `crates/desktop/src/live.rs` `OWN_KINDS` has custom and automatic redemption subscriptions, hold/update AutoMod kinds and more; subscription/retry/gap diagnostics and duplicate/parse tests. `workspace.rs` exposes `own_eventsub`, `own_extras_refused`. `activity_feed.rs` classifies reward events. | A *real, naturally received* custom or automatic redemption reaching the UI and reconnect/revocation behavior still need acceptance. Do not manufacture/redemption-spend merely to test. |
| AutoMod | `automod.rs` implements hold/update parsing, bounded held queue, dedup, Allow/Deny through Helix and resolution status; `automod_actions.rs` / `main.rs` present UI actions. | The separate sender-facing `channel.chat.user_message_update` contract was **not found** in the inspected source; first research its current official requirements and whether it is possible/useful. This is **not** a claim that moderator AutoMod is missing. |
| Typed chat notices | `live.rs` processes chat notifications, `rich_messages.rs` renders labels/notice fallback and attachments; `activity_feed.rs` classifies events. | Audit specific event types and unknown/future fallbacks against official examples; prove an actual defect before implementing. |
| Combined feeds | `combined.rs` supports multi-source Twitch/YouTube panes and origin identities; `workspace.rs` routes sends through an explicit `send_to` or replied-to origin; `main.rs` renders source markers. | **Potential high-impact source-routing bug** in *message-origin moderation* below. Combined feeds themselves are NOT missing. |
| Media and selection | `inline_chat.rs` has byte-preserving text/emote/Cheers layout, wrapping, hit testing, selection/search highlights and image drawing. `rich_messages.rs` supports GIFs, typed accents and fallback. Supporting 7TV/BTTV/FFZ/paint modules exist. | Preview.16 changed wrapping/geometry; obtain narrow/native acceptance, including links, emote stacks, copies, and deleted-line fade. Existing functionality is not “missing” merely because edge QA is incomplete. |

The September/October coverage matrix has old source baselines and sometimes stale “missing” statuses. **Always look at the current implementation first**.

## First concrete investigation: combined-feed moderation origin

Source-backed potential defect; **not yet reproduced or fixed**.

1. In `crates/desktop/src/message_actions.rs`, `ChannelPane::message_action` (~line 921) handles a clicked message. The Twitch `Action::ModDelete`, `ModTimeout`, `ModBan`, and custom moderation routes emit `PaneEvent::TwitchCommand { request, draft }` without a separate clicked-message **origin/channel**. The ban dialog currently uses `self.name`, which may be a combined-pane display label. A Shared Chat `source_id` override exists for delete, so do not flatten Shared Chat and combined-feed behavior together.
2. In `crates/desktop/src/workspace.rs`, the `PaneEvent::TwitchCommand` handler (~line 2754) selects the destination with `this.destination(pane, None, &channel, cx)`. `destination()` (~line 3955) uses the replied-to origin only when given a reply parent; with `None`, it falls back to `p.send_to` (the *chosen send destination*) or the first source.
3. Thus clicking a message from source B in a combined feed with `send_to=A` **may act on A** instead of B. For a moderation action that is a serious correctness issue. Verify the complete dispatch/permission path and whether command request-specific room IDs or message metadata already override it before calling this a confirmed user-visible bug.
4. Add **isolated deterministic tests**, without actual Twitch moderation: source A selected for sending; message from B receives delete/timeout/ban/custom action; confirm intended origin and capability checks use B. Test same-channel, Shared Chat `source_id`, YouTube, and special Mentions tabs. If the bug is real, implement the smallest source-aware routing correction and recheck dialogs, API broadcaster IDs, role/scope checks and inactive sources.
5. No action should silently moderate the combined label, the wrong broadcaster or a fallback channel. Reject safely if origin is ambiguous.

Do not send a real ban, timeout, delete, raid, redemption, or chat message just to test this.

## Next batch after the concrete issue

**A. Preview.16 native acceptance (not rebuilding the renderer).**
The owner may already have installed preview.16. At the last read-only check, Windows uninstall registry reported `DisplayVersion=0.1.0-preview.16` and a new `Jawjack.exe` process was running; **recheck** current state. The release is published and verified. Changes from commits `de790fd5e` and `fe788982a`:
- The first visual line contains timestamp, badges and author followed by text. **All later soft-wrapped lines start at normal left padding**.
- Metadata spacing is 2px with measured timestamp slots.
- Deleted rows cap opacity at 48% and have **body-only strikethrough**, with no space-consuming deleted badge; existing “Hide deleted messages” setting is preserved.
- Windows Rust 1.99 desktop suite: 113 passed, 5 ignored; installer sandbox and public HTTPS SHA-256 checks passed. **Not yet accepted visually** in a 420–440px split with real deleted lines and mixed media.
- Validate selection/copy original bytes, links, mention hitboxes, 7TV/BTTV/FFZ emotes, Cheers/Bits, animated/stacked media, long bot announcements, source labels, moderation buttons, resize/reflow, scroll/tail, timestamp modes, DPI/scaling, and ordinary width. Record what was actually observed vs not. Do not manufacture live traffic or default OAuth tokens.

**B. Genuine EventSub acceptance.** Work with the existing diagnostics rather than duplicate them. Distinguish subscribed/no event, refused/retrying, delivered but unrendered/unmapped, and unrecoverable WebSocket gaps. Wait for natural real provider events or use only source/unit tests when available. No paid actions or scope changes without owner approval.

**C. Targeted API/Chatterino research.** Inspect current official Twitch contracts for sender-held AutoMod lifecycle (`channel.chat.user_message_update`), elevated/hype typed notifications, shared-chat origin routing, and unsupported/private functionality. Compare with original and upstream Chatterino7 code. Document contract versions, required scopes, dates, exact source references, and what actually benefits Jawjack. Do not implement private endpoints to fill a checklist.

**D. Higher-value new capability discovery.** Only after measuring the above, identify genuinely absent or inferior behavior relative to Chatterino7 and worthwhile Jawjack-specific enhancements. Prioritize by user impact, confirmed correctness, risk, and verification cost; avoid cosmetic checkboxes.

## Required audit deliverable and workflow

Create or update an evidence-based gap matrix in `docs/CHATTERINO_PARITY.md`, `docs/FEATURE_COVERAGE.md` or a small focused audit doc. For each proposed task record:
- **Feature / exact behavior** and where it is visible to users.
- **Current Jawjack state:** implemented + works; implemented but unverified; implemented but bug with reproduction; missing; intentionally different; or public API unavailable/research.
- **Jawjack evidence:** file + symbols/line ranges, tests and named native observations.
- **Reference:** original Chatterino7 code and current upstream behavior, or official Twitch docs dated/cited when API-specific.
- **Proposed minimal change** and **precise native/unit acceptance**.
- **Priority and risk**; explicit owner approval requirement for sensitive actions.

Do not begin with a speculative “build all EventSub/AutoMod/combined/rich media” plan. Start by verifying one concrete scenario, fixing it if necessary, and continue in coherent tested slices. Update validation and parity docs after real evidence. Keep live-unverified items unverified.

## Source/read order

1. Verify checkout status, HEAD, branch, installed Jawjack process and version. Preserve any dirty edits.
2. Read **this handoff**, then `docs/HANDOFF_2026-10-15_PREVIEW16.md`, `docs/VALIDATION.md`, `docs/CHATTERINO_PARITY.md`, `docs/FEATURE_COVERAGE.md`, `docs/PARITY_QUEUE_2026-10-14.md` (historical), `docs/APP_CONTROL.md`, `docs/LIVE_CONNECTIONS.md`.
3. Audit `crates/desktop/src/message_actions.rs`, `main.rs`, `workspace.rs`, `combined.rs`, `twitch_actions.rs` for origin routing. Also inspect `live.rs`, `automod.rs`, `automod_actions.rs`, `rich_messages.rs`, `inline_chat.rs`, `activity_feed.rs`.
4. Compare actual original Chatterino7 C++ sources present in the repository and current upstream Chatterino7 using GitHub/web when helpful; verify upstream revision/date. Research official Twitch documentation for API-specific claims.
5. Test Rust 1.99 using the explicit toolchain if `cargo +1.99.0` does not resolve: `%USERPROFILE%\.cargo\bin\rustup.exe run 1.99.0 cargo fmt --all -- --check`; `cargo test -p chat-workbench --bin chat-workbench --locked` from `recreation`, plus focused release/native verification for changed code.
6. Only inspect/drive QA in a fresh `--profile qa-...` and `--control-session`. Never duplicate default Twitch refresh credentials: parallel refresh can revoke a working sign-in.

## Operational boundaries

- Preserve the installed/running owner Jawjack, saved accounts, drafts, workspace and all dirty/unrelated code. Do not reset, rebase, broad-kill or overwrite. No unsolicited CI/Actions or PRs.
- Do not perform real Twitch sends, raids, deletes, bans, timeouts, redemptions, permission grants or other live actions without **explicit approval**.
- No version bump, public preview release, installation or Git push without owner approval. Do not republish `0.1.0-preview.16` with different bytes.
- Existing `0.1.0-preview.16` is publicly available at `https://apps.zombie.digital/jawjack/releases`; release source commit `dfed4f403`, latest handoff commit `6efc76403`.
- This handoff only prepares the next session. **No changes to app source or parity acceptance are claimed from creating it.**

## Paste into the next chat

> Use my HaiwireMCP connection as lead developer for Jawjack in `E:\git\chatterino7`, branch `recreation/gpui-foundation`. **Read `recreation/docs/HANDOFF_2026-10-15_EVIDENCE_PARITY_AUDIT.md` first.** Preview.16 is already published; don't redo its completed features. I want an **independent evidence-first Chatterino7 parity audit**, not speculative TODOs from old documentation. For every suggested feature, inspect today's Jawjack Rust, the original/current Chatterino source, and current official Twitch docs where relevant; classify it as implemented, broken, partially implemented, unverified, truly missing, intentionally different, or unsupported. Start by investigating a suspected combined-feed moderation bug: clicking a message from source B may route ban/timeout/delete through source A because pane-selected send destination is used. Reproduce safely with unit/mock/native QA, fix if confirmed, and protect real source identity and permissions. Then validate preview.16's compact wrapping and deleted-message display, and progress actual gaps in EventSub, AutoMod, typed notices and rich media only where evidence supports changes. Preserve my installed app, tokens, drafts and dirty work. Use isolated profiles and locked Rust 1.99 tests. Update the parity/validation evidence. Do not push, install, publish or perform live Twitch actions without my approval.
