# Chat logs (PF04, part)

Settings → Chat → **Save chat logs**. It's off by default. When on, Jawjack writes one UTF-8 text file per
channel per day:

`<folder>/<channel>/<channel>-YYYY-MM-DD.log`, with lines in Chatterino's format:
`[12:34:56] Name: message`, or `[12:34:56] 名前 (login): message` when the display name differs
from the login. Notices (subs, raids, announcements) are logged as their notice text.

- Moderation is logged too: "A moderator deleted X's message: …", "X was timed out or banned; their
  messages were cleared.", "Chat was cleared by a moderator."
- Only live events are logged. Recent history loaded when a channel opens isn't, so restarts don't
  duplicate lines.
- **Folder:** defaults to `Documents\Jawjack Logs`. Change… picks another, and Open shows it in
  Explorer. Channel names must be valid Twitch logins before they're used as path parts.
- Writing happens on its own thread. Files stay open until the day or folder changes, and are reopened if
  deleted meanwhile.

Not yet: per-channel allow-list, retention/cleanup, and logging whispers.

Evidence (Windows, 2026-10-11): turned on for about a minute, it created per-channel daily files
(alphakep, xqc, …) with `[HH:MM:SS] name: text` lines. It was then turned off and the test folder removed.
