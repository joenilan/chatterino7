# Local chatter nicknames and colors

Right-click a retained Twitch message and choose **Personalize chatter…**. Set a nickname, choose a pastel name color, or leave either setting at its Twitch default. Manage saved appearances from **Settings → Chatter nicknames & colors**.

Preferences are keyed by Twitch's numeric user ID and saved with the local workspace profile. They apply across channels and workspaces, including retained messages and reopened tabs. This is a local presentation feature: original display names, logins, user IDs, reply targets, mentions, moderation actions and sender-search filters remain canonical.

## Interaction details

- Nicknames have a 40-character limit. Line breaks and directional-control characters are rejected.
- Up to 256 custom appearances can be saved.
- Save or Ctrl+S applies the change and queues the normal workspace save.
- Selecting another chatter with unsaved edits requires a second selection to discard them.
- Closing the dialog preserves its unfinished edit during the app session.
- Reset to Twitch requires confirmation and removes that chatter's saved override.
- Appearance changes clear existing transcript selection and remeasure retained rows so Unicode nickname lengths do not leave stale copy or link offsets.
- Copied transcript lines use the displayed nickname. **Copy username** uses the real Twitch login.
- Deleted messages remain deleted; changing appearance cannot restore their contents.
- Transcript, activity, retained-conversation and search-result labels use the local nickname. A reply outside retained history falls back to its saved original name.
- Global search is a snapshot. Rerun a query after changing nicknames to discover newly matching rows.
- Profile files and their backups contain saved nicknames in plaintext; these preferences are not sent to Twitch.

## Verification boundary

The Linux locked development build is required for this slice. Windows editor behavior, native clipboard interactions, persistence across a real restart and visual review remain runtime checks; a successful build does not establish those results.
