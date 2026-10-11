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
                el.child(div().h_flex().flex_wrap().gap_1().children(
                    state.scenes.iter().enumerate().map(|(index, scene)| {
                        let current = *scene == state.scene;
                        let name = scene.clone();
                        div().w(px(134.)).child(
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
                    }),
                ))
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
