# Jawjack Twitch account

The app uses its owner-provided Public Client ID. Client IDs identify an app and
are not secrets; no client secret is embedded. Browser authorization stays on
Twitch. The user selects Sign in, copies the short-lived device code if needed,
and opens the provided Twitch HTTPS verification page. No redirect listener is
required for device-code login.

Implementation:
- Normal and native-control launches use the same saved-account restoration path.
  Startup reads the OS vault and validates the existing grant before connecting;
  enabling app control does not require another device-code sign-in. No new
  OAuth scope or grant is created by restoration.
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
- Native owner sign-in and authenticated channel subscriptions were verified on
  Windows (7466e04b, 2026-10-09). Build b8379a6 subsequently restored the saved
  owner account automatically and received visible HutchMF messages (counts
  1 → 6 → 29 → 48 → 60). Confirmed message delivery remains unverified.

Current limits: sign-in and restart restoration are verified. Refresh rotation,
interrupted login and vault-failure recovery are not independently verified. Linux and Windows builds passed; the cloud display
cannot start. Concurrent instances are not coordinated for refresh-token use yet;
run one Jawjack instance for account work. This is not production-ready auth.

References:
- https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#device-code-grant-flow
- https://dev.twitch.tv/docs/authentication/validate-tokens/
- https://dev.twitch.tv/docs/authentication/refresh-tokens/
- https://dev.twitch.tv/docs/chat/authenticating/

## Current transport limits

The renderer preserves Twitch EventSub emote fragments and has a first native
inline image path. Third-party emote catalogs and overlays are not integrated yet.
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

## Native integration observation, 2026-10-09

The owner completed Twitch authorization. The native app showed the owner signed
in and the owner channel Connected, with no subscription error. The channel was
quiet, so zero messages is not evidence of receive failure. One authorized test
send encountered a controller transport error without dispatch confirmation;
the draft remained and there was no observed echo. It was not retried. Adding a
busy channel was interrupted by the external PC-tool host closing. No claim of
successful message delivery or live incoming traffic is made from this pass.

## Saved-account startup correction

The earlier native-control launch explicitly skipped the credential-vault read,
which made an already-authorized account appear signed out after each controlled
restart. That conditional path has been removed. Startup now behaves consistently
across launch modes. Windows b8379a6 locked release compilation passed in 9.03s,
exit 0. A fresh native-control launch restored dreadedzombie without a new grant,
and both channels connected. The owner independently saw incoming HutchMF chat.

## Username presentation

EventSub name colors are accepted only as six-digit hexadecimal RGB. The dark
renderer lifts colors below 4.5:1 contrast against its chat canvas, and uses a
stable user-ID-derived palette when Twitch supplies no color. Resolution happens
once per received message, outside the animation/render loop. Username weight and
color use the same shaped text and byte offsets as selection, preserving copied
text and Unicode names. Windows 8b0b54f visibly confirmed colored, heavier
usernames while receiving real HutchMF messages; build passed in 8.49s.

Contract: https://dev.twitch.tv/docs/eventsub/eventsub-reference/#channel-chat-message-event

## Twitch inline emotes

Ordered EventSub fragments retain original text bytes. Only valid Twitch emote
IDs resolve to the fixed Twitch CDN; no Twitch credential is sent to the media
host. Static PNG and animated GIF are decoded off the GUI thread. Reduced motion
selects static assets. Failed/over-budget animations fall back to static, then
original text if the static asset also fails.

The anonymous media cache has two workers, a 32-job queue, a two-result queue,
128 cache entries and a 48 MiB decoded-data budget. Individual responses are
limited to 2 MiB, dimensions to 256 square pixels, and decoded animations to
120 frames / 8 MiB. Eviction releases associated GPUI image textures. This is a
bounded design, not a measured whole-process memory or frame-rate claim.

Mixed rows reserve 28px image boxes, wrap text around them and preserve original
source-byte selection/copy semantics. Unloaded slots show a short label until
ready; failed assets regain their full original text. Double-clicking an emote
selects its source token. Deleted messages take the plain redacted-text path.
The native Windows image/selection observation is pending for this implementation.
