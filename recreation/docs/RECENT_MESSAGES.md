# Recent messages on open (AC10)

When a channel opens, Jawjack loads up to 500 recent messages from
`recent-messages.robotty.de`, the service Chatterino uses. Visible channels are
loaded first, then the rest of the workspace in connection order. Each open channel is
loaded once.

- **Parsing:** IRC lines are converted into the EventSub event shape and built by the same
  code as live chat (`live::chat_message`). That covers badges, colours, Twitch
  emotes by position, 7TV/BTTV/FFZ expansion, replies, Bits, highlighted and intro
  messages, sub/raid/announcement notices (`system-msg`), and `/me`.
  CLEARCHAT/CLEARMSG in the history redact the affected rows.
- **Display:** history rows are dimmed (60% opacity), don't animate, and never count as
  unread. Highlights still mark them.
- **Overlap with live chat:** history is applied only while a pane is still empty, so live
  messages that arrived first are never duplicated or reordered. Prepending to a
  non-empty pane is not implemented.
- **Privacy:** Appearance & memory → "Load recent messages" (on by default, as in
  Chatterino) explains that the service learns which channels you open.

Evidence (Windows, 2026-10-11): #thewisegrape opened with 500 dimmed history
rows (badges, Twitch and 7TV emotes, replies) followed by live rows; unread stayed 0.
One-word messages (IRC trailing without a colon) were first shown empty; fixed and
re-observed.
