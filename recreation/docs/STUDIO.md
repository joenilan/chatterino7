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
     ad break (30/60/90 s, confirmed), raid (channel field, confirmed). Then Streamer.bot: one primary
     Connect when not set up; otherwise status, an even two-column grid of pinned actions, and a quiet footer
     (Edit pins · Disconnect).

## Visual rules
- Section labels: 10 px, uppercase, muted, 16 px above and 8 px below. Cards only for list items.
- One primary button per section. Everything else is ghost, with icons from GPUI Kit.
- Help text: one short sentence, muted 11 px. No paragraphs.
- Disabled rather than hidden when an action needs you to be live, with the hint saying why.
- Motion: tab content fades in (150 ms); new feed items ease in (300 ms); the panel slides
  (200 ms). All of it respects reduced motion.
