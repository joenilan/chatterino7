## Jawjack preview.18 publication and independent verification — 2026-10-15

**Published:** `0.1.0-preview.18`, committed release source `2d1dc5519`
on `recreation/gpui-foundation`, including raid and avatar fix
`1f758ac77`. Normal Git push (`39fdcaa70..2d1dc5519`),
no force push or CI request. Preview.17 remains immutable.

**What's fixed:** Incoming Twitch raid notifications now render as distinct
green system announcements, not authored chat from the raider. Actual chat
from the raider and existing highlights/Studio raid metadata remain intact.
Public Twitch and 7TV user-card avatars remain visible in Streamer mode;
Twitch replaces missing/slow 7TV art, with a visible initial as last fallback.
Other streamer privacy settings, combined-feed moderation fixes and
preview.16 chat layout are unchanged.

**Windows Rust 1.99 gates:** Locked desktop/setup suite
`cargo test -q -p chat-workbench --bin chat-workbench -p jawjack-setup --locked`:
**122 passed, 0 failed, 5 ignored**. `cargo fmt --all -- --check`
and `git diff --check` passed. `scripts/Build-Release.ps1` exported
committed HEAD to the release source folder, compiled optimized Jawjack and
setup binaries and packaged the unsigned Windows releases with checksums.
Only the existing `chat_text.rs` dead-code warning was emitted.

**Isolated installer smoke test:** `scripts/Test-InstallerSandbox.py --fresh-setup`
passed damage refusal, installation, upgrade with data retained, uninstall
with and without data removal, in-app updater wait/result/relaunch/failure
paths, and single-setup locking. Before/after snapshots confirmed the
**real installed profile remained unchanged**. The optional
`--with-app` step was skipped to avoid a second signed-in app.

**Publisher:** Offline `.local-release/Publish-Release.py` preflight
verified both artifacts, hashes and release metadata. The owner-approved
SFTP publisher activated the immutable version archive and public pointer.
Independent **public HTTPS** verification confirmed `latest.json` and
`manifest.json` report preview.18, `archive/index.json` lists preview.18
first (18 releases), the releases page and archived notes returned HTTP 200,
and full independent downloads of both binaries matched the local artifact
sizes and SHA-256 values:

- Installer: `Jawjack-0.1.0-preview.18-setup.exe` —
  **15,477,592 bytes**,
  SHA-256 `b57d96e91a8dca257aaf5a41b81da5bde942f244b72766e1b5bb966ce17674a8`
- Portable: `Jawjack-0.1.0-preview.18-x64-portable.zip` —
  **14,988,631 bytes**,
  SHA-256 `550b5c1bd30b38532b65f9eb1482c3841e48b5989113fae87e0fb48eb6259042`

**Links:** https://apps.zombie.digital/downloads/jawjack/latest.json
and https://apps.zombie.digital/jawjack/releases

**Not performed:** No assistant-initiated installation, restart or change
to the real Twitch profile, credentials, OAuth scopes, raids or moderation.
The registry still reported the owner's installed version as
`0.1.0-preview.17` after publication. The native visual result and an
actual incoming raid still need everyday-use acceptance.

---

## Preview.17 follow-up — incoming raid announcement and public avatars (shipped in preview.18)

**Owner-observed defects:** An incoming Twitch raid showed as a generic
`raid` heading followed by a normal `Smizay: 20 raiders from Smizay have
joined!` chat line, visually implying the raider typed the notice. The
owner also questioned why Streamer mode blanks public Twitch/7TV viewer-card
avatars; the installed user's workspace had `streamer_mode=automatic`
and the former `hide.card_avatars=true` option.

**Source evidence:** Twitch
[IRC USERNOTICE](https://dev.twitch.tv/docs/chat/irc/) identifies incoming
raids with `msg-id=raid`, `system-msg` and
`msg-param-viewerCount`. Its
[EventSub channel.chat.notification](https://dev.twitch.tv/docs/eventsub/eventsub-reference/)
provides `notice_type=raid`, a Twitch-authored `system_message` and typed
raid count. Jawjack's prior `Message::copy_line` prepended the raider's
name for all notices and `rich_messages::heading` printed the bare
`raid` label. The existing Streamer mode explicitly hid profile pictures,
while empty-label image placeholders could appear blank.

**Source correction (shipped in preview.18):** `rich_messages::raid_announcement`
provides the announcement wording without a chatter prefix, e.g.
`Smizay is raiding with a party of 20.`. Transcript rows for
`raid` and `shared_chat_raid` render a distinct muted-green event notice:
no raider badges, redundant raid heading, or inline moderator actions.
The underlying EventSub/IRC `Message` stays intact for the activity feed,
shoutout card, timestamp, deduplication and subsequent raider chat, which
remains a separate normal message. Raid right-click moderation controls
are likewise suppressed. We do not start/cancel raids or send any chat.

`streamer_mode.rs` no longer offers or enforces the
`card_avatars` hiding flag. Public Twitch/7TV profile pictures are
displayed regardless of Streamer mode. The card prefetches both avatar
sources and prefers 7TV when present, falling back to Twitch when unavailable,
then to a visible circular initial during loading/failure. Other Streamer
mode settings retain their original keys, defaults and effective meanings.
No user settings were modified; a legacy `hide.card_avatars` key is
ignored until the regular workspace save updates preferences.

**Locked verification:** Windows Rust 1.99
`cargo test -q -p chat-workbench --bin chat-workbench --locked`
**122 passed, 0 failed, 5 ignored**. New regressions cover literal
IRC USERNOTICE, synthetic EventSub raid versus a separate raider PRIVMSG,
notice presentation/fallback behavior, and the removal of the avatar-hide
setting. `cargo fmt --all` passed; locked optimized Windows app build
also passed with only the existing `chat_text.rs` dead-code warning.

**Remaining gate:** A signed-out isolated native visual check and then
normal streamer use after preview.18 is installed; no actual incoming
live raid or native avatar was simulated as passing. Preview.18 is published;
preview.17 was still the owner's installed version at publication time.

---

## Jawjack preview.17 publication and independent verification — 2026-10-15

**Published:** `0.1.0-preview.17` from committed source `d24071832`
on `recreation/gpui-foundation`. Normal Git push
(`6efc76403..d24071832`), no force push or CI request. This release
includes the source-aware combined-feed moderation and Shared Chat
receiving-room safety corrections, plus channel-bound normal and pinned
chatter cards, separate per-channel follower/subscription caches, and
fail-closed actions for unknown sources. Preview.16 chat wrapping and
deleted-message layout were not changed.

**Locked Windows checks:** Rust 1.99 desktop/setup command
`cargo test -q -p chat-workbench --bin chat-workbench -p jawjack-setup --locked`:
**117 passed, 0 failed, 5 ignored**. Cargo formatting and `git diff --check`
passed. `scripts/Build-Release.ps1` exported committed HEAD and produced
the release installer and portable ZIP. Optimized build passed with only the
existing `chat_text.rs` dead-code warning.

**Isolated sandbox:** `scripts/Test-InstallerSandbox.py --fresh-setup`
passed payload integrity/corruption refusal, install, upgrade retaining data,
uninstall both with and without data removal, updater wait/relaunch/results,
one-installer lock, and failure paths. Before-and-after real-profile snapshots
were identical. The `--with-app` option was not used; we did not launch a
second potentially signed-in native app.

**Publisher:** Local `.local-release/Publish-Release.py` preflight verified
both artifacts and notes without remote access. The owner-authorized
`--publish` call activated the immutable archive and latest feed through
SFTP. Its success was verified **independently over public HTTPS**:
`latest.json` reported `0.1.0-preview.17`, `archive/index.json` listed
preview.17 first among 17 releases, and both downloaded binaries matched the
published manifest sizes and SHA-256 hashes. The release listing and current
and archived notes URLs returned HTTP 200.

- **Installer:** `Jawjack-0.1.0-preview.17-setup.exe`,
  **15,476,808 bytes**,
  SHA-256 `0f67084c09e651bf445f4018b7b74b6d131f12e46df04dc410bda55cd4188f2f`.
- **Portable:** `Jawjack-0.1.0-preview.17-x64-portable.zip`,
  **14,987,627 bytes**,
  SHA-256 `78417c4a78af6cc783664f5bfd537a4f7a21884b1f8c74a32eedecad64883f78`.
- **Public:** https://apps.zombie.digital/downloads/jawjack/latest.json
  and https://apps.zombie.digital/jawjack/releases.

**Owner's installed copy preserved:** read-only uninstall-registry check
still reported `0.1.0-preview.16` after publication. Nothing was
installed, restarted, uninstalled or sent on Twitch; no live moderation,
OAuth, raid or reward changes were exercised. The new per-source moderated
behavior still needs actual signed-in acceptance. The owner reported no
observed preview.16 visual issue so far and requested proceeding with the
release; this is not an exhaustive narrow/selection/media test.

---

## Jawjack preview.16 publication — 2026-10-15

## User-card source provenance and cache separation — local 2026-10-15

**Preview.16 field observation:** owner reports it works fine in daily use
so far. The released UI remains unchanged and should not be retested without
a concrete regression report; exhaustive edge cases are not thereby proven.

**Source defect and fix:** `chatter_card` previously derived role, labels,
follow-age and related actions from a combined pane label, and cached follow
dates only by user ID. The same chatter appearing in two channels could be
displayed with the wrong broadcaster context; card actions could inherit
`send_to`. `combined::chatter_card_channel` and
`chatter_card_message` now resolve and filter per clicked origin, including
Mentions. `inspect_chatter_for_source` and pinned-card windows retain
the selected source; ordinary Chatters list and account-global Block remain
separate. Moderator/broadcaster controls use per-source role knowledge;
`TwitchMessageCommand` dispatch rechecks permissions on that source.
Follow-age/subscription caches and the `WantFollowAge` event are keyed by
(broadcaster, user), with a destination and role check before the Helix read.
Unknown source and YouTube remain read-only in Twitch cards; `Other…`
timeout and `Warn…` are suppressed in combined/special cards because their
legacy draft path uses the composer destination.

**Reference evidence:** original Chatterino7
`src/widgets/dialogs/UserInfoPopup.cpp:895–976` separates context channel
from opening channel and filters recent messages against the context.
Twitch [Get Channel Followers](https://dev.twitch.tv/docs/api/reference/#get-channel-followers)
requires `moderator:read:followers` and broadcaster/moderator identity for
user-specific results. No OAuth scopes were added or changed.

**Windows Rust 1.99 test result:** `cargo test -p chat-workbench --bin
chat-workbench --locked`: **117 passed, 0 failed, 5 ignored**, including two
new combined/card-source routing/filter tests. `cargo fmt --all` passed.
Locked Windows Rust 1.99 optimized build (`cargo build -p chat-workbench
--bin chat-workbench --locked --release`) passed, with the pre-existing
`chat_text.rs` dead-code warning only. `cargo fmt --all -- --check` and
`git diff --check` passed. The existing installed preview.16 is untouched;
this local build was not published or installed.
No auth changes, sends, bans, timeouts, or other live Twitch actions.

## Combined-feed moderator origin / Shared Chat ID safety — local 2026-10-15

**Implementation:** `message_actions.rs` now emits message-origin-bound
`TwitchMessageCommand`, and the workspace permission checker targets that
actual channel, never `send_to`. Message menu/inline controls gate against
each Twitch source. A Shared Chat copy is deleted in the receiving room:
`message.id` belongs there; `presentation.source_id` points to a distinct
broadcaster and cannot be paired with that message ID. Any explicit override
is refused. YouTube actions remain separate. See
[EVIDENCE_PARITY_2026-10-15.md](EVIDENCE_PARITY_2026-10-15.md).

**Checks:** Windows MSVC Rust 1.99 `cargo fmt --all` succeeded and
`cargo test -p chat-workbench --bin chat-workbench --locked` completed
**115 passed, 0 failed, 5 ignored**. Tests cover normal and combined
channels, Mentions origin, Delete/Timeout/Ban/Warn, mismatched/unknown source,
YouTube exclusion, receiving-room Shared Chat delete and refusal of foreign
broadcaster ID overrides. Final locked release build and `git diff --check`
are separate acceptance gates.

**Additional build:** locked Windows Rust 1.99 `cargo build -p chat-workbench --bin chat-workbench --locked --release` passed (optimized build, existing `chat_text.rs` dead-code warning only). `cargo fmt --all -- --check` and `git diff --check` passed. The local binary was **not installed or published**.

**Not yet claimed:** actual Twitch moderation, authenticated role/source
changes, narrow native rendering on 420–440px panes, links/selection/copy
across real media. No push, installation, preview publication, credential
change, OAuth scope change or live provider mutation.

**Published, independently verified over HTTPS:** `0.1.0-preview.16`,
from committed source `dfed4f403`, branch `recreation/gpui-foundation`.
This release includes the full-width chat wrapping from `de790fd5e` and
deleted-message dim/strikethrough plus denser timestamps/badges from
`fe788982a`. Normal Git push completed; no force push or CI requested.

- Rust 1.99 locked Windows desktop suite: **113 passed, 5 ignored**.
  `Build-Release.ps1` exported committed HEAD, built the installer and
  portable ZIP, and verified setup payload hashes.
- `Test-InstallerSandbox.py --fresh-setup` against preview.16 passed
  corruption refusal, install/upgrade/uninstall, data retention and updater
  wait/relaunch/result/error/lock tests; it verified **real profile unchanged**.
- Local publisher preflight passed. Publisher activated the immutable
  archive and public `latest.json` through the existing owner workflow.
- **Independent public HTTPS check on Windows:** downloaded the versioned
  installer, verified **15,466,368 bytes** and SHA-256
  `0bef3896fb6e558b42d81684c49d81cda70ac38a4d2e379d3f90064c2d048cac`
  against `latest.json`; `archive/index.json` listed preview.16 first
  (16 release records); archived notes were reachable.
- Portable ZIP: **14,977,049 bytes**; SHA-256
  `b9239cf5e14fb33799684e5a6e37459e469594408ac33e32dd252bef0b596c93`.
- Links: `https://apps.zombie.digital/downloads/jawjack/latest.json`,
  `https://apps.zombie.digital/jawjack/releases`.
- The assistant did not stop or install over the owner's running
  Jawjack. At publication it was still the prior running process.
  **Later read-only check:** running Jawjack PID 62516 and
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Jawjack`
  `DisplayVersion=0.1.0-preview.16`. Installation/restart occurred
  separately, without any assistant-initiated update. No post-install
  native visual acceptance has yet been recorded.

The next work is native **acceptance**, not reimplementation or republication:
verify 420–440px split wrapping, actual deleted message rows,
copy/selection/link/emote geometry and DPI/resize. Wider read-only guest
QA already observed 168 messages without obvious text overlap, but an
actual deletion and narrow split were not observed. See
[post-publication handoff](HANDOFF_2026-10-15_PREVIEW16.md).
Do not mark full native P0 parity accepted from this source/build evidence.

---

## Deleted-message and metadata-density polish — 2026-10-15 (local; before next preview)

**Source behavior:** Removed the space-consuming red `deleted` badge and its
first-line width budget. Deleted chat now caps the **entire** row
at 48% opacity after the entrance/historical/similar/Shared Chat factors
are considered; already-dim lines are not faded a second time.
Previously an early `.opacity(0.45)` was overridden by the final
opacity refinement, so deletion dimming was not reliably visible. The shared `InlineChat` renderer draws a 1px strike
through only the **message body**, following existing measured lines and
including emote spans; timestamp, sender label and badges remain uncrossed.
This does not mutate `Message::copy_line()`, original byte ranges,
search/selection, clickable links or emote hit-testing. Existing
**Hide deleted messages** behavior remains unchanged.

**Density:** Metadata gap reduced from 4px to 2px and the timestamp's
previous wide fixed slots replaced by the measured maximum glyph width
for each supported time format (+2px safety padding). The same width is
used for visible timestamp and first-line reserved inset, preserving
stable alignment; continuation lines still begin at full row padding.

- **Windows verification:** locked Rust 1.99 desktop suite **113 passed,
  5 ignored, 0 failed** (two additional deletion/opacity regressions);
  `cargo fmt --all` and locked release build passed (58s, exit 0).
- **Read-only native QA:** ran the built release in isolated profile
  `qa-delete-1015`, control session `jj-delete1015a`, with no shared
  credentials. Guest read-only `#dreadedzombie` loaded 168 historical
  messages. Enabled `13:34` timestamps only inside QA and resized
  to a 1050×640 requested window; actual OS bounds 1066×648 and
  transcript viewport 1020×403. A native `PrintWindow` screenshot
  showed the rows, timestamps and badge/media rendering without
  obvious text/metadata overlap at that width; `send_pending=false`,
  draft empty and account remained signed out.
- **Acceptance gaps:** no actual deleted chat row was present for a
  before/after native strike/dim assertion. A 420–440px split pane,
  light theme/DPI variants, hover/link hitboxes, selection over deleted
  emotes and scrollbar/tail reflow still require owner/native checks.
  Build/tests and one normal-width screenshot are **not full P0
  native acceptance**.
- **No publication or live actions:** no chat sends, paid rewards,
  moderation, raids, OAuth scope changes, installation, or push. The
  default installed Jawjack remains untouched. This is preparatory
  source work for the next preview.

---

## Compact chat wrapping — source/test checkpoint (2026-10-15, unpublished)

**Implementation:** the timestamp, optional moderation tools, channel-origin
markers, Shared Chat avatar, and badges are rendered as clickable
first-line-only metadata over the full-width text row. The measured text renderer
reserves their calculated width on visual line 1 and allows every soft-wrapped
continuation to begin at normal row padding. Plain messages and emote/Cheers
messages use the same byte-preserving InlineChat layout, including selection,
hover, link and emote hitboxes. System/moderation notices also use the shared
wrap but stay nonselectable, matching their previous interaction behavior.
No literal line breaks or metadata are inserted into copyable message text.

- **Build/tests:** Rust 1.99 locked Windows desktop tests **111 passed, 5
  ignored, 0 failed** (four new focused wrapping/geometry tests). `cargo fmt
  --all` completed. The final locked Windows release build **passed**
  after the typed-notice changes (1m 04s, exit 0). This is the local
  development binary; nothing was installed or published.
- **Native profile:** isolated `--profile qa-wrap-20261015`, control session
  `jawjackwrap1015a`, started signed out. A native `inspect` returned the
  independent profile, `control_enabled=true`, and zero visible panes; no
  credentials were copied and the installed Jawjack was not disturbed.
  Subsequent UI-control input was blocked before execution. Consequently
  **no 420–440px transcript with real messages or screenshot was validated**;
  this is not native visual acceptance.
- **Still unverified:** actual first-line gap width with all combinations of
  custom mod controls/source labels/badges, plain and animated media at narrow
  widths, link hover/click and multi-line selection/copy in the real window,
  reflow/tail anchor when resizing, CJK/combining/emoji/long tokens, light theme,
  100%/125% DPI and accessibility. The shared text path changes how plain chat
  is measured; long-session CPU/memory performance needs real observation.
- **Restrictions:** no live Twitch actions, send, redemption, raid, app
  installation, publication, CI, OAuth changes or default-profile edits.
  Parity register remains open until native visual and interaction checks pass.

---

## Preview.15 publication and public verification — 2026-10-14

**Published:** `0.1.0-preview.15` from committed source `492d2f28a` on
`recreation/gpui-foundation` (normal Git push, no force push or CI).
Changes from preview.14 include Enter/completion and repeated-send reliability,
optional EventSub/channel-point diagnostics and a raid countdown with
actual Cancel. The browser-only "Raid Now" shortcut is **not** shipped.

- Tests: Rust 1.99 locked desktop suite **107 passed, 5 ignored**; setup
  package tests, formatting and `git diff --check` passed.
- Packaging: `scripts/Build-Release.ps1` exported committed HEAD and
  successfully built both Windows release binaries. The installer/updater
  sandbox passed, including atomic upgrade/uninstall, damage refusal, results,
  wait/relaunch, contention and preservation of the real installed profile.
- Local release validation passed with the unchanged publisher preflight.
  Installer `Jawjack-0.1.0-preview.15-setup.exe`: 15,496,064 bytes;
  SHA-256 `0e16aa02438f7b0d7c12c575e48fd231621d22e184891322618f069610e59a2a`.
  The portable ZIP and checksum sidecars validated too.
- Publisher activation reported success, followed by **independent HTTPS
  verification**: `latest.json` returned preview.15, public installer
  downloaded with HTTP 200 and the exact declared size and SHA-256,
  `archive/index.json` listed preview.15 first, and public archived release
  notes were served successfully.
- Public feed: https://apps.zombie.digital/downloads/jawjack/latest.json
  Releases: https://apps.zombie.digital/jawjack/releases

**Not performed:** no local install/restart/kill, Twitch raid, chat send,
redeeming rewards or OAuth permission changes. The already running installed
Jawjack was kept intact; its updater follows notify/download/confirm-to-install.
A genuine live channel-point redemption and raid experience are still
unverified; publication does not imply those live behaviors passed.

---

## BTTV / FFZ implementation checkpoint — 2026-10-09

Linux offline locked build passed against the new catalog/renderer integration.
Windows build and native BTTV/FFZ images are not yet verified. Existing native
preview remains ba9bb3b until the connected Windows tooling can safely update it.
No synthetic messages or new test harness were added. No OAuth scopes changed.

The remaining history-settings check encountered intermittent Deadlink envelope
errors even on a standalone read-only inspect, without a terminalID or app exit.
Independent process/log/screenshot checks found the app responsive and receiving
chat, with no crash evidence and no outstanding control request/response files.
History was still10k and original drafts49/0. Root has not changed the limit.

## Native keyboard correction accepted — 2026-10-09 21:00 UTC

Windows ba9bb3b675a9bb8a9e63163a1db33777c47416a8 built with Rust1.99 locked
release, exit0 in12.08s. In a verified disconnected channel-not-found pane:
- Arrows selected DansGame; Enter inserted DansGame once.
- Tab completed :Di to DinoDance once.
- Neither set send_pending or showed Sending/Not sent.
The temporary pane was removed; original three tabs and both exact drafts were
restored (49 and0 characters). No send was repeated. App left running.

History controls are compiled, but saving another cap is not yet accepted.
Deadlink returned `terminal request failed: Failed` on the Appearance click;
two read-only reconciliation attempts failed. Click outcome is unknown. Last
confirmed history limit10,000; counts0 and265. Do not claim settings persistence
passed until the actual saved workspace and UI are inspected.

## Completion action-routing correction — 2026-10-09

Native b5ad20e found a real defect: Enter completion also triggered a send attempt
in HutchMF; Twitch rejected it as followers-only. No delivery or retry claimed.
Both saved drafts were restored exactly. Tab navigated focus instead of completing,
and arrows did not retain the selected candidate. Picker search/paging and Escape
worked. Populated-cache inspection (15,590 bytes) and subsequent commands worked.

Correction: composer Enter has a single semantic action owner. The independent
InputEvent::PressEnter submit handler and raw-key completion handling are removed.
Toolkit MoveUp/MoveDown/IndentInline actions and composer-scoped Tab now route to
completion before editor/default focus actions. Picker Enter uses the same owner.
Linux build passed; native corrected behavior is pending. Do not use public chat
as a safe fallback when checking whether a key might send.

## Configurable history cap — 2026-10-09

Linux offline locked build passed (5.87s). Added persisted per-channel history
cap controls while preserving the existing10k default. Native settings/persistence
interaction pending; no large-traffic performance measurement claimed.

## Native badge/picker result and control fix — 2026-10-09

Windows 9fae62f build exit 0 (1.04s confirming incremental build; initial build
terminal result was lost during a Haiwire restart). Real incoming messages show
Twitch badges. HutchMF picker loaded 1,065 choices. DinoDance search returned one
result without changing drafts; paired native snapshots visibly show its animated
preview, closing the earlier exact-emote visual gap. Clicking inserted unsent text
and closed the picker. Both original drafts were restored by local comparison.

Inspection then failed because the media-rich response exceeded the old 16 KiB
shared request/response limit, leaving that response unresolved. This correction
separates the bounded response budget (256 KiB) from the unchanged small request
budget and ensures server oversize errors retain response identity for cleanup.
Native paging/completion/channel switching/tooltips still await the corrected
preview. No chat messages were sent.

## Jawjack P0 EventSub, composer and raids — 2026-10-14 (unreleased)

Entry state: branch `recreation/gpui-foundation`, HEAD `66de13cb1`,
working tree clean. The installed default-profile Jawjack remained running
at `C:\Users\ZOMBIEBOX\AppData\Local\Programs\Jawjack\Jawjack.exe`
(separately rechecked after test attempts). It was not replaced or restarted.

- `cargo +1.99.0 test -p chat-workbench --bin chat-workbench --locked`:
  **107 passed, 5 ignored, 0 failed** (112 total), following P0, Enter,
  raid UI and repeated-message acknowledgement changes. New cases test
  custom/automatic redemption delivery, duplicate-ID suppression, malformed
  reward counting, failed enablement without a subscription ID, optional
  retry/socket-gap state, keeping the original draft distinct from a
  Twitch repeated-message wire suffix, and recording an unmapped reward
  notification once without leaking its private payload. A repeated live
  send or naturally arriving redemption was not attempted.
- `cargo +1.99.0 fmt --all`: passed. Locked Rust 1.99
  `cargo +1.99.0 build -p chat-workbench --bin chat-workbench
  --release --locked`: passed on Windows. Binary built locally only,
  never installed or published.
- Initial isolated `--profile qa-p0-20261014` control attempts
  timed out or exited with the owned launcher. **Superseded native result:**
  keeping the QA window alive inside the same owned Python process
  produced successful control-session inspection, reporting the new
  `workspace.own_eventsub` keys with zero counts (expected:
  the separate QA account was deliberately signed out).
- A final smoke pass against the **latest** locked release binary ran in
  `--profile qa-p0-final-20261014`, control session
  `p0qa20261014h`. Native `inspect` returned
  `workspace.own_eventsub.unmapped_reward_notifications: 0`
  and the complete diagnostic keys. In the signed-out test pane,
  `/uptime` + first Enter cleared the draft (7 → 0), a second
  Enter kept it empty, Tab completion produced 6 → 8, and
  Down + Enter selected a suggestion before a later Enter
  submitted locally. The specific child process exited afterward.
- A fresh `--profile qa-p0-keyboard2` Windows GUI accepted actual
  controlled key/text events in a disconnected `#dreadedzombie` pane.
  Typing local `/uptime` opened one suggestion (7 draft chars).
  **First Enter cleared/executed immediately (7 → 0)**; repeating Enter
  left 0 with `send_pending=false`. Before the autocomplete fix,
  the same native scenario was independently reproduced as 7 → 8 on
  first Enter and 8 → 0 on second Enter.
  `/uptim` + Tab completed to `/uptime ` (6 → 8), then Enter
  executed (8 → 0). `/uptim` + Down + Enter intentionally
  completed (6 → 8), and the next Enter executed (8 → 0).
  This is verified **isolated native input control**, not a
  genuine Twitch message send or visual screenshot review.
- A separate sending-code audit identified a second duplicate-send cause:
  Twitch repeats gain an invisible suffix on the wire, but the old success
  handler compared that suffixed wire payload against the unchanged draft,
  preventing the editor from clearing after acknowledgement. The send path
  now retains the original draft for acknowledgement/history and sends the
  suffix only in the wire payload. The focused unit test exercises the exact
  two-value distinction; native **live success acknowledgement** remains open.
- The isolated QA process was terminated by exact child PID; the installed
  Jawjack was untouched. No real raid, cancellation, chat post, redemption,
  moderation, account credential or OAuth scope change occurred.
  **Actual send acknowledgement and duplicate protection in a connected
  Twitch session**, a naturally arriving reward and raid UI/countdown
  acceptance remain unverified.

- Follow-up before release: removed the nonfunctional browser-only
  `Raid Now on Twitch` shortcut. The source now retains only the
  estimated countdown and the authorized Cancel action. The unused
  broadcaster URL state was removed as well. This is a source/compile
  correction, **not** live raid or Cancel acceptance.

# Current checkpoint: badges and emote composition — 2026-10-09

Linux offline locked build passed against the implementation tree (5.46s).
Windows build and real-app interaction for this batch are pending; no runtime
acceptance is implied by the compiler. No test harness or synthetic traffic added.

Previous b7b744d native result: animated Twitch and 7TV media visibly changed poses;
owner subsequently confirmed animations. DinoDance was identified independently
as emotesv2_dcd06b30a5c24f6eb871e8f5edbd44f7 with 25 decoded frames and frame clock
2→19 over 487ms; that exact emote was not in the captured visible rows.

This batch requires ordinary native inspection of badge rows, picker search,
paging and insertion, completion keyboard handling, Escape/focus restoration,
long-draft protection and retained owner draft. Never send a chat message merely
to inspect these controls. The earlier ambiguous send remains unrepeated.

---

# Foundation validation

2026-10-09 cloud Linux, Rust 1.99.0.

- `cargo test -p chat-core --locked --offline`: passed, 13 tests.
- `cargo clippy -p chat-core --locked --offline --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check`: passed.
- GPUI `cargo check -p chat-workbench --locked`: dependency download was blocked
  by a CONNECT proxy HTTP 403 before application compilation. Not passed.

The initial Windows Actions run for 76f433a passed formatting and all eight
then-existing core tests. It reached the desktop crate and failed on exactly one
reported diagnostic: unused `ButtonVariants` import with warnings denied. That
import has been removed. The revised desktop has not yet been rebuilt on Windows.

Owner requested no CI/Actions. The recreation workflow was removed in f8e36f4.
No rerun is planned; future commits use `[skip ci]`. The owner will build/run locally
and report errors. Release linking, GUI runtime and performance remain unverified.

Source review fixes:
- Disjoint, monotonic selection-order ranges prevent equal-index rows in separate
  panes from being silently included in same-pane selection. True per-pane scope
  isolation and virtualized offscreen copy remain future acceptance gates.
- Redaction uses item-anchor-preserving remeasurement and skips unchanged results.
- Overlay grouping preserves source copy text, limits layers per stack, and handles
  orphan overlays, intervening text, Unicode and newlines. No emote images rendered.

No live Twitch session, moderation actions, credentials or installed applications
were used. No Windows, browser, accessibility or owner acceptance is claimed.

## Dark frame and selection follow-up

Owner ran the prior preview on Windows and supplied a screenshot: native app
opened, but default light appearance/OS titlebar were rejected and text selection
was reported barely usable. The screenshot alone does not prove replay/timeout
results; the owner reported clicking those controls.

Patch 7cb9e984 (owner subsequently reported running it and liking the design):
- Dark-only Studio palette, Segoe UI 14px body, 40px custom caption, 46px Windows
  hit targets and a 6px top resize strip, based on Haiwire's documented design.
- Model-owned independent per-pane UTF-8 byte selections; no ephemeral toolkit
  selection registration. Copy reads current retained messages, including rows
  outside the viewport. Selection clears on endpoint pruning or redaction.
- Drag, reverse drag, Shift extension, double-click word, triple-click message,
  Ctrl+C/copy button; wheel during drag reprojects against the newly painted rows.
- 21 core tests pass, core Clippy with denied warnings passes, rustfmt passes.

Exact new native UI compile and runtime checks must be performed on Windows.
Check dragging/resizing/maximizing, title controls, pane-isolated copying,
selection while scrolling/bursting, and redaction. Automatic edge auto-scroll,
full grapheme-aware word semantics, touch and screen-reader verification remain.

## Terminal-style copy follow-up

The owner requested highlight then right-click copying, alongside normal editing
shortcuts. Right-click inside a transcript now invokes that pane's existing
model-backed copy operation without changing the selection. Empty selection leaves
the clipboard unchanged. Ctrl+C remains supported. Cut/paste belongs to editable
fields; a live composer is still future work. Native right-click behavior remains
pending Windows verification; the owner's design feedback is not full interaction
acceptance.

## Interaction polish batch (Preview 02)

Owner feedback after 76412f7: right-click copying appeared ineffective, and there
was no highlight clearing or confirmation. No successful native right-click
acceptance is claimed for that revision. Source inspection found the toolkit
scrollbar consumes right mouse-down in its bubble handler; ordinary text event
interception was not proven.

This batch includes:
- Viewport-gated capture-phase right-click handling, before scrollbar bubbling.
- One copy path for right-click, keyboard and button; clipboard read-back must
  match before clearing selection and showing a deduplicated in-app Copied toast.
  Failed verification retains selection and explains retry; empty selection
  preserves existing clipboard contents and gives an instruction.
- Transcript-only Ctrl+C/Ctrl+Insert, Ctrl+A and Escape contexts, separate from
  editable draft input. The draft uses toolkit cut/copy/paste and undo/redo.
  It is explicitly local, unsent and not persisted after closing.
- 22px line boxes instead of dead inter-row padding; wider clipped text hit areas
  excluding the scrollbar gutter.
- Per-pane Latest buttons, Following latest / Reading history status, tooltips
  describing synthetic replay/moderation, wrapping action rows, and shortcut help.
- Visible INTERACTION PREVIEW 02 label to identify the running UI.

25 core tests, core Clippy with denied warnings, and rustfmt pass. The offline
native check stops because accesskit 0.24.1 is not cached. Desktop catalog remains
offline at the latest check. This is not a native compilation or UX pass.

Windows acceptance for this batch, once native execution is available:
1. Select forward/backward in each pane; right-click text, padding and scrollbar.
   Paste into the local draft: exact content, selection gone, one success toast.
2. Repeat via Ctrl+C, Ctrl+Insert and Copy selection. Try empty selection with
   existing clipboard contents: clipboard unchanged; no success claim.
3. Select across offscreen rows, use Ctrl+A, then Escape; verify pane isolation.
4. Focus local draft: Ctrl+A/X/C/V/Z work only there; right-click exposes editing
   behavior rather than copying chat. Draft text never sends to a network.
5. Scroll up, replay a burst, verify reading position/status, then pane Latest.
6. Resize to minimum window size; toolbar wraps without obscuring transcript or
   draft. Check repeated copies replace the in-app notice instead of piling up.
7. Redaction clears old selection and changes copy source; app close/reopen
   respects the documented nonpersistent draft behavior.

Still pending: automatic selection edge scrolling, grapheme-aware selection,
native accessibility/IME/DPI review, and live Twitch/network features.

## Owner workflow correction

The owner requested removal of test-only UI and repeated test-script/checklist
workflows. Replay/timeout controls and their development-only handlers are removed.
Future development prioritizes ordinary builds and manually driving the real app.
See APP_CONTROL.md for the new explicit-session local driver; its native execution
is unverified. Existing historical test results above are not new release gates.

### 3f18e079 native recovery and provider catalogs — 2026-10-09

Windows locked Rust 1.99 release build passed in 30.47 seconds. Existing account,
tabs and exact saved drafts restored. History 5k saved, then 10k restored/saved;
both pane headers displayed /10000. Catalog counts: BTTV/FFZ globals54/9,
dreadedzombie19/18, HutchMF33/0. Natural Twitch/7TV images observed in busy chat.
BTTV/FFZ-specific image/picker acceptance remains open. Native input stopped on
apparent owner activity. A later exited preview was relaunched successfully;
account, drafts and 10k state remained intact. No chat messages sent in this run.

### Recursive docking batch — implementation, not native acceptance

Linux offline locked build covers recursive split geometry, grouped channel tabs,
separate persisted live filters, broadcast metadata worker and updated inspection.
Required native checks: old layout restore; three-plus panes; nested horizontal
and vertical edge drops; center regrouping; resize persistence; canceled drag;
channel switch focus and exact drafts; middle-click cancel/close; live-only filters
and selected-offline visibility; reconnect and restart. Do not infer these passes
from compilation or the preceding 3f18e079 runtime.

### dc404246 native docking findings

Windows locked release build passed16.55s. Three visible panes with nested axes
were proven using a top-edge drop. A bottom-edge drop showed Place below over the
right panel but regrouped into the left group on release: confirmed defect.
The compact batch resolves drop target/edge from current painted panel bounds and
release pointer rather than retaining the drag-hover state/callback's pane name.
This correction is compiled, not yet verified natively. Control inspection now
records the last drop's source, target, edge and pointer coordinates for diagnosis.
Owner layout and exact drafts were restored; temporary QA workspace9 remains.

### e7d4f409 compact native acceptance — 2026-10-09

Windows Rust1.99 locked release build passed (exit0,14.04s). Native inspection at
540x470 verified account-dialog open/close without auth changes, gear/settings
overlay without changing pane widths, sidebar toggle, group + add-form/cancel,
and channel-tab right-click Add/live-filter/Close menu. Default launch size was
not changed; original1280x820 client size was restored after the temporary check.

No unexpected drops occurred during those controls. A controlled bottom-edge
move then passed: QA B at release(958,700), local-control input, target QA C,
Place below; dock and screenshot confirmed A left / C above B right. Captured
MouseUpEvent position avoids relying on mutable cursor state. This proves that
case; it does not establish the cause of earlier unexplained native changes.

QA workspace9 was removed through normal confirmed close. Owner workspace4,
layout, sidebar state and every preexisting draft matched their saved baseline;
history remained10000 and live messages arrived in both owner channels. Preview
left running with overlays closed. No messages sent or account grants changed.

Still not claimed: full Chatterino parity, every divider/reset/cancel permutation,
all provider-specific image variants, and comprehensive live-status transition
coverage. The old two-pane cap is removed; three-pane nested layouts were observed.

## 2026-10-10: channel regrouping and input ownership repair

Owner reported that panels could not merge, then observed an apparently held tab
when merely hovering. The Windows reproduction did leave a synthetic drag held
between a move call and screenshot capture. Release inspection reported a native
input at a different coordinate, so the prior center-drop result is not valid
center-merge acceptance. No owner click was observed. The test stopped and the
owner's workspace and size were restored; temporary QA workspace 10 remains.

Source changes: dedicated tab-strip merge targets, larger center merge area,
shared hover/release resolver, drag tab ordering, context-menu left/right ordering,
and Ctrl+PageUp/PageDown cycling within the focused channel group. Added atomic
control drag and source-matched pointer releases, plus hover cancellation for
stale drags. Linux build passed; Windows build and manual acceptance pending.

### Native acceptance: 7c1d662c

Windows locked release build passed with Rust 1.99.0, exit 0, 18.80 seconds.
Using only atomic drag calls in temporary QA workspace 10:
- Center B (70,430) to A (640,210): merged [A,B].
- B (210,48) to right edge (1170,400): A left / B right.
- B (710,48) to A tab strip (75,48): merged [B,A].
- B (75,48) to end of strip (270,48): reordered [A,B].
- Ctrl+PageUp selected A; Ctrl+PageDown selected B.

All drag responses reported released:true, pointer_owner:null and local-control
input. Screenshots showed no drag left active; no routing failures or unexpected
layout changes. QA workspace 10 was closed through normal confirmation. Owner
workspace 4, exact saved layout/drafts/sidebar, history 10000, original 1280x820
size and window position were preserved. No chat sends or auth changes. Running
PID 23160, control session jj-manual-7c1d662c-atomic, owned terminal hw-2-52.
Evidence: isolated-profile screenshots 104–109. This establishes the exercised
regrouping paths, not complete Chatterino feature parity or all device input cases.

## 2026-10-10: retained-history Find implementation

Linux locked offline build passed after adding per-pane search, independent exact
source highlights in plain and inline-media rows, keyboard navigation, incremental
arrival matching, and channel URL actions. Compiler evidence only so far; native
Windows behavior and keyboard/no-send acceptance remain pending. The existing
virtualized list is retained. Its scroll-to-item positions a matching message's
start and may resume tail following when bottom is reached; exact occurrence
visibility and restoration to a pre-search pixel offset are not claimed.

### Native acceptance: 75e7725c

Windows locked release build passed with Rust 1.99.0, exit 0, 17.98 seconds.
Deadlink briefly stopped replying; the uncertain add-channel operation was
reconciled before more mutations. Access recovered and native checks finished.
On actual HutchMF traffic, two StreamElements matches visibly highlighted.
Enter/F3 navigated row 11 to 16, Shift+Enter/Shift+F3 returned to 11. Ctrl+F
worked from composer, transcript and selected-tab/workspace focus; header Find
worked. Escape/reopen retained query, no-match search reported 0/null, and an
unchanged query acquired additional matches from real incoming messages.
Search state remained independent across two panes. Enter in Find preserved a
14-character disconnected draft with no send pending. Selection endpoints
survived query/navigation changes and copying returned clipboard_verified.
Latest closed Find and resumed following. Browser/URL menu entries were visible;
browser opening and emote-label highlighting were not separately exercised.

Temporary QA workspace 11 was removed through normal confirmation. Owner layout,
all preexisting drafts, sidebar and history 10000 matched exactly; original
1280x820 client geometry at (1072,306) was restored, overlays closed,
pointer_owner:null and active_drag:false. No sends, auth changes or source edits.
Running PID 47220, session jj-manual-75e7725c-find2, owned terminal hw-1-3.
Last successful inspection hw-1-29; isolated-profile screenshots 114–119.

## 2026-10-10: message action implementation

Linux locked offline build passed (7.77 seconds) after message menus, retained
chatter cards, validated provider logins and source-byte link interactions were
added. Existing selection and search rendering remain separate from activation.
Windows compilation and native interaction evidence are pending. No new tests,
CI, external profile requests, chat sends or OAuth scopes were added.

### Native acceptance: 8b1bcc6d

Windows Rust 1.99.0 locked release build passed, exit 0, 18.38 seconds; checkout
remained clean. On actual HutchMF traffic, no-selection message menu, chatter
inspection, Ctrl-click username and Escape dismissal passed. The card fit the
360x280 minimum window. Username copy, selected-text right-click copy, and exact
real-link copy produced visible feedback. Mention insertion replaced only the
selected composer span, sent nothing, and its temporary draft was cleared.

QA workspace 12 was closed through normal confirmation. Owner tab, all preexisting
drafts, sidebar, history limit and original 1280x820 geometry matched baseline.
Both owner channels were connected, overlays closed, active_drag:false and
pointer_owner:null. Running PID 44632, session jj-manual-8b1bcc6d-actions,
owned terminal hw-1-35. Native evidence includes isolated-profile screenshots
122–127 (menus/cards), 131 (unsent mention), 133 (link copy feedback), and
135 (restored owner). No browser opens, sends or account authorization changes.
Browser activation, remaining individual copy variants, and live deletion while
an action/card remains open were not exercised; code guards are not runtime proof.


## Replies implementation checkpoint — 2026-10-10

Linux offline locked build passed for the initial reply integration. Native Windows
build and runtime verification are pending. The next native pass must exercise
actual message Reply / View conversation, cancellation without draft loss, target
replacement, channel/workspace isolation, saved target after relaunch, compact
360×280 layout, retained jump and naturally arriving reply previews where available.
Use existing real traffic only; never send a message to validate the controls.
Outgoing delivery, live moderation redaction and unavailable-parent edge cases
remain unverified until directly observed. Restore owner geometry/layout/drafts.

## Native replies accepted — 2026-10-10 01:43 UTC

Windows exact da39e26b2c7767646a53d333adfbe3e2da157c96 fast-forwarded cleanly.
Rust 1.99 locked release build exited 0 in 16.91s. Actual native interactions:
- Selecting/replacing reply targets preserved draft text.
- × and Escape cancelled targets without erasing text.
- Channel/workspace switching kept draft and target isolated.
- Naturally arriving replies showed context; conversation, Jump and Reply worked.
- Reply target and conversation dialog fit 360×280.
- Find navigated reply rows; atomic selection/right-copy returned clipboard_verified.
- Controlled restart restored exact target and draft with outside-history label.
QA13 text/target cleared and workspace removed through normal confirmation.
Owner drafts, layout, sidebar, history cap and original bounds were restored;
both owner channels connected, overlays closed, no active drag/pointer owner.
Running session jj-manual-da39e26b-replies-restart, PID 45920, owned terminal hw-1-99.
No messages sent, URLs opened or authentication actions performed.
Outgoing delivery, real moderation/deletion and a thread with its original parent
retained remain untested. Build/runtime evidence is not full Chatterino parity.


## Attention and font QoL implementation checkpoint — 2026-10-10

Code adds upward reply arrow, per-pane unread/highlight references, visible-row
acknowledgement, compact badges, global retained activity, optional highlight words,
and Ctrl+wheel / keyboard font controls. Initial Linux build passed (10.00s).
Native acceptance is pending. Required bounded real-app checks: hidden channel and
workspace unread, inactive/background window if safely possible, returning to tail,
scrolled-back retention, explicit mark read, live-filter-hidden activity access,
activity jump, keyword highlights using naturally arriving traffic, compact layout,
Ctrl+wheel versus plain scrolling, reset and saved font/keyword settings. Preserve
owner's newly edited five-channel grouped workspace. No outgoing QA messages.
Unavailable real self-mentions/account switching/moderation must remain unverified.


### Stream marker polish — 2026-10-10 02:30 UTC

Pastel state markers and explanatory tooltips added to the pending attention/font
batch. Native validation remains blocked by Deadlink's no-project-selected error
and the desktop executor setup-refresh failure. No Windows pull or relaunch has
occurred; da39e26b is still the last-confirmed preview. The error does not establish
that selecting a Haiwire project is an intended connector requirement.

## 2026-10-10 — Windows recovery and Settings hit testing

- Deadlink recovered; checkout 535e6a7 was clean and built on Windows with Rust
  1.99.0, locked release, exit 0 in 20.24 seconds. Owned preview PID 60088,
  session `jj-manual-535e6a7c-attention`.
- Genuine HutchMF messages and upward reply-context glyph observed. Settings
  suppressed read acknowledgement; unread cleared after returning to visible tail.
- Native defect: clicking the highlight input could open a reply conversation
  underneath Settings. Added GPUI hitbox occlusion to the Settings card, including
  scroll isolation. Child controls remain interactive; background rows should no
  longer receive pointer input through the card. Native retest pending.
- No sends, authentication changes or moderation performed. Remaining font/activity
  checks and owner-state restoration are tracked by the existing native task.

### ec327c2 native follow-up and compact close correction

Windows locked release build passed (20.69s). Actual Settings typing/editing,
right-click and scrolling no longer activated underlying chat. Temporary highlight
word matched genuine HutchMF rows in transcript/activity. Activity shortcut,
All unread, Mark all read, dialog read suppression, tail clearing, hidden-channel
and sidebar badges, font shortcuts/reset/plain scrolling, pastel stream markers,
upward reply glyph and compact activity were observed. Restart and final cleanup
were still pending at this report.

At 360×280 the scrolling Settings Done label overlapped the footer and did not
close when clicked. Moved Done into a non-scrolling header; only the settings body
scrolls, and dismissal returns focus to the workspace. Linux build passed (3.88s);
native compact retest remains pending. No outbound chat or auth changes.

### 0cfe832 final native acceptance

Exact Windows revision 0cfe83251850745e10db5f02e7ab0a474826d0ed built with
Rust 1.99.0, locked release, exit 0 in 18.76 seconds; checkout clean.
At normal size and 360×280, Settings Done remained visible and dismissed before
and after scrolling. Input accepted typing without touching the chat draft; no
footer overlap observed. Earlier checks also verified font controls, custom-word
highlighting, All unread, Mark all read and saved-settings persistence.

QA14 was removed. Saved owner tabs, preexisting drafts, preferences and highlight
terms matched the original snapshot exactly; original geometry/sidebar restored,
jennybunnybean selected. Isolated preview PID 65088, session
`jj-manual-0cfe8325-settings`, left running. No sends, auth or moderation actions.
Activity Jump, inactive/offscreen acknowledgment and natural own-mention/reply
matching remain unverified. This is acceptance of named behaviors, not full parity.

## 2026-10-10 — Jenny purple-heart first-line alignment

Owner reported username movement beside a heart. Native read-only capture on
0cfe8325 showed JennyBunnyBean remained on the same line as Twitch static purple
heart 555555584, with ample width, but lower than the badge strip. Font14,
window1280×820, five retained messages, no active entrance effects. Evidence in
isolated profile: 167-jenny-heart-original.png and inspection JSON. Raw message
IDs/fragments/dimensions are not exposed by current inspection; no claims on them.

Cause: inline media text uses a minimum30px line box; badges used a fixed22px
box. Badge slots now share the actual first-line height and center their18px
images, for both plain and media rows and changed font sizes. Inline layout is
created once and shares its line-height helper with badge placement. No wrapping
or decoded-media sizing changes are claimed in this narrow fix. Native retest pending.

### b09e578 native alignment acceptance

Windows fast-forward checkout clean; Rust1.99 locked release exit0 in15.76s.
Genuine Summit incoming emote and plain rows showed badges centered with usernames
at14px and18px; no separate-line name or badge displacement observed. Evidence
170-b09e5780-summit-font14.png and171-b09e5780-summit-font18.png in isolated profile.
Restored jennybunnybean, font14,1280×820 and original sidebar; saved owner tabs,
drafts/preferences matched pre-update snapshot exactly. PID65220,
session`jj-manual-b09e5780-emote` left running.
Jenny's exact heart did not recur after restart and was not rechecked. Original
167 screenshot preserved. Decode-time relayout and narrow wrapping remain unverified.
No fabricated messages, sends, auth actions or unrelated QA.

## 2026-10-10 — B1 rich-message implementation, native pending

Typed GIF/Cheermote/unknown fragments, server classifications/rewards/Bits and notices
added. GIF cards, Cheer catalog/inline media, enlarged emotes and compact status accents
implemented. Existing bounded media workers/cache reused with a separate GIF budget.
Full GIF URL retained, credentials absent from media client, redirects disabled,
known-origin validation and explicit unavailable/loading fallbacks. No live rendering
claim for GIFs, Cheers, rewards or notifications until actual events are observed.

Official contracts audited: EventSub reference, Get Cheermotes, IRC GIF example and
Twitch changelog. GIF id/gif_id alias tolerated due documentation discrepancy. Native
access readiness confirmed at prior b09e578: owner workspace4, five channels,
jennybunnybean selected, font14, history10000, sidebarhidden, emptyhighlightwords,
livefilterenabled. No state mutation in readiness check.

Final Linux exact-source build passed, exit0 in3.54s. Source review corrections:
optional notice subscription installs after mandatory chat/redaction feeds and fails
independently with a visible partial-service status; metadata refresh preserves prior
successful categories; fragment-text mismatch copies original text only once; GIF
animation cache admission falls back to static under budget pressure with cooldown
instead of continuously evicting/reloading other visible animations.

## 2026-10-10 — B2 channel context, native pending

Added Ctrl+I/tab context-menu channel details, batched live metadata, title-copy
feedback and separate broadcast/chat state. Public room snapshots plus live settings
events normalize REST/EventSub duration fields. Optional room feeds follow mandatory
redaction subscriptions; delayed snapshots cannot replace newer updates. Unknown
room settings are explicit; sending remains server-authoritative. Current native
B1 checks run independently; no B2 native acceptance is claimed before its handoff.

B2 source review corrected Ctrl+I target resolution to focused visible pane, avoided
blocking chat startup with optional settings fetch via one bounded snapshot worker,
and seeded duplicate channel panes from existing connection/room state. Final Linux
build passed on this source; native checks remain pending.

### 317c9b4 B1 Windows result

Rust1.99 locked release exit0,36.39s, clean checkout. Real incoming chat,
badges/emotes, normal/360×280 layouts, drag selection and Select All copying passed
(clipboard_verified). Summit/Jenny connected without optional-notice warning.
One server notice was received among189 Summit messages; its rendered row was not
captured, so visual notice acceptance remains pending. No GIF/Cheer/reward/intro/
Power-up/gigantify events occurred; do not claim those rendered branches verified.
Drafts/replydrafts/preferences matched pre-update snapshot; font14/history10000,
sidebarhidden/livefilter/originalgeometry restored. Jenny restored and captured,
but a later inspection showed Summit without another control tab action; cause
unknown and possible owner change was preserved. PID17480,
session`jj-manual-317c9b4a-rich`. No sends/purchases/auth/moderation/source changes.

### Profile cards and composer tools source checkpoint

Added public profile metadata, bounded JPEG avatars, @recent-speaker completion
and session-local successful-send recall (Alt+Up/Down). Source review found and
corrected missing JPEG decoding, open-card retry recovery and retained-history
identity fallback. Exact locked offline Linux build passed (exit 0, 5.19 seconds).
Windows checks pending: real public avatar/details, compact card scrolling, mention
completion insertion/dismissal with unchanged drafts, and ordinary chat continuity.
Successful-send recall awaits an owner-originated send; no fake history or automated
chat transmission is authorized.

### 3f2af135 B2 Windows result

Rust 1.99 locked release passed (17.91 seconds), clean checkout. Real Summit title,
category, viewers, start and language appeared; viewer count refreshed. Copy title
matched exactly. Followers 60 minutes appeared in details and composer. Jenny
showed broadcast offline, chat connected and no public restrictions. Ctrl+I followed
the selected visible channel; Summit context-menu details targeted Summit while
Jenny stayed selected. At 360×280, body scrolling and Done worked.

A real Prime resubscription notice visibly rendered label, system text and chatter
message, with one notice recorded by inspection. GIF/Cheer/Power-up and live room
mode changes still await real samples. Latest owner geometry, selection, tabs,
drafts, reply drafts and preferences restored exactly. Preview PID32936, session
`jj-manual-3f2af135-details`; no sends, auth changes or source edits.

Composer review also corrected pending-send completion erasing a recalled draft,
explicitly advanced composer revision on history recall, and cleared histories for
closed panes on account changes. Final Linux source build passed as above.

### B3 live catalog source checkpoint

Anonymous object subscriptions and REST invalidation compile in the locked Linux
build. Source review corrected permanent-close retry, duplicate in-flight fetches
and initial SUBSCRIBE ACK reconciliation. No live EventAPI connection, rename/
remove event or native refresh acceptance is claimed yet. Windows inspection
should verify real subscription ACK status and unaffected emote/chat rendering;
external set changes require real samples rather than editing a streamer's set.

### Compact switcher source checkpoint

Ctrl+P switcher source builds on Linux. Native acceptance pending for keyboard
selection, filtered/empty results, different workspaces, hidden offline channels,
compact-height paging, correct composer focus and preserved drafts.

Switcher source review corrected Enter to one synchronous capture handler, avoiding
dialog confirmation also consuming it. Final locked offline Linux build passed
(exit 0, 3.63 seconds). Native focus/compact acceptance remains pending.

### 807de665 combined Windows result

Rust 1.99 locked release passed, exit 0, 20.76 seconds; checkout clean. Public
chatter avatar, creation date and bio appeared; compact profile scrolling and
dismissal passed. Enter and Tab inserted @kungfujo23 without a send; Escape kept
the prefix and dismissed suggestions. Ctrl+P found hidden offline Jenny, clicked
selection focused her composer, and keyboard selection returned to Summit. Both
normal and 360×280 layouts passed.

7TV live_updates=true with Live catalog updates status confirmed real subscription
ACKs; actual remote set-change invalidation remains untested. Real Twitch/7TV/BTTV
media continued. A genuine highlighted reward visibly rendered label, tinted row
and text and appeared in inspection. Successful-send recall and broadcaster-role
presentation remain untested. Final saved drafts, reply drafts, tabs and preferences
matched the latest snapshot exactly. PID60260/session jj-manual-807de665-switcher
left open; no sends, auth changes or source edits.

### Optional local timestamps source checkpoint

Adds Off/Hours-minutes/With-seconds settings, saved locally and defaulting off.
Uses parsed Twitch EventSub metadata time, shown in OS local timezone; hover includes
date and UTC offset. Unknown timestamps stay absent. Separate fixed-width gutter
preserves body/name selection offsets and existing copy format. Pane remeasurement
on setting changes includes retained closed panes. Also guarded pending-send draft
clearing against stale session results. Native timestamp acceptance pending.

Final timestamp-source locked offline Linux build passed (exit 0, 2.84 seconds).
Chrono0.4.45 was already in the lockfile; added as a direct desktop dependency.

### Own-account personal catalog source checkpoint

Source implements only public first-owned Personal set bootstrap for the signed-in
Twitch ID, with sender-scoped expansion and own picker entries. Review corrected
valid empty-set handling (emotes may be omitted), identity/flag validation, and
autocomplete refresh after account/catalog changes. Native catalog loading and
real personal-emote rendering remain pending; zero owned sets is valid absence.
Other-sender entitlements/paints are not implemented by this slice.

Personal request generations reject delayed A→B→A results as well as responses
from another current account. Enqueue paths share the failure backoff.

Final own-personal catalog locked offline Linux build passed (exit 0, 3.03 seconds).

### ff0504f4 timestamp Windows checkpoint

Rust1.99 locked release passed (exit0,20.24 seconds), clean checkout. Live chat
showed default Off, HH:mm and HH:mm:ss gutters at normal size. Restored Off and
closed Appearance; no original-property differences against the owner snapshot,
empty composer and no pending send. Preview PID63004/session
jj-manual-ff0504f4-time left open on Summit. Hover tooltip, compact timestamp layout
and copy exclusion remain unverified. Earlier stale-coordinate Done clicks did not
close the panel; clicking its actual current position worked. No product defect
is claimed from that stale-coordinate attempt.

### c3177eb5 Windows build and tooling boundary

Fast-forward and Rust1.99 locked release build passed, exit0,21.51 seconds; clean
checkout. PID65988/session jj-manual-c3177eb5-personal left open on Summit, empty
composer and no pending send. Saved owner state matched the snapshot exactly.
Personal catalog loading/count and current 7TV transport state remain unverified:
Deadlink returned INVALID_ARGUMENT, "terminal request failed: Haiwire's terminal
action limit for this session was reached; start a new turn". One supported new
turn retry returned the same error; no permissions/settings or alternate execution
routes were used to evade it. Prior807 subscription acceptance remains valid for
that earlier revision, not a claimed check of c3177eb5. No sends/auth/source edits.

### Passive 7TV entitlement slice — 2026-10-10

Cloud locked/offline chat-workbench build passed after implementation (7.14s).
Read-only source review found and fixed per-grant expiry and Reset fetch-state
cleanup. Full-set refetches replace sparse patch mutation; versions reject late
HTTP results. No new harness or artificial chat messages were added.
Windows build and real entitlement-event rendering are NOT yet run for this slice.
Existing native app remains c3177eb5; user is debugging Deadlink's terminal-action
limit. Do not retry that blocked execution route until access is restored.

### Passive 7TV badges and cloud recovery — 2026-10-10

The initial badge implementation passed a locked offline cloud build in 3.07s.
The cloud executor then restarted during publication and lost its local checkout
and build tools. GitHub still held 7004fae5. Badge changes were reconstructed on
that exact commit. A new build of this restored tree is required; the earlier
build is historical evidence, not acceptance of reconstructed bytes.

Read-only review identified and fixed a misplaced expiry field and incorrect
badge-definition expiry. Grant expiry is bounded; definitions survive until
capacity eviction/session reset. No fake chat traffic or test UI was added.
Windows badge animation, compact wrapping, copy/selection and sender mapping
remain unverified while the owner repairs Deadlink's terminal-action limit.

Recovered exact-tree locked/offline cloud build PASSED: Rust 1.99.0, 3m 48s, exit 0.
This includes all reconstructed badge source and corrected expiry fields.

### Windows native acceptance of the Oct 11 batch — 2026-10-11

Owner profile `JawjackManual-b92a50a5-20261009`, locked Rust 1.99.0 release builds,
control sessions plus `PrintWindow` screenshots. Saved account restored each launch.

Observed working (real UI, live Twitch traffic):
- Manual pause held the view while 46 rows arrived; header showed Paused; resume
  returned to Latest and cleared unread. Last-read divider appeared on departure.
- Scrolling into history: `n/10000 · History`, unread kept counting.
- Ctrl-click on a typed mention opened the **mentioned** chatter's card with their
  retained messages; Ctrl-click on a username opened that sender's card.
- Private note: unsaved draft survived close/reopen; Ctrl+S wrote workspace.json;
  two-step delete removed it from disk.
- Real send in the owner's own channel (Kappa, LUL) echoed and rendered; the
  Recently used collection recorded the emote. First verified delivered send.
- FFZ supporter catalog loaded live: 4 definitions, 41,553 grants; `ffz-badge:2`
  art decoded and rendered. FFZ room-role replacement art was not encountered.
- Drag-to-split created a two-pane layout; both composers independent.

Defects found and fixed in this pass (each re-observed after the fix):
- Animated 7TV badges/emotes over 120 frames failed (`frame_limit`) and left blank
  badge slots → frames are thinned preserving duration; 0 failures afterwards.
- Display names with trailing whitespace ("1Hugges :") → trimmed like Chatterino.
- Notes dialog moved Save/Delete under the cursor after saving → stable layout.
- Chatter cards showed raw ISO dates and badge set IDs → local date, badge titles + art.
- Jump button covered the centered last-read label → bottom-right, "N new" counting
  only unread rows below the viewport.
- 14 of 25 subscription collections stayed "Twitch channel <id>" with initials →
  batched Helix profile lookups; all names and avatars resolved.
- Own sent message counted as unread → excluded.
- Composer grew a "Sent" row, misaligning split composers → inline status.
- Two splits overflowed a 420 px window, close button clipped → 200 px pane
  minimum and truncating titles; verified at 436×328.
- Divider drawn above the first retained row → hidden there.
- All observed 7TV paints fell back → approximate rendering (6/7 drawn).

Not verified in this pass: hover/Alt-hover pause modes, sent-history browser UI,
FFZ room-role art, multi-account, long-session memory, reduced motion.
