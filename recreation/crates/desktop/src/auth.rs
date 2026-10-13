//! Twitch public-client device login. Secrets remain in memory and the OS vault.
use crate::theme;
use gpui_kit::component::{
    Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::{Value, json};
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub const CLIENT_ID: &str = "cf1jr97a5fduscdul5yv9b5hnzm005";
/// The first account's vault entry (existing installs keep it); added accounts
/// use `{VAULT}/{n}`.
pub const VAULT: &str = "https://jawjack.zombie.digital/twitch/account-v1";
/// Everything Jawjack's viewer, moderator and streamer tools use, requested
/// together (owner decision, 2026-10-11). Twitch still grants moderator and
/// broadcaster actions only where the account actually holds that role.
pub const SCOPES: &[&str] = &[
    // Chat and the account's own emotes, colour, whispers, blocks and follows.
    "user:read:chat",
    "user:write:chat",
    "user:read:emotes",
    "user:manage:chat_color",
    "user:read:whispers",
    "user:manage:whispers",
    "user:read:blocked_users",
    "user:manage:blocked_users",
    "user:read:follows",
    "user:read:subscriptions",
    "user:read:moderated_channels",
    // Moderation in channels where the account is a moderator.
    "moderator:manage:banned_users",
    "moderator:manage:chat_messages",
    "moderator:manage:chat_settings",
    "moderator:manage:announcements",
    "moderator:manage:shoutouts",
    "moderator:manage:warnings",
    "moderator:manage:automod",
    "moderator:manage:automod_settings",
    "moderator:manage:blocked_terms",
    "moderator:manage:unban_requests",
    "moderator:manage:shield_mode",
    "moderator:manage:suspicious_users",
    "moderator:read:suspicious_users",
    "moderator:read:chatters",
    "moderator:read:followers",
    "moderator:read:moderators",
    "moderator:read:vips",
    // Streamer tools on the account's own channel.
    "channel:moderate",
    "channel:manage:broadcast",
    "channel:manage:moderators",
    "channel:manage:vips",
    "channel:manage:raids",
    "channel:manage:polls",
    "channel:read:polls",
    "channel:manage:predictions",
    "channel:read:predictions",
    "channel:manage:redemptions",
    "channel:read:redemptions",
    "channel:read:subscriptions",
    "channel:read:hype_train",
    "channel:read:goals",
    "channel:read:charity",
    "channel:read:ads",
    "channel:manage:ads",
    "channel:edit:commercial",
    "bits:read",
    "clips:edit",
];
/// Older saved grants keep chat working until the owner updates permissions.
const EXISTING_CHAT_SCOPES: &[&str] = &["user:read:chat", "user:write:chat"];
fn requested_scopes() -> String {
    SCOPES.join(" ")
}
struct Account {
    login: String,
    tokens: Value,
    expires_at: Instant,
}
enum Event {
    Code(String, String),
    Ready(Account),
    Rotated(Value, mpsc::SyncSender<bool>),
    Error(String),
    Invalidated,
    Renewing(mpsc::SyncSender<bool>),
}
pub struct TwitchAccount {
    status: String,
    busy: bool,
    saving: bool,
    validating: bool,
    renewing: bool,
    /// A new sign-in that keeps the current account active until it succeeds.
    upgrading: bool,
    vault_unavailable: bool,
    /// The vault has nothing saved for this account (so nothing to remove).
    nothing_saved: bool,
    code: Option<(String, String)>,
    copied_at: Option<Instant>,
    /// The sign-in code stays masked until revealed (screen sharing, streams).
    code_revealed: bool,
    cancel: Arc<AtomicBool>,
    generation: u64,
    account: Option<Account>,
    /// This account's entry in the OS credential vault.
    vault: SharedString,
    /// The refresh token the current check started from, so a rejected one can
    /// be told apart from a newer one another Jawjack window saved meanwhile.
    started_refresh: Option<String>,
    /// One retry with the vault's newer token per rejected refresh.
    retried_newer: bool,
}
impl Drop for TwitchAccount {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl TwitchAccount {
    pub fn label(&self) -> String {
        self.account
            .as_ref()
            .map(|a| a.login.clone())
            .unwrap_or_else(|| "Sign in".into())
    }
    /// Why there is no signed-in account, once checking has finished (None while
    /// signed in or still checking).
    pub fn signed_out_reason(&self) -> Option<String> {
        (self.account.is_none() && !self.busy).then(|| self.status.clone())
    }
    /// A rejected refresh token may just be stale: Twitch refresh tokens are
    /// one-use, so if another Jawjack window renewed first, the vault now holds
    /// a newer one. Use it once before giving up.
    fn retry_newer(&mut self, message: String, cx: &mut Context<Self>) {
        self.retried_newer = true;
        let read = cx.read_credentials(&self.vault);
        let used = self.started_refresh.clone();
        cx.spawn(async move |view, cx| {
            let newer = read
                .await
                .ok()
                .flatten()
                .and_then(|(_, bytes)| serde_json::from_slice::<Value>(&bytes).ok())
                .filter(|t| {
                    t["refresh_token"]
                        .as_str()
                        .is_some_and(|r| Some(r) != used.as_deref())
                });
            let _ = view.update(cx, |this, cx| {
                this.renewing = false;
                this.validating = false;
                match newer {
                    Some(tokens) => this.start(Some(tokens), cx),
                    None => {
                        this.busy = false;
                        this.code = None;
                        this.account = None;
                        this.status = message;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    /// Restoring the saved sign-in at startup (reading the vault, validating or
    /// renewing it), as opposed to a sign-in the user started.
    pub fn restoring(&self) -> bool {
        self.account.is_none()
            && self.busy
            && self.code.is_none()
            && (self.generation == 0 || self.started_refresh.is_some())
    }
    pub fn status(&self) -> &str {
        &self.status
    }
    pub fn login(&self) -> Option<String> {
        self.account.as_ref().map(|a| a.login.clone())
    }
    pub fn identity(&self) -> Option<crate::live::Identity> {
        self.account
            .as_ref()
            .filter(|a| Instant::now() < a.expires_at)
            .map(|a| crate::live::Identity {
                user_id: a.tokens["user_id"].as_str().unwrap_or("").into(),
                access: a.tokens["access_token"].as_str().unwrap_or("").into(),
                can_read_emotes: a.tokens["scopes"]
                    .as_array()
                    .is_some_and(|s| s.iter().any(|s| s == "user:read:emotes")),
                scopes: granted(&a.tokens).into(),
            })
    }
    /// Requested scopes the saved grant does not include yet.
    pub fn missing_scopes(&self) -> Vec<&'static str> {
        let Some(account) = &self.account else {
            return Vec::new();
        };
        let granted = granted(&account.tokens);
        SCOPES
            .iter()
            .copied()
            .filter(|s| !granted.iter().any(|g| g == s))
            .collect()
    }
    pub fn vault(&self) -> &SharedString {
        &self.vault
    }
    /// An account stored under its own vault entry.
    pub fn with_vault(vault: SharedString, cx: &mut Context<Self>) -> Self {
        let read = cx.read_credentials(&vault);
        cx.spawn(async move |view, cx| {
            let result = read.await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != 0 || !this.busy { return; }
                match result {
                    Ok(Some((_, bytes))) => match serde_json::from_slice::<Value>(&bytes) {
                        Ok(tokens) => this.start(Some(tokens), cx),
                        Err(_) => { this.busy = false; this.status = "Saved Twitch credentials could not be read. Sign in again to replace them.".into(); cx.notify(); }
                    },
                    Ok(None) => { this.busy = false; this.nothing_saved = true; this.status = "Sign in with Twitch for chat and all emotes available to your account.".into(); cx.notify(); }
                    Err(_) => { this.busy = false; this.vault_unavailable = true; this.status = "OS credential vault unavailable. Sign-in requires secure token storage.".into(); cx.notify(); }
                }
            });
        }).detach();
        Self {
            status: "Checking saved Twitch account…".into(),
            busy: true,
            saving: false,
            validating: false,
            renewing: false,
            upgrading: false,
            vault_unavailable: false,
            nothing_saved: false,
            code: None,
            copied_at: None,
            code_revealed: false,
            cancel: Arc::new(AtomicBool::new(false)),
            generation: 0,
            account: None,
            vault,
            started_refresh: None,
            retried_newer: false,
        }
    }
    fn start(&mut self, saved: Option<Value>, cx: &mut Context<Self>) {
        if self.saving || self.renewing {
            return;
        }
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        let generation = self.generation;
        self.busy = true;
        self.saving = false;
        self.code = None;
        self.copied_at = None;
        self.code_revealed = false;
        self.started_refresh = saved
            .as_ref()
            .and_then(|t| t["refresh_token"].as_str())
            .map(str::to_owned);
        self.validating = saved.is_some() && self.account.is_some();
        self.upgrading = saved.is_none() && self.account.is_some();
        if !self.validating && !self.upgrading {
            self.account = None;
        }
        self.status = if self.validating {
            "Checking your Twitch session… chat stays connected."
        } else if self.upgrading {
            "Requesting a Twitch code for the updated permissions… chat stays connected."
        } else if saved.is_some() {
            "Validating your Twitch account…"
        } else {
            "Requesting a Twitch sign-in code…"
        }
        .into();
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || match connect(saved, &cancel, &tx) {
            Ok(account) if !cancel.load(Ordering::Relaxed) => {
                let _ = tx.send(Event::Ready(account));
            }
            Err(error) if !cancel.load(Ordering::Relaxed) => {
                let _ = tx.send(Event::Error(error));
            }
            _ => {}
        });
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let active = view.update(cx, |this, _| this.generation == generation && this.busy).unwrap_or(false);
                if !active { break; }
                match rx.try_recv() {
                    Ok(event) => {
                        let done = matches!(event, Event::Ready(_) | Event::Error(_));
                        let _ = view.update(cx, |this, cx| {
                            if this.generation != generation { return; }
                            match event {
                                Event::Code(code, url) => { this.code = Some((code, url)); this.status = if this.upgrading { "Open Twitch, check its code matches this one (the eye shows it), and approve the updated permissions. You stay signed in meanwhile." } else { "Open Twitch, check its code matches this one (the eye shows it), and approve Jawjack. The code expires shortly." }.into(); }
                                Event::Renewing(ack) => { this.renewing = true; this.status = "Renewing Twitch session securely…".into(); let _ = ack.send(true); }
                                Event::Invalidated => { this.account = None; this.status = "Renewing expired Twitch authorization…".into(); }
                                Event::Error(message) if this.upgrading => this.keep_account(format!("Permissions were not updated: {message}"), cx),
                                Event::Error(message) if message.contains("expired or was revoked") && !this.retried_newer && this.started_refresh.is_some() => this.retry_newer(message, cx),
                                Event::Error(message) => { this.renewing = false; this.validating = false; this.busy = false; this.code = None; this.status = message; this.account = None; }
                                Event::Ready(account) => this.persist(account, generation, cx),
                                Event::Rotated(tokens, ack) => this.save_rotation(tokens, ack, generation, cx),
                            }
                            cx.notify();
                        });
                        if done { break; }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => break,
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
        }).detach();
        cx.notify();
    }
    fn save_rotation(
        &mut self,
        tokens: Value,
        ack: mpsc::SyncSender<bool>,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        self.saving = true;
        let bytes = serde_json::to_vec(&tokens).expect("JSON value serialization");
        let save = cx.write_credentials(&self.vault, "twitch", &bytes);
        cx.spawn(async move |view, cx| {
            let ok = save.await.is_ok();
            let active = view
                .update(cx, |this, cx| {
                    if this.generation != generation {
                        return false;
                    }
                    this.saving = false;
                    this.vault_unavailable |= !ok;
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            let _ = ack.send(ok && active);
        })
        .detach();
    }
    /// End an abandoned permission update without disturbing the active account.
    fn keep_account(&mut self, reason: String, cx: &mut Context<Self>) {
        self.upgrading = false;
        self.busy = false;
        self.code = None;
        self.status = format!("{reason} Still signed in as {}.", self.label());
        self.schedule_validation(self.generation, cx);
    }
    fn persist(&mut self, account: Account, generation: u64, cx: &mut Context<Self>) {
        self.retried_newer = false;
        self.renewing = false;
        self.upgrading = false;
        // A routine validation of unchanged credentials must not interrupt live chat
        // or rewrite the credential vault every half hour.
        if self
            .account
            .as_ref()
            .is_some_and(|old| old.tokens == account.tokens)
        {
            self.account = Some(account);
            self.busy = false;
            self.validating = false;
            self.status = format!("Signed in as {} · session checked", self.label());
            self.schedule_validation(generation, cx);
            cx.notify();
            return;
        }
        self.saving = true;
        self.code = None;
        self.status = "Saving account in the OS credential vault…".into();
        let bytes = serde_json::to_vec(&account.tokens).expect("JSON value serialization");
        let save = cx.write_credentials(&self.vault, &account.login, &bytes);
        cx.spawn(async move |view, cx| {
            let result = save.await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation { return; }
                this.busy = false;
                this.saving = false;
                this.validating = false;
                match result {
                    Ok(()) => {
                        this.status = format!("Signed in as {} · chat connects to your open channels", account.login);
                        this.account = Some(account);
                        this.schedule_validation(generation, cx);
                    }
                    Err(_) => { this.vault_unavailable = true; this.account = None; this.status = "Could not save tokens securely. Account was not activated; retry sign-in after fixing the OS vault.".into(); }
                }
                cx.notify();
            });
        }).detach();
    }
    fn schedule_validation(&self, generation: u64, cx: &mut Context<Self>) {
        let delay = self
            .account
            .as_ref()
            .map(|account| {
                account
                    .expires_at
                    .saturating_duration_since(Instant::now())
                    .saturating_sub(Duration::from_secs(60))
                    .clamp(Duration::from_secs(1), Duration::from_secs(1800))
            })
            .unwrap_or(Duration::from_secs(1800));
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(delay).await;
            let _ = view.update(cx, |this, cx| {
                if this.generation == generation
                    && !this.busy
                    && let Some(account) = &this.account
                {
                    this.start(Some(account.tokens.clone()), cx);
                }
            });
        })
        .detach();
    }
    fn copy_code(&mut self, code: &str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(code.to_owned()));
        let copied_at = Instant::now();
        self.copied_at = Some(copied_at);
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = view.update(cx, |this, cx| {
                if this.copied_at == Some(copied_at) {
                    this.copied_at = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
    fn retry_vault(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.saving || self.renewing {
            return;
        }
        self.busy = true;
        self.generation += 1;
        let generation = self.generation;
        self.status = "Checking secure account storage…".into();
        let read = cx.read_credentials(&self.vault);
        cx.spawn(async move |view, cx| {
            let result = read.await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation { return; }
                this.busy = false;
                match result {
                    Ok(saved) => {
                        this.vault_unavailable = false;
                        if let Some((_, bytes)) = saved {
                            match serde_json::from_slice::<Value>(&bytes) {
                                Ok(tokens) => this.start(Some(tokens), cx),
                                Err(_) => this.status = "Saved Twitch credentials could not be read. Sign in again to replace them.".into(),
                            }
                        } else {
                            this.status = "Secure storage is available. Sign in with Twitch to connect.".into();
                        }
                    }
                    Err(_) => {
                        this.vault_unavailable = true;
                        this.status = "Secure storage is still unavailable. Unlock your OS credential vault and retry.".into();
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.saving || self.validating || self.renewing {
            return;
        }
        self.cancel.store(true, Ordering::Relaxed);
        self.generation += 1;
        if self.upgrading {
            self.keep_account("Permission update cancelled.".into(), cx);
            cx.notify();
            return;
        }
        self.busy = false;
        self.code = None;
        self.status = "Sign-in cancelled. No account was connected.".into();
        cx.notify();
    }
    fn sign_out(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.saving || self.renewing {
            return;
        }
        self.generation += 1;
        self.account = None;
        self.busy = true;
        self.saving = true;
        self.status = "Removing saved account…".into();
        let delete = cx.delete_credentials(&self.vault);
        cx.spawn(async move |view, cx| {
            let result = delete.await;
            let _ = view.update(cx, |this, cx| {
                this.busy = false;
                this.saving = false;
                this.nothing_saved = result.is_ok();
                this.status = if result.is_ok() {
                    "Signed out on this device."
                } else {
                    "Account disconnected, but vault removal failed. Retry Remove saved account."
                }
                .into();
                cx.notify();
            });
        })
        .detach();
    }
}
impl Render for TwitchAccount {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .px_3()
            .py_2()
            .gap_2()
            .bg(rgb(theme::PANEL))
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .v_flex().w_full().min_w_0()
                    .gap_3()
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .text_size(px(12.))
                            .child(self.status.clone()),
                    )
                    .when(!self.busy && !self.saving && self.account.is_some() && !self.missing_scopes().is_empty(), |el|
                        el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED))
                            .child(format!("Your login is missing {} Twitch permissions Jawjack uses for subscription emotes, whispers, blocks, moderation and streamer tools (title, category, polls, raids). Updating keeps you signed in; Twitch only allows moderator and streamer actions where you hold that role.", self.missing_scopes().len())))
                        .child(Button::new("twitch-update-scopes").small().primary()
                            .label("Update Twitch permissions")
                            .on_click(cx.listener(|this, _, _, cx| this.start(None, cx)))))
                    .when(self.vault_unavailable, |el| el.child(div().text_size(px(11.))
                        .text_color(rgb(theme::MUTED))
                        .child("Jawjack needs your operating system's secure credential storage to remember Twitch safely. Unlock it, then retry here.")))
                    .child(div().h_flex().w_full().flex_wrap().gap_2()
                    .when(!self.busy && !self.saving && self.account.is_none() && !self.vault_unavailable, |el| {
                        el.child(
                            Button::new("twitch-sign-in")
                                .small()
                                .label("Sign in with Twitch")
                                .on_click(cx.listener(|this, _, _, cx| this.start(None, cx))),
                        )
                    })
                    .when(!self.busy && !self.saving && self.vault_unavailable, |el| {
                        el.child(Button::new("retry-twitch-vault").small()
                            .label("Retry secure storage")
                            .on_click(cx.listener(|this, _, _, cx| this.retry_vault(cx))))
                    })
                    .when(!self.busy && !self.saving && !self.vault_unavailable && (self.account.is_some() || !self.nothing_saved), |el| {
                        el.child(
                            Button::new("twitch-sign-out")
                                .small()
                                .label(if self.account.is_some() {
                                    "Sign out"
                                } else {
                                    "Remove saved account"
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.sign_out(cx))),
                        )
                    })
                    .when(self.busy && !self.saving && !self.validating && !self.renewing, |el| {
                        el.child(
                            Button::new("twitch-cancel")
                                .small()
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                        )
                    })),
            )
            .when_some(self.code.clone(), |el, (code, url)| {
                el.child(
                    div()
                        .v_flex().w_full().min_w_0()
                        .gap_3()
                        .child({
                            let revealed = self.code_revealed;
                            let shown = if revealed {
                                code.clone()
                            } else {
                                code.chars().map(|c| if c.is_alphanumeric() { '•' } else { c }).collect()
                            };
                            div()
                                .h_flex()
                                .items_center()
                                .gap_2()
                                .child(div().font_family("Consolas").text_size(px(20.)).child(shown))
                                .child(
                                    Button::new("reveal-twitch-code")
                                        .ghost()
                                        .small()
                                        .icon(if revealed { gpui_kit::component::IconName::EyeOff } else { gpui_kit::component::IconName::Eye })
                                        .tooltip(if revealed { "Hide code" } else { "Show code (Twitch shows it too; just check they match)" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.code_revealed = !this.code_revealed;
                                            cx.notify();
                                        })),
                                )
                        })
                        .child(
                            Button::new("copy-twitch-code")
                                .small()
                                .label(if self.copied_at.is_some() { "Copied!" } else { "Copy code" })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.copy_code(&code, cx)
                                })),
                        )
                        .child(
                            Button::new("open-twitch")
                                .small()
                                .label("Open Twitch authorization")
                                .on_click(move |_, _, cx| cx.open_url(&url)),
                        ),
                )
            })
    }
}

fn granted(tokens: &Value) -> Vec<String> {
    tokens["scopes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_owned))
        .collect()
}
fn string(value: &Value, key: &str) -> Result<String, String> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() < 16384)
        .map(str::to_owned)
        .ok_or_else(|| "Twitch returned an incomplete response. Try signing in again.".into())
}
fn response(response: reqwest::blocking::Response) -> Result<(u16, Value), String> {
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    response
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read Twitch response")?;
    if bytes.len() > 65536 {
        return Err("Twitch response exceeded the allowed size".into());
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|_| "Twitch returned an unexpected response. Try again shortly.")?;
    Ok((status, value))
}
fn validate(client: &reqwest::blocking::Client, tokens: &Value) -> Result<(u16, Value), String> {
    let access = string(tokens, "access_token")?;
    response(
        client
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("OAuth {access}"))
            .send()
            .map_err(|_| "Could not reach Twitch to validate the account")?,
    )
}
fn connect(
    saved: Option<Value>,
    cancel: &AtomicBool,
    events: &mpsc::Sender<Event>,
) -> Result<Account, String> {
    // Older saved grants keep chat working until the owner signs in again.
    let required_scopes = if saved.is_some() {
        EXISTING_CHAT_SCOPES
    } else {
        SCOPES
    };
    let scopes = requested_scopes();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialize secure Twitch networking")?;
    let mut tokens = if let Some(saved) = saved {
        saved
    } else {
        let (status, value) = response(
            client
                .post("https://id.twitch.tv/oauth2/device")
                .form(&[("client_id", CLIENT_ID), ("scopes", scopes.as_str())])
                .send()
                .map_err(|_| "Could not reach Twitch. Check your connection and retry.")?,
        )?;
        if status != 200 {
            return Err(format!(
                "Twitch could not start sign-in (HTTP {status}). Check that Jawjack is registered as a Public client."
            ));
        }
        let code = string(&value, "user_code")?;
        let device = string(&value, "device_code")?;
        let url = string(&value, "verification_uri")?;
        let parsed = reqwest::Url::parse(&url).map_err(|_| "Invalid Twitch authorization URL")?;
        if parsed.scheme() != "https"
            || !matches!(
                parsed.host_str(),
                Some("www.twitch.tv" | "twitch.tv" | "id.twitch.tv")
            )
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err("Unexpected authorization destination; sign-in stopped".into());
        }
        let mut interval = value["interval"].as_u64().unwrap_or(5).clamp(5, 60);
        let lifetime = value["expires_in"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 3600)
            .ok_or("Invalid sign-in expiry")?;
        let deadline = Instant::now() + Duration::from_secs(lifetime);
        let _ = events.send(Event::Code(code, url));
        loop {
            let next = Instant::now() + Duration::from_secs(interval);
            while Instant::now() < next {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                if Instant::now() >= deadline {
                    return Err("Twitch sign-in code expired. Start sign-in again.".into());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let (status, value) = response(
                client
                    .post("https://id.twitch.tv/oauth2/token")
                    .form(&[
                        ("client_id", CLIENT_ID),
                        ("scopes", scopes.as_str()),
                        ("device_code", device.as_str()),
                        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ])
                    .send()
                    .map_err(|_| "Connection interrupted during sign-in. Please start again.")?,
            )?;
            if status == 200 {
                break value;
            }
            match value["message"]
                .as_str()
                .or(value["error"].as_str())
                .unwrap_or("")
            {
                "authorization_pending" => {}
                "slow_down" => interval = (interval + 5).min(120),
                "access_denied" => return Err("Twitch authorization was declined.".into()),
                "expired_token" => return Err("Twitch sign-in code expired. Start again.".into()),
                _ => {
                    return Err(format!(
                        "Twitch sign-in failed (HTTP {status}). Please start again."
                    ));
                }
            }
        }
    };
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    let (mut status, mut identity) = validate(&client, &tokens)?;
    if status == 401 {
        // Stop exposing a token Twitch has explicitly rejected before refreshing it.
        let _ = events.send(Event::Invalidated);
    }
    if status == 401 || (status == 200 && identity["expires_in"].as_u64().unwrap_or(0) < 3600) {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        // Commit the renewal transaction on the UI thread before rotating credentials.
        // A cancellation queued during validation cannot orphan a rotated refresh token.
        let (ready, wait) = mpsc::sync_channel(1);
        events
            .send(Event::Renewing(ready))
            .map_err(|_| "Account window closed")?;
        if !wait.recv_timeout(Duration::from_secs(30)).unwrap_or(false)
            || cancel.load(Ordering::Relaxed)
        {
            return Err("Session renewal cancelled before contacting Twitch".into());
        }
        let refresh = string(&tokens, "refresh_token")?;
        let (refresh_status,replacement) = response(client.post("https://id.twitch.tv/oauth2/token").form(&[("client_id",CLIENT_ID),("grant_type","refresh_token"),("refresh_token",refresh.as_str())]).send().map_err(|_|"Token refresh interrupted. Sign in again if the saved token no longer works.")?)?;
        if refresh_status != 200 {
            return Err("Saved Twitch authorization expired or was revoked. Sign in again.".into());
        }
        // Persist rotated one-use refresh credentials before any further network call.
        string(&replacement, "access_token")?;
        string(&replacement, "refresh_token")?;
        let (ack, receive) = mpsc::sync_channel(1);
        events
            .send(Event::Rotated(replacement.clone(), ack))
            .map_err(|_| "Account window closed")?;
        if !receive
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or(false)
        {
            return Err("Could not securely save renewed authorization. Sign in again.".into());
        }
        tokens = replacement;
        (status, identity) = validate(&client, &tokens)?;
    }
    if status != 200 {
        return Err(format!("Twitch account validation failed (HTTP {status})."));
    }
    if identity["client_id"].as_str() != Some(CLIENT_ID) {
        return Err("Saved authorization belongs to a different app. Sign in again.".into());
    }
    for scope in required_scopes {
        if !identity["scopes"]
            .as_array()
            .is_some_and(|s| s.iter().any(|v| v.as_str() == Some(*scope)))
        {
            return Err("Twitch authorization is missing chat permissions. Sign in again.".into());
        }
    }
    let login = string(&identity, "login")?;
    let user_id = string(&identity, "user_id")?;
    let access = string(&tokens, "access_token")?;
    let refresh = string(&tokens, "refresh_token")?;
    let remaining = identity["expires_in"]
        .as_u64()
        .filter(|n| *n > 30 && *n <= 31_536_000)
        .ok_or("Twitch returned an expired or too-short session. Please sign in again")?;
    // Conservatively cover validation-request time and avoid exposing nearly expired tokens.
    let expires_at = Instant::now() + Duration::from_secs(remaining.saturating_sub(30));
    Ok(Account {
        login,
        expires_at,
        tokens: json!({"access_token":access,"refresh_token":refresh,"user_id":user_id,"client_id":CLIENT_ID,"scopes":identity["scopes"]}),
    })
}
