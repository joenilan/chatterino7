//! Streamer stream-info editor: title and category of the signed-in account's own
//! channel. Fetches current values first, saves only on an explicit Save, detects
//! changes made elsewhere meanwhile, and reads the result back from Twitch.
use crate::{auth::CLIENT_ID, auth::TwitchAccount, live::Identity, theme};
use gpui_kit::component::{
    Disableable, Selectable, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::{Value, json};
use std::time::Duration;

const TITLE_LIMIT: usize = 140;

#[derive(Clone, PartialEq, Eq, Default)]
struct Info {
    title: String,
    game_id: String,
    game_name: String,
}
#[derive(Clone)]
struct Category {
    id: String,
    name: String,
}

pub struct StreamEditor {
    account: Entity<TwitchAccount>,
    title: Entity<InputState>,
    search: Entity<InputState>,
    /// Values when the form was loaded; used to detect changes made elsewhere.
    snapshot: Option<Info>,
    category: Option<Category>,
    results: Vec<Category>,
    search_revision: u64,
    busy: bool,
    status: String,
    error: bool,
    /// Shown inside the Studio panel rather than a dialog.
    embedded: bool,
}

fn helix(
    identity: &Identity,
    request: reqwest::blocking::RequestBuilder,
) -> Result<(u16, Value), String> {
    let response = request
        .header("Client-Id", CLIENT_ID)
        .bearer_auth(&identity.access)
        .send()
        .map_err(|_| "Could not reach Twitch. Your edits are kept.")?;
    let status = response.status().as_u16();
    if status == 204 {
        return Ok((status, Value::Null));
    }
    crate::live::response_json(response)
}
fn explain(status: u16, value: &Value) -> String {
    let message = value["message"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    match status {
        401 => {
            "Twitch rejected the session. Update Twitch permissions from the account menu.".into()
        }
        403 => "Twitch did not allow this change. Update Twitch permissions from the account menu."
            .into(),
        429 => "Twitch is rate limiting requests. Try again in a moment.".into(),
        _ if !message.is_empty() => format!("Twitch: {message}"),
        _ => format!("Twitch returned HTTP {status}."),
    }
}
fn fetch(identity: &Identity) -> Result<Info, String> {
    let client = crate::live::client()?;
    let (status, value) = helix(
        identity,
        client
            .get("https://api.twitch.tv/helix/channels")
            .query(&[("broadcaster_id", identity.user_id.as_str())]),
    )?;
    if status != 200 {
        return Err(explain(status, &value));
    }
    let data = &value["data"][0];
    if data.is_null() {
        return Err("Twitch returned no channel information.".into());
    }
    let text = |key: &str| data[key].as_str().unwrap_or("").to_owned();
    Ok(Info {
        title: text("title"),
        game_id: text("game_id"),
        game_name: text("game_name"),
    })
}
fn search(identity: &Identity, query: &str) -> Result<Vec<Category>, String> {
    let client = crate::live::client()?;
    let (status, value) = helix(
        identity,
        client
            .get("https://api.twitch.tv/helix/search/categories")
            .query(&[("query", query), ("first", "8")]),
    )?;
    if status != 200 {
        return Err(explain(status, &value));
    }
    Ok(value["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            Some(Category {
                id: c["id"].as_str()?.to_owned(),
                name: c["name"].as_str()?.chars().take(100).collect(),
            })
        })
        .collect())
}
enum Saved {
    Confirmed(Info),
    /// Twitch accepted the change but still reports something else.
    Pending(Info),
    /// Someone changed the channel elsewhere after the form loaded.
    Conflict(Info),
}
fn save(identity: &Identity, snapshot: &Info, wanted: &Info) -> Result<Saved, String> {
    let current = fetch(identity)?;
    if current != *snapshot {
        return Ok(Saved::Conflict(current));
    }
    let mut body = serde_json::Map::new();
    if wanted.title != current.title {
        body.insert("title".into(), json!(wanted.title));
    }
    if wanted.game_id != current.game_id {
        body.insert("game_id".into(), json!(wanted.game_id));
    }
    let client = crate::live::client()?;
    let (status, value) = helix(
        identity,
        client
            .patch("https://api.twitch.tv/helix/channels")
            .query(&[("broadcaster_id", identity.user_id.as_str())])
            .header("Content-Type", "application/json")
            .body(Value::Object(body).to_string()),
    )?;
    if status != 204 {
        return Err(explain(status, &value));
    }
    let after = fetch(identity)
        .map_err(|e| format!("Twitch accepted the change, but reading it back failed: {e}"))?;
    Ok(
        if after.title == wanted.title && after.game_id == wanted.game_id {
            Saved::Confirmed(after)
        } else {
            Saved::Pending(after)
        },
    )
}

impl StreamEditor {
    pub fn new(
        account: Entity<TwitchAccount>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Stream title")
                .validate(|v, _| {
                    v.chars().count() <= TITLE_LIMIT && !v.chars().any(char::is_control)
                })
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search categories"));
        cx.subscribe_in(
            &title,
            window,
            |this: &mut Self, _, event, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.save(window, cx),
                _ => {}
            },
        )
        .detach();
        cx.subscribe_in(&search, window, |this: &mut Self, _, event, window, cx| {
            if let InputEvent::Change = event {
                this.schedule_search(window, cx);
            }
        })
        .detach();
        let mut this = Self {
            account,
            title,
            search,
            snapshot: None,
            category: None,
            results: Vec::new(),
            search_revision: 0,
            busy: false,
            status: String::new(),
            error: false,
            embedded: false,
        };
        this.load(window, cx);
        this
    }
    /// Use inside a panel: Cancel reverts the edits and reloads instead of closing.
    pub fn embed(&mut self) {
        self.embedded = true;
    }
    fn identity(&self, cx: &App) -> Result<Identity, String> {
        let identity = self
            .account
            .read(cx)
            .identity()
            .ok_or("Sign in with Twitch first.")?;
        // Checked again right before every write: the grant may have changed.
        if !identity.has("channel:manage:broadcast") {
            return Err("Editing stream info needs updated Twitch permissions. Update them from the account menu.".into());
        }
        Ok(identity)
    }
    fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.busy = false;
        self.error = true;
        self.status = message;
        cx.notify();
    }
    /// Fetch current values; replaces the form only when it was not edited.
    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let identity = match self.identity(cx) {
            Ok(identity) => identity,
            Err(e) => return self.fail(e, cx),
        };
        self.busy = true;
        self.error = false;
        self.status = "Loading current stream info…".into();
        let task = cx.background_spawn(async move { fetch(&identity) });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, window, cx| match result {
                Ok(info) => {
                    this.busy = false;
                    this.status.clear();
                    this.apply(info, window, cx);
                }
                Err(e) => this.fail(e, cx),
            });
        })
        .detach();
        cx.notify();
    }
    fn apply(&mut self, info: Info, window: &mut Window, cx: &mut Context<Self>) {
        self.title
            .update(cx, |i, cx| i.set_value(info.title.clone(), window, cx));
        self.category = (!info.game_id.is_empty()).then(|| Category {
            id: info.game_id.clone(),
            name: info.game_name.clone(),
        });
        self.snapshot = Some(info);
        cx.notify();
    }
    fn wanted(&self, cx: &App) -> Info {
        Info {
            title: self.title.read(cx).value().trim().to_owned(),
            game_id: self
                .category
                .as_ref()
                .map(|c| c.id.clone())
                .unwrap_or_default(),
            game_name: self
                .category
                .as_ref()
                .map(|c| c.name.clone())
                .unwrap_or_default(),
        }
    }
    fn changed(&self, cx: &App) -> bool {
        self.snapshot.as_ref().is_some_and(|s| {
            let wanted = self.wanted(cx);
            wanted.title != s.title || wanted.game_id != s.game_id
        })
    }
    fn schedule_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_revision += 1;
        let revision = self.search_revision;
        let query = self.search.read(cx).value().trim().to_owned();
        if query.is_empty() {
            self.results.clear();
            cx.notify();
            return;
        }
        let Ok(identity) = self.account.read(cx).identity().ok_or(()) else {
            return;
        };
        cx.spawn_in(window, async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let current = view
                .update(cx, |this, _| this.search_revision == revision)
                .unwrap_or(false);
            if !current {
                return;
            }
            let result = cx
                .background_spawn(async move { search(&identity, &query) })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.search_revision != revision {
                    return;
                }
                match result {
                    Ok(results) => {
                        if results.is_empty() {
                            this.status = "No matching categories.".into();
                            this.error = false;
                        } else if !this.error {
                            this.status.clear();
                        }
                        this.results = results;
                    }
                    Err(e) => {
                        this.status = e;
                        this.error = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !self.changed(cx) {
            return;
        }
        let wanted = self.wanted(cx);
        if wanted.title.is_empty() {
            return self.fail("Enter a title.".into(), cx);
        }
        if wanted.title.chars().count() > TITLE_LIMIT {
            return self.fail(format!("Titles can be up to {TITLE_LIMIT} characters."), cx);
        }
        let identity = match self.identity(cx) {
            Ok(identity) => identity,
            Err(e) => return self.fail(e, cx),
        };
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        self.busy = true;
        self.error = false;
        self.status = "Saving to Twitch…".into();
        let task = cx.background_spawn(async move { save(&identity, &snapshot, &wanted) });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, _, cx| {
                this.busy = false;
                match result {
                    Ok(Saved::Confirmed(info)) => {
                        this.snapshot = Some(info);
                        this.status = "Saved. Twitch confirms the new stream info.".into();
                    }
                    Ok(Saved::Pending(info)) => {
                        this.snapshot = Some(info);
                        this.status = "Twitch accepted the change but still reports the old value; it may take a moment to update.".into();
                    }
                    Ok(Saved::Conflict(current)) => {
                        // Keep the owner's edits; show what changed and require another Save.
                        this.error = true;
                        this.status = format!(
                            "Changed elsewhere since this opened: now “{}” · {}. Your edits are kept; Save again to replace them.",
                            current.title,
                            if current.game_name.is_empty() { "no category" } else { &current.game_name }
                        );
                        this.snapshot = Some(current);
                    }
                    Err(e) => {
                        this.error = true;
                        this.status = e;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
impl Render for StreamEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let changed = self.changed(cx);
        let count = self.title.read(cx).value().chars().count();
        let current = self.category.as_ref().map(|c| c.id.clone());
        let label = |text: &'static str| {
            div()
                .text_size(px(11.))
                .text_color(rgb(theme::MUTED))
                .child(text)
        };
        div()
            .v_flex()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .child(label("Title"))
                    .child(
                        label(if count > TITLE_LIMIT { "Too long" } else { "" })
                            .child(format!(" {count}/{TITLE_LIMIT}")),
                    ),
            )
            .child(
                Input::new(&self.title)
                    .small()
                    .disabled(self.snapshot.is_none()),
            )
            .child(label("Category"))
            .child(div().text_size(px(13.)).child(match &self.category {
                Some(c) => format!("✓ {}", c.name),
                None => "No category".into(),
            }))
            .child(
                Input::new(&self.search)
                    .small()
                    .disabled(self.snapshot.is_none()),
            )
            .when(!self.results.is_empty(), |el| {
                el.child(div().h_flex().flex_wrap().gap_1().children(
                    self.results.iter().enumerate().map(|(index, category)| {
                        let selected = current.as_deref() == Some(category.id.as_str());
                        let choice = category.clone();
                        Button::new(SharedString::from(format!("category-{index}")))
                            .small()
                            .selected(selected)
                            .when(selected, |b| b.primary())
                            .label(category.name.clone())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.category = Some(choice.clone());
                                cx.notify();
                            }))
                    }),
                ))
            })
            .when(!self.status.is_empty(), |el| {
                el.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(if self.error { 0xE9C99C } else { theme::MUTED }))
                        .child(self.status.clone()),
                )
            })
            .child(
                div()
                    .h_flex()
                    .justify_end()
                    .gap_2()
                    .pt_1()
                    .when(self.snapshot.is_none() && !self.busy, |el| {
                        el.child(
                            Button::new("stream-info-retry")
                                .small()
                                .label("Retry")
                                .on_click(cx.listener(|this, _, w, cx| this.load(w, cx))),
                        )
                    })
                    .child(if self.embedded {
                        Button::new("stream-info-cancel")
                            .small()
                            .label(if changed { "Revert" } else { "Reload" })
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, w, cx| this.load(w, cx)))
                    } else {
                        Button::new("stream-info-cancel")
                            .small()
                            .label(if changed { "Cancel" } else { "Close" })
                            .on_click(|_, window, cx| window.close_dialog(cx))
                    })
                    .child(
                        Button::new("stream-info-save")
                            .small()
                            .primary()
                            .disabled(self.busy || !changed)
                            .label(if self.busy && self.snapshot.is_some() {
                                "Saving…"
                            } else {
                                "Save"
                            })
                            .on_click(cx.listener(|this, _, w, cx| this.save(w, cx))),
                    ),
            )
    }
}
