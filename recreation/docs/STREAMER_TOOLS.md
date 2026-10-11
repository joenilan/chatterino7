# Twitch permissions, channel roles and streamer tools

## Permissions (owner decision, 2026-10-11)

Jawjack requests every Twitch scope its viewer, moderator and streamer tools use
in one grant (`auth.rs::SCOPES`): Chatterino's list without the legacy IRC scopes,
plus whispers, subscriptions, hype train, goals, charity, ads and Bits. This
replaces the incremental per-feature grants proposed in ROLE_EXPERIENCES.md §3/§8;
role authority still comes only from the account's real channel role.

Saved logins with fewer scopes keep working. The account menu shows `login •`, and
the account dialog explains what is missing and offers **Update Twitch
permissions**: a new device sign-in while the current account stays active. Cancel
or failure keeps the existing account and chat connections; success replaces the
saved grant. Signing out is never needed.

## Capability model (`capabilities.rs`)

- `Role` per channel: Broadcaster (channel login is the signed-in login),
  Moderator (Twitch's moderated-channel list, `user:read:moderated_channels`,
  refreshed every 10 minutes) or Viewer. Unknown until the list loads.
- `Check` for an `Action`: Available, SignedOut, Checking, NeedsRole(reason) or
  NeedsPermission. Tools check when shown and again right before writing.
- Control inspection reports `capabilities` (status, granted-scope count,
  moderated channels) and per-pane `roles`.

Token refresh or newly granted scopes no longer reset EventSub: connections are
rebuilt only when the signed-in user changes.

## Stream info editor

The ✎ button in the own channel's header (Broadcaster role only) opens
**Stream info**: current title and category are fetched first; categories come from
Twitch's category search (no free-text game IDs). Save is explicit. Before writing
Jawjack re-reads the channel; if the title or category changed elsewhere since the
form loaded, nothing is written, the new values are shown and the edits stay for a
second Save. After a write the result is read back and reported as confirmed or
still propagating. Errors keep the edits. Without `channel:manage:broadcast` the
button opens the account dialog instead.

## Twitch commands (`twitch_actions.rs`)

Chatterino's Helix-backed commands, listed under /help: /ban, /timeout (`10m`,
`1h`, `1d`, `1w`; default 10 minutes), /unban and /untimeout, /warn, /delete,
/clear, /slow and /slowoff, /followers and /followersoff, /subscribers and
/subscribersoff, /emoteonly and /emoteonlyoff, /uniquechat and /uniquechatoff,
/announce (plus blue, green, orange and purple), /shoutout, /mod and /unmod, /vip and /unvip,
/color, /w, /block and /unblock, /settitle, /setgame, /marker, /commercial, /raid and /unraid.

Each command is checked against the channel role and its scope before running
(moderator commands in moderated or own channels; /mod and /vip only as broadcaster;
title, marker, ad and raid always act on the signed-in channel). The
draft is cleared only after Twitch confirms; refusals show Twitch's message and
keep the draft. Custom commands with the same name take precedence, as in
Chatterino. Typed moderation commands run without a review step (Chatterino parity);
right-clicking a message in a channel you moderate or own adds Delete message,
Timeout (1 minute, 10 minutes, 1 hour, 1 day) and Ban…, which first asks for confirmation naming the
user and channel. Timeout and ban are not offered on the broadcaster's own messages.

## Evidence (Windows, 2026-10-11)

After the permission upgrade: 48 scopes granted, 53 moderated channels loaded,
and all connections were kept. The moderation menu items appear in #dreadedzombie
(Broadcaster) and not in #xqc (Viewer). Delete message on the owner's own test
message in #dreadedzombie was confirmed by Twitch and redacted live.
`/slow 10` in #xqc with the old grant: "You are not a moderator in this channel",
nothing sent, draft kept.


Twitch accepted the full scope list (device code issued). Update → code → Cancel
kept the account signed in and both visible panes connected. Owner's channel
reports Broadcaster; ✎ appears only there and, with the old 3-scope grant, explains
the missing permission. Not yet exercised: completing the upgraded grant,
moderated-channel loading, loading/saving stream info, conflict and failure paths.
