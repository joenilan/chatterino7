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
- [x] UI scale separate from chat text — done (Appearance › UI scale, 75–150%; a zoom in the vendored gpui window)
- [x] Username display: Username / Localized / Both (S)
- [x] Message separator lines (S)
- [x] Collapse long messages after N lines (M) — Appearance → Collapse long messages (Off, 2, 3, 5, 10) with Show more
- [x] Hide deleted messages; show/hide moderation & deletion lines (S)
- [x] Timeout/ban lines with duration for viewers; stack/combine (M) — an anonymous IRC connection runs beside EventSub for lengths (live and history), and carries chat while a channel's EventSub is down
- [x] Badge visibility toggles per category; animated 7TV badges (M)
- [x] Chatterino supporter badges (S) — from api.chatterino.com; Appearance → Badges → Chatterino
- [x] Join/part messages (via IRC) (M) — gathered every 5 s into "Joined: …" / "Left: …" lines; off by default
- [ ] Pinned message bar, /pin /unpin (M) [dead for viewers]
- [x] Chat notices when 7TV/BTTV emotes are added/removed/renamed (S)
- [x] Inline whispers in channel splits (S) — Chat → Whispers in chat (off by default; not yet seen with a real whisper)
- [x] Slow-mode / timeout send countdown (S)
- [x] Header: stream title, game, viewers, uptime toggles (S)
- [x] Emote options: animations off/when focused, images off, zero-width stacking, unlisted 7TV, remove spaces between emotes, combine bits (S each)
- [x] 7TV paint options (paints, shadows); animated 7TV avatar — done (Appearance → 7TV; avatars on user cards)
- [x] Emoji image styles: ~~System, Twitter, Google, Apple, Facebook~~ done
- [x] Watch-streak highlight — done (Event highlights › Watch streaks)

### Windows, tabs, splits
- [x] Always on top for the main window (S)
- [x] Start with Windows; beta update channel; open cache folder (S)
- [ ] Overlay window: transparent, click-through, opacity (L)
- [x] Tab menu: Close others / to the right, Sort A–Z, Duplicate (workspace), Lock layout, highlight on new messages (Count new messages), Hide all tabs (Ctrl+U)
- [ ] Tab position and style: ~~position (top, bottom, left, right)~~ done; tab style (S) [design]
- [x] Change channel of an existing split (tab menu → Change channel…)
- [x] Input options: ~~send button, reply on hover, hide reply context, strip leading mention, hide when empty~~ done
- [x] Header: ~~Subscribe, Mod view, room modes menu, changelog (Settings › Updates › Release history), custom player~~ done

### Menus and links
- [x] Emote right-click: copy name, copy/open link, provider page (S)
- [x] Link actions: ~~open Twitch link as a tab, Streamlink from link, click / double-click to open, lowercase domains, open in a private window~~ done
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
- [x] Hotkeys: ~~streamer mode, emote picker, settings, sidebar, whispers, move tab, highlights, hide tabs, chatter list, Streamlink, open in browser, player pop-out~~ done (39 rebindable); ~~clip (Alt+X), last channel (Alt+`), save and quit (Ctrl+Q)~~ done; ~~run a command (4 slots)~~ done
- [x] Highlight rules: ~~per-rule flash, show in mentions, custom sound file, free colour (#RRGGBB)~~ done
- [ ] Event highlights: ~~own messages, thread replies (as mentions), whispers, AutoMod, per-kind sound, per-kind flash~~ done; elevated/hype chat (not parsed yet) (S)
- [x] User card: ~~block, ignore, open channel, warn, avatar card, follower count, custom timeout, emotes in recent messages, 7TV profile, pin card (own window)~~ done
- [x] Custom command placeholders (S–M) — {my.*} {channel.id} {stream.*}, and {user.name} {user.id} {msg.id} {msg.text}: commands using them appear in the message menu
- [ ] Mod buttons: ~~any command~~ done (any /command, incl. custom commands); image icons (S)
- [x] Timestamps: 12-hour, custom format, hide while live (S)
- [x] Streamer mode: hide avatars, link previews, viewers/uptime, mod actions, restricted users, blocked terms; suppress live alerts (S)
- [x] Live alerts: followed channels not open as tabs, /live tab, sound and taskbar flash (Notifications → With each live alert; not yet seen on a real go-live) (M)
- [x] Link previews: thumbnails; Twitch clip/VOD/channel cards (M)
- [ ] AutoMod: ~~inline caught rows with Allow/Deny (and the outcome after)~~ done; own-held notice (S)
- [x] Chat logs: channel allow-list (tab menu → Log this channel), timestamp format, a file per stream (S)
- [x] Recent messages: ~~limit~~ done; ~~reconnect gap fill~~ covered by the IRC fallback, and live chat now waits for history; ~~custom URL~~ done
- [x] Repeated-message filter options (S) — how alike, window, count, same chatter only, own messages, repeats highlight
- [x] Nicknames: ~~regex, case sensitivity~~ done (Settings › Nickname rules; imported from Chatterino)
- [x] Completion options (S) — names only after @, comma after a leading name, add @, include the streamer; names complete as display names
- [x] Mentions: bold/coloured in the chatter's colour, detect names without @ (option), colourless names get a stable colour
- [x] Settings search; restore backups (S) — search across all sections; daily backups (last 7) with restore in General
- [ ] Spellcheck: ~~personal dictionary~~ done (+ beside a flagged word); more languages (S)
- [ ] Theme: light / system / custom (M) [design]
- [x] /mentions /whispers /live as dockable tabs (Add channel or type the command)
- [x] Scroll speed, smooth scrolling (S) — mouse wheel; touchpads keep native scrolling
- [ ] Shared chat: badge on all messages; source routing (M)
- [x] Twitch GIF messages toggle (S) — Appearance → Twitch GIF messages (not yet seen with a real GIF message)
- [x] Blocked users: ~~hide already-shown messages, display modes (hide / placeholder / show)~~ done
- [ ] Multiple windows with workspaces; drag between windows (M)
- [x] Exclude own messages from filters (S) — Filter view → Always show my own messages
