# Releasing Jawjack

Owner decision, 2026-10-11: release from the owner's site first. Add a Microsoft
Store version later. Jawjack's licence remains the owner's choice; neither channel
requires open source.

## 1. Owner's site (first)

- Unsigned per-user installer `Jawjack-<v>-setup.exe` plus a portable zip under
  `apps.zombie.digital/downloads/jawjack/`, with `latest.json` for the in-app updater
  ([UPDATES.md](UPDATES.md)).
- The download page tells first-time users: Windows SmartScreen → **More info →
  Run anyway**. Only the browser-downloaded first install is affected; updates
  downloaded by Jawjack itself are not marked as internet files, so they skip the prompt.
- Windows 11 **Smart App Control** (when enabled) blocks unsigned apps outright;
  mention it on the download page and link to the Store version once it exists.
- Still to build: the installer (port of Haiwire's `apps/haiwire-setup`) and the
  release package/publish scripts (Haiwire keeps these outside git in `.local-release/`).

## 2. Microsoft Store (when ready)

- **Account:** individual developer registration has been free since September 2025:
  government ID plus selfie verification. One account covers all of the owner's apps
  (Haiwire too).
  [Announcement](https://blogs.windows.com/windowsdeveloper/2025/09/10/free-developer-registration-for-individual-developers-on-microsoft-store/),
  [Get started](https://learn.microsoft.com/en-us/windows/apps/publish/faq/get-started-with-the-microsoft-store).
- **Package as MSIX.** The Store signs MSIX packages with Microsoft's certificate at no cost.
  A plain EXE/MSI submission must already be Authenticode-signed by us, so MSIX is
  the free path.
- **Updates come from the Store.** The Store build must hide or disable the
  built-in updater, e.g. with a `store` cargo feature, or by detecting a package
  identity at runtime.
- **Things to verify when packaging:**
  - Settings stay in LocalAppData. Packaged apps get their writes redirected to a
    per-package folder unless the manifest opts out. Plan a one-time import from
    the site edition's `%LOCALAPPDATA%\ChatWorkbench`.
  - Credentials stay in the Windows Credential Manager (expected to work).
  - Outbound network: the `internetClient` capability.
  - The `--control-session` pipes still work for QA.
- **Store listing:** privacy policy URL (Twitch login, 7TV/BTTV/FFZ requests, the
  update check), screenshots and age rating. Review usually takes hours to a few
  days per release.
- **Owner's site** keeps the unsigned installer with the in-app updater and links
  to the Store version with the "Get it from Microsoft" badge.

## 3. Other options considered

- **SignPath Foundation:** free signing, but only for fully open-source projects
  (OSI licence, public repo, free downloads, CI release builds, per-release approval).
  The publisher shown would be "SignPath Foundation". Not chosen while the licence
  is undecided. CI would be acceptable for release builds only.
- **winget:** free listing, and unsigned installers are allowed. Worth adding after
  the first site release.
- **Self-signed certificates** only help on machines that import the certificate.
  **Paid options** for later: Azure Trusted Signing (about US$10/month, individual
  tier limited to the US and Canada) and a Certum open-source certificate.
