# Popout windows (WS08, part)

Right-click a channel tab → **Pop out to its own window**. The channel opens in a separate
420×720 window with a slim title bar (drag area, minimise, maximise, close).

- The popout is a full chat pane with its own message box, search, emote picker, replies, moderation,
  AutoMod strip and "send as". It's fed by the same live connection as the tabs: no extra
  connection, and popout channels get top connection priority.
- It opens with the chat already loaded in the tab: retained messages are copied, without
  pings and marked read. A popout whose channel isn't in any tab loads recent messages instead.
- One popout per channel: choosing Pop out again brings the existing window forward.
- Closing the popout (window close or its pane's ✕) closes only the popout; the tab keeps
  the channel.
- **Saved:** popouts still open when Jawjack closes reopen on the next launch (at most 8).
  Closing the main window quits the app and closes its popouts.
- Unread and highlight counts in the title bar and sidebar come from tabs only, so a popout doesn't
  double-count.

Not yet: always-on-top (the GPUI Windows backend only offers a borderless, non-resizable topmost
pop-up), remembered window size and position, dragging tabs between windows, and whole-workspace
popouts.

Evidence (Windows, 2026-10-11): popping out #dreadedzombie opened a second window
showing the tab's history and a working composer area. The channel was saved to `popouts`, and closing
the window removed it (`popouts: []`) while the app kept running.
