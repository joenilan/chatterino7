# Split navigation and layout recovery

Channel tabs remain inside each chat pane. Workspaces remain in the sidebar.
These shortcuts move between visible split panes without changing any deck's
active channel, draft, reply target or retained search:

- F6: next visible pane in layout order, wrapping at the end.
- Shift+F6: previous visible pane.
- Ctrl+Alt+Left/Right/Up/Down: nearest pane in that direction. Overlapping rows or
  columns are preferred over diagonal panes. A missing neighbor leaves focus alone.

Navigation focuses the destination composer and dismisses transient emote pickers
and completion lists. Open dialogs, channel-entry UI, settings and active dragging
keep their own interaction. Existing Alt+Up/Down input history and channel/workspace
cycling shortcuts remain unchanged. Some operating systems reserve directional
shortcuts globally; F6 is the alternate route.

Right-click a channel tab for:

- Focus next pane
- Equalize this split: reset sibling proportions in the nearest enclosing split.
- Equalize all splits: reset proportions throughout the current workspace.

Equalizing preserves channel order, decks, active tabs, split orientations, drafts
and reply targets. Minimum pane sizes still apply, so equal proportions do not
promise identical pixels when nested panes have different minimum sizes. New
proportions are saved through the existing workspace storage owner.

The pane-header close button now follows the same confirmation flow as channel-tab
closure. Cancel preserves the pane; confirmation keeps its saved draft under the
existing workspace/channel key. This does not introduce a closed-channel history
or claim exact layout undo.

## Verification checkpoint

The Linux locked offline desktop build passed after the input-context and
context-menu-origin fixes. Native review confirmed F6/Shift+F6 move the typing
focus between two split composers, the context-menu next-pane action starts from
the clicked pane, an uneven divider returns to equal proportions, and the header
close action opens a dialog whose Cancel retains the channel and draft.
Ctrl+Alt+Left did not move focus on the cloud Linux desktop; OS shortcut capture
has not been isolated. Windows directional shortcuts and nested-layout behavior
still require native verification. No live message was sent in this review.
