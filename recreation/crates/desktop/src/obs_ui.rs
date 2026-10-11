//! OBS in the Studio panel: connection, live/recording state, mic warning,
//! stream health and one-click scenes.
use super::*;
use crate::streamer_bot::Settings;

const VAULT: &str = "https://jawjack.zombie.digital/obs/websocket-v1";

pub(super) fn load_obs_settings(value: &Value) -> Settings {
    Settings {
        enabled: value["enabled"].as_bool().unwrap_or(false),
        host: value["host"]
            .as_str()
            .filter(|h| !h.is_empty() && h.len() <= 253)
            .unwrap_or("127.0.0.1")
            .to_owned(),
        port: value["port"]
            .as_u64()
            .and_then(|p| u16::try_from(p).ok())
            .unwrap_or(crate::obs::DEFAULT_PORT),
        password: None,
    }
}

/// A scene used as a divider in OBS's list (e.g. "=====[ Live ]=====",
/// "--- Alerts ---"): its label, or None for a real scene.
pub(super) fn divider(scene: &str) -> Option<String> {
    let fill = |c: char| matches!(c, '=' | '-' | '_' | '~' | '—' | '–' | '#' | '*' | '.' | '|');
    if scene.chars().take_while(|c| fill(*c)).count() < 3 {
        return None;
    }
    Some(
        scene
            .trim_matches(|c: char| {
                fill(c) || c.is_whitespace() || matches!(c, '[' | ']' | '(' | ')')
            })
            .to_owned(),
    )
}

fn clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

impl Workbench {
    pub(super) fn obs_json(&self) -> Value {
        json!({"enabled": self.obs_settings.enabled, "host": self.obs_settings.host, "port": self.obs_settings.port})
    }
    /// Read the optional password from the OS vault, then connect.
    pub(super) fn start_obs(&mut self, cx: &mut Context<Self>) {
        let read = cx.read_credentials(VAULT);
        cx.spawn(async move |view, cx| {
            let password = read
                .await
                .ok()
                .flatten()
                .and_then(|(_, bytes)| String::from_utf8(bytes).ok());
            let _ = view.update(cx, |this, _| {
                this.obs_settings.password = password;
                this.obs.configure(this.obs_settings.clone());
            });
        })
        .detach();
    }
    pub(super) fn pump_obs(&mut self, cx: &mut Context<Self>) {
        if self.obs.poll() {
            cx.notify();
        }
        // The uptime ticks once a second while the Studio panel shows it.
        if self.obs.state.streaming && self.studio_open {
            let second = self.obs.sampled.elapsed().as_secs();
            if self.obs_tick != second {
                self.obs_tick = second;
                cx.notify();
            }
        }
    }
    fn set_obs_settings(&mut self, settings: Settings, cx: &mut Context<Self>) {
        self.obs_settings = settings;
        self.obs.configure(self.obs_settings.clone());
        self.schedule_save(cx);
        cx.notify();
    }
    pub(super) fn open_obs_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let host = cx.new(|cx| InputState::new(window, cx).placeholder("127.0.0.1"));
        let port = cx.new(|cx| InputState::new(window, cx).placeholder("4455"));
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("From OBS: Tools → WebSocket Server Settings")
                .masked(true)
        });
        host.update(cx, |i, cx| {
            i.set_value(self.obs_settings.host.clone(), window, cx)
        });
        port.update(cx, |i, cx| {
            i.set_value(self.obs_settings.port.to_string(), window, cx)
        });
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(200., 380.);
        self.obs_found = None;
        window.open_dialog(cx, move |dialog, _, cx| {
            let found = owner.upgrade().map(|o| o.read(cx).obs_found.clone()).unwrap_or_default();
            let (owner, host, port, password) = (owner.clone(), host.clone(), port.clone(), password.clone());
            let (find_owner, find_port, pick_host, pick_port) = (owner.clone(), port.clone(), host.clone(), port.clone());
            dialog
                .w(px(width))
                .title("OBS connection")
                .on_ok(|_, _, _| false)
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                            "In OBS: Tools → WebSocket Server Settings → Enable WebSocket server. \"Show Connect Info\" has the password. Jawjack connects locally; the password is kept in Windows Credential Manager.",
                        ))
                        .child(div().text_size(px(12.)).child("Address"))
                        .child(
                            div()
                                .h_flex()
                                .gap_1()
                                .child(div().flex_1().child(Input::new(&host).small()))
                                .child(div().w(px(80.)).child(Input::new(&port).small())),
                        )
                        .child(
                            div()
                                .h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("obs-find")
                                        .xsmall()
                                        .ghost()
                                        .label(if found.as_ref().is_some_and(|f| f.is_none()) { "Searching…" } else { "Find OBS on my network" })
                                        .on_click(move |_, window, cx| {
                                            let port = find_port.read(cx).value().trim().parse::<u16>().unwrap_or(crate::obs::DEFAULT_PORT);
                                            let _ = find_owner.update(cx, |this, cx| this.find_obs(port, window, cx));
                                        }),
                                )

                        )
                        .when_some(found.clone().flatten(), |el, list| {
                                    el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(if list.is_empty() {
                                        "None found. Is OBS's WebSocket server on, and port open in the firewall? A custom port: type it above first.".to_owned()
                                    } else {
                                        "Found:".to_owned()
                                    }))
                                })
                        .when_some(found.flatten().filter(|l| !l.is_empty()), |el, list| {
                            el.child(div().h_flex().flex_wrap().gap_1().children(list.into_iter().enumerate().map(|(index, found)| {
                                let (pick, pick_port) = (pick_host.clone(), pick_port.clone());
                                let label = format!("{}:{}", found.host, found.port);
                                if !found.supported {
                                    return Button::new(("obs-found", index))
                                        .xsmall()
                                        .ghost()
                                        .label(format!("{label} · old obs-websocket 4.x"))
                                        .tooltip("Update OBS to 28 or newer; its built-in WebSocket server (port 4455) is what Jawjack uses")
                                        .disabled(true);
                                }
                                Button::new(("obs-found", index)).xsmall().label(label).on_click(move |_, window, cx| {
                                    pick.update(cx, |i, cx| i.set_value(found.host.clone(), window, cx));
                                    pick_port.update(cx, |i, cx| i.set_value(found.port.to_string(), window, cx));
                                })
                            })))
                        })
                        .child(div().text_size(px(12.)).child("Password"))
                        .child(Input::new(&password).small().mask_toggle()),
                )
                .footer(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap_2()
                        .child(Button::new("obs-cancel").small().label("Cancel").on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(Button::new("obs-save").small().primary().label("Connect").on_click(move |_, w, cx| {
                            let host_value = host.read(cx).value().trim().to_owned();
                            let port_value = port.read(cx).value().trim().parse::<u16>().ok();
                            let password_value = password.read(cx).value().to_string();
                            let (Some(port_value), false) = (
                                port_value,
                                host_value.is_empty() || host_value.len() > 253 || host_value.contains(['/', ' ', '@']),
                            ) else {
                                w.push_notification(Notification::info("Enter a host name or IP and a port number."), cx);
                                return;
                            };
                            w.close_dialog(cx);
                            let save = (!password_value.is_empty())
                                .then(|| cx.write_credentials(VAULT, "obs", password_value.as_bytes()));
                            let _ = owner.update(cx, |this, cx| {
                                let mut settings = this.obs_settings.clone();
                                settings.enabled = true;
                                settings.host = host_value;
                                settings.port = port_value;
                                if !password_value.is_empty() {
                                    settings.password = Some(password_value);
                                }
                                this.set_obs_settings(settings, cx);
                            });
                            if let Some(save) = save {
                                cx.spawn(async move |_| {
                                    let _ = save.await;
                                })
                                .detach();
                            }
                        })),
                )
        });
    }
    /// Scan the local network for OBS; results show in the settings dialog.
    fn find_obs(&mut self, port: u16, window: &mut Window, cx: &mut Context<Self>) {
        if self.obs_found.as_ref().is_some_and(|f| f.is_none()) {
            return;
        }
        self.obs_found = Some(None);
        let task = cx.background_spawn(async move { crate::obs::discover(port) });
        cx.spawn_in(window, async move |view, cx| {
            let found = task.await;
            let _ = view.update(cx, |this, cx| {
                this.obs_found = Some(Some(found));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn render_obs_card(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.obs.state;
        if !self.obs_settings.enabled {
            return div()
                .v_flex()
                .gap_2()
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                    "Switch scenes, see if you're live, your mic state and stream health, from OBS 28 or newer.",
                ))
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(Button::new("obs-connect").small().primary().icon(Icon::new(gpui_kit::assets::IconName::Tv)).label("Connect").on_click(cx.listener(|this, _, _, cx| {
                            let mut settings = this.obs_settings.clone();
                            settings.enabled = true;
                            this.set_obs_settings(settings, cx);
                        })))
                        .child(Button::new("obs-settings").small().ghost().label("Settings…").on_click(cx.listener(|this, _, w, cx| this.open_obs_settings(w, cx)))),
                );
        }
        let pill = |label: String, color: u32| {
            div()
                .h_flex()
                .items_center()
                .gap_1()
                .px(px(6.))
                .h(px(20.))
                .rounded(px(10.))
                .border_1()
                .border_color(rgb(color))
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color))
                .child(div().size(px(6.)).rounded_full().bg(rgb(color)))
                .child(label)
        };
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .when(state.connected, |el| {
                        el.child(if state.streaming {
                            pill(
                                format!(
                                    "LIVE {}",
                                    self.obs.uptime().map(clock).unwrap_or_default()
                                ),
                                0xF07A7A,
                            )
                        } else {
                            pill("OFFLINE".into(), theme::MUTED)
                        })
                        .when(state.recording, |el| el.child(pill("REC".into(), 0xE8C468)))
                    })
                    .when(!state.connected, |el| {
                        el.child(
                            // Wraps: the reason (password, server off) matters here.
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(11.))
                                .text_color(rgb(theme::MUTED))
                                .child(state.status.clone()),
                        )
                    })
                    .child(div().flex_1())
                    .when_some(state.mic.clone(), |el, (name, muted)| {
                        el.child(
                            Button::new("obs-mic")
                                .xsmall()
                                .ghost()
                                .icon(Icon::new(if muted {
                                    gpui_kit::assets::IconName::MicOff
                                } else {
                                    gpui_kit::assets::IconName::Mic
                                }))
                                .when(muted, |b| b.label("Muted"))
                                .tooltip(format!(
                                    "{name}: {} · click to toggle",
                                    if muted { "muted" } else { "on" }
                                ))
                                .on_click(cx.listener(|this, _, _, _| this.obs.toggle_mute())),
                        )
                    })
                    .child(
                        Button::new("obs-settings")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Settings)
                            .tooltip("OBS connection settings")
                            .on_click(cx.listener(|this, _, w, cx| this.open_obs_settings(w, cx))),
                    ),
            )
            // A muted mic while live is the classic mistake: make it loud.
            .when(
                state.streaming && state.mic.as_ref().is_some_and(|(_, m)| *m),
                |el| {
                    el.child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(px(6.))
                            .bg(rgb(0x3A1D20))
                            .border_1()
                            .border_color(rgb(0xF07A7A))
                            .text_size(px(11.))
                            .text_color(rgb(0xF2B8B8))
                            .child("Your mic is muted while you're live."),
                    )
                },
            )
            .when_some(state.health.clone().filter(|_| state.streaming), |el, h| {
                let warn = h.dropped >= 1.;
                el.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(if warn { 0xE8C468 } else { theme::MUTED }))
                        .child(format!(
                            "{} kbps · {:.1}% dropped · CPU {:.0}% · {:.0} fps",
                            h.kbps, h.dropped, h.cpu, h.fps
                        )),
                )
            })
            .when(state.connected && !state.scenes.is_empty(), |el| {
                el.child(
                    div()
                        .id("obs-scenes")
                        .max_h(px(300.))
                        .overflow_y_scroll()
                        .child(div().h_flex().flex_wrap().gap_1().children(
                            state.scenes.iter().enumerate().map(|(index, scene)| {
                                // Divider scenes ("=====[ Live ]=====") become full-width section labels.
                                if let Some(label) = divider(scene) {
                                    return div()
                                        .w_full()
                                        .pt_1()
                                        .text_size(px(10.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(theme::MUTED))
                                        .child(label.to_uppercase())
                                        .into_any_element();
                                }
                                let current = *scene == state.scene;
                                let name = scene.clone();
                                div()
                                    .w(px(134.))
                                    .child(
                                        Button::new(("obs-scene", index))
                                            .small()
                                            .w_full()
                                            .when(current, |b| b.primary())
                                            .label(scene.clone())
                                            .tooltip(if current {
                                                format!("“{scene}” is on air")
                                            } else {
                                                format!("Switch to “{scene}”")
                                            })
                                            .on_click(cx.listener(move |this, _, _, _| {
                                                this.obs.set_scene(name.clone())
                                            })),
                                    )
                                    .into_any_element()
                            }),
                        )),
                )
            })
            .child(
                div().h_flex().justify_end().child(
                    Button::new("obs-disconnect")
                        .ghost()
                        .xsmall()
                        .label("Disconnect")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let mut settings = this.obs_settings.clone();
                            settings.enabled = false;
                            this.set_obs_settings(settings, cx);
                        })),
                ),
            )
    }
}

impl Workbench {
    /// Whether the signed-in account's own channel is live.
    pub(super) fn own_live(&self, cx: &App) -> bool {
        self.account
            .read(cx)
            .login()
            .and_then(|own| self.streams.detail(&own).map(|(info, _)| info.live))
            .unwrap_or(false)
            || self.obs.state.streaming
    }
    /// Ad schedule while live, plus a one-minute heads-up per ad.
    pub(super) fn pump_ads(
        &mut self,
        identity: Option<&crate::live::Identity>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let live = self.own_live(cx);
        if self.ads.pump(identity, live) {
            cx.notify();
        }
        if let Some(notice) = self.ads.notice.take() {
            window.push_notification(Notification::info(notice), cx);
        }
        let Some(at) = self.ads.schedule.as_ref().and_then(|s| s.next_ad_at) else {
            return;
        };
        let left = (at - chrono::Utc::now()).num_seconds();
        if live && (1..=60).contains(&left) && self.ads.warned != Some(at) {
            self.ads.warned = Some(at);
            let owner = cx.entity().downgrade();
            let length = self.ads.schedule.as_ref().map_or(0, |s| s.duration);
            window.push_notification(
                Notification::new()
                    .title("Ad break in under a minute")
                    .message(format!("{length}s ad. Click to put a heads-up for chat in your message box (you send it)."))
                    .in_app_and_system()
                    .on_click(move |_, window, cx| {
                        let _ = owner.update(cx, |this, cx| this.draft_ad_heads_up(length, window, cx));
                    }),
                cx,
            );
        }
        // Keep the countdown ticking while Studio shows it.
        if live && self.studio_open && left > 0 {
            let second = left as u64;
            if self.obs_tick != second {
                self.obs_tick = second;
                cx.notify();
            }
        }
    }
    /// Put an ad heads-up in the own channel's message box; never sends by itself.
    pub(super) fn draft_ad_heads_up(
        &mut self,
        seconds: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(own) = self.account.read(cx).login() else {
            return;
        };
        let Some(pane) = self
            .tab_panes()
            .into_iter()
            .find(|p| p.read(cx).name.as_ref() == own)
        else {
            return;
        };
        let text = if seconds > 0 {
            format!("Ad break coming up ({seconds}s). Stretch, grab a drink, I'll be right here!")
        } else {
            "Ad break coming up. Stretch, grab a drink, I'll be right here!".to_owned()
        };
        self.draft_in_own(&text, window, cx);
        let _ = (pane, own);
    }
}

impl Workbench {
    /// Thin live bar under the Studio header: hype train, goals, charity, Shield Mode.
    pub(super) fn render_hype_strip(&self) -> Div {
        let bar = |ratio: f32, color: u32| {
            div()
                .h(px(4.))
                .w_full()
                .rounded(px(2.))
                .bg(rgb(theme::CONTROL))
                .child(
                    div()
                        .h_full()
                        .rounded(px(2.))
                        .bg(rgb(color))
                        .w(gpui_kit::relative(ratio.clamp(0., 1.))),
                )
        };
        let item = |icon: gpui_kit::assets::IconName,
                    color: u32,
                    title: String,
                    right: String,
                    ratio: Option<f32>| {
            div()
                .v_flex()
                .gap(px(3.))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.))
                        .child(div().text_color(rgb(color)).child(Icon::new(icon).xsmall()))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_color(rgb(theme::MUTED))
                                .child(right),
                        ),
                )
                .when_some(ratio, |el, r| el.child(bar(r, color)))
        };
        let mut strip = div()
            .v_flex()
            .gap_2()
            .mx_3()
            .mb_2()
            .p_2()
            .rounded(px(8.))
            .bg(rgb(theme::ELEVATED))
            .border_1()
            .border_color(rgb(theme::BORDER));
        if let Some(by) = &self.hype.shield {
            strip = strip.child(item(
                gpui_kit::assets::IconName::Shield,
                0xF07A7A,
                "Shield Mode is on".into(),
                if by.is_empty() {
                    String::new()
                } else {
                    format!("by {by}")
                },
                None,
            ));
        }
        if let Some(train) = &self.hype.train {
            let left = train
                .expires_at
                .and_then(crate::ads::countdown)
                .unwrap_or_default();
            strip = strip.child(item(
                gpui_kit::assets::IconName::Rocket,
                if train.golden {
                    0xE8C468
                } else {
                    theme::ACCENT
                },
                format!(
                    "{}Hype Train · Level {}",
                    if train.golden { "Golden Kappa " } else { "" },
                    train.level
                ),
                left,
                Some(if train.goal > 0 {
                    train.progress as f32 / train.goal as f32
                } else {
                    0.
                }),
            ));
        }
        for goal in self.hype.goals.values() {
            strip = strip.child(item(
                gpui_kit::assets::IconName::Trophy,
                0x91D7BA,
                goal.label.clone(),
                format!("{} / {}", goal.current, goal.target),
                Some(if goal.target > 0 {
                    goal.current as f32 / goal.target as f32
                } else {
                    0.
                }),
            ));
        }
        if let Some(charity) = &self.hype.charity {
            strip = strip.child(item(
                gpui_kit::assets::IconName::HandCoins,
                0xE69DCD,
                format!("Charity · {}", charity.name),
                format!("{} / {}", charity.raised, charity.target),
                Some(charity.ratio),
            ));
        }
        strip
    }
}

impl Workbench {
    /// Whether a moderator (not you) chatted in your channel in the last 15 minutes.
    fn mod_recently(&self, own: &str, cx: &App) -> bool {
        let since = chrono::Utc::now().timestamp() - 15 * 60;
        self.tab_panes()
            .iter()
            .filter(|p| p.read(cx).name.as_ref() == own)
            .any(|p| {
                p.read(cx)
                    .timeline
                    .borrow()
                    .messages()
                    .iter()
                    .rev()
                    .take(500)
                    .any(|m| {
                        m.sent_at.is_some_and(|t| t >= since)
                            && m.badges
                                .iter()
                                .any(|b| b.set_id == "moderator" || b.set_id == "lead_moderator")
                    })
            })
    }
    /// Go-live checklist at the top of Studio > Stream: before going live and for
    /// the first 10 minutes, unless dismissed for this stream.
    pub(super) fn render_checklist(
        &mut self,
        own: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let live = self.own_live(cx);
        let early = self
            .went_live_at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(600));
        if self.checklist_dismissed || (live && !early) {
            return None;
        }
        if self.checklist_input.is_none() {
            let input = cx.new(|cx| {
                InputState::new(window, cx).placeholder("Add your own item, e.g. Lights on")
            });
            cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, input, event, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        let text: String = input.read(cx).value().trim().chars().take(60).collect();
                        if !text.is_empty()
                            && this.checklist_items.len() < 12
                            && !this.checklist_items.contains(&text)
                        {
                            this.checklist_items.push(text);
                            input.update(cx, |i, cx| i.set_value("", window, cx));
                            this.schedule_save(cx);
                            cx.notify();
                        }
                    }
                },
            )
            .detach();
            self.checklist_input = Some(input);
        }
        let saved = self.studio_editor.as_ref().and_then(|e| e.read(cx).saved());
        let obs = &self.obs.state;
        let mic = obs.mic.as_ref().map(|(_, muted)| !*muted);
        // (ok, label, hint when not ok); None = can't tell, shown neutral.
        let auto: Vec<(Option<bool>, String, &str)> = vec![
            (
                saved.as_ref().map(|(t, c)| !t.is_empty() && !c.is_empty()),
                match &saved {
                    Some((t, c)) if !t.is_empty() && !c.is_empty() => {
                        format!("Title & category · {c}")
                    }
                    _ => "Title & category".into(),
                },
                "set below",
            ),
            (
                self.obs_settings.enabled.then_some(obs.connected),
                "OBS connected".into(),
                "not connected",
            ),
            (
                mic.filter(|_| obs.connected),
                "Mic on".into(),
                "muted in OBS",
            ),
            (
                Some(self.mod_recently(own, cx)),
                "A moderator is around".into(),
                "none in 15 min",
            ),
        ];
        let check = |state: Option<bool>| {
            let (icon, color) = match state {
                Some(true) => (IconName::Check, theme::STREAM_LIVE),
                Some(false) => (IconName::CircleX, 0xE8C468),
                None => (IconName::Minus, theme::MUTED),
            };
            div()
                .size(px(16.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(color))
                .child(Icon::new(icon).xsmall())
        };
        let mut card = div()
            .v_flex()
            .gap_1()
            .p_2()
            .mb_2()
            .rounded(px(8.))
            .bg(rgb(theme::ELEVATED))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if live {
                                "Just went live"
                            } else {
                                "Before you go live"
                            }),
                    )
                    .child(
                        Button::new("checklist-dismiss")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("Hide until the next stream")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.checklist_dismissed = true;
                                cx.notify();
                            })),
                    ),
            );
        for (state, label, hint) in auto {
            card = card.child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .child(check(state))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(label),
                    )
                    .when(state == Some(false), |el| {
                        el.child(div().text_color(rgb(theme::MUTED)).child(hint))
                    }),
            );
        }
        for (index, item) in self.checklist_items.clone().into_iter().enumerate() {
            let done = self.checklist_done.contains(&item);
            let (toggle, remove) = (item.clone(), item.clone());
            card = card.child(
                div()
                    .id(("checklist-item", index))
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .cursor_pointer()
                    .child(check(Some(done)).when(!done, |el| el.text_color(rgb(theme::MUTED))))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .when(done, |el| el.text_color(rgb(theme::MUTED)))
                            .child(item),
                    )
                    .child(
                        div()
                            .id(("checklist-remove", index))
                            .px_1()
                            .text_color(rgb(theme::MUTED))
                            .hover(|s| s.text_color(rgb(theme::TEXT)))
                            .child("×")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.checklist_items.retain(|i| *i != remove);
                                this.checklist_done.remove(&remove);
                                this.schedule_save(cx);
                                cx.notify();
                            })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.checklist_done.remove(&toggle) {
                            this.checklist_done.insert(toggle.clone());
                        }
                        cx.notify();
                    })),
            );
        }
        if let Some(input) = &self.checklist_input
            && self.checklist_items.len() < 12
        {
            card = card.child(div().pt_1().child(Input::new(input).xsmall()));
        }
        Some(card)
    }
}

impl Workbench {
    /// Five minutes after a raid on your channel: shout them out or thank them.
    pub(super) fn render_raid_card(
        &self,
        own: String,
        pane: Option<Entity<ChannelPane>>,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let (login, name, viewers, at) = self.last_raid.clone()?;
        if at.elapsed() > Duration::from_secs(300) {
            return None;
        }
        let shouted = self.shouted.contains(&login);
        let owner = cx.entity().downgrade();
        let shout = {
            let (owner, pane, own, login) =
                (owner.clone(), pane.clone(), own.clone(), login.clone());
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                let _ = owner.update(cx, |this, _| {
                    this.shouted.insert(login.clone());
                });
                Self::studio_run_public(
                    owner.clone(),
                    pane.clone(),
                    own.clone(),
                    crate::twitch_actions::Request::Shoutout {
                        login: login.clone(),
                    },
                    window,
                    cx,
                );
            }
        };
        let thank = {
            let (owner, name) = (owner.clone(), name.clone());
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                let text = format!("Thank you for the raid @{name}! Welcome in, raiders!");
                let _ = owner.update(cx, |this, cx| this.draft_in_own(&text, window, cx));
            }
        };
        Some(
            div()
                .v_flex()
                .gap_2()
                .mx_3()
                .mb_2()
                .p_2()
                .rounded(px(8.))
                .bg(rgb(0x192D27))
                .border_1()
                .border_color(rgb(0x91D7BA))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_color(rgb(0x91D7BA))
                                .child(Icon::new(gpui_kit::assets::IconName::Rocket).xsmall()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(if viewers > 0 {
                                    format!("{name} raided with {viewers} viewers")
                                } else {
                                    format!("{name} raided you")
                                }),
                        )
                        .child(
                            Button::new("raid-dismiss")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.last_raid = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            Button::new("raid-shoutout")
                                .small()
                                .primary()
                                .label(if shouted { "Shouted out" } else { "Shout out" })
                                .disabled(shouted)
                                .tooltip("Twitch /shoutout (has a cooldown)")
                                .on_click(shout),
                        )
                        .child(
                            Button::new("raid-thank")
                                .small()
                                .ghost()
                                .label("Thank in chat…")
                                .tooltip("Puts a thank-you in your message box to edit and send")
                                .on_click(thank),
                        ),
                ),
        )
    }
    /// Put text in your own channel's message box (you send it).
    pub(super) fn draft_in_own(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(own) = self.account.read(cx).login() else {
            return;
        };
        let Some(pane) = self
            .tab_panes()
            .into_iter()
            .find(|p| p.read(cx).name.as_ref() == own)
        else {
            return;
        };
        self.reveal_channel(&own, cx);
        pane.update(cx, |p, cx| p.command_draft(text, window, cx));
    }
}
