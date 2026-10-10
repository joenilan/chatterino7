# Composer commands

Jawjack resolves supported slash commands locally before the chat connection and
send checks. Unknown slash commands keep the complete draft and show feedback.
The transport also rejects leading-slash input before rate-limit accounting or
HTTP dispatch, so an unsupported moderation command or whisper cannot silently
be posted as a normal public message.

## Supported commands

| Command | Behavior |
| --- | --- |
| `/help` | Open the compact, scrollable command reference. Selecting an entry inserts its command with the caret ready for arguments. |
| `/find [query]` | Open retained-message search in this channel; accepts its filters and quoted terms, up to 256 characters. |
| `/usercard login` | Open the native card for a user found in this channel's retained messages. `/user` is a local alias. |
| `/reply login [message]` | Prepare a reply to the latest retained, nondeleted, replyable message from that login. Review and send separately. |
| `/unreply` | Cancel the reply target. `/cancelreply` is an alias. |
| `/latest` | Close retained search and return to the live end of chat. |

Login arguments accept an optional `@`. Command names are case-insensitive.
Message bodies and search text retain their case and interior spacing.
Malformed commands, missing users and unsupported commands preserve the draft.
A prepared reply replaces only its command text; it does not send anything.
Local commands do not enter successful-send history.

Type `/` at the beginning of the composer for descriptions and keyboard completion.
Up/Down selects, Enter/Tab inserts, Escape dismisses. Insertion never sends or runs
a command. The button changes from Send to Run for slash input and local commands
remain usable while disconnected. Ordinary chat still requires a connection.

## Deliberate boundaries

This is not full Chatterino command parity. `/reply` prepares instead of sending
immediately. `/user` opens the native card rather than a web usercard. `/clear`,
`/ban`, `/timeout`, `/w`, `/me`, custom commands and other unimplemented commands
are blocked. In particular, `/clear` is not repurposed as local transcript clearing:
its moderation meaning must not be silently changed.

A card/reply target must come from current retained data. No fabricated user ID,
roster, synthetic message or new authentication permission is used. Existing
reply eligibility and deletion rules remain authoritative.
