# Personal composer commands

Open Settings → Custom commands, or enter `/commands` in a channel composer.
Definitions belong to the local Jawjack profile and are shared across workspaces.
They persist with workspace settings; they are not uploaded to Twitch.

Create up to 32 named templates. Names use 1–24 ASCII letters, digits or
underscores; names are normalized to lowercase. Built-in and Twitch command names
are reserved. The editor supports create, edit, rename, two-step deletion and a
live preview with sample arguments. Switching definitions with unsaved edits asks
for a second click to discard those edits.

Supported placeholders:
- `{channel}`: the invoking channel login
- `{args}`: the complete argument text
- `{1}` through `{9}`: individual whitespace-delimited arguments
- `{{` and `}}`: literal braces

Example: `/welcome Joe` with template `Welcome {1} to {channel}!` prepares a
message for review. Autocomplete lists personal commands alongside built-ins.
Choosing a completion inserts the command. Running it expands the draft; a
separate Send action sends that draft through the ordinary chat path. Expansion
retains the current reply target, if any.

Missing required arguments, invalid placeholders, empty output, multiline/control
characters, slash-prefixed output and messages over 500 characters are rejected
without replacing the original draft. Substituted arguments are literal text and
are never recursively expanded. No shell commands, network commands, scripts or
moderation actions are executed by this feature.

This is an initial text-template feature, not complete compatibility with every
Chatterino custom-command variable or command type.
