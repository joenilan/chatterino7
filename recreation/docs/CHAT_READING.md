# Reading and sent-message tools

Advances Chatterino parity RD01 (reading pause), RD09 (read markers),
AT01–02 (unread/highlight navigation), and IN04 (sent history).

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

## Catch-up and highlight navigation

Right-click a channel tab and open Reading & highlights:
- First unread / last read jumps to the oldest retained unread message, or the
  frozen last-read boundary when unread has already been acknowledged.
- Previous / Next highlight cycles through retained highlights, including ones
  already read. Alt+PageUp and Alt+PageDown are channel shortcuts.
- Ctrl+Home performs the unread/last-read jump when the transcript is focused;
  composer editing keeps its usual Ctrl+Home behavior.

A quiet Last read position divider marks where new messages began when leaving
live reading. It survives ordinary unread acknowledgment and is updated on the
next departure. Explicit Mark read dismisses it. Pruned boundaries disappear
rather than moving to a misleading replacement. Account changes reset it.

Catch-up jumps hold scrolling until Resume/Latest, so a short history cannot
immediately re-enter live following and acknowledge content unintentionally.
Targets are re-resolved from retained, nondeleted rows. Deletions and history
limits remain authoritative; there is no stored body copy or remote backfill.

Navigation gently accents the target for 1.2 seconds. Reduced motion keeps the
accent static. Rapid repeated jumps cannot let an earlier cleanup timer erase a
new target. Thread/activity jumps also receive this location feedback.

## Evidence

Locked Linux development compilation passed for this implementation. Windows 2026-10-11:
manual pause/resume, last-read divider, history counting and compact splits were
observed working (VALIDATION.md). Hover and Alt-hover modes, selection while paused
and the sent-history browser still need runtime acceptance. No new testing UI or automated runtime harness was added.
