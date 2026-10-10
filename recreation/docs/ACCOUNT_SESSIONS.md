# Twitch account sessions

## Session continuity

The account popup remains compact and wraps its status above its action row.
Routine validation retains the current valid account instead of emitting a temporary
signed-out identity. Unchanged credentials update the private expiry deadline and
schedule the next validation without rewriting the OS vault. Validation runs no
later than 30 minutes, and earlier when the token is approaching expiry.

An explicit Twitch 401 invalidates the exposed identity before renewal. Terminal
validation or persistence errors disconnect conservatively. A private monotonic
expiry deadline stops an expired access token from being exposed to chat consumers.
Token rotation may still reconnect the transport; optimizing that requires separating
credential updates from subscription epochs and is deliberately a separate change.

Renewal first obtains an acknowledgement on the UI thread. Once the renewal
transaction starts, cancellation is unavailable until it finishes, preventing a
cancelled login from rotating credentials without saving them. Vault writes remain
serialized even when the worker's acknowledgement wait times out. A pending write
must settle before another sign-in or removal starts.

## Recovery and feedback

- Known unavailable OS storage shows **Retry secure storage**, with explanatory
  text, instead of offering a sign-in which cannot safely persist.
- Retrying reads the existing vault and can restore saved credentials after recovery.
- Tokens never fall back to workspace files or plaintext settings.
- Copying a device code gives a temporary **Copied!** label.
- Validated scopes are retained as metadata. Normal sign-in includes chat and
  account-emote reading, with consent through Twitch. Older chat-only saved sessions
  stay usable and can be updated by signing out and in once. Inventory loads
  automatically; there is no separate enable button.

## Verification and remaining work

The Linux build completed successfully against the final source. In a separate
native cloud preview, the account popup was readable and its secure-storage retry
returned an accurate unavailable state. The existing owner preview was left running.

Authenticated long-running refresh, revocation, rotation, copy-code, and Windows
vault recovery have not yet been exercised end to end for this revision. Source
review and compilation are not runtime acceptance for those paths. EventSub
`authorization_revoked` propagation back to account state remains follow-up work;
transport revocation already stops the affected connection but account state also
needs immediate reconciliation.
