# Jawjack — preview.16 released; next Chatterino parity handoff

**Updated next-session mission:** Read [HANDOFF_2026-10-15_EVIDENCE_PARITY_AUDIT.md](HANDOFF_2026-10-15_EVIDENCE_PARITY_AUDIT.md) **first**. The previous broad parity list must not be treated as missing functionality; most features already have substantial implementations. Investigate the specific possible combined-feed source-routing problem and independently verify actual gaps before writing new code.

**Date:** 2026-10-15
**Repository:** `E:\git\chatterino7` · branch `recreation/gpui-foundation`
**Application:** `E:\git\chatterino7\recreation`
**Latest published release:** `0.1.0-preview.16`
**Published source commit:** `dfed4f403` (`release(jawjack): prepare 0.1.0-preview.16 [skip ci]`)

## Start here — do not restart completed work

The owner asked for compact continuation-line chat wrapping, then deleted-message
fading/strikethrough and tighter timestamp–badge spacing. **Both source batches
have shipped in preview.16.** The previous `HANDOFF_2026-10-15.md` describes
their original diagnosis and tests but is now a historical design handoff.
Read this document first, then `VALIDATION.md`, `CHATTERINO_PARITY.md`,
`FEATURE_COVERAGE.md`, `PARITY_QUEUE_2026-10-14.md` (older order is superseded),
`APP_CONTROL.md`, `CURRENT_IMPLEMENTATION.md` and `LIVE_CONNECTIONS.md`.

Published commits:
- `6b95eb68e`: original parity and wrapping handoff.
- `de790fd5e`: full-width soft-wrapped continuation lines; a unified
  byte-offset-aware plain/emote/Cheers renderer and four geometry/hit tests.
- `fe788982a`: remove the deleted badge; cap deleted-row opacity at
  48% after other fade factors; strike **message body only**, across wraps
  and emotes; reduce metadata gaps 4px → 2px and measure timestamp slots;
  two more regression tests.
- `dfed4f403`: bump Cargo.toml and the four workspace Cargo.lock entries
  from preview.15 to preview.16 (no dependency upgrades).

## Release outcome — verified 2026-10-15

- Normal Git push to `origin/recreation/gpui-foundation` succeeded; no
  force-push, PR or CI workflow was triggered intentionally. Version bump and
  features are committed and on GitHub.
- `scripts/Build-Release.ps1` exported **committed HEAD** and produced a
  Windows x64 installer and portable ZIP from Rust 1.99.
- Locked Windows desktop suite: **113 passed, 5 ignored, 0 failed**;
  formatting and locked release build passed.
- `Test-InstallerSandbox.py --fresh-setup --app-folder
  release-out\0.1.0-preview.16\app\Jawjack` passed: payload validation,
  corruption rejection, sandbox install/upgrade/uninstall, kept user data,
  updater wait/result/relaunch/error paths, lock and contention. Real profile
  comparison reported **unchanged**.
- Local publisher preflight passed. The existing owner publisher activated
  preview.16 over SFTP with `latest.json` last.
- Independently fetched the public HTTPS feed, versioned installer and
  `archive/index.json` from the **Windows machine**. Feed version was
  preview.16, the archive index had preview.16 at the top and 16 entries,
  and the downloaded installer matched the feed's exact byte size/hash.
- Installer: `Jawjack-0.1.0-preview.16-setup.exe` —
  **15,466,368 bytes**, SHA-256
  `0bef3896fb6e558b42d81684c49d81cda70ac38a4d2e379d3f90064c2d048cac`.
- Portable: `Jawjack-0.1.0-preview.16-x64-portable.zip` —
  **14,977,049 bytes**, SHA-256
  `b9239cf5e14fb33799684e5a6e37459e469594408ac33e32dd252bef0b596c93`.
- Public feed: `https://apps.zombie.digital/downloads/jawjack/latest.json`.
  Release page: `https://apps.zombie.digital/jawjack/releases`.
- **The assistant did not close, upgrade or replace the installed app.**
  During publication it was still PID 49224 at
  `C:\Users\ZOMBIEBOX\AppData\Local\Programs\Jawjack\Jawjack.exe`.
  A later read-only check found the app running as PID 62516 and the
  Windows uninstall registry reporting **DisplayVersion 0.1.0-preview.16**.
  The update happened separately while this handoff was being finished;
  it was not initiated by the assistant. Check the current process/version
  and get owner feedback; do not assume in-app visual acceptance passed.

## Actual native evidence vs outstanding P0 acceptance

- An isolated signed-out QA profile, `qa-delete-1015` (now closed), loaded
  168 read-only historical `#dreadedzombie` messages. A native
  PrintWindow capture showed timestamp, badges, emotes and chat content
  at 1020px transcript viewport width with no obvious overlap.
- The QA profile's Twitch account stayed **signed out**, drafts remained empty
  and no Twitch commands, paid events, chat sends, raids or moderation actions
  were made. Its process was stopped and temporary capture files cleaned.
- **Unverified:** real deleted-message rendering (no deleted row available),
  a 420–440px multi-split pane, rich emote stacks across wraps, links and
  hover hitboxes, text drag/selection/copy across lines, moderator buttons,
  long timestamps, source labels/Shared Chat avatars, DPI 100/125%,
  CJK/combining/bidi/emoji, light theme, and scroller/tail anchoring on resize.
- P0 **source work is shipped**, but **full native visual/interaction parity
  remains open** until observed and logged. First collect owner feedback
  from their preview.16 installation. Fix confirmed regressions before
  broadening the queue; do not invent fake chat traffic or live Twitch events.

## Next useful development batch

1. **P0 finish native chat QA.** Reproduce long raid/bot announcements and
   deleted messages at ~420px and normal width. Verify first line contains
   timestamp/badges/name + start of text; soft wraps begin at left padding.
   Validate fading and body-only strike-through, selection/copy original bytes,
   rich emotes/Bits/link hitboxes, scrolling and resize. Preserve old
   `Hide deleted messages` option. Document screenshots and gaps.
2. **P1 EventSub/channel-point live acceptance** (AC04, AC08, AT04, TW06,
   TW16, WS10): preview.15 already shipped optional subscription
   refusal/retry/gap diagnostics and rendered/unmapped reward counters.
   Use `workspace.own_eventsub` to distinguish subscribed/no event,
   refused/pending, delivered but unmapped, and socket gaps. A naturally
   arriving redemption and reconnection/revocation acceptance are still
   unverified. Do not buy/redeem a reward or claim Twitch replay.
3. **P2 AutoMod and typed notices** (MD09, AT04, TW08/TW16): check official
   current EventSub/scopes for `channel.chat.user_message_update` before
   handling sender outcome; never treat a vanished held message as approval.
   Preserve unknown/elevated/hype notice text without privacy leaks.
4. **P3 Combined feed** (WS11, AC09, TW09, MD04–05): resolve replies,
   moderation, room settings, chatter actions and explicit send destinations
   using the **real originating broadcaster/channel**, not a combined label.
5. **P4 rich-media and daily-use robustness** (TW01–08, EM07–16, RD01–04,
   PF02): stacked/animated media, badge paints, selection and clipboard,
   accessibility, IME and DPI, long-session reconnect and memory.

Treat coverage checkboxes as feature *presence*, not blanket live acceptance.
Do not restore the browser-only **Raid Now** button: Twitch's public API did
not support that action; the real Cancel and countdown shipped in preview.15.

## Operational boundaries

Use HaiwireMCP with the existing Windows checkout. Begin by reading
`git status --short --branch`, HEAD, current running installed process and
any dirty work. **Never** reset/rebase/pull, kill unrelated processes, copy
default OAuth credentials into QA profiles, change scopes, or alter the
installed default profile to test. Use a fresh `--profile qa-...` and a fresh
`--control-session`; observe real native UI/inspect results. No broad
process cleanup.

Rust is pinned to 1.99.0 (`%USERPROFILE%\.cargo\bin\rustup.exe run 1.99.0
cargo ...`). From `recreation`, run:
- `cargo fmt --all -- --check`
- `cargo test -p chat-workbench --bin chat-workbench --locked`
- targeted locked release build and native screenshot/control acceptance.

**Do not release, version-bump, push, install or perform live Twitch
chat/moderation/reward actions without separate owner approval.** Preview.16
has already been published and verified: never republish it with different bytes.

## Paste into the next chat

> Use my **HaiwireMCP** connection as Jawjack lead developer in
> `E:\git\chatterino7`, branch `recreation/gpui-foundation`.
> **Read `recreation/docs/HANDOFF_2026-10-15_PREVIEW16.md` first**,
> then the parity and validation docs. Preview.16 was already published
> from `dfed4f403`, verified over HTTPS, and includes full-width wrapping,
> 2px metadata gaps/measured timestamps and dimmed body-struck deleted chat
> with no deleted badge. **Don't redo or republish it.** First collect my
> visual feedback and finish real native acceptance at 420–440px and normal
> widths (plain/rich chat, badges, link/selection, copy, scrolling, deletion).
> Continue the remaining Chatterino parity queue afterward: natural
> EventSub/channel-point reliability, AutoMod/typed notices, combined-feed
> source targeting and rich-media/daily-use robustness. Preserve installed
> app and any dirty work. Work on Rust/GPUI, test with isolated profiles and
> locked Rust 1.99; no live Twitch actions or publication without my approval.
