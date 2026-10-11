# Local ignores

Gear menu → **Ignores**. Applied as messages arrive; messages already shown stay.
Nothing is sent to Twitch, and Twitch account blocks are separate (AT07).

- **Chatters:** hide every message from a Twitch login (up to 256).
- **Phrases:** plain text or regex, case-sensitive or not (up to 128). Each either
  hides the whole message or replaces the matched text (empty replacement removes
  it). Hiding matches the whole original message, emote codes included, as
  Chatterino does; replacement edits text only, so emotes and mentions stay intact.
- Regexes compile with bounded size limits. Ignores are saved in the profile's
  workspace file and can be imported from Chatterino.

Windows 2026-10-11: a replace phrase (`e` → `3`) rewrote live #xqc text
case-insensitively while names, emotes and mentions were untouched; removing it
updated the saved profile. Chatter ignores and hide-phrases are not yet exercised natively.
