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
- Planned next: Streamer.bot action buttons ([STREAMER_BOT.md](STREAMER_BOT.md)),
  an audience event feed, polls and predictions.

## Evidence (Windows, 2026-10-11)

- Choosing Streamer pinned `📌 #dreadedzombie` first in both workspaces and opened Studio.
- Studio loaded the real title and category (Just Chatting).
- Middle-clicking the pinned tab was refused with the explanatory notice.
- In the owner's channel, Slow 30s turned on (✓ appeared after Twitch's room update), then turned off again.
- 🛡 showed on #bessvibes and #3putt_ (moderated channels).
- Not yet exercised live: markers, ads and raids (they need a live stream or a target),
  saving stream info, and Clear chat.
