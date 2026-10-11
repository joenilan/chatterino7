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
        let mut changed = self.order_pinned_tabs(cx);
        let Some(own) = self.anchor(cx) else {
            if changed {
                self.schedule_save(cx);
                cx.notify();
            }
            return;
        };
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
    /// Keep pinned tabs at the front of their group (after the Streamer anchor).
    fn order_pinned_tabs(&mut self, cx: &mut Context<Self>) -> bool {
        let anchor = self.anchor(cx);
        let mut changed = false;
        for tab in &mut self.tabs {
            let names: Vec<String> = tab
                .panes
                .iter()
                .map(|p| p.read(cx).name.to_string())
                .collect();
            tab.pinned.retain(|c| names.contains(c));
            let mut order: Vec<String> = anchor.iter().cloned().collect();
            order.extend(
                tab.pinned
                    .iter()
                    .filter(|c| Some(*c) != anchor.as_ref())
                    .cloned(),
            );
            if !order.is_empty()
                && let Some(dock) = &mut tab.dock
            {
                changed |= dock.pin_front(&order);
            }
        }
        changed
    }
    pub(super) fn is_pinned(&self, channel: &str, cx: &App) -> bool {
        self.anchor(cx).as_deref() == Some(channel)
            || self
                .tabs
                .get(self.active)
                .is_some_and(|t| t.pinned.iter().any(|c| c == channel))
    }
    pub(super) fn toggle_pin(&mut self, channel: &str, cx: &mut Context<Self>) {
        if self.anchor(cx).as_deref() == Some(channel) {
            self.notice = Some((
                "Your channel is pinned by the Streamer setup.".into(),
                std::time::Instant::now(),
            ));
            cx.notify();
            return;
        }
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        if let Some(at) = tab.pinned.iter().position(|c| c == channel) {
            tab.pinned.remove(at);
        } else {
            tab.pinned.push(channel.to_owned());
        }
        self.order_pinned_tabs(cx);
        self.schedule_save(cx);
        cx.notify();
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
    /// Request repaints for a short animation (no-op with reduced motion).
    pub(super) fn animate_for(&self, duration: Duration, cx: &mut Context<Self>) {
        if cx.reduce_motion() {
            return;
        }
        let frames = (duration.as_millis() / 16) as usize + 1;
        cx.spawn(async move |view, cx| {
            for _ in 0..frames {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if view.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
    }
    fn studio_run(
        owner: WeakEntity<Workbench>,
        pane: Option<Entity<ChannelPane>>,
        own: String,
        request: Request,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(pane) = pane else { return };
        let _ = owner.update(cx, |this, cx| {
            this.run_twitch_command(pane, own, request, String::new(), window, cx)
        });
    }
    /// A small confirmation dialog for consequential broadcast actions.
    pub(super) fn studio_confirm(
        title: String,
        body: &'static str,
        action: &'static str,
        run: impl Fn(&mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut App,
    ) {
        let run = std::rc::Rc::new(run);
        window.open_dialog(cx, move |dialog, _, _| {
            let run = run.clone();
            dialog
                .title(title.clone())
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(theme::MUTED))
                        .child(body),
                )
                .footer(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("studio-cancel")
                                .small()
                                .ghost()
                                .label("Cancel")
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(
                            Button::new("studio-confirm")
                                .small()
                                .primary()
                                .label(action)
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    run(w, cx);
                                }),
                        ),
                )
        });
    }
    fn studio_section(title: &'static str) -> Div {
        div()
            .pt_4()
            .pb_2()
            .text_size(px(10.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::MUTED))
            .child(title.to_uppercase())
    }
    fn feed_icon(kind: crate::activity_feed::Kind) -> gpui_kit::assets::IconName {
        use crate::activity_feed::Kind;
        use gpui_kit::assets::IconName as I;
        match kind {
            Kind::Follow => I::UserPlus,
            Kind::Subscription => I::Star,
            Kind::Bits => I::Gem,
            Kind::Raid => I::Rocket,
            Kind::Redemption => I::Sparkles,
            Kind::Donation => I::HandCoins,
            Kind::Moderation => I::Shield,
            Kind::Other => I::Activity,
        }
    }
    /// Activity tab: filter chips, then the newest-first feed.
    fn render_feed(&mut self, cx: &mut Context<Self>) -> AnyElement {
        use crate::activity_feed::Kind;
        let filters: [(Option<Kind>, &str); 7] = [
            (None, "All"),
            (Some(Kind::Follow), "Follows"),
            (Some(Kind::Subscription), "Subs"),
            (Some(Kind::Bits), "Bits"),
            (Some(Kind::Redemption), "Rewards"),
            (Some(Kind::Donation), "Tips"),
            (Some(Kind::Moderation), "Mods"),
        ];
        let chips = div().h_flex().flex_wrap().gap_1().pt_3().children(
            filters
                .into_iter()
                .enumerate()
                .map(|(index, (kind, label))| {
                    let selected = self.feed_filter == kind;
                    div()
                        .id(("feed-filter", index))
                        .px(px(7.))
                        .py(px(2.))
                        .rounded_full()
                        .text_size(px(11.))
                        .cursor_pointer()
                        .border_1()
                        .map(|el| {
                            if selected {
                                el.bg(rgb(0x272237))
                                    .border_color(rgb(0xA99CF4))
                                    .text_color(rgb(0xD9D2FA))
                            } else {
                                el.border_color(rgb(theme::BORDER))
                                    .text_color(rgb(theme::MUTED))
                                    .hover(|s| s.bg(rgb(theme::HOVER)).text_color(rgb(theme::TEXT)))
                            }
                        })
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.feed_filter = kind;
                            cx.notify();
                        }))
                }),
        );
        let items: Vec<_> = self
            .feed
            .items
            .iter()
            .filter(|i| self.feed_filter.is_none_or(|k| i.kind == k))
            .take(60)
            .cloned()
            .collect();
        if items.is_empty() {
            return div()
                .v_flex()
                .child(chips)
                .child(
                    div()
                        .v_flex()
                        .items_center()
                        .gap_2()
                        .py_8()
                        .child(
                            div()
                                .size(px(36.))
                                .rounded_full()
                                .bg(rgb(theme::CONTROL))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(theme::MUTED))
                                .child(gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Inbox)),
                        )
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(if self.feed_filter.is_some() { "Nothing of this kind yet" } else { "No activity yet" }))
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).text_center().child("Follows, subs, Bits, raids, rewards and donations land here as they happen.")),
                )
                .into_any_element();
        }
        let now = chrono::Local::now();
        let reduced = cx.reduce_motion();
        div()
            .v_flex()
            .child(chips)
            .child(
                div()
                    .v_flex()
                    .pt_2()
                    .children(items.into_iter().enumerate().map(|(index, item)| {
                        let (_, accent, background) =
                            crate::highlight_rules::COLORS[item.kind.color()];
                        // The newest item eases in from above.
                        let age = (now - item.at).num_milliseconds().max(0) as f32;
                        let t = if reduced {
                            1.
                        } else {
                            (age / 300.).clamp(0., 1.)
                        };
                        let eased = 1. - (1. - t).powi(3);
                        div()
                            .id(("feed-item", index))
                            .h_flex()
                            .items_start()
                            .gap_2()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(theme::BORDER))
                            .opacity(0.2 + 0.8 * eased)
                            .top(px(-6. * (1. - eased)))
                            .child(
                                div()
                                    .id(("feed-kind", index))
                                    .tooltip({
                                        let label = item.kind.label();
                                        move |w, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new(label)
                                                .build(w, cx)
                                        }
                                    })
                                    .size(px(26.))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .bg(rgb(background))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(accent))
                                    .child(
                                        gpui_kit::component::Icon::new(Self::feed_icon(item.kind))
                                            .xsmall(),
                                    ),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .text_ellipsis()
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(item.who.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .text_size(px(10.))
                                                    .text_color(rgb(theme::MUTED))
                                                    .child(item.at.format("%H:%M").to_string()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(rgb(theme::MUTED))
                                            .line_clamp(2)
                                            .child(if item.source == "Twitch" {
                                                item.detail.clone()
                                            } else {
                                                format!("{} · {}", item.source, item.detail)
                                            }),
                                    ),
                            )
                    })),
            )
            .into_any_element()
    }
    /// Stream tab: stream info editor, then chat modes as switches.
    fn render_stream_tab(
        &mut self,
        pane: Option<Entity<ChannelPane>>,
        own: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let modes = pane.as_ref().and_then(|p| p.read(cx).room_settings.clone());
        let rows: Vec<(&str, String, bool, &str, &str)> = match &modes {
            Some(m) => vec![
                (
                    "Slow mode",
                    m.slow
                        .map(|s| format!("{s} s between messages"))
                        .unwrap_or_else(|| "30 s between messages".into()),
                    m.slow.is_some(),
                    "/slow",
                    "30",
                ),
                (
                    "Followers only",
                    match m.followers {
                        Some(0) => "Any follower".into(),
                        Some(n) => format!("Followed for {n} min"),
                        None => "Only followers can chat".into(),
                    },
                    m.followers.is_some(),
                    "/followers",
                    "",
                ),
                (
                    "Subscribers only",
                    "Only subscribers can chat".into(),
                    m.subscribers,
                    "/subscribers",
                    "",
                ),
                (
                    "Emote only",
                    "Messages must be emotes".into(),
                    m.emote,
                    "/emoteonly",
                    "",
                ),
                (
                    "Unique chat",
                    "No repeated messages".into(),
                    m.unique,
                    "/uniquechat",
                    "",
                ),
            ],
            None => Vec::new(),
        };
        div()
            .v_flex()
            .child(Self::studio_section("OBS"))
            .child(self.render_obs_card(cx))
            .child(Self::studio_section("Stream info"))
            .children(self.studio_editor.clone())
            .child(Self::studio_section("Chat modes"))
            .when(rows.is_empty(), |el| {
                el.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme::MUTED))
                        .child("Available once your chat is connected."),
                )
            })
            .children(rows.into_iter().enumerate().map(
                |(index, (label, detail, on, command, arg))| {
                    let (owner, pane, own) = (owner.clone(), pane.clone(), own.clone());
                    div()
                        .h_flex()
                        .items_center()
                        .gap_3()
                        .py_2()
                        .child(
                            div()
                                .v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_size(px(12.)).child(label))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(rgb(theme::MUTED))
                                        .child(detail),
                                ),
                        )
                        .child(
                            gpui_kit::component::switch::Switch::new(("chat-mode", index))
                                .checked(on)
                                .on_click(move |checked: &bool, window, cx| {
                                    let name = if *checked {
                                        command.to_owned()
                                    } else {
                                        format!("{command}off")
                                    };
                                    let args = if *checked { arg } else { "" };
                                    if let Some(Ok(request)) =
                                        crate::twitch_actions::parse(&name, args)
                                    {
                                        Self::studio_run(
                                            owner.clone(),
                                            pane.clone(),
                                            own.clone(),
                                            request,
                                            window,
                                            cx,
                                        );
                                    }
                                }),
                        )
                },
            ))
            .into_any_element()
    }
    /// Tools tab: quick broadcast actions and Streamer.bot.
    fn render_tools_tab(
        &mut self,
        pane: Option<Entity<ChannelPane>>,
        own: String,
        live: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let raid_input = self.raid_input.clone().expect("created by render_studio");
        let row = Self::tool_row;
        let marker = {
            let (owner, pane, own) = (owner.clone(), pane.clone(), own.clone());
            Button::new("studio-marker")
                .xsmall()
                .ghost()
                .label("Add")
                .disabled(!live)
                .on_click(move |_, window, cx| {
                    Self::studio_run(
                        owner.clone(),
                        pane.clone(),
                        own.clone(),
                        Request::Marker {
                            description: String::new(),
                        },
                        window,
                        cx,
                    );
                })
        };
        let schedule = self.ads.schedule.clone().filter(|_| live);
        let snoozes = schedule.as_ref().map_or(0, |s| s.snoozes);
        let ad_line = match &schedule {
            Some(s) => match s.next_ad_at.and_then(crate::ads::countdown) {
                Some(left) => format!(
                    "Next ad in {left}{} · {snoozes} snooze{} left",
                    if s.duration > 0 {
                        format!(" ({}s)", s.duration)
                    } else {
                        String::new()
                    },
                    if snoozes == 1 { "" } else { "s" }
                ),
                None => format!(
                    "No ad scheduled · {snoozes} snooze{} left",
                    if snoozes == 1 { "" } else { "s" }
                ),
            },
            None if live => "Run an ad now".into(),
            None => "Available while live".into(),
        };
        let ads = div()
            .h_flex()
            .gap_1()
            .when(
                schedule.as_ref().is_some_and(|s| s.next_ad_at.is_some()),
                |el| {
                    el.child(
                        Button::new("studio-ad-snooze")
                            .xsmall()
                            .ghost()
                            .label("Snooze")
                            .tooltip("Push the next ad back 5 minutes")
                            .disabled(snoozes == 0)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(identity) = this.account.read(cx).identity() {
                                    if identity.has("channel:manage:ads") {
                                        this.ads.snooze(&identity);
                                    } else {
                                        this.ads.notice = Some(
                                            "Snoozing ads needs updated Twitch permissions.".into(),
                                        );
                                    }
                                }
                                cx.notify();
                            })),
                    )
                },
            )
            .children([30u32, 60, 90].into_iter().map(|seconds| {
                let (owner, pane, own) = (owner.clone(), pane.clone(), own.clone());
                Button::new(("studio-ad", seconds as usize))
                    .xsmall()
                    .ghost()
                    .label(format!("{seconds}s"))
                    .disabled(!live)
                    .on_click(move |_, window, cx| {
                        let (owner, pane, own) = (owner.clone(), pane.clone(), own.clone());
                        Self::studio_confirm(
                            format!("Run a {seconds}-second ad?"),
                            "Viewers see an ad break now.",
                            "Run ad",
                            move |w, cx| {
                                Self::studio_run(
                                    owner.clone(),
                                    pane.clone(),
                                    own.clone(),
                                    Request::Commercial { seconds },
                                    w,
                                    cx,
                                )
                            },
                            window,
                            cx,
                        );
                    })
            }));
        let raid = {
            let (owner, pane, own, input) =
                (owner.clone(), pane.clone(), own.clone(), raid_input.clone());
            Button::new("studio-raid")
                .xsmall()
                .ghost()
                .label("Raid")
                .on_click(move |_, window, cx| {
                    let target = input
                        .read(cx)
                        .value()
                        .trim()
                        .trim_start_matches('@')
                        .to_ascii_lowercase();
                    let Some(Ok(request)) = crate::twitch_actions::parse("/raid", &target) else {
                        window.push_notification(
                            Notification::info("Enter the channel to raid."),
                            cx,
                        );
                        return;
                    };
                    let (owner, pane, own) = (owner.clone(), pane.clone(), own.clone());
                    Self::studio_confirm(
                        format!("Raid #{target}?"),
                        "Twitch starts a countdown; /unraid cancels it until it ends.",
                        "Start raid",
                        move |w, cx| {
                            Self::studio_run(
                                owner.clone(),
                                pane.clone(),
                                own.clone(),
                                request.clone(),
                                w,
                                cx,
                            )
                        },
                        window,
                        cx,
                    );
                })
        };
        div()
            .v_flex()
            .child(Self::studio_section("Broadcast"))
            .child(
                row(
                    gpui_kit::assets::IconName::BookmarkPlus,
                    "Stream marker",
                    if live {
                        "Mark this moment for highlights"
                    } else {
                        "Available while live"
                    },
                )
                .child(marker),
            )
            .child(Self::tool_row(gpui_kit::assets::IconName::Tv, "Ad break", ad_line).child(ads))
            .child(row(
                gpui_kit::assets::IconName::Rocket,
                "Raid",
                "Send your viewers to another channel",
            ))
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .pl(px(40.))
                    .child(div().flex_1().child(Input::new(&raid_input).small()))
                    .child(raid),
            )
            .child(Self::studio_section("Polls & predictions"))
            .child(self.render_engagement(window, cx))
            .child(Self::studio_section("Streamer.bot"))
            .child(self.render_bot_card(cx))
            .into_any_element()
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
            let (account, presets) = (self.account.clone(), self.stream_presets.clone());
            let editor = cx.new(|cx| {
                let mut editor = crate::studio::StreamEditor::new(account, presets, window, cx);
                editor.embed();
                editor
            });
            cx.subscribe(&editor, |this, _, _: &crate::studio::PresetsChanged, cx| {
                this.schedule_save(cx)
            })
            .detach();
            self.studio_editor = Some(editor);
        }
        if self.raid_input.is_none() {
            self.raid_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Channel")));
        }
        let detail = self.streams.detail(&own).map(|(info, _)| info.clone());
        let live = detail.as_ref().is_some_and(|i| i.live);
        let (tag_color, tag) = match &detail {
            Some(info) if info.live => (
                theme::STREAM_LIVE,
                format!(
                    "LIVE{}",
                    info.uptime().map(|u| format!(" · {u}")).unwrap_or_default()
                ),
            ),
            Some(_) => (theme::STREAM_OFFLINE, "OFFLINE".into()),
            None => (theme::STREAM_UNKNOWN, "CHECKING".into()),
        };
        let chat_problem = pane
            .as_ref()
            .map(|p| p.read(cx))
            .filter(|p| !p.connected)
            .map(|p| p.connection.clone());
        if self.studio_tab == 0 {
            self.feed.unseen = 0;
        }
        let unseen = self.feed.unseen;
        // Tab content fades in when switching.
        let fade = if cx.reduce_motion() {
            1.
        } else {
            (self.studio_tab_switched.elapsed().as_secs_f32() / 0.15).clamp(0., 1.)
        };
        let body = match self.studio_tab {
            1 => self.render_stream_tab(pane.clone(), own.clone(), cx),
            2 => self.render_tools_tab(pane.clone(), own.clone(), live, window, cx),
            _ => self.render_feed(cx),
        };
        div()
            .v_flex()
            .w(px(STUDIO_WIDTH))
            .h_full()
            .flex_shrink_0()
            .bg(rgb(theme::PANEL))
            .border_l_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("#{own}")),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .px_2()
                                    .py(px(1.))
                                    .rounded(px(10.))
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(tag_color))
                                    .border_1()
                                    .border_color(rgb(tag_color))
                                    .child(tag),
                            ),
                    )
                    // Always exactly one status line, so the tabs never shift.
                    .child(
                        div()
                            .text_size(px(11.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .map(
                                |el| match (&chat_problem, detail.as_ref().filter(|i| i.live)) {
                                    (Some(problem), _) => el
                                        .text_color(rgb(theme::STREAM_UNKNOWN))
                                        .child(format!("Chat · {problem}")),
                                    (None, Some(info)) => el.text_color(rgb(theme::MUTED)).child(
                                        format!("{} viewers · {}", info.viewers, info.category),
                                    ),
                                    (None, None) => {
                                        el.text_color(rgb(theme::MUTED)).child("Chat connected")
                                    }
                                },
                            ),
                    ),
            )
            .when(!self.hype.is_empty(), |el| {
                el.child(self.render_hype_strip())
            })
            .child(
                div().px_3().child(
                    gpui_kit::component::tab::TabBar::new("studio-tabs")
                        .segmented()
                        .selected_index(self.studio_tab)
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            if this.studio_tab != *index {
                                this.studio_tab = *index;
                                this.studio_tab_switched = std::time::Instant::now();
                                this.animate_for(Duration::from_millis(160), cx);
                                this.schedule_save(cx);
                                cx.notify();
                            }
                        }))
                        .child(gpui_kit::component::tab::Tab::new().label(if unseen > 0 {
                            format!("Activity · {unseen}")
                        } else {
                            "Activity".into()
                        }))
                        .child(gpui_kit::component::tab::Tab::new().label("Stream"))
                        .child(gpui_kit::component::tab::Tab::new().label("Tools")),
                ),
            )
            .child(
                div()
                    .id("studio-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_3()
                    .pb_3()
                    .opacity(fade)
                    .child(body),
            )
            .into_any_element()
    }
}
