# Event highlights and name completion

## Event highlights (gear menu → Event highlights, or Appearance & memory)

Each kind of chat event can be off or use one of the five highlight colours. Your
highlight rules win when both match. Defaults: **mentions and replies to you →
Mint (green)**, **raids → Mint**, subs/gifts → Lilac, Bits → Peach, channel point
rewards → Sky, announcements → Sky, first-time chatters → Rose.

- **Mentions:** @you, your login in text, replies to your messages.
- **Raids:** the raid notice, plus the raiding streamer's messages for 30 minutes.
  Twitch doesn't reveal which viewers arrived with a raid.
- **Subs:** sub, resub, gifted and community gifts, upgrades, pay-it-forward (also from
  Shared Chat).
- **First-time chatters:** Twitch's first-chat introduction messages. EventSub has
  no general first-message flag.

Saved locally as `event_highlights` in the workspace file. Highlights also count
towards the @ badge in the activity menu.

## Name completion

- Typing `@` lists names immediately: recent speakers first, then Twitch's chatter
  list (Get Chatters, refreshed every 2 minutes for visible channels you moderate or own).
- Typing filters by login or display name. Prefix matches rank above substring matches;
  up to 10 are shown. ↑/↓ choose, Tab or Enter inserts, Esc closes.
- **Tab on a bare word** completes emotes and chatter names (without `@`), as Chatterino does.

## Evidence (Windows, 2026-10-11)

`@` in #dreadedzombie listed the Twitch chatter list ("In chat"). `hey imp` + Tab
in #xqc completed to `hey imprabs`. The Event highlights dialog showed the defaults.
A live mention, raid or sub highlight has not been observed yet.
