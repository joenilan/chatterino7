# Private chatter notes

Right-click a message and choose Private chatter note, or choose Private note from a chatter card. Notes are keyed to the provider's user ID, independently of display names or local nicknames. Mention cards can open the same editor.

The editor supports multiline text, a character counter, Ctrl+S, saved-chatter navigation, and deliberate Save and Delete actions. Switching chatters with unsaved edits asks you to save first or explicitly discard; closing and reopening the dialog in the same running app preserves the draft. Unsaved drafts do not survive app exit.

Saved notes remain in this Jawjack profile's local workspace file. They are plain text, not encrypted: anyone who can access those files can read them. They are not attached to Twitch accounts or synchronized between PCs. Ordinary cards and transcripts do not show their contents, and copying chat or sending messages never includes notes. Opening the private editor can expose its content on a shared screen.

Limits: 2,000 characters per note, 256 saved chatters, and a 192 KiB serialized notes budget to leave space for other workspace preferences. Save queues the existing atomic workspace persistence; application-level storage failures remain surfaced by the workspace save status. Delete requires a second click, and reopening resets that confirmation.

Linux locked build passed. Native focus, storage reload and compact-window appearance are pending manual acceptance.
