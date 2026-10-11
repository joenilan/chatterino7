# Import from Chatterino

Gear menu → **Import from Chatterino…** (also under Appearance & memory). Jawjack
reads the current Windows user's `%APPDATA%\Chatterino2\Settings` and shows a
preview before anything changes. Chatterino's files are only read, never written.
Existing Jawjack workspaces and settings are kept; imported items are added.

## What carries over

| Chatterino | Jawjack |
| --- | --- |
| Tabs with one Twitch split (`window-layout.json`, main window then popups) | One **Chatterino** workspace holding every channel as a tab; the selected tab stays selected. Duplicate tabs are merged and listed. |
| Tabs with several splits | Their own workspace with the same horizontal/vertical geometry (`flexh`/`flexv` weights). |
| Message and user highlights (`/highlighting/highlights`, `/highlighting/users`) | Highlight rules, plain or regex, case option, nearest Jawjack accent color. Plain phrases match whole words, as in Chatterino. |
| Nicknames (`/nicknames`) | Local nicknames, keyed by the stable Twitch user ID after a batched login lookup (requires sign-in). |
| Ignored phrases (`/ignore/phrases`) | Local ignores: block → hide message, otherwise replace text. Chatterino's default is case-sensitive. |
| Commands (`commands.json`) | Custom commands: `{1}`–`{9}`, `{1+}` → `{args}`, `{channel}`/`{channel.name}` → `{channel}`. |
| Timestamps (`/appearance/messages/showTimestamps`, `timestampFormat`) | Off / hours:minutes / with seconds; Chatterino's defaults (on, `h:mm`) apply when never changed. |
| Font size, scrollback (`currentFontSize`, `/misc/scrollback/splitLimit`) | Only when explicitly set: points → pixels (12–24 px), 500–10,000 messages. |

## What is reported as not imported

Mentions/whispers/live/automod/other special splits; badge highlights; highlight
blacklist; pattern (regex) nicknames; commands that run a Twitch `/command` or use
placeholders Jawjack does not have; items beyond Jawjack's caps (24 highlight rules,
32 commands, 128 ignored phrases). Twitch blocked users are account data, not part
of Chatterino's files (see AT07). Accounts and tokens in `settings.json` are never read.

## Many channels

Twitch EventSub allows about 150 live channels per account in Jawjack (see
[Live connections](LIVE_CONNECTIONS.md)). An import beyond that connects the
visible workspace first; the rest connect when there is room. Logins that no
longer exist on Twitch stay in the layout with an explanatory status.

## Evidence (Windows, 2026-10-11)

Owner install: 75 tabs → preview of 73 channels (2 duplicate tabs merged), 1
highlight rule, timestamps on, blocked-users note. Import created the workspace;
all 78 existing accounts of the 86 total channels connected (~80 s cold start);
8 logins no longer exist on Twitch. Chatterino keeps only names, so the owner identified the renames: stephsteinx3 →
yospacebabe, sircheetogg → sircheet0, comraderisloo → comraderuskii, justfocuszk →
flyzk, kaylowrl → kaylow_rl, schizzzrl → schizzlit and lydless → valyd__. Only
**thedreamlizard** is unresolved. Nickname lookup and multi-split tabs were not
present in this install and remain unexercised natively.
