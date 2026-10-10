# Chat content hover previews

The transcript no longer attaches an interaction tooltip to the complete message
row. Username and link hints use their actual laid-out text bounds. Blank space
remains available for selection without presenting a misleading Ctrl+click hint.

Inline emotes have content-sized hover targets. Their hover card enlarges the
already-decoded image, keeps animation subject to reduced-motion preferences, and
shows the emote name and provider beneath it. Overlay emote layers are retained.
The picker uses the same card with collection/source information. Previewing does
not issue a separate download or change the composer.

Targets are clipped to the transcript viewport and omitted during selection drag.
Existing selection, Ctrl+click and right-click actions remain handled by their
original owners. An unloaded preview is explicitly labeled rather than appearing
as an empty card. Native acceptance should include blank row space, an emote,
username/link hints and a selection drag across content.
