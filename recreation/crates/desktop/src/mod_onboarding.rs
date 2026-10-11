//! Moderator onboarding: offer to open the channels you moderate as a new
//! "Moderating" workspace, with live ones marked and each one optional.
use super::*;
use std::collections::BTreeSet;

/// Live logins among `channels` (Helix Get Streams, 100 per request).
fn live_among(identity: &crate::live::Identity, channels: &[String]) -> BTreeSet<String> {
    let mut live = BTreeSet::new();
    let Ok(client) = crate::live::client() else {
        return live;
    };
    for chunk in channels.chunks(100) {
        let mut query: Vec<(&str, &str)> =
            chunk.iter().map(|c| ("user_login", c.as_str())).collect();
        query.push(("first", "100"));
        let Some((200, value)) = client
            .get("https://api.twitch.tv/helix/streams")
            .query(&query)
            .header("Client-Id", crate::auth::CLIENT_ID)
            .bearer_auth(&identity.access)
            .send()
            .ok()
            .and_then(|r| crate::live::response_json(r).ok())
        else {
            continue;
        };
        live.extend(
            value["data"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s["user_login"].as_str().map(str::to_ascii_lowercase)),
        );
    }
    live
}

impl Workbench {
    pub(super) fn offer_moderated_channels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut channels) = self.capabilities.moderated() else {
            window.push_notification(
                Notification::info("Still checking which channels you moderate. Try again from Settings > General in a moment."),
                cx,
            );
            return;
        };
        if channels.is_empty() {
            return;
        }
        channels.sort();
        let open: BTreeSet<String> = self
            .tab_panes()
            .iter()
            .map(|p| p.read(cx).name.to_string())
            .collect();
        self.mod_pick = channels
            .iter()
            .filter(|c| !open.contains(*c))
            .cloned()
            .collect();
        self.mod_live = None;
        if let Some(identity) = self.account.read(cx).identity() {
            let list = channels.clone();
            let task = cx.background_spawn(async move { live_among(&identity, &list) });
            cx.spawn(async move |view, cx| {
                let live = task.await;
                let _ = view.update(cx, |this, cx| {
                    this.mod_live = Some(live);
                    cx.notify();
                });
            })
            .detach();
        }
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(260., 440.);
        let height = (f32::from(window.viewport_size().height) - 260.).clamp(160., 420.);
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(this) = owner.upgrade() else { return dialog.title("Workspace closed") };
            let (picked, live) = {
                let this = this.read(cx);
                (this.mod_pick.clone(), this.mod_live.clone())
            };
            let set = |pick: BTreeSet<String>| {
                let owner = owner.clone();
                move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    let pick = pick.clone();
                    let _ = owner.update(cx, |this, cx| {
                        this.mod_pick = pick;
                        cx.notify();
                    });
                }
            };
            let all: BTreeSet<String> = channels.iter().cloned().collect();
            let live_only: BTreeSet<String> = live
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter(|c| all.contains(c) && !open.contains(c))
                .collect();
            let count = picked.len();
            let rows = channels.iter().enumerate().map(|(index, channel)| {
                let on = picked.contains(channel);
                let is_open = open.contains(channel);
                let is_live = live.as_ref().is_some_and(|l| l.contains(channel));
                let (owner, toggle) = (owner.clone(), channel.clone());
                div()
                    .id(("mod-pick", index))
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .h(px(26.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::HOVER)))
                    .child(
                        div()
                            .size(px(14.))
                            .flex_shrink_0()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(rgb(if on { theme::ACCENT } else { theme::MUTED }))
                            .when(on, |el| el.bg(rgb(theme::ACCENT)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(0x16121F))
                            .when(on, |el| el.child(Icon::new(IconName::Check).xsmall())),
                    )
                    .child(div().flex_1().min_w_0().text_size(px(13.)).child(channel.clone()))
                    .when(is_live, |el| el.child(div().size(px(7.)).rounded_full().bg(rgb(theme::STREAM_LIVE))))
                    .when(is_open, |el| el.child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).child("open")))
                    .on_click(move |_, _, cx| {
                        let toggle = toggle.clone();
                        let _ = owner.update(cx, |this, cx| {
                            if !this.mod_pick.remove(&toggle) {
                                this.mod_pick.insert(toggle);
                            }
                            cx.notify();
                        });
                    })
            });
            let create_owner = owner.clone();
            dialog
                .w(px(width))
                .title("Open the channels you moderate?")
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!(
                            "You moderate {} channels. Each one you pick opens as a tab in one “Moderating” workspace (made once, then reused). You can close or add channels anytime. Live ones have a green dot{}.",
                            channels.len(),
                            if live.is_none() { " (checking…)" } else { "" }
                        )))
                        .child(
                            div()
                                .h_flex()
                                .gap_1()
                                .child(Button::new("mod-pick-all").xsmall().ghost().label("All").on_click(set(all.clone())))
                                .child(Button::new("mod-pick-live").xsmall().ghost().label("Live only").disabled(live.is_none()).on_click(set(live_only)))
                                .child(Button::new("mod-pick-none").xsmall().ghost().label("None").on_click(set(BTreeSet::new()))),
                        )
                        .child(div().id("mod-pick-list").h(px(height)).overflow_y_scroll().children(rows)),
                )
                .footer(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap_2()
                        .child(Button::new("mod-pick-skip").small().label("Not now").on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(
                            Button::new("mod-pick-create")
                                .small()
                                .primary()
                                .label(format!("Open {count} as tabs in “Moderating”"))
                                .disabled(count == 0)
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = create_owner.update(cx, |this, cx| this.create_moderating_workspace(window, cx));
                                }),
                        ),
                )
        });
    }
    fn create_moderating_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let channels: Vec<String> = std::mem::take(&mut self.mod_pick).into_iter().collect();
        if channels.is_empty() {
            return;
        }
        // One "Moderating" workspace: reuse it if it exists, else make it.
        if let Some(index) = self.tabs.iter().position(|t| t.name == "Moderating") {
            self.select_tab(index, cx);
        } else {
            let id = self.next_id;
            self.next_id += 1;
            self.tabs.push(WorkspaceTab {
                id,
                name: "Moderating".into(),
                panes: vec![],
                vertical: false,
                sizes: vec![],
                split: cx.new(|_| ResizableState::default()),
                dock: None,
                dock_states: BTreeMap::new(),
                pinned: Vec::new(),
            });
            self.select_tab(self.tabs.len() - 1, cx);
        }
        let existing: Vec<String> = self.tabs[self.active]
            .panes
            .iter()
            .map(|p| p.read(cx).name.to_string())
            .collect();
        let channels: Vec<String> = channels
            .into_iter()
            .filter(|c| !existing.contains(c))
            .collect();
        for channel in &channels {
            self.add_channel_to_active(channel.clone(), window, cx);
        }
        self.notice = Some((
            format!(
                "Opened {} channels you moderate in “Moderating”.",
                channels.len()
            ),
            std::time::Instant::now(),
        ));
        self.schedule_save(cx);
        cx.notify();
    }
}
