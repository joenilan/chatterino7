# Reading and sent-message tools

Advances Chatterino parity RD01 (reading pause) and IN04 (sent history).

## Pause scrolling

- Each visible chat has a compact pause/resume control beside its search control.
- Ctrl+Space toggles manual pause in the focused channel.
- Appearance offers Manual, On hover, and Alt + hover. The choice is saved.
- Pause only stops following new rows. Incoming events, moderation updates,
  bounded retention and indexing continue normally.
- Manual and hover holds are independent. Leaving the chat cannot cancel manual
  pause; typing does not accidentally remove a hover hold.
- Hover release resumes latest only if the pause began at latest and the user did
  not scroll, navigate history or select text while paused.
- Latest/Resume clears pause and bypasses hover until the pointer exits once.
- Hidden or inactive panes clear transient hover, preserving manual pause.
- Ctrl+End resumes when transcript focus is active; composer text editing retains
  its normal end-of-document behavior.

The source-local scroller adapter retains GPUI Component 0.7.1 geometry and
virtualization with an Apache-2.0 notice and license. Held pause uses Normal
follow mode: GPUI's transient pause alone immediately re-engages at the tail.
Existing front eviction can still release the oldest selected history.

## Sent-message browser

The composer history icon or Ctrl+Shift+H opens searchable successful sends for
that channel. Newest entries appear first. Up/Down selects, Enter or click restores
text into the composer, and Escape closes. It never sends a message.

The previous draft is retained through input-history navigation. Alt+Down advances
through newer sent entries back to that draft. A chosen entry is revalidated against
the current account/session history. Account changes clear session history.

History remains memory-only, capped at 100 entries per channel. It does not add
persistent chat logging, backfill requests or failed-send records.

## Evidence

Locked Linux development compilation passed for this implementation. Native hover,
Alt transitions, selection, compact sizing and sent-history interaction still need
runtime acceptance. No new testing UI or automated runtime harness was added.
