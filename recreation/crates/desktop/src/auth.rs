//! Twitch public-client device login. Secrets remain in memory and the OS vault.
use crate::theme;
use gpui_kit::component::{Sizable, StyledExt, button::Button};
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
const VAULT: &str = "https://jawjack.zombie.digital/twitch/account-v1";
const SCOPES: &str = "user:read:chat user:write:chat";
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
    upgrading: bool,
    vault_unavailable: bool,
    code: Option<(String, String)>,
    copied_at: Option<Instant>,
    cancel: Arc<AtomicBool>,
    generation: u64,
    account: Option<Account>,
}
impl Drop for TwitchAccount {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl TwitchAccount {
    pub fn label(&self)->String {self.account.as_ref().map(|a|a.login.clone()).unwrap_or_else(||"Sign in".into())}
    pub fn identity(&self) -> Option<crate::live::Identity> {
        self.account.as_ref().filter(|a| Instant::now() < a.expires_at).map(|a| crate::live::Identity {
            user_id: a.tokens["user_id"].as_str().unwrap_or("").into(),
            access: a.tokens["access_token"].as_str().unwrap_or("").into(),
            can_read_emotes: a.tokens["scopes"].as_array().is_some_and(|s|s.iter().any(|s|s=="user:read:emotes")),
        })
    }
    pub fn new(cx: &mut Context<Self>) -> Self {
        let read = cx.read_credentials(VAULT);
        cx.spawn(async move |view, cx| {
            let result = read.await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != 0 || !this.busy { return; }
                match result {
                    Ok(Some((_, bytes))) => match serde_json::from_slice::<Value>(&bytes) {
                        Ok(tokens) => this.start(Some(tokens), cx),
                        Err(_) => { this.busy = false; this.status = "Saved Twitch credentials could not be read. Sign in again to replace them.".into(); cx.notify(); }
                    },
                    Ok(None) => { this.busy = false; this.status = "Sign in with Twitch to connect your account.".into(); cx.notify(); }
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
            code: None,
            copied_at: None,
            cancel: Arc::new(AtomicBool::new(false)),
            generation: 0,
            account: None,
        }
    }
    fn start(&mut self, saved: Option<Value>, cx: &mut Context<Self>) {
        self.start_scoped(saved, SCOPES.to_owned(), false, cx);
    }
    fn start_scoped(&mut self, saved: Option<Value>, scopes: String, upgrade: bool, cx: &mut Context<Self>) {
        if self.saving || self.renewing { return; }
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        let generation = self.generation;
        self.busy = true;
        self.saving = false;
        self.code = None;
        self.copied_at = None;
        self.upgrading = upgrade && self.account.is_some();
        let expected_user = if self.upgrading {self.account.as_ref().and_then(|a|a.tokens["user_id"].as_str()).map(str::to_owned)}else{None};
        self.validating = saved.is_some() && self.account.is_some();
        if !self.validating && !self.upgrading { self.account = None; }
        self.status = if self.validating {
            "Checking your Twitch session… chat stays connected."
        } else if saved.is_some() {
            "Validating your Twitch account…"
        } else {
            "Requesting a Twitch sign-in code…"
        }
        .into();
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || match connect(saved, &scopes, expected_user.as_deref(), &cancel, &tx) {
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
                                Event::Code(code, url) => { this.code = Some((code, url)); this.status = "Enter this code on Twitch and approve Jawjack. This code expires shortly.".into(); }
                                Event::Renewing(ack) => { this.renewing = true; this.status = "Renewing Twitch session securely…".into(); let _ = ack.send(true); }
                                Event::Invalidated => { if !this.upgrading {this.account = None;} this.status = "Renewing expired Twitch authorization…".into(); }
                                Event::Error(message) => {
                                    this.renewing = false; this.validating = false; this.busy = false; this.code = None;
                                    if this.upgrading && this.account.is_some() {
                                        this.status = format!("{message} Your existing chat account is unchanged.");
                                        this.upgrading = false; this.schedule_validation(generation,cx);
                                    } else {this.status = message; this.account = None; this.upgrading = false;}
                                }
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
        let save = cx.write_credentials(VAULT, "twitch", &bytes);
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
    fn persist(&mut self, account: Account, generation: u64, cx: &mut Context<Self>) {
        self.renewing = false;
        let upgrading = self.upgrading;
        self.upgrading = false;
        // A routine validation of unchanged credentials must not interrupt live chat
        // or rewrite the credential vault every half hour.
        if self.account.as_ref().is_some_and(|old| old.tokens == account.tokens) {
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
        let save = cx.write_credentials(VAULT, &account.login, &bytes);
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
                    Err(_) => {
                        this.vault_unavailable = true;
                        if upgrading && this.account.is_some() {
                            this.status = "Could not save the emote upgrade securely. Your existing chat session remains active; retry secure storage before upgrading again.".into();
                            this.schedule_validation(generation,cx);
                        } else {this.account = None; this.status = "Could not save tokens securely. Account was not activated; retry sign-in after fixing the OS vault.".into();}
                    }
                }
                cx.notify();
            });
        }).detach();
    }
    fn schedule_validation(&self, generation: u64, cx: &mut Context<Self>) {
        let delay = self.account.as_ref().map(|account| {
            account.expires_at.saturating_duration_since(Instant::now())
                .saturating_sub(Duration::from_secs(60))
                .clamp(Duration::from_secs(1), Duration::from_secs(1800))
        }).unwrap_or(Duration::from_secs(1800));
        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(delay)
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.generation == generation && !this.busy {
                    if let Some(account) = &this.account {
                        this.start(Some(account.tokens.clone()), cx);
                    }
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
        }).detach();
        cx.notify();
    }
    fn retry_vault(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.saving || self.renewing { return; }
        self.busy = true;
        self.generation += 1;
        let generation = self.generation;
        self.status = "Checking secure account storage…".into();
        let read = cx.read_credentials(VAULT);
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
        self.busy = false;
        self.code = None;
        self.status = if self.upgrading && self.account.is_some() {
            self.upgrading = false;
            self.schedule_validation(self.generation,cx);
            "Emote permission setup cancelled. Your existing chat account is unchanged."
        } else {"Sign-in cancelled. No account was connected."}.into();
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
        let delete = cx.delete_credentials(VAULT);
        cx.spawn(async move |view, cx| {
            let result = delete.await;
            let _ = view.update(cx, |this, cx| {
                this.busy = false;
                this.saving = false;
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
                    .when(!self.busy && !self.saving && !self.vault_unavailable && self.account.is_some()
                        && !self.identity().is_some_and(|i|i.can_read_emotes), |el| {
                        el.child(Button::new("enable-twitch-owned-emotes").small()
                            .label("Enable subscription emotes")
                            .tooltip("Requests Twitch permission to read emotes available to your account. Existing chat stays connected during setup.")
                            .on_click(cx.listener(|this,_,_,cx| this.start_scoped(None,format!("{SCOPES} user:read:emotes"),true,cx))))
                    })
                    .when(!self.busy && !self.saving && self.vault_unavailable, |el| {
                        el.child(Button::new("retry-twitch-vault").small()
                            .label("Retry secure storage")
                            .on_click(cx.listener(|this, _, _, cx| this.retry_vault(cx))))
                    })
                    .when(!self.busy && !self.saving && !self.vault_unavailable, |el| {
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
                        .child(
                            div()
                                .font_family("Consolas")
                                .text_size(px(20.))
                                .child(code.clone()),
                        )
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
    requested_scopes: &str,
    expected_user: Option<&str>,
    cancel: &AtomicBool,
    events: &mpsc::Sender<Event>,
) -> Result<Account, String> {
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
                .form(&[("client_id", CLIENT_ID), ("scopes", requested_scopes)])
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
                        ("scopes", requested_scopes),
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
    if let Some(expected)=expected_user {
        if status!=200 || identity["user_id"].as_str()!=Some(expected) || identity["client_id"].as_str()!=Some(CLIENT_ID) {
            return Err("Authorize the same Twitch account already signed into Jawjack. The emote upgrade was not applied.".into());
        }
    }
    if status == 401 {
        // Stop exposing a token Twitch has explicitly rejected before refreshing it.
        let _ = events.send(Event::Invalidated);
    }
    if status == 401 || (status == 200 && identity["expires_in"].as_u64().unwrap_or(0) < 3600) {
        if cancel.load(Ordering::Relaxed) { return Err("Cancelled".into()); }
        // Commit the renewal transaction on the UI thread before rotating credentials.
        // A cancellation queued during validation cannot orphan a rotated refresh token.
        let (ready, wait) = mpsc::sync_channel(1);
        events.send(Event::Renewing(ready)).map_err(|_| "Account window closed")?;
        if !wait.recv_timeout(Duration::from_secs(30)).unwrap_or(false)
            || cancel.load(Ordering::Relaxed) {
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
    for scope in requested_scopes.split_whitespace() {
        if !identity["scopes"]
            .as_array()
            .is_some_and(|s| s.iter().any(|v| v.as_str() == Some(scope)))
        {
            return Err("Twitch authorization is missing chat permissions. Sign in again.".into());
        }
    }
    let login = string(&identity, "login")?;
    let user_id = string(&identity, "user_id")?;
    let access = string(&tokens, "access_token")?;
    let refresh = string(&tokens, "refresh_token")?;
    let remaining = identity["expires_in"].as_u64().filter(|n| *n > 30 && *n <= 31_536_000)
        .ok_or("Twitch returned an expired or too-short session. Please sign in again")?;
    // Conservatively cover validation-request time and avoid exposing nearly expired tokens.
    let expires_at = Instant::now() + Duration::from_secs(remaining.saturating_sub(30));
    Ok(Account {
        login,
        expires_at,
        tokens: json!({"access_token":access,"refresh_token":refresh,"user_id":user_id,"client_id":CLIENT_ID,"scopes":identity["scopes"]}),
    })
}
