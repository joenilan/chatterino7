# Streamer mode (PF03, part)

Settings → Streamer mode. Modes: **Off**, **On**, and **Automatic** (the default). Automatic turns on
while broadcasting software runs: OBS (obs64/obs32/obs), Streamlabs (OBS/Desktop), XSplit, vMix,
Twitch Studio, PRISM Live Studio or Meld Studio. A background thread runs `tasklist` every 15 s with
no console window.

While it's active:
- A **Streamer mode** pill appears in the titlebar (click it for the settings page).
- **Private chatter notes:** the notes editor won't open, and Jawjack explains why.
- **Whisper notifications** say "New whisper" with no sender or text (clicking it opens Whispers,
  which you chose to open).
- **Stream thumbnails:** the channel-header hover card shows title, game and viewers but no
  preview image.
- **Mute ping sounds** (optional, off by default): mention and highlight pings stay off desktop
  audio.

Not yet: hiding viewer counts, restricted/suspicious-user details, and per-item toggles.

Evidence (Windows, 2026-10-11): with OBS not running, Automatic stayed off. Setting it to On showed the
titlebar pill, and "Private chatter note…" was refused with the explanation. The mode was then set back to
Automatic. The process-list parser is unit tested. Not yet observed: automatic activation with OBS running.
