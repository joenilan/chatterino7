# Search open channels

Open from Settings → Search open channels or Ctrl+Shift+F. Pane-local Ctrl+F
keeps its existing behavior; its All chats action seeds the shared search dialog
with the current query without changing the pane search or composer.

Enter or Search starts a local snapshot scan. Arrow keys select a result; Enter
jumps after scanning finishes. Jump and Previous/Next controls support the mouse.
Escape closes the dialog. Search again to include subsequent arrivals.

The query parser is shared with pane Find: plain text, quoted phrases, exclusions,
from:, from-id:, badge:, has: and is: predicates retain the same meaning and error
messages. There are no new provider requests or additional permissions.

Every currently open workspace and hidden channel tab participates. Closed
workspaces do not. Duplicate channel views retain their own result identity and
workspace label because their retained timelines can differ. Jump resolves the
exact pane in its current workspace, including after moves, and rechecks both
message retention and the query before navigating.

The scan captures absolute retained-row bounds, advances round-robin in slices
of at most 256 messages or 6 ms, and yields 16 ms between slices. New input cancels the
old scan. Only the newest 200 result references are retained; a full scan counts
all matches. Equal timestamps use the newest-first traversal order. Missing
timestamps use traversal order rather than inventing a sent time.

Result storage contains weak pane references and message IDs, never a second
message log. Visible previews re-read current messages, so pruning and moderation
cannot resurrect old text. Counts are explicitly snapshot counts. A changed or
closed result becomes unavailable and cannot jump to a different duplicate pane.
Only a small page of previews is rendered at a time.

Compilation and live-message acceptance are reported separately. A no-message
cloud preview cannot establish match correctness or live moderation behavior.
