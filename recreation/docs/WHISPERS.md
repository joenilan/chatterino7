# Whispers

Titlebar message icon (with unread count), gear menu → Whispers, or click a whisper
notification.

- **Receive:** one account-wide EventSub subscription (`user.whisper.message`, requires
  `user:read:whispers`). It's made on any live socket, and made again if that socket reconnects or closes.
  Duplicate deliveries are dropped by whisper ID.
- **Send:** Helix `POST /whispers` (requires `user:manage:whispers`). The reply appears in the thread only
  after Twitch accepts it. Errors are shown and the draft is kept. `/w login message` also works from any composer.
- **Window:** conversations on the left, most recent first, with unread badges and a last-line
  preview. Typing a login and pressing Enter starts a conversation. On the right are the thread (your messages
  right-aligned in lilac) and the reply box, where Enter sends. Enter never closes the window.
- A whisper arriving while that conversation isn't open shows an in-app notification
  that opens it.
- **Privacy:** session-only (50 conversations, 200 messages each). Nothing is written to disk.

Evidence (Windows, 2026-10-11): the window, the empty state and starting a conversation work.
No whisper was sent or received in testing. Receiving needs someone to whisper the owner.
