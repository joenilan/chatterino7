# Combined feeds (WS11, part)

Add channel (Ctrl+K) accepts `a+b+c` or `a, b, c` (2 to 8 channels). That makes one pane that shows those
channels interleaved in arrival order.

- **Origin tag:** each message starts with a small `#channel` chip, colour-keyed per channel.
- **Emotes:** each message uses its own channel's 7TV/BTTV/FFZ set.
- **Sending:** a "to #channel" picker in the message box chooses one destination. A reply goes to the replied-to
  message's channel. Moderation and Twitch commands go to the chosen destination too. Nothing is ever sent
  to all of them.
- **Connection:** shared with the normal subscriptions for those channels; no extra connection. The pane
  counts as connected while any source is, and as live (for "live channel tabs") unless all are offline.
- **Clears:** a chat clear in one channel doesn't wipe the combined pane. Deletions and timeouts apply to
  that channel's messages.
- **Saved** like a tab (name `+a,b`, never a valid Twitch login, so it can't reach Twitch as a channel;
  re-validated on load).

Not yet: room modes, chatter list and moderation buttons per message origin; recent-history fill.

Evidence (Windows, 2026-10-11): `xqc+summit1g` showed 35 interleaved messages in 30 s with
#xqc/#summit1g chips and "to #xqc". The test workspace was then closed and the owner's workspaces were unchanged.
