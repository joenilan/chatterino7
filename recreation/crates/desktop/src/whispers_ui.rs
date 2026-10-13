//! Whispers window: conversation list on the left, thread and reply on the right.
use super::*;
use crate::twitch_actions::Request;

impl Workbench {
    /// Whether the whispers window drew within the last moment (it is open).
    fn whispers_visible(&self) -> bool {
        self.whispers_drawn.elapsed() < Duration::from_millis(500)
    }
    pub(super) fn whisper_received(
        &mut self,
        id: String,
        from_login: String,
        from_name: String,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reading = self.whispers_visible();
        if !self
            .whispers
            .incoming(&id, &from_login, &from_name, &text, reading)
        {
            return;
        }
        // Whispers tabs get every whisper (only "a whisper arrived" in streamer mode).
        let whisper_tabs: Vec<_> = self
            .panes()
            .into_iter()
            .filter(|p| p.read(cx).name.as_ref() == crate::combined::WHISPERS)
            .collect();
        if !whisper_tabs.is_empty() {
            let hidden = self.streamer_live;
            let message = chat_core::Message {
                sent_at: Some(chrono::Utc::now().timestamp()),
                id: format!("whisper:{id}"),
                channel_id: crate::combined::WHISPERS.into(),
                user_id: format!("whisper:{from_login}"),
                display_name: if hidden {
                    "Whisper".into()
                } else if from_name.is_empty() {
                    from_login.clone()
                } else {
                    from_name.clone()
                },
                login: (!hidden).then(|| from_login.clone()),
                reply: None,
                mentions: vec![],
                replyable: false,
                name_color: None,
                badges: vec![],
                fragments: vec![chat_core::Fragment::Text(if hidden {
                    "(hidden while streamer mode is on)".into()
                } else {
                    text.clone()
                })],
                presentation: Default::default(),
                deleted: false,
            };
            for pane in whisper_tabs {
                let copy = message.clone();
                pane.update(cx, |p, cx| p.received(chat_core::Event::Message(copy), cx));
            }
        }
        // Chatterino's "show whispers inline": a line in the chats on screen.
        if crate::whispers::INLINE.load(std::sync::atomic::Ordering::Relaxed) && !self.streamer_live
        {
            let name = if from_name.is_empty() {
                &from_login
            } else {
                &from_name
            };
            let text = format!("Whisper from {name}: {text}");
            for pane in self.visible_panes(cx) {
                if crate::combined::is_special(&pane.read(cx).name) {
                    continue;
                }
                let id = format!("system:whisper:{id}:{}", self.next_notice);
                self.next_notice += 1;
                let channel = pane.read(cx).name.to_string();
                let mut notice = chat_core::system_notice(
                    &channel,
                    id,
                    Some(chrono::Utc::now().timestamp()),
                    text.clone(),
                );
                // Coloured by Event highlights › Whispers.
                notice.presentation.notice_type = Some("whisper".into());
                pane.update(cx, |p, cx| {
                    p.received(chat_core::Event::Message(notice), cx)
                });
            }
        }
        let looking = reading && self.whispers.selected.as_deref() == Some(from_login.as_str());
        // Event highlights › Whispers › Sound, unless you're reading that conversation.
        if !looking
            && !(self.streamer_live && self.streamer_mute)
            && (self.sound_focused || !window.is_window_active())
            && let Some(tone) = self
                .event_highlights
                .borrow()
                .tone(crate::event_highlights::Kind::Whisper, &self.sound_tone)
        {
            crate::sound::play(&tone, self.sound_volume);
        }
        if !looking && self.streamer_live {
            // Streamer mode: say that one arrived, never who or what.
            let owner = cx.entity().downgrade();
            window.push_notification(
                Notification::new()
                    .title("New whisper")
                    .message("Hidden while streamer mode is on.")
                    .on_click(move |_, window, cx| {
                        let _ = owner.update(cx, |this, cx| this.open_whispers(None, window, cx));
                    }),
                cx,
            );
        } else if !looking {
            let owner = cx.entity().downgrade();
            let login = from_login.clone();
            window.push_notification(
                Notification::new()
                    .title(format!(
                        "Whisper from {}",
                        if from_name.is_empty() {
                            &from_login
                        } else {
                            &from_name
                        }
                    ))
                    .message(text.chars().take(140).collect::<String>())
                    .on_click(move |_, window, cx| {
                        let login = login.clone();
                        let _ = owner
                            .update(cx, |this, cx| this.open_whispers(Some(login), window, cx));
                    }),
                cx,
            );
        }
        cx.notify();
    }
    fn send_whisper(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(login), Some(input)) =
            (self.whispers.selected.clone(), self.whisper_input.clone())
        else {
            return;
        };
        let text = input.read(cx).value().trim().to_owned();
        if text.is_empty() {
            return;
        }
        let (Some(identity), Some(me)) = (
            self.account.read(cx).identity(),
            self.account.read(cx).login(),
        ) else {
            window.push_notification(Notification::info("Sign in with Twitch to whisper."), cx);
            return;
        };
        if !identity.has("user:manage:whispers") {
            window.push_notification(
                Notification::info("Whispers need updated Twitch permissions (account menu)."),
                cx,
            );
            return;
        }
        let request = Request::Whisper {
            login: login.clone(),
            message: text.clone(),
        };
        let task = cx
            .background_spawn(async move { crate::twitch_actions::run(&identity, &me, &request) });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, window, cx| {
                match result {
                    Ok(_) => {
                        this.whispers.outgoing(&login, &text);
                        input.update(cx, |i, cx| i.set_value("", window, cx));
                    }
                    Err(e) => window.push_notification(Notification::info(e), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn open_whispers(
        &mut self,
        login: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(login) =
            login.or_else(|| self.whispers.conversations.first().map(|c| c.login.clone()))
        {
            self.whispers.select(&login);
        }
        let reply = cx.new(|cx| InputState::new(window, cx).placeholder("Whisper…"));
        cx.subscribe_in(&reply, window, |this: &mut Self, _, event, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.send_whisper(window, cx);
            }
        })
        .detach();
        self.whisper_input = Some(reply.clone());
        let start = cx.new(|cx| InputState::new(window, cx).placeholder("New: Twitch login"));
        cx.subscribe_in(
            &start,
            window,
            |this: &mut Self, input, event, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let value = input.read(cx).value().to_string();
                    if let Some(login) =
                        chat_core::twitch_login(value.trim().trim_start_matches('@'))
                    {
                        this.whispers.outgoing_start(&login);
                        this.whispers.select(&login);
                        input.update(cx, |i, cx| i.set_value("", window, cx));
                        cx.notify();
                    }
                }
            },
        )
        .detach();
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(320., 620.);
        let height = (f32::from(window.viewport_size().height) - 180.).clamp(220., 460.);
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(this) = owner.upgrade() else {
                return dialog.title("Workspace closed");
            };
            this.update(cx, |this, _| {
                this.whispers_drawn = std::time::Instant::now()
            });
            let (conversations, selected) = {
                let this = this.read(cx);
                (
                    this.whispers.conversations.clone(),
                    this.whispers.selected.clone(),
                )
            };
            let current = conversations
                .iter()
                .find(|c| Some(&c.login) == selected.as_ref())
                .cloned();
            let list = div()
                .v_flex()
                .w(px(180.))
                .flex_shrink_0()
                .h_full()
                .border_r_1()
                .border_color(rgb(theme::border()))
                .child(div().p_2().child(Input::new(&start).small()))
                .when(conversations.is_empty(), |el| {
                    el.child(
                        div()
                            .p_2()
                            .text_size(px(11.))
                            .text_color(rgb(theme::muted()))
                            .child("No whispers this session."),
                    )
                })
                .child(
                    div()
                        .id("whisper-list")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .children(conversations.iter().enumerate().map(|(index, c)| {
                            let active = Some(&c.login) == selected.as_ref();
                            let (owner, login) = (owner.clone(), c.login.clone());
                            div()
                                .id(("whisper-conversation", index))
                                .v_flex()
                                .px_2()
                                .py(px(6.))
                                .cursor_pointer()
                                .when(active, |el| el.bg(rgb(theme::control())))
                                .hover(|s| s.bg(rgb(theme::hover())))
                                .child(
                                    div()
                                        .h_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .text_size(px(12.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(c.name.clone()),
                                        )
                                        .when(c.unread > 0, |el| {
                                            el.child(
                                                div()
                                                    .px(px(5.))
                                                    .rounded_full()
                                                    .bg(rgb(theme::accent()))
                                                    .text_color(rgb(crate::theme::tone(0x16121F)))
                                                    .text_size(px(10.))
                                                    .child(c.unread.to_string()),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(rgb(theme::muted()))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(
                                            c.lines
                                                .back()
                                                .map(|l| {
                                                    if l.mine {
                                                        format!("You: {}", l.text)
                                                    } else {
                                                        l.text.clone()
                                                    }
                                                })
                                                .unwrap_or_default(),
                                        ),
                                )
                                .on_click(move |_, _, cx| {
                                    let _ = owner.update(cx, |this, cx| {
                                        this.whispers.select(&login);
                                        cx.notify();
                                    });
                                })
                        })),
                );

            let thread = match current {
                Some(c) => div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(theme::border()))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if c.name.eq_ignore_ascii_case(&c.login) {
                                c.name.clone()
                            } else {
                                format!("{} (@{})", c.name, c.login)
                            }),
                    )
                    .child(
                        div()
                            .id("whisper-thread")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .v_flex()
                            .gap_2()
                            .p_3()
                            .children(c.lines.iter().map(|line| {
                                div().h_flex().when(line.mine, |el| el.justify_end()).child(
                                    div()
                                        .max_w(px(300.))
                                        .px_2()
                                        .py_1()
                                        .rounded(px(8.))
                                        .bg(rgb(if line.mine {
                                            0x2D2648
                                        } else {
                                            theme::control()
                                        }))
                                        .v_flex()
                                        .child(div().text_size(px(13.)).child(line.text.clone()))
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(rgb(theme::muted()))
                                                .child(line.at.format("%H:%M").to_string()),
                                        ),
                                )
                            })),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .p_2()
                            .border_t_1()
                            .border_color(rgb(theme::border()))
                            .child(div().flex_1().child(Input::new(&reply).small()))
                            .child(
                                Button::new("whisper-send")
                                    .small()
                                    .primary()
                                    .label("Send")
                                    .on_click({
                                        let owner = owner.clone();
                                        move |_, window, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.send_whisper(window, cx)
                                            });
                                        }
                                    }),
                            ),
                    ),
                None => div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .text_color(rgb(theme::muted()))
                    .child("Pick a conversation or type a login to start one."),
            };
            dialog
                .w(px(width))
                .title("Whispers")
                // Enter belongs to the inputs (start a conversation, send), not "OK".
                .on_ok(|_, _, _| false)
                .child(
                    div()
                        .h(px(height))
                        .h_flex()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(rgb(theme::border()))
                        .overflow_hidden()
                        .child(list)
                        .child(thread),
                )
        });
        cx.notify();
    }
}
