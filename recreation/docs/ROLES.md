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

## Resetting splits

Double-click any divider between side-by-side or stacked panes to even out that
split. It eases over 0.2 s, or snaps instantly with reduced motion. Each split resets on its own, so a 2×2
grid becomes even by double-clicking each line. "Equalize this split" and "Equalize all
splits" in the tab menu still work. Dividers block clicks from reaching the elements behind them, so the
double-click is caught by a window-level listener limited to the split's area.

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

Toggle it with the panel icon right of the settings gear, **Ctrl+Shift+S**, or gear menu →
"Studio panel". Layout, tabs and visual rules are in [STUDIO.md](STUDIO.md). Feed sources:
Twitch follows and redemptions (optional EventSub extras for the own channel; 49 channels per socket,
147 in total), own-channel subs/gifts/raids/Bits from chat, and Streamer.bot donations plus YouTube/Kick
events ([STREAMER_BOT.md](STREAMER_BOT.md)).

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

## Moderator onboarding (owner idea, 2026-10-11)

Choosing **Moderator** in "Choose your setup" (or Settings → General → Channels you moderate → Choose…)
lists every channel you moderate (Helix moderated channels) with tick boxes. Channels already open are tagged
and start unticked, and live channels get a green dot (Helix Get Streams, checked when the dialog opens).
**All**, **Live only** (live and not yet open) and **None** shortcuts are there too. **Open N as tabs in “Moderating”** adds each ticked channel as a tab in one "Moderating"
workspace, which is created the first time and reused after that (channels already in it are skipped). Evidence: 53 moderated channels listed, 26 pre-ticked, Live only = 2. Create was not
pressed, to keep the owner's profile unchanged.

