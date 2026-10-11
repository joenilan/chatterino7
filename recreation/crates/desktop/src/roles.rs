//! Viewer / Moderator / Streamer experiences (ROLE_EXPERIENCES.md).
//!
//! The experience only changes presentation. Authority still comes from the
//! channel role and granted scopes (capabilities.rs), checked again on dispatch.
//! Streamer pins the account's own channel as the first tab of the primary group
//! in every workspace and shows the Studio panel beside chat.
use super::*;
use crate::twitch_actions::Request;

pub const STUDIO_WIDTH: f32 = 300.;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Experience {
    #[default]
    Viewer,
    Moderator,
    Streamer,
}
impl Experience {
    pub const ALL: [Experience; 3] = [Self::Viewer, Self::Moderator, Self::Streamer];
    pub fn key(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Moderator => "moderator",
            Self::Streamer => "streamer",
        }
    }
    pub fn from_key(key: &str) -> Self {
        match key {
            "moderator" => Self::Moderator,
            "streamer" => Self::Streamer,
            _ => Self::Viewer,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Viewer => "Viewer",
            Self::Moderator => "Moderator",
            Self::Streamer => "Streamer",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Viewer => "Clean chat, emotes, replies and mentions. No tool clutter.",
            Self::Moderator => {
                "A 🛡 moderation button in channels you moderate: chat modes, clear chat, and the right-click mod actions."
            }
            Self::Streamer => {
                "Your own chat pinned first in every workspace, plus the Studio panel: title, category, chat modes, markers, ads and raids."
            }
        }
    }
}

/// Chat-mode toggles for `channel`, dispatched through the capability-checked
/// command path. Works in a panel or a dialog.
pub(super) fn mode_buttons(
    owner: WeakEntity<Workbench>,
    pane: Entity<ChannelPane>,
    channel: String,
    settings: Option<crate::room_settings::RoomSettings>,
) -> Div {
    let Some(settings) = settings else {
        return div()
            .text_size(px(11.))
            .text_color(rgb(theme::MUTED))
            .child("Chat modes appear once the channel is connected.");
    };
    let modes: [(&str, bool, Request); 5] = [
        (
            "Slow 30s",
            settings.slow.is_some(),
            if settings.slow.is_some() {
                crate::twitch_actions::parse("/slowoff", "")
            } else {
                crate::twitch_actions::parse("/slow", "30")
            }
            .and_then(Result::ok)
            .expect("valid built-in command"),
        ),
        (
            "Followers",
            settings.followers.is_some(),
            crate::twitch_actions::parse(
                if settings.followers.is_some() {
                    "/followersoff"
                } else {
                    "/followers"
                },
                "",
            )
            .and_then(Result::ok)
            .expect("valid built-in command"),
        ),
        (
            "Subscribers",
            settings.subscribers,
            crate::twitch_actions::parse(
                if settings.subscribers {
                    "/subscribersoff"
                } else {
                    "/subscribers"
                },
                "",
            )
            .and_then(Result::ok)
            .expect("valid built-in command"),
        ),
        (
            "Emote-only",
            settings.emote,
            crate::twitch_actions::parse(
                if settings.emote {
                    "/emoteonlyoff"
                } else {
                    "/emoteonly"
                },
                "",
            )
            .and_then(Result::ok)
            .expect("valid built-in command"),
        ),
        (
            "Unique chat",
            settings.unique,
            crate::twitch_actions::parse(
                if settings.unique {
                    "/uniquechatoff"
                } else {
                    "/uniquechat"
                },
                "",
            )
            .and_then(Result::ok)
            .expect("valid built-in command"),
        ),
    ];
    div()
        .h_flex()
        .flex_wrap()
        .gap_1()
        .children(modes.into_iter().map(|(label, on, request)| {
            let (owner, pane, channel) = (owner.clone(), pane.clone(), channel.clone());
            Button::new(SharedString::from(format!("mode-{channel}-{label}")))
                .xsmall()
                .label(if on {
                    format!("✓ {label}")
                } else {
                    label.to_owned()
                })
                .selected(on)
                .when(on, |b| b.primary())
                .tooltip(if on {
                    "On · click to turn off"
                } else {
                    "Off · click to turn on"
                })
                .on_click(move |_, window, cx| {
                    let (pane, channel, request) = (pane.clone(), channel.clone(), request.clone());
                    let _ = owner.update(cx, |this, cx| {
                        this.run_twitch_command(pane, channel, request, String::new(), window, cx)
                    });
                })
        }))
}

impl Workbench {
    /// The account's own channel while Streamer is selected.
    pub(super) fn anchor(&self, cx: &App) -> Option<String> {
        (self.experience == Experience::Streamer)
            .then(|| self.account.read(cx).login())
            .flatten()
    }
    /// Keep the own channel first in the primary group of every workspace.
    /// Runs after every pump, so drags, closes and new workspaces all converge.
    pub(super) fn ensure_anchor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(own) = self.anchor(cx) else {
            return;
        };
        let mut changed = false;
        for index in 0..self.tabs.len() {
            if !self.tabs[index]
                .panes
                .iter()
                .any(|p| p.read(cx).name.as_ref() == own)
            {
                let id = self.tabs[index].id;
                let pane = self.make_pane(id, &own, window, cx);
                self.tabs[index].panes.push(pane);
                changed = true;
            }
            let tab = &mut self.tabs[index];
            let (dock, moved) = match tab.dock.take() {
                Some(dock) => dock.pin_first(&own),
                None => (Dock::Leaf(own.clone()), true),
            };
            tab.dock = Some(dock);
            if moved {
                tab.dock_states.clear();
                tab.sizes.clear();
                tab.split = cx.new(|_| ResizableState::default());
                changed = true;
            }
        }
        if changed {
            self.schedule_save(cx);
            cx.notify();
        }
    }
    pub(super) fn set_experience(&mut self, experience: Experience, cx: &mut Context<Self>) {
        self.experience = experience;
        self.studio_open = experience == Experience::Streamer;
        self.notice = Some((
            match experience {
                Experience::Streamer => "Streamer: your channel is pinned first in every workspace. Studio is on the right.".into(),
                Experience::Moderator => "Moderator: use 🛡 in the header of channels you moderate.".into(),
                Experience::Viewer => "Viewer: tools hidden; nothing was removed.".into(),
            },
            std::time::Instant::now(),
        ));
        for pane in self.panes() {
            pane.update(cx, |p, cx| {
                p.mod_tools = experience != Experience::Viewer;
                cx.notify();
            });
        }
        self.schedule_save(cx);
        cx.notify();
    }
    pub(super) fn open_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(200., 460.);
        window.open_dialog(cx, move |dialog, _, cx| {
            let current = owner.upgrade().map(|o| o.read(cx).experience).unwrap_or_default();
            dialog
                .w(px(width))
                .title("Choose your setup")
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                            "Changes what's in reach, not what you're allowed to do. Channels, drafts and layouts are kept. Change anytime.",
                        ))
                        .children(Experience::ALL.into_iter().map(|experience| {
                            let owner = owner.clone();
                            let selected = experience == current;
                            div()
                                .id(SharedString::from(format!("setup-{}", experience.key())))
                                .v_flex()
                                .gap_1()
                                .p_2()
                                .rounded(px(6.))
                                .border_1()
                                .border_color(rgb(if selected { 0xA99CF4 } else { theme::BORDER }))
                                .bg(rgb(if selected { 0x272237 } else { theme::CONTROL }))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(theme::HOVER)))
                                .child(div().font_weight(FontWeight::SEMIBOLD).child(if selected {
                                    format!("✓ {}", experience.label())
                                } else {
                                    experience.label().to_owned()
                                }))
                                .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child(experience.description()))
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = owner.update(cx, |this, cx| this.set_experience(experience, cx));
                                })
                        })),
                )
        });
    }
    /// Moderation tools for a channel the account moderates or owns.
    pub(super) fn open_mod_tools(
        &mut self,
        channel: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(200., 420.);
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(this) = owner.upgrade() else {
                return dialog.title("Workspace closed");
            };
            let pane = this.read(cx).panes().into_iter().find(|p| p.read(cx).name.as_ref() == channel);
            let Some(pane) = pane else {
                return dialog.title("Channel closed");
            };
            let settings = pane.read(cx).room_settings.clone();
            let role = pane.read(cx).role.map(|r| r.label()).unwrap_or("Checking role…");
            let clear_owner = owner.clone();
            let clear_pane = pane.clone();
            let clear_channel = channel.clone();
            dialog
                .w(px(width))
                .title(format!("Moderation · #{channel}"))
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("Your role here: {role}. Twitch checks every action.")))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("Chat modes"))
                        .child(mode_buttons(owner.clone(), pane.clone(), channel.clone(), settings))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("Chat"))
                        .child(
                            Button::new("mod-clear-chat")
                                .small()
                                .danger()
                                .label("Clear chat for everyone…")
                                .on_click(move |_, window, cx| {
                                    let (owner, pane, channel) = (clear_owner.clone(), clear_pane.clone(), clear_channel.clone());
                                    window.open_dialog(cx, move |dialog, _, _| {
                                        let (owner, pane, channel) = (owner.clone(), pane.clone(), channel.clone());
                                        dialog
                                            .title(format!("Clear #{channel} for everyone?"))
                                            .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Every viewer's chat is emptied. This can't be undone."))
                                            .footer(div().h_flex().justify_end().gap_2()
                                                .child(Button::new("clear-cancel").small().label("Cancel").on_click(|_, w, cx| w.close_dialog(cx)))
                                                .child(Button::new("clear-confirm").small().danger().label("Clear chat").on_click(move |_, w, cx| {
                                                    w.close_dialog(cx);
                                                    let (pane, channel) = (pane.clone(), channel.clone());
                                                    let _ = owner.update(cx, |this, cx| this.run_twitch_command(pane, channel, Request::Clear, String::new(), w, cx));
                                                })))
                                    });
                                }),
                        )
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                            "Right-click a message to delete, time out or ban. Type /help for every moderator command.",
                        )),
                )
        });
    }
    pub(super) fn render_studio(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let own = self.anchor(cx).unwrap_or_default();
        let pane = self.tabs[self.active]
            .panes
            .iter()
            .find(|p| p.read(cx).name.as_ref() == own)
            .cloned();
        if self.studio_editor.is_none() {
            let account = self.account.clone();
            self.studio_editor = Some(cx.new(|cx| {
                let mut editor = crate::studio::StreamEditor::new(account, window, cx);
                editor.embed();
                editor
            }));
        }
        if self.raid_input.is_none() {
            self.raid_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Channel to raid")));
        }
        let detail = self.streams.detail(&own).map(|(info, _)| info.clone());
        let owner = cx.entity().downgrade();
        let action = |id: &'static str,
                      label: &'static str,
                      request: Request,
                      confirm: Option<String>| {
            let (owner, pane, own) = (owner.clone(), pane.clone(), own.clone());
            Button::new(id)
                .xsmall()
                .label(label)
                .on_click(move |_, window, cx| {
                    let Some(pane) = pane.clone() else { return };
                    let (owner, own, request) = (owner.clone(), own.clone(), request.clone());
                    match confirm.clone() {
                        None => {
                            let _ = owner.update(cx, |this, cx| {
                                this.run_twitch_command(
                                    pane,
                                    own,
                                    request,
                                    String::new(),
                                    window,
                                    cx,
                                )
                            });
                        }
                        Some(question) => window.open_dialog(cx, move |dialog, _, _| {
                            let (owner, pane, own, request) =
                                (owner.clone(), pane.clone(), own.clone(), request.clone());
                            dialog.title(question.clone()).footer(
                                div()
                                    .h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("studio-cancel")
                                            .small()
                                            .label("Cancel")
                                            .on_click(|_, w, cx| w.close_dialog(cx)),
                                    )
                                    .child(
                                        Button::new("studio-confirm")
                                            .small()
                                            .primary()
                                            .label("Confirm")
                                            .on_click(move |_, w, cx| {
                                                w.close_dialog(cx);
                                                let (pane, own, request) =
                                                    (pane.clone(), own.clone(), request.clone());
                                                let _ = owner.update(cx, |this, cx| {
                                                    this.run_twitch_command(
                                                        pane,
                                                        own,
                                                        request,
                                                        String::new(),
                                                        w,
                                                        cx,
                                                    )
                                                });
                                            }),
                                    ),
                            )
                        }),
                    }
                })
        };
        let raid_input = self.raid_input.clone().expect("created above");
        let raid_owner = owner.clone();
        let raid_pane = pane.clone();
        let raid_own = own.clone();
        let card = |title: &'static str| {
            div()
                .v_flex()
                .gap_2()
                .p_2()
                .rounded(px(8.))
                .bg(rgb(theme::ELEVATED))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(rgb(theme::MUTED))
                        .child(title.to_uppercase()),
                )
        };
        let live = detail.as_ref().is_some_and(|i| i.live);
        let (pill_color, pill_text) = match &detail {
            Some(info) if info.live => (
                theme::STREAM_LIVE,
                format!(
                    "● LIVE{}",
                    info.uptime().map(|u| format!(" · {u}")).unwrap_or_default()
                ),
            ),
            Some(_) => (theme::STREAM_OFFLINE, "○ OFFLINE".into()),
            None => (theme::STREAM_UNKNOWN, "◌ CHECKING".into()),
        };
        div()
            .id("studio")
            .v_flex()
            .w(px(STUDIO_WIDTH))
            .h_full()
            .flex_shrink_0()
            .gap_2()
            .p_2()
            .overflow_y_scroll()
            .bg(rgb(theme::PANEL))
            .border_l_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div().h_flex().items_center().gap_2().px_1()
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().font_weight(FontWeight::SEMIBOLD).child(format!("Studio · #{own}")))
            )
            .child(card("Broadcast status")
                .child(div().h_flex().items_center().gap_2()
                    .child(div().px_2().py(px(2.)).rounded(px(10.)).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(pill_color)).border_1().border_color(rgb(pill_color)).child(pill_text))
                    .when_some(detail.as_ref().filter(|_| live), |el, info| el.child(div().text_size(px(12.)).child(format!("{} viewers", info.viewers)))))
                .when_some(detail.as_ref().filter(|i| i.live && !i.category.is_empty()), |el, info| el.child(div().text_size(px(12.)).text_color(rgb(0xC5B8E6)).child(info.category.clone())))
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                    pane.as_ref().map(|p| format!("Chat · {}", p.read(cx).connection)).unwrap_or_default(),
                )))
            .child(card("Stream info").children(self.studio_editor.clone()))
            .child(card("Chat modes").child(match &pane {
                Some(pane) => mode_buttons(owner.clone(), pane.clone(), own.clone(), pane.read(cx).room_settings.clone()),
                None => div(),
            }))
            .child(card("Broadcast")
                .child(div().h_flex().flex_wrap().gap_1()
                    .child(action("studio-marker", "Add marker", Request::Marker { description: String::new() }, None))
                    .child(action("studio-ad-30", "Ad 30s", Request::Commercial { seconds: 30 }, Some("Run a 30-second ad now?".into())))
                    .child(action("studio-ad-60", "Ad 60s", Request::Commercial { seconds: 60 }, Some("Run a 60-second ad now?".into())))
                    .child(action("studio-ad-90", "Ad 90s", Request::Commercial { seconds: 90 }, Some("Run a 90-second ad now?".into()))))
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(if live { "Markers and ads apply to your current stream." } else { "Markers and ads work once you're live." }))
                .child(div().h_flex().gap_1()
                    .child(div().flex_1().child(Input::new(&raid_input).small()))
                    .child(Button::new("studio-raid").xsmall().label("Raid…").on_click(move |_, window, cx| {
                        let target = raid_input.read(cx).value().trim().trim_start_matches('@').to_ascii_lowercase();
                        let Some(Ok(request)) = crate::twitch_actions::parse("/raid", &target) else {
                            window.push_notification(Notification::info("Enter the channel to raid."), cx);
                            return;
                        };
                        let (owner, pane, own) = (raid_owner.clone(), raid_pane.clone(), raid_own.clone());
                        let Some(pane) = pane else { return };
                        window.open_dialog(cx, move |dialog, _, _| {
                            let (owner, pane, own, request) = (owner.clone(), pane.clone(), own.clone(), request.clone());
                            dialog.title(format!("Raid #{target}?"))
                                .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Twitch starts a countdown; you can cancel with /unraid until it ends."))
                                .footer(div().h_flex().justify_end().gap_2()
                                    .child(Button::new("raid-cancel").small().label("Cancel").on_click(|_, w, cx| w.close_dialog(cx)))
                                    .child(Button::new("raid-confirm").small().primary().label("Start raid").on_click(move |_, w, cx| {
                                        w.close_dialog(cx);
                                        let (pane, own, request) = (pane.clone(), own.clone(), request.clone());
                                        let _ = owner.update(cx, |this, cx| this.run_twitch_command(pane, own, request, String::new(), w, cx));
                                    })))
                        });
                    }))))
            .into_any_element()
    }
}
