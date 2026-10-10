# Quiet live-chat counters

A visible chat following the live tail shows only Latest in its header. Retained
row/capacity numbers remain available in History, where they provide useful
context rather than changing on every arrival.

Unread and highlight badges are suppressed for a visible, focused-window chat
following the tail with no Find, modal or setup view obscuring it. Hidden channel
tabs, other workspaces, history views and inactive windows retain their counts.
This applies consistently to channel tabs, workspace summaries and the titlebar
activity total.

Suppression is presentation-only. Messages are still acknowledged only after
actual rows are painted through the existing attention owner. No messages are
marked read early merely to eliminate a transient badge between arrival and the
next visibility acknowledgement.
