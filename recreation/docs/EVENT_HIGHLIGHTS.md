# Event highlights and name completion

## Event highlights (gear menu → Event highlights, or Appearance & memory)

Each kind of chat event can be off or use one of the five highlight colours (Lilac, Mint, Sky, Rose, Peach, Red). Your highlight rules win when
both match. Defaults follow Twitch: **mentions and replies to you → Red**, **raids →
Mint (green)**, subs/gifts → Lilac, Bits → Peach, channel point
rewards → Sky, announcements → Sky, first-time chatters → Rose.

- **Mentions:** @you, your login in text, replies to your messages.
- **Raids:** the raid notice, plus the raiding streamer's messages for 30 minutes.
  Twitch's website marks individual viewers who arrived with a raid. No public
  API documents that marker; Chatterino doesn't show it either. If it ever shows up
  as a badge in EventSub message badges, Jawjack will show it like any badge.
  To be checked during a real raid.
- **Subs:** sub, resub, gifted and community gifts, upgrades, pay-it-forward (also from
  Shared Chat).
- **First-time chatters:** Twitch's first-chat introduction messages. EventSub has
  no general first-message flag.

Saved locally as `event_highlights` (version 2; profiles saved with the earlier green
mention default are moved to red) in the workspace file. Highlights also count
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
