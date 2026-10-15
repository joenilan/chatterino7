# Studio panel design

Streamer setup's right-side panel (300 px, toggled from the titlebar or with Ctrl+Shift+S). Every
new Studio feature has to fit this structure rather than add another card.

## Structure
1. **Header (fixed):** `#channel` plus a LIVE/OFFLINE tag with uptime. Below it, always exactly
   one status line: a chat problem (amber), otherwise viewers · category while live, otherwise
   "Chat connected". The line is always there, so the tabs never move.
2. **Tabs** (GPUI Kit segmented `TabBar`, choice saved):
   - **Activity** (default, the job while live): filter chips (All, Follows, Subs, Bits, Rewards, Tips)
     and the newest-first feed. A row is a kind icon in a colour-matched circle (with a tooltip),
     the name, the time, and a two-line detail; non-Twitch sources are prefixed. The tab label shows
     the unseen count.
   - **Stream:** stream info (title with counter, category, a compact search result list,
     Revert only when edited, Save as the single primary action) and **chat modes as switches**,
     one per row with a one-line explanation.
   - **Tools:** broadcast rows (icon, title, one-line hint, action on the right): marker,
     ad break (30/60/90 s, confirmed), raid (channel field, confirmed). Then **Polls & predictions**:
     a row each when nothing is running ("New…" opens a compact form with a title, 2–5 options for polls or
     2–4 outcomes for predictions, and a 1/2/5/10 min duration). While one runs, a card shows the title, the time left
     and result bars that ease to new shares (votes and % for polls; points and predictors for predictions),
     with End poll, or Lock and Cancel & refund. Once locked, each outcome gets a "pick winner" button with a
     payout confirmation. The state is re-read from Twitch every 3 s while running (30 s otherwise) and only
     while the Tools tab is open. Then Streamer.bot: one primary
     Connect when not set up; otherwise status, an even two-column grid of pinned actions, and a quiet footer
     (Edit pins · Disconnect).

## Visual rules
- Section labels: 10 px, uppercase, muted, 16 px above and 8 px below. Cards only for list items.
- One primary button per section, in the lilac accent with dark text (the theme sets both
  the colors and the kit's button tokens). Everything else is ghost, with icons from GPUI Kit.
- Help text: one short sentence, muted 11 px. No paragraphs.
- Disabled rather than hidden when an action needs you to be live, with the hint saying why.
- Motion: tab content fades in (150 ms); new feed items ease in (300 ms); the panel slides
  (200 ms). All of it respects reduced motion.

Polls/predictions evidence (Windows, 2026-10-11, in #dreadedzombie with the owner's go-ahead):
- **Poll:** "Jawjack test poll" (Yes/No, 1 min) started ("Poll started", card counting down 0:55), then
  End poll worked ("Poll ended", card showed Ended).
- **Prediction:** "Jawjack test prediction" (Works/Broken, 2 min) started, Lock showed
  "Locked · pick the winner" with per-outcome winner buttons, and Cancel & refund (confirmed) returned
  "Prediction canceled; points refunded".
- Resolving with a payout was not exercised.

## Stream presets (2026-10-11)

In Stream → Stream info, **Save as preset** stores the current title and category. Presets are named after the
category, numbered if that name is taken, and you can keep up to 12. They show as chips above the title.
Clicking a chip fills the form ("Loaded … Review, then Save."). Nothing changes on Twitch until
you press Save, which goes through the same conflict check and readback as a manual edit. The chip's ×
removes it, and the tooltip shows the full title and category. Presets are saved with the workspace, and the
Stream info dialog has them too.

Evidence: saved, applied (status shown, no Twitch write) and removed in the owner's profile, and
`stream_presets` went back to empty.

## Mod log (2026-10-11)

`channel.moderate` v2 (own-channel extra; subscription accepted live) adds a **Mods** chip to the
Activity feed: "Mod: banned Bob: spam", "timed out Ann for 10m", "turned on slow mode (30s)",
"deleted X's message: …", VIP/mod changes, raids, blocked terms, unban requests, and shared-chat
variants. The moderator who acted is the entry's name. No live moderation has been observed in the feed yet.

## Ad manager (2026-10-11)

While your channel is live (Twitch says so, or OBS is streaming), Helix Get Ad Schedule is polled every
60 s. The Ad break row in Tools shows "Next ad in 12:34 (90s) · 2 snoozes left", ticking each second, and has
a **Snooze** button (Helix Snooze Next Ad, +5 min, disabled with no snoozes left) beside the existing
30s/60s/90s run buttons. About a minute before an ad, a notification ("Ad break in under a minute") offers a heads-up
for chat. Clicking it puts the text into your channel's message box, and you choose whether to send it. Jawjack never
posts on its own. Times are accepted as RFC 3339 or Unix seconds.

Evidence: offline state renders ("Available while live"); parsing unit tested. Not yet observed live
(needs a live stream with ads).

## Hype strip (2026-10-11)

A card under the Studio header, shown only while something is happening:
- **Shield Mode** (red) and who turned it on.
- **Hype Train**: level, progress bar to the next level, and time left. A Golden Kappa train is shown in gold.
- **Creator goals** (follower/sub/Bits/cheerer, up to 4) with current/target.
- **Charity** campaign: raised / target in its currency.

Source: own-channel EventSub `channel.hype_train.*` v2, `channel.goal.*`, `channel.charity_campaign.*`
and `channel.shield_mode.*` (all accepted live, `own_extras_refused: []`). At sign-in Jawjack also loads what's already running: Get Creator Goals, Get Charity Campaign and Get Shield Mode Status. Verified with the owner's sub goal (11/250) and follower goal (2453/3900).

**Budget change:** only the socket carrying your own channel pays for its 17 extras. Each socket has
280 subscriptions for channels (20 reserved for moderated AutoMod), the own channel costs 6 + 17, and the cap is
3 × 46 − 3 = 135 channels. Verified: all 81 live channels connected.

## Go-live checklist (2026-10-11)

Top of Studio → Stream, shown before going live and for the first 10 minutes of a stream (×
hides it until the next stream). Automatic checks:
- title & category set (from Twitch, with the category name)
- OBS connected (only if the OBS connection is set up)
- mic on (from OBS)
- a moderator chatted in your channel in the last 15 min

Plus up to 12 of your own items (Enter adds one, a click ticks it, × removes it). Ticks reset when a stream ends, and
the items are saved with the workspace. "Live" means Twitch reports it or OBS is streaming.

Evidence: shown with title/category, OBS and mic ticked from live data and the mod check amber. A
custom item was added, ticked and removed, and the saved list went back to empty.

## Stream recap (2026-10-11)

When your stream ends (it must have run at least 5 minutes), a "Stream recap" window shows length, peak viewers,
chat messages and unique chatters in your channel, activity counts (follows, subs, Bits, raids,
rewards, tips), and the top five chatters. **Copy as text** is for Discord or notes. Peak viewers come from
Twitch's stream status sampled while you're live. Unit tested, but not yet seen at the end of a real stream.

## Live stats (owner request, 2026-10-11)

While you're live (Twitch reports it or OBS is streaming), a card at the top of Studio, above the tabs, shows:
- viewers (with this stream's peak) and uptime,
- chat messages per minute, the busiest minute, and a 30-minute sparkline (one bar per minute) from your
  channel's messages,
- this stream's follows, subs, Bits events and raids (from the Activity feed since going live),
- OBS bitrate, dropped frames and fps when OBS is connected.

It refreshes every 5 s while Studio is open. The pace bucketing is unit tested, but the card hasn't been seen during a live stream yet.

## Raid finder (2026-10-11)

Under the Raid box in Tools, up to 8 live channels you follow (from the LIVE NOW list, most viewers first)
are listed with category and viewers, and the title shows on hover. Clicking one fills the Raid box, and Raid still asks
for confirmation. Verified: the list shows and a click fills the box (no raid was started).

## Raid countdown and Twitch control — 2026-10-14 source build

After a successful `/raid target` (or Studio raid request), the originating
composer shows a **locally estimated** 90-second countdown, a **Raid Now on
Twitch** button, and **Cancel**. The first button opens the broadcaster's
Twitch Stream Manager in a browser. It is deliberately *not* a Jawjack
"raid immediately" API call: the
[official Twitch raid API](https://dev.twitch.tv/docs/api/raids)
exposes queue/start and cancel; only Twitch's own UX offers Raid Now.
Cancel sends the already-supported authorized `DELETE /raids`.
The banner clears on a successful cancel or once its local estimate expires.
This is **not** actual raid confirmation; the official way to confirm a raid
is the separate `channel.raid` EventSub event, which Jawjack does not
currently subscribe to for outbound raid completion.
The running installed app has **not** been updated, and no live raid or cancel
was exercised in testing.

## Channel points (2026-10-11)

Studio → Tools → Channel points lists every reward with its cost. Twitch only lets the app that created a
reward change it, so rewards made on the Twitch dashboard are read-only and show a lock that explains why.
**New reward** (name, cost, description, require a message) creates a reward from Jawjack. Jawjack rewards can
be paused or resumed with a switch and deleted with ×, and their unfulfilled redemptions appear at the top
with **Complete** / **Refund**. The list refreshes every 60 s while Tools is open and right after a redemption
arrives (EventSub). Evidence: the owner's 45 dashboard rewards listed as locked. Not exercised: creating,
pausing or deleting a reward, or completing a redemption.

