# Jawjack: multiplatform chat direction

Owner-approved future direction, 2026-10-09. This is planned scope, not a claim
that additional platforms are implemented or that their APIs permit every action.

Jawjack is a full-featured chat client for watching other people's streams and a
multistream chat desk for the owner's broadcasts. Preserve the Chatterino/7TV
feature depth; avoid turning the product into only a broadcast-overlay tool.

## User experience

- Connections / Integrations manages platform accounts, permissions, connection
  health, reconnect and sign-out. No mandatory permanent navigation sidebar.
- Add a channel or livestream URL from any supported provider, including someone
  else's YouTube/Kick stream. Owner/broadcaster status is not a universal condition
  for adding a feed; each adapter declares actual platform requirements.
- Separate tabs and splits remain first-class. A combined feed is optional and
  can contain channels across platforms, including the user's own broadcasts.
- Every message retains its platform, channel, account context and original ID.
  Clear badges and source links make attribution visible without wasting space.
- A combined-feed composer has an explicit destination. Replies lock to the
  original platform/channel. Never silently cross-post or send to all streams.
- Moderation controls appear only where the signed-in account has permission.
  Receiving, posting, deleting, timing out and banning are distinct capabilities.
- Show unsupported, unavailable, rate-limited and authorization-required states
  honestly. Unsupported actions are not implemented through credential scraping.

## Shared model and boundaries

Define provider-neutral identities before adding a second network implementation:
`ProviderId`, `AccountId`, `ChannelId`, `MessageId` and per-channel capabilities.
IDs are namespaced by provider/account/channel where appropriate. Display names
are not identifiers. Preserve provider-native rich message fragments, emotes,
badges, timestamps and deletion metadata instead of flattening everything to text.

Each provider owns authentication, token storage, discovery/URL resolution,
subscriptions, reconnect/backoff, send acknowledgements and moderation mapping.
UI subscriptions refer to stable feed keys, not physical panes. Multiple panes
watching one channel should share network work while retaining independent scroll,
selection and draft state. Mixed feeds deduplicate by namespaced original IDs;
ordering must tolerate reconnect gaps, clock differences and delayed arrivals.

Keep authentication separate from being connected to a live feed. A signed-in
account does not prove a subscription exists or that a sent message was accepted.
Tokens remain in the OS credential vault, isolated by provider and account.

## Delivery order

1. Solid Twitch sign-in, live receive/send, native workspace interactions.
2. Chatterino/7TV parity, including emotes, channel discovery, replies and account-
   appropriate moderation. Continue product polish alongside functionality.
3. Connections / Integrations and provider-neutral feed interfaces, with a second
   provider selected from verified official API feasibility.
4. YouTube and Kick adapters where official access supports the intended use.
5. Optional merged feeds for both broadcaster and viewer workflows.
6. TikTok and other providers only after confirming supported developer access,
   permissions, costs, quotas and practical live-chat availability.

Before committing to a provider, record current official documentation for:
viewer access to third-party streams, authentication, receive transport, posting,
moderation, quotas/cost, public distribution and review requirements. Do not
promise parity based solely on another app appearing to support that provider.

## Official API feasibility snapshot — 2026-10-09

Read-only documentation research; no live integrations or account actions tested.
Recommended order is Twitch, YouTube, then Kick with a hosted webhook relay.
The owner explicitly made TikTok low priority; it must not delay those integrations.

- **YouTube:** public live-video chat reading supports API-key or OAuth access;
  streamList offers gRPC streaming and REST list provides a polling fallback.
  A pasted live-video URL can resolve activeLiveChatId without broad search.
  Viewer posting requires OAuth and chat permission; owner/mod actions are distinct.
  Quotas require careful budgeting across the whole app's user population.
  https://developers.google.com/youtube/v3/live/streaming-live-chat
  https://developers.google.com/youtube/v3/live/docs/liveChatMessages/insert
  https://developers.google.com/youtube/v3/determine_quota_cost
- **Kick:** current official event documentation explicitly permits app-token
  subscriptions for other channels by broadcaster ID. Chat events arrive through
  public HTTPS webhooks, suggesting a hosted relay; do not embed an app secret in
  the native client. Viewer posting and moderation have separate user scopes and
  channel authority. Verify distribution limits, rates and recovery before shipping.
  https://github.com/KickEngineering/KickDevDocs/blob/main/events/introduction.md
  https://docs.kick.com/apis/chat
  https://github.com/KickEngineering/KickDevDocs/blob/main/apis/faqs.md
- **TikTok:** no general public LIVE-chat receive/send/moderation interface was
  found in the official developer catalog. Research-video comments and historical
  data exports do not provide live chat. Keep this integration conditional on a
  documented supported API or approved partnership, not an unofficial scraper.
  https://developers.tiktok.com/docs/en/welcome
  https://developers.tiktok.com/docs/en/research-api-faq

Recheck these sources before implementation; provider capabilities and limits can
change. Documentation feasibility is not a measured latency or reliability result.
