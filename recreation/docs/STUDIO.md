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

