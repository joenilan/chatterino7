# Ping sounds (AT05, part)

Settings → Notifications → Sounds. Defaults follow Chatterino: on for mentions, off for other
highlights, and silent while Jawjack is the active window.

- **Mentions:** your login in the text, an @mention of you, or a reply to you.
- **Highlights (opt-in):** your highlight words and highlight rules. Event colours (subs,
  raids, first messages, etc.) never make a sound.
- Never for your own messages, deleted messages, or recent-history rows loaded when a channel opens.
- The same message open in several panes pings once (de-duplicated by message ID), and pings closer
  than 700 ms merge into one.
- **Sound:** Chime (two rising notes) or Soft (one low note). Both are synthesized in code, so no
  audio files ship. You can also pick your own WAV file (4 MB at most; Jawjack checks it can decode the file before using it.
  If the file later becomes unreadable, the chime plays instead). Choosing a sound plays it once.
- **Volume:** Quiet, Normal or Loud, relative to the system volume.
- Playback runs on its own thread (`rodio`, WAV only). The output device opens on first use and is
  reopened if it disappears.

Evidence (Windows, 2026-10-11): the settings page renders; choosing Soft and then Chime ran without
error and saved `{"tone":"chime","volume":0.6}`. Tone synthesis is unit tested. Not yet: hearing a
real mention ping (it needs someone else to mention the owner while another window is in front).
Per-rule and per-channel sounds and taskbar flashing are still to do.
