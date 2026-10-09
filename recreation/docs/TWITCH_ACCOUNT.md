# Jawjack Twitch account

The app uses its owner-provided Public Client ID. Client IDs identify an app and
are not secrets; no client secret is embedded. Browser authorization stays on
Twitch. The user selects Sign in, copies the short-lived device code if needed,
and opens the provided Twitch HTTPS verification page. No redirect listener is
required for device-code login.

Implementation:
- Explicit native control sessions do not automatically load a saved Twitch account.
  Manual layout inspection can run without accessing the credential vault.
- Bounded HTTPS requests off the GUI thread, redirects disabled and verification
  destination restricted to Twitch hosts.
- Device-code polling honors interval, slow-down, expiry, cancellation and denial.
- Token validation checks the client ID, user identity and required read/write-chat
  scopes before activating an account.
- Access/refresh credentials are stored through GPUI's native OS credential API,
  separate from plaintext workspace preferences. Vault failures do not activate
  the account; there is no plaintext token fallback.
- Saved accounts validate at startup and every 30 minutes. Near-expiry/expired
  access tokens refresh serially within this account session. Replacement refresh
  credentials are acknowledged by the vault before another validation request.
- Sign-out removes this app's local vault entry. It is not a claim that the Twitch
  server grant has been revoked; users can manage connected apps on Twitch.
- Repeated sign-in clicks are unavailable during an active attempt. Cancellation
  invalidates the generation; a late response cannot activate that cancelled UI.
- A signed-in account is distinct from channel readiness. The shared EventSub
  worker subscribes each open channel to messages, deletions, user clears and chat
  clears. Channel readiness requires successful enabled subscriptions.
- Receiving updates every open pane, including background tabs, through a bounded
  queue. Retained transcripts and EventSub envelope deduplication are bounded.
- Helix sending checks the actual `is_sent` result. Failure/hold/uncertain delivery
  keeps the draft. A successful response clears only an unchanged submitted draft.
  There is no automatic send retry. The initial local limit is conservatively
  20 messages/30 seconds and one message/second/channel, even for privileged users.
- Reconnect URLs stay on the Twitch EventSub host. Graceful migration waits for
  the replacement welcome; ordinary reconnect recreates subscriptions and shows
  a missing-history warning. Revocation waits for account/channel reconfiguration.
- These transport paths are implemented and compile, not yet live verified.

Current limits: real Twitch authorization, Windows vault behavior and interrupted
login flows have not been manually exercised. The Linux executable builds; the
cloud display cannot start. Concurrent application instances are not coordinated
for refresh-token use yet; run one Jawjack instance for account work. Do not call
this production-ready authentication until the native flow has been exercised.

References:
- https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#device-code-grant-flow
- https://dev.twitch.tv/docs/authentication/validate-tokens/
- https://dev.twitch.tv/docs/authentication/refresh-tokens/
- https://dev.twitch.tv/docs/chat/authenticating/

## Current transport limits

The first integration renders plain text (including emote names), not emote images.
Adding/removing an open channel rebuilds the shared subscription connection; missed
messages cannot be recovered from Twitch. Authentication revalidation currently
also briefly disconnects channels. Incremental subscription updates, full room
settings, AutoMod hold updates, reply UI, and privileged rate-limit policy remain
future work. Affiliate status does not require extra scopes for basic read/send.
The current UI cap is 32 panes across 16 tabs, with shared subscriptions for duplicate
channels. No moderator or broadcaster mutation scopes are requested.

Sources:
- https://dev.twitch.tv/docs/eventsub/handling-websocket-events/
- https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channelchatmessage
- https://dev.twitch.tv/docs/api/reference/#send-chat-message
- https://dev.twitch.tv/docs/chat/#rate-limits
