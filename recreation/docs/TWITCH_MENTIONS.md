# Typed Twitch mention cards

EventSub `channel.chat.message` mentions retain their exact fragment text and a separate provider-authored identity. Ctrl-click an underlined mention to inspect that chatter. The message context menu also lists mentioned accounts. This opens the existing chatter card, including retained messages, profile information and observed 7TV cosmetics; it does not send a message.

Mention hit regions cover the actual text in both plain and emote-bearing messages. Whitespace outside the text is not interactive. Selection and copied text retain the original bytes, including localized display names. Custom phrase highlights continue matching across mention boundaries.

The action re-resolves the retained message and its mention before opening. Deleted or evicted messages cannot activate stale mention targets. Metadata must provide a nonempty opaque user ID, a valid canonical login, and a nonempty display name; malformed or application-limit-exceeding metadata falls back to ordinary text. A fragment/body mismatch also removes mention interactivity rather than guessing identity from text.

Application limits are 255 bytes for IDs and 400 bytes for display names; these are defensive limits, not Twitch schema guarantees. IDs are opaque strings. Public profile lookup uses query encoding and verifies the returned identity. Profile requests happen on card activation, never merely on hover.

References:
- https://dev.twitch.tv/docs/eventsub/eventsub-reference/#channel-chat-message-event
- https://dev.twitch.tv/docs/api/guide#ids-are-opaque

Linux locked build verified during implementation. Windows 2026-10-11: Ctrl-click on a real typed mention in #xqc opened the mentioned chatter's card with retained messages. A mention inside an emote-bearing message is still unverified.
