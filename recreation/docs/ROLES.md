# Viewer, Moderator and Streamer setups (implemented slice)

Gear menu → **Choose your setup…** (offered once in the status bar on existing
installs). The setup only changes presentation; every action is still checked
against the channel role and Twitch permissions (`capabilities.rs`) when shown and
again on dispatch. Switching setups keeps channels, drafts and layouts.

| Setup | What changes |
| --- | --- |
| Viewer | Nothing extra: tools stay out of the way. The right-click mod actions still appear where you moderate. |
| Moderator | 🛡 in the header of channels you moderate or own: chat-mode toggles with live state, and Clear chat with confirmation. |
| Streamer | Moderator tools, plus your own channel pinned (📌) as the first tab of the primary group in **every** workspace, plus the **Studio** panel. |

## Pinned tabs (any setup)

Right-click a channel tab → **Pin tab** / **Unpin tab**. Pinned tabs:
- show the pin icon;
- stay at the front of their group, behind the Streamer anchor (`Dock::pin_front`, applied
  on every update);
- can't be closed until they're unpinned, and the notice says how;
- are never hidden by "Only show live channel tabs".

Pins are saved per workspace (`pinned` in the workspace file).

## Streamer anchor

- `Dock::pin_first` puts the own channel first in the primary group, the first leaf or deck
  in layout order. The tab you were looking at stays selected.
- `ensure_anchor` runs after every pump for every workspace, so new workspaces, drags
  and closes all converge back to the rule. It adds the channel if a workspace lacks it.
- The pinned tab can't be closed (middle-click, ×, or the menu). A notice explains that
  switching to Viewer unpins it. The live-only filter never hides it.
- Not yet: an account-level shared draft for the pin across workspaces (each
  workspace keeps its own draft), and a drag preview explaining the pin.

## Studio panel (right side)

Toggle it with the panel icon right of the settings gear (panel-right open/close),
**Ctrl+Shift+S**, or gear menu → "Studio panel". The choice is saved.

- Live/offline, uptime, viewers and category for your channel, plus its chat connection.
- **Stream info:** the title/category editor embedded (Revert/Reload instead of Close).
- **Chat modes:** Slow 30s, Followers, Subscribers, Emote-only and Unique chat. Each shows ✓ from
  Twitch's live room state and toggles through the same command path as `/slow`.
- **Broadcast:** Add marker, ads of 30, 60 or 90 s (each asks first), and Raid… (asks first, explains
  the countdown and /unraid).
- **Activity:** newest-first feed, capped at 200 items for the session, with kind pills coloured to match
  event highlights:
  - Twitch: follows (`channel.follow` v2) and channel-point redemptions. These are extra EventSub subscriptions,
    made only for the signed-in broadcaster's own channel, and they are optional: a failure doesn't affect chat. Each
    socket now carries at most 49 channels (147 in total) to leave room for them.
  - Own-channel subs, gifts, raids, Bits and charity notices taken from chat.
  - Streamer.bot: donations (Ko-fi, Streamlabs, StreamElements, Fourthwall, TipeeeStream,
    Patreon) and YouTube/Kick events. Jawjack asks Streamer.bot which events it supports (`GetEvents`) and
    subscribes to those.
- **Streamer.bot** card and pinned action buttons ([STREAMER_BOT.md](STREAMER_BOT.md)).
- Planned next: polls and predictions.

## Evidence (Windows, 2026-10-11)

- Choosing Streamer pinned `📌 #dreadedzombie` first in both workspaces and opened Studio.
- Studio loaded the real title and category (Just Chatting).
- Middle-clicking the pinned tab was refused with the explanatory notice.
- In the owner's channel, Slow 30s turned on (✓ appeared after Twitch's room update), then turned off again.
- 🛡 showed on #bessvibes and #3putt_ (moderated channels).
- Not yet exercised live: markers, ads and raids (they need a live stream or a target),
  saving stream info, and Clear chat.

Activity evidence (Windows, 2026-10-11): Streamer.bot accepted the event subscription
("Connected · events from YouTube, Kick, Ko-fi, …"). No follow, redemption or donation
happened during testing, so live items haven't been observed yet. Feed parsing is unit-tested.
