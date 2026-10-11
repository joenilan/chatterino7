//! Streamer.bot in the Studio panel: connection, pinned action buttons, the
//! action picker, and running an action from a chat message's menu.
use super::*;
use crate::streamer_bot::{Action, MAX_PINNED, Settings};

const VAULT: &str = "https://jawjack.zombie.digital/streamerbot/websocket-v1";

pub(super) fn load_settings(value: &Value) -> (Settings, Vec<Action>) {
    let settings = Settings {
        enabled: value["enabled"].as_bool().unwrap_or(false),
        host: value["host"]
            .as_str()
            .filter(|h| !h.is_empty() && h.len() <= 253)
            .unwrap_or("127.0.0.1")
            .to_owned(),
        port: value["port"]
            .as_u64()
            .and_then(|p| u16::try_from(p).ok())
            .unwrap_or(8080),
        password: None,
    };
    let pinned = value["pinned"]
        .as_array()
        .into_iter()
        .flatten()
        .take(MAX_PINNED)
        .filter_map(|a| {
            Some(Action {
                id: a["id"].as_str()?.to_owned(),
                name: a["name"].as_str()?.to_owned(),
                group: String::new(),
                enabled: true,
            })
        })
        .collect();
    (settings, pinned)
}

impl Workbench {
    pub(super) fn streamer_bot_json(&self) -> Value {
        json!({
            "enabled": self.bot_settings.enabled,
            "host": self.bot_settings.host,
            "port": self.bot_settings.port,
            "pinned": self.bot_pinned.iter().map(|a| json!({"id": a.id, "name": a.name})).collect::<Vec<_>>(),
        })
    }
    /// Read the optional password from the OS vault, then start the connection.
    pub(super) fn start_streamer_bot(&mut self, cx: &mut Context<Self>) {
        let read = cx.read_credentials(VAULT);
        cx.spawn(async move |view, cx| {
            let password = read
                .await
                .ok()
                .flatten()
                .and_then(|(_, bytes)| String::from_utf8(bytes).ok());
            let _ = view.update(cx, |this, _| {
                this.bot_settings.password = password;
                this.streamer_bot.configure(this.bot_settings.clone());
            });
        })
        .detach();
    }
    pub(super) fn pump_streamer_bot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (changed, ran, feed) = self.streamer_bot.poll();
        for item in feed {
            self.feed.push(item);
            self.animate_for(Duration::from_millis(320), cx);
            cx.notify();
        }
        for (request, result) in ran {
            let name = self.bot_runs.remove(&request).unwrap_or_default();
            let text = match result {
                Ok(_) => format!("Streamer.bot ran “{name}”"),
                Err(e) => format!("“{name}” didn't run: {e}"),
            };
            window.push_notification(Notification::info(text), cx);
        }
        if changed {
            // Keep pinned names current if they were renamed in Streamer.bot.
            for pin in &mut self.bot_pinned {
                if let Some(action) = self.streamer_bot.actions.iter().find(|a| a.id == pin.id) {
                    pin.name = action.name.clone();
                }
            }
            self.share_bot_actions(cx);
            cx.notify();
        }
    }
    /// Panes offer pinned actions in the message menu while connected.
    pub(super) fn share_bot_actions(&mut self, cx: &mut Context<Self>) {
        let actions = std::rc::Rc::new(if self.streamer_bot.connected {
            self.bot_pinned.clone()
        } else {
            Vec::new()
        });
        for pane in self.panes() {
            pane.update(cx, |p, _| p.bot_actions = actions.clone());
        }
    }
    pub(super) fn run_bot_action(&mut self, action: Action, args: Value, cx: &mut Context<Self>) {
        let name = action.name.clone();
        let request = self.streamer_bot.run(action, args);
        self.bot_runs.insert(request, name);
        cx.notify();
    }
    fn set_bot_settings(&mut self, settings: Settings, cx: &mut Context<Self>) {
        self.bot_settings = settings;
        self.streamer_bot.configure(self.bot_settings.clone());
        self.schedule_save(cx);
        cx.notify();
    }
    pub(super) fn open_bot_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let host = cx.new(|cx| InputState::new(window, cx).placeholder("127.0.0.1"));
        let port = cx.new(|cx| InputState::new(window, cx).placeholder("8080"));
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Only if set in Streamer.bot")
                .masked(true)
        });
        host.update(cx, |i, cx| {
            i.set_value(self.bot_settings.host.clone(), window, cx)
        });
        port.update(cx, |i, cx| {
            i.set_value(self.bot_settings.port.to_string(), window, cx)
        });
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(200., 380.);
        window.open_dialog(cx, move |dialog, _, _| {
            let (owner, host, port, password) = (owner.clone(), host.clone(), port.clone(), password.clone());
            dialog
                .w(px(width))
                .title("Streamer.bot connection")
                .child(div().v_flex().gap_2()
                    .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("In Streamer.bot: Servers/Clients → WebSocket Server → Auto Start + Start. Jawjack connects locally; the password is kept in Windows Credential Manager."))
                    .child(div().text_size(px(12.)).child("Address"))
                    .child(div().h_flex().gap_1().child(div().flex_1().child(Input::new(&host).small())).child(div().w(px(80.)).child(Input::new(&port).small())))
                    .child(div().text_size(px(12.)).child("Password"))
                    .child(Input::new(&password).small()))
                .footer(div().h_flex().justify_end().gap_2()
                    .child(Button::new("bot-cancel").small().label("Cancel").on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(Button::new("bot-save").small().primary().label("Connect").on_click(move |_, w, cx| {
                        let host_value = host.read(cx).value().trim().to_owned();
                        let port_value = port.read(cx).value().trim().parse::<u16>().ok();
                        let password_value = password.read(cx).value().to_string();
                        let (Some(port_value), false) = (port_value, host_value.is_empty() || host_value.len() > 253 || host_value.contains(['/', ' ', '@'])) else {
                            w.push_notification(Notification::info("Enter a host name or IP and a port number."), cx);
                            return;
                        };
                        w.close_dialog(cx);
                        let save = (!password_value.is_empty()).then(|| cx.write_credentials(VAULT, "streamerbot", password_value.as_bytes()));
                        let _ = owner.update(cx, |this, cx| {
                            let mut settings = this.bot_settings.clone();
                            settings.enabled = true;
                            settings.host = host_value;
                            settings.port = port_value;
                            if !password_value.is_empty() {
                                settings.password = Some(password_value);
                            }
                            this.set_bot_settings(settings, cx);
                        });
                        if let Some(save) = save {
                            cx.spawn(async move |_| { let _ = save.await; }).detach();
                        }
                    })))
        });
    }
    pub(super) fn open_bot_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.streamer_bot.refresh();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter actions"));
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(200., 440.);
        let height = (f32::from(window.viewport_size().height) - 200.).clamp(120., 480.);
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(this) = owner.upgrade() else { return dialog.title("Workspace closed") };
            let query = filter.read(cx).value().to_lowercase();
            let (actions, pinned) = {
                let this = this.read(cx);
                (this.streamer_bot.actions.clone(), this.bot_pinned.clone())
            };
            let rows = actions
                .into_iter()
                .filter(|a| query.is_empty() || a.name.to_lowercase().contains(&query) || a.group.to_lowercase().contains(&query))
                .take(300)
                .enumerate()
                .map(|(index, action)| {
                    let on = pinned.iter().any(|p| p.id == action.id);
                    let owner = owner.clone();
                    let toggle = action.clone();
                    div().h_flex().gap_2().items_center().min_w_0()
                        .child(Button::new(("bot-pin", index)).xsmall().label(if on { "✓ Pinned" } else { "Pin" }).selected(on).when(on, |b| b.primary())
                            .on_click(move |_, w, cx| {
                                let toggle = toggle.clone();
                                let _ = owner.update(cx, |this, cx| {
                                    if let Some(at) = this.bot_pinned.iter().position(|p| p.id == toggle.id) {
                                        this.bot_pinned.remove(at);
                                    } else if this.bot_pinned.len() >= MAX_PINNED {
                                        w.push_notification(Notification::info(format!("Up to {MAX_PINNED} pinned actions.")), cx);
                                        return;
                                    } else {
                                        this.bot_pinned.push(toggle);
                                    }
                                    this.share_bot_actions(cx);
                                    this.schedule_save(cx);
                                    cx.notify();
                                });
                            }))
                        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().when(!action.enabled, |el| el.text_color(rgb(theme::MUTED))).child(action.name.clone()))
                        .when(!action.group.is_empty(), |el| el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).whitespace_nowrap().child(action.group.clone())))
                })
                .collect::<Vec<_>>();
            let empty = rows.is_empty();
            dialog
                .w(px(width))
                .title("Streamer.bot actions")
                .child(div().v_flex().gap_2()
                    .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("Pin up to {MAX_PINNED} actions as Studio buttons; they also appear when you right-click a message, with that user and message passed as arguments.")))
                    .child(Input::new(&filter).small())
                    .child(div().id("bot-actions").v_flex().gap_1().max_h(px(height)).overflow_y_scroll()
                        .children(rows)
                        .when(empty, |el| el.child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("No actions yet. Is Streamer.bot connected?")))))
        });
    }
    pub(super) fn render_bot_card(&self, cx: &mut Context<Self>) -> Div {
        let connected = self.streamer_bot.connected;
        let enabled = self.bot_settings.enabled;
        if !enabled {
            // Not set up: one sentence on what it adds, one primary action.
            return div()
                .v_flex()
                .gap_2()
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                    "Run your Streamer.bot actions from here, and see donations plus YouTube and Kick events in Activity.",
                ))
                .child(
                    div().h_flex().gap_2()
                        .child(Button::new("bot-connect").small().primary().icon(IconName::Bot).label("Connect").on_click(cx.listener(|this, _, _, cx| {
                            let mut settings = this.bot_settings.clone();
                            settings.enabled = true;
                            this.set_bot_settings(settings, cx);
                        })))
                        .child(Button::new("bot-settings").small().ghost().label("Settings…").on_click(cx.listener(|this, _, w, cx| this.open_bot_settings(w, cx)))),
                );
        }
        let status_color = if connected {
            theme::STREAM_LIVE
        } else {
            theme::STREAM_UNKNOWN
        };
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(7.))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(rgb(status_color)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(11.))
                            .text_color(rgb(theme::MUTED))
                            .child(self.streamer_bot.status.clone()),
                    )
                    .child(
                        Button::new("bot-settings")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Settings)
                            .tooltip("Connection settings")
                            .on_click(cx.listener(|this, _, w, cx| this.open_bot_settings(w, cx))),
                    ),
            )
            .when(connected && self.bot_pinned.is_empty(), |el| {
                el.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme::MUTED))
                        .child("Pin the actions you use live to get one-click buttons here."),
                )
            })
            // Pinned actions as an even two-column grid.
            .when(connected && !self.bot_pinned.is_empty(), |el| {
                el.child(div().h_flex().flex_wrap().gap_1().children(
                    self.bot_pinned.iter().enumerate().map(|(index, action)| {
                        let action = action.clone();
                        div().w(px(134.)).child(
                            Button::new(("bot-run", index))
                                .small()
                                .w_full()
                                .label(action.name.clone())
                                .tooltip(format!("Run “{}” in Streamer.bot", action.name))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run_bot_action(
                                        action.clone(),
                                        json!({"source": "jawjack"}),
                                        cx,
                                    )
                                })),
                        )
                    }),
                ))
            })
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .child(div().when(connected, |el| {
                        el.child(
                            Button::new("bot-choose")
                                .ghost()
                                .xsmall()
                                .icon(gpui_kit::component::Icon::new(
                                    gpui_kit::assets::IconName::Pin,
                                ))
                                .label(if self.bot_pinned.is_empty() {
                                    "Pin actions"
                                } else {
                                    "Edit pins"
                                })
                                .on_click(
                                    cx.listener(|this, _, w, cx| this.open_bot_actions(w, cx)),
                                ),
                        )
                    }))
                    .child(
                        Button::new("bot-disconnect")
                            .ghost()
                            .xsmall()
                            .label("Disconnect")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let mut settings = this.bot_settings.clone();
                                settings.enabled = false;
                                this.set_bot_settings(settings, cx);
                            })),
                    ),
            )
    }
}
