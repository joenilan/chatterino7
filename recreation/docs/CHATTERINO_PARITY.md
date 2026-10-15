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

## Current open parity work (reviewed 2026-10-14)

The checkboxes below track **implemented feature presence**, not every Twitch
contract edge or native/live acceptance. Preview.13 added custom/automatic
channel-point notifications, connection retry and tab navigation; preview.14
added the mentions-only notification bell. A **real redemption arriving in
Studio has not yet been observed**, so do not mark event reliability complete.

**First next-chat batch:** source-backed EventSub optional-subscription
diagnostics, reward visibility and native/reconnect evidence (AC04, TW06,
WS10). Then research the sender-held AutoMod outcome and elevated/hype-chat
notices (MD09, AT04, TW08). Later: combined-feed source-appropriate controls,
rich media/DPI/selection acceptance and the unchecked long-term integrations.
See [PARITY_QUEUE_2026-10-14.md](PARITY_QUEUE_2026-10-14.md) and
[HANDOFF_2026-10-14.md](HANDOFF_2026-10-14.md).

**2026-10-14 P0 source slice:** built/tested per-kind optional EventSub
inspection, last-delivered reward counters, refused-reward banner, retry/gap
reporting, and missing-ID rejection. The original unchecked native/live
redemption case stays unchecked. Slash-command auto-repeat suppression and
a raid countdown with actual Cancel were also added. The browser-only
"Raid Now" shortcut was removed before release because it did not initiate
early completion. Composer Enter was verified in isolated native QA: first Enter executes local
`/uptime` despite default suggestions, repeated Enter stays empty, and
Tab or deliberate Arrow+Enter still completes. A repeat-send audit also
fixed a wire-only invisible suffix incorrectly preventing acknowledged
messages from clearing the original draft (unit tested; no live repeat
attempted). Connected Twitch chat sends and the raid banner remain unverified;
a real raid needs separate approval.

## A. Missing

### Chat display
- [ ] **Full-width soft-wrapped continuation lines (P0, 2026-10-15):** timestamp, badges, and username share the first visual line; subsequent lines start at normal chat-row left padding instead of remaining indented under the metadata column. Preserve both rich emotes and plain text, copy/selection, links, hitboxes, accessibility, compact panes and scrolling. [Owner request and acceptance plan](HANDOFF_2026-10-15.md).
  - **Source/test slice implemented 2026-10-15, unpublished:** metadata overlaid only on first line, shared plain/media line wrapper plus typed-notice adaptation, and four geometry tests. Windows desktop **111 passed / 5 ignored**; isolated native instance inspected, but narrow-width visual/interaction acceptance was blocked. Keep this unchecked until real UI evidence, selection, link, media, accessibility and resize tests are observed. [Validation notes](VALIDATION.md).
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
- [x] Overlay window: ~~transparent, click-through (lock), opacity~~ done (tab menu › Layout › Open as overlay)
- [x] Tab menu: Close others / to the right, Sort A–Z, Duplicate (workspace), Lock layout, highlight on new messages (Count new messages), Hide all tabs (Ctrl+U)
- [x] Tab position and style: ~~position (top, bottom, left, right), style (normal / compact)~~ done
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

- [ ] **Real channel-point/live-event acceptance (AC04/TW06):** verify Studio
  gets a naturally arriving custom/automatic redemption and shows actionable
  refused/reconnecting subscription states. Source/unit tests are present,
  live redemption evidence is pending. Never purchase/redeem merely to test.
- [ ] **Per-origin combined-feed controls (WS11/TW09):** room restrictions,
  usercards/chatters and moderation actions must use the actual source channel,
  never the combined display ID.
- [x] Hotkeys: ~~streamer mode, emote picker, settings, sidebar, whispers, move tab, highlights, hide tabs, chatter list, Streamlink, open in browser, player pop-out~~ done (39 rebindable); ~~clip (Alt+X), last channel (Alt+`), save and quit (Ctrl+Q)~~ done; ~~run a command (4 slots)~~ done
- [x] Highlight rules: ~~per-rule flash, show in mentions, custom sound file, free colour (#RRGGBB)~~ done
- [ ] Event highlights: ~~own messages, thread replies (as mentions), whispers, AutoMod, per-kind sound, per-kind flash~~ done; elevated/hype chat (not parsed yet) (S)
- [x] User card: ~~block, ignore, open channel, warn, avatar card, follower count, custom timeout, emotes in recent messages, 7TV profile, pin card (own window)~~ done
- [x] Custom command placeholders (S–M) — {my.*} {channel.id} {stream.*}, and {user.name} {user.id} {msg.id} {msg.text}: commands using them appear in the message menu
- [x] Mod buttons: ~~any command, icons (:ban, :timer, :gavel…)~~ done
- [x] Timestamps: 12-hour, custom format, hide while live (S)
- [x] Streamer mode: hide avatars, link previews, viewers/uptime, mod actions, restricted users, blocked terms; suppress live alerts (S)
- [x] Live alerts: followed channels not open as tabs, /live tab, sound and taskbar flash (Notifications → With each live alert; not yet seen on a real go-live) (M)
- [x] Link previews: thumbnails; Twitch clip/VOD/channel cards (M)
- [ ] AutoMod: ~~inline caught rows with Allow/Deny (and the outcome after); own-held notice (Twitch's reason shown when a send is held)~~ done; approved/denied follow-up for your own held message (needs channel.chat.user_message_update per channel) (S)
- [x] Chat logs: channel allow-list (tab menu → Log this channel), timestamp format, a file per stream (S)
- [x] Recent messages: ~~limit~~ done; ~~reconnect gap fill~~ covered by the IRC fallback, and live chat now waits for history; ~~custom URL~~ done
- [x] Repeated-message filter options (S) — how alike, window, count, same chatter only, own messages, repeats highlight
- [x] Nicknames: ~~regex, case sensitivity~~ done (Settings › Nickname rules; imported from Chatterino)
- [x] Completion options (S) — names only after @, comma after a leading name, add @, include the streamer; names complete as display names
- [x] Mentions: bold/coloured in the chatter's colour, detect names without @ (option), colourless names get a stable colour
- [x] Settings search; restore backups (S) — search across all sections; daily backups (last 7) with restore in General
- [x] Spellcheck: ~~personal dictionary (+ beside a flagged word); 10 languages (downloaded on first use)~~ done
- [x] Theme: ~~dark / light / follow Windows~~ done; custom colour themes not planned
- [x] /mentions /whispers /live as dockable tabs (Add channel or type the command)
- [x] Scroll speed, smooth scrolling (S) — mouse wheel; touchpads keep native scrolling
- [x] Shared chat: ~~source avatar on messages from other channels; deletes routed to the source channel~~ done
- [x] Twitch GIF messages toggle (S) — Appearance → Twitch GIF messages (not yet seen with a real GIF message)
- [x] Blocked users: ~~hide already-shown messages, display modes (hide / placeholder / show)~~ done
- [x] Multiple windows: ~~a workspace in its own window (full layout, reopened at launch); tab menu › Move to workspace moves channels between windows~~ done (no cross-window drag: GPUI drags stay in one window)
- [x] Exclude own messages from filters (S) — Filter view → Always show my own messages
