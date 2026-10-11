# Multiple Twitch accounts (AC02)

Owner choice (2026-10-11): a switcher plus "send as".

- **Accounts window** (click the account name in the titlebar): every saved account with a
  status dot, an ACTIVE tag on the active one, Switch for the others, Remove (forgets a non-active account and deletes its vault
  entry), and **Add account** (a new device sign-in). Clicking a row shows that account's panel
  (sign-in code, permission update, sign out).
- **Active account** reads chat and runs tools (Studio, moderation, whispers). Switching
  reconnects chat as that account; "send as" choices and the Studio editor reset.
- **Send as:** with two or more accounts signed in, each composer shows an "as login" picker. The
  choice is remembered per channel. Messages go out through Helix with the chosen account's token, and
  reading stays on the active account. The shared rate limiter still applies.
- **Storage:** each account has its own Windows Credential Manager entry. The original account
  keeps `…/twitch/account-v1`, so existing installs need no migration; added accounts use
  `…/account-v1/{n}`. The workspace file stores only the list of vault entries, the active one and the
  per-channel send-as choice, never tokens.

Evidence (Windows, 2026-10-11): the existing login loaded as the active account and chat stayed connected.
Add account created a "Not signed in" slot with its sign-in panel, and Remove deleted it (the
saved list went back to one account). Not yet exercised: signing in a second account, switching, and
sending as an alt.
