# Chatterino parity checklist

From an audit of the Chatterino source (2026-10-12), each item checked against
Jawjack's code. Tick items as they land. Size: S/M/L. **[dead]** = depends on a
removed/private API; **[design]** = intentionally different in Jawjack.

Done before this audit: highlights (words, rules, badges, per-rule sound),
ignores, nicknames, custom commands, mod buttons, user card (moderate, follow
age, sub, pronouns), emote picker, 7TV/BTTV/FFZ, recent messages, logs,
spellcheck, streamer mode, live alerts, ping sounds, taskbar flash, link
previews, tab menu (reconnect, reload emotes, clear), saved filters, popouts
on top, emote size, alternating rows, provider toggles, hotkeys, guest reading,
Streamlink/player, Activity and Whispers dialogs, AutoMod queue, shared chat,
Chatterino import.

## A. Missing

### Chat display
- [x] Font family / weight choice (S)
- [ ] UI scale separate from chat text (M)
- [x] Username display: Username / Localized / Both (S)
- [x] Message separator lines (S)
- [ ] Collapse long messages after N lines (M)
- [x] Hide deleted messages; show/hide moderation & deletion lines (S)
- [x] Timeout/ban lines with duration for viewers; stack/combine (M) — an anonymous IRC connection runs beside EventSub for lengths (live and history), and carries chat while a channel's EventSub is down
- [x] Badge visibility toggles per category; animated 7TV badges (M)
- [ ] Chatterino supporter badges (S)
- [x] Join/part messages (via IRC) (M) — gathered every 5 s into "Joined: …" / "Left: …" lines; off by default
- [ ] Pinned message bar, /pin /unpin (M) [dead for viewers]
- [x] Chat notices when 7TV/BTTV emotes are added/removed/renamed (S)
- [ ] Inline whispers in channel splits (S)
- [x] Slow-mode / timeout send countdown (S)
- [x] Header: stream title, game, viewers, uptime toggles (S)
- [x] Emote options: animations off/when focused, images off, zero-width stacking, unlisted 7TV, remove spaces between emotes, combine bits (S each)
- [ ] 7TV paint options (paints, shadows); animated 7TV avatar (S)
- [ ] Emoji image styles (Twemoji, Apple, Google…) (M)
- [ ] Watch-streak header/highlight (S) [research]

### Windows, tabs, splits
- [x] Always on top for the main window (S)
- [x] Start with Windows; beta update channel; open cache folder (S)
- [ ] Overlay window: transparent, click-through, opacity (L)
- [ ] Tab menu: ~~Close others / to the right, Sort A–Z, Duplicate (workspace), Lock layout, highlight on new messages (Count new messages)~~ done; Hide all tabs (S)
- [ ] Tab position and style (M) [partly design]
- [x] Change channel of an existing split (tab menu → Change channel…)
- [ ] Input options: ~~send button, reply on hover, hide reply context, strip leading mention~~ done; hide when empty (S)
- [ ] Header: Subscribe, Mod view, Changelog, room modes menu, custom player URI (S each)

### Menus and links
- [x] Emote right-click: copy name, copy/open link, provider page (S)
- [ ] Link actions: incognito, open Twitch link as split, Streamlink from link, double-click, lowercase domains (S)
- [x] Web search of selected text, configurable engine (S) — Shift+right-click a selection (right-click still copies)
- [x] Username right-click behaviour (mention vs card, modifier) (S) — plain click opens the card; right-click: menu / mention / card; Shift+right-click = menu
- [x] "Go to message" from mentions (channel tag) and search (Jump); Copy message details

### Commands
- [x] /ignore /unignore /copy /streamlink /clearmessages /banid /requests /lowtrust (S each)
- [x] /monitor /unmonitor /restrict /unrestrict (S–M)
- [x] /poll /endpoll /cancelpoll /prediction /lockprediction /cancelprediction /completeprediction (S)
- /follow /unfollow [dead]; /raw, /fakemsg, /debug-*, /c2-* not needed

### Integrations
- [ ] Lua plugins (L)
- [ ] Image upload (M) — parked, see IMAGE_UPLOAD.md
- [ ] Kick (L)
- [ ] Generic IRC servers (L)
- [ ] Browser extension / native messaging (M)

## B. Partial
- [ ] Hotkeys: ~~streamer mode, emote picker, settings, sidebar, whispers, move tab, highlights~~ done (31 rebindable); run command, clip, Streamlink, chatter list, tab history, quit (M)
- [ ] Highlight rules: custom sound file, per-rule flash, show in mentions, free colour picker (S–M)
- [ ] Event highlights: whispers, own messages, elevated/hype chat, AutoMod, thread replies; per-kind sound/flash (M)
- [ ] User card: ~~block, ignore, open channel, warn, avatar card, follower count, custom timeout, emotes in recent messages~~ done; 7TV profile, pin card (M)
- [x] Custom command placeholders (S–M) — {my.*} {channel.id} {stream.*}, and {user.name} {user.id} {msg.id} {msg.text}: commands using them appear in the message menu
- [ ] Mod buttons: ~~any command~~ done (any /command, incl. custom commands); image icons (S)
- [x] Timestamps: 12-hour, custom format, hide while live (S)
- [x] Streamer mode: hide avatars, link previews, viewers/uptime, mod actions, restricted users, blocked terms; suppress live alerts (S)
- [ ] Live alerts: ~~followed channels not open as tabs~~ done; sound, flash, /live split (M)
- [x] Link previews: thumbnails; Twitch clip/VOD/channel cards (M)
- [ ] AutoMod: inline caught rows, blocked-term messages, own-held notice (M)
- [ ] Chat logs: ~~channel allow-list (tab menu → Log this channel), timestamp format~~ done; per-stream logs (S)
- [ ] Recent messages: ~~limit~~ done; ~~reconnect gap fill~~ covered by the IRC fallback, and live chat now waits for history; custom URL (S)
- [x] Repeated-message filter options (S) — how alike, window, count, same chatter only, own messages, repeats highlight
- [ ] Nicknames: regex, case sensitivity (S)
- [x] Completion options (S) — names only after @, comma after a leading name, add @, include the streamer; names complete as display names
- [x] Mentions: bold/coloured in the chatter's colour, detect names without @ (option), colourless names get a stable colour
- [ ] Settings search; restore backups (S)
- [ ] Spellcheck: more languages, personal dictionary (S)
- [ ] Theme: light / system / custom (M) [design]
- [x] /mentions /whispers /live as dockable tabs (Add channel or type the command)
- [x] Scroll speed, smooth scrolling (S) — mouse wheel; touchpads keep native scrolling
- [ ] Shared chat: badge on all messages; source routing (M)
- [ ] Twitch GIF messages toggle (S)
- [ ] Blocked users: display modes; hide already-shown messages (S)
- [ ] Multiple windows with workspaces; drag between windows (M)
- [ ] Exclude own messages from filters (S)
