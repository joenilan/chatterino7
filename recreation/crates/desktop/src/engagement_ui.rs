//! Studio → Tools → Polls & predictions (layout rules in docs/STUDIO.md).
use super::*;
use crate::engagement::{Choice, Job};
use gpui_kit::assets::IconName as Kit;

fn remaining(until: Option<chrono::DateTime<chrono::Utc>>) -> Option<String> {
    let seconds = (until? - chrono::Utc::now()).num_seconds();
    (seconds > 0).then(|| format!("{}:{:02} left", seconds / 60, seconds % 60))
}

impl Workbench {
    fn engagement_job(&mut self, job: Job, cx: &mut Context<Self>) {
        if let Some(identity) = self.account.read(cx).identity() {
            self.engagement.send(&identity, job);
            cx.notify();
        }
    }
    pub(super) fn pump_engagement(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let identity = self.account.read(cx).identity();
        let visible = self.studio_open && self.studio_tab == 2 && self.anchor(cx).is_some();
        let (changed, results) = self.engagement.pump(identity.as_ref(), visible);
        for result in results {
            let text = match result {
                Ok(text) => text,
                Err(e) => e,
            };
            window.push_notification(Notification::info(text), cx);
        }
        if changed {
            cx.notify();
        }
    }
    /// One vote/points bar per choice; widths ease toward the new share.
    fn result_bars(
        &self,
        key: &str,
        choices: &[Choice],
        prediction: bool,
        winner: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let total: u64 = choices
            .iter()
            .map(|c| if prediction { c.points } else { c.votes })
            .sum();
        div()
            .v_flex()
            .gap_2()
            .children(choices.iter().map(|choice| {
                let value = if prediction {
                    choice.points
                } else {
                    choice.votes
                };
                let share = if total > 0 {
                    value as f32 / total as f32
                } else {
                    0.
                };
                let width = gpui_kit::base::motion::transition(
                    SharedString::from(format!("bar-{key}-{}", choice.id)),
                    share,
                    gpui_kit::base::motion::Transition::new(Duration::from_millis(400)),
                    window,
                    cx,
                );
                let won = winner == Some(choice.id.as_str());
                div()
                    .v_flex()
                    .gap(px(3.))
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .text_size(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .when(won, |el| el.font_weight(FontWeight::SEMIBOLD))
                                    .child(if won {
                                        format!("✓ {}", choice.title)
                                    } else {
                                        choice.title.clone()
                                    }),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::MUTED))
                                    .child(if prediction {
                                        format!("{} pts · {}", choice.points, choice.users)
                                    } else {
                                        format!("{} · {:.0}%", choice.votes, share * 100.)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .h(px(6.))
                            .w_full()
                            .rounded_full()
                            .bg(rgb(theme::CONTROL))
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(rgb(if won { theme::STREAM_LIVE } else { 0xA99CF4 }))
                                    .w(relative(width.clamp(0., 1.))),
                            ),
                    )
            }))
    }
    pub(super) fn render_engagement(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let busy = self.engagement.busy;
        let mut section = div().v_flex().gap_1();
        if let Some(problem) = self.engagement.problem.clone() {
            section = section.child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(theme::MUTED))
                    .child(problem),
            );
        }
        // Poll
        let poll = self.engagement.poll.clone();
        let poll_card = match poll
            .filter(|p| p.running() || p.status == "COMPLETED" || p.status == "TERMINATED")
        {
            Some(poll) => {
                let running = poll.running();
                let id = poll.id.clone();
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
                            .h_flex()
                            .gap_2()
                            .items_center()
                            .child(gpui_kit::component::Icon::new(Kit::ChartBar).xsmall())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(poll.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(if running {
                                        theme::STREAM_LIVE
                                    } else {
                                        theme::MUTED
                                    }))
                                    .child(if running {
                                        remaining(poll.ends).unwrap_or_else(|| "Ending…".into())
                                    } else {
                                        "Ended".into()
                                    }),
                            ),
                    )
                    .child(self.result_bars(&poll.id, &poll.choices, false, None, window, cx))
                    .child(
                        div()
                            .h_flex()
                            .justify_end()
                            .gap_2()
                            .when(running, |el| {
                                el.child(
                                    Button::new("poll-end")
                                        .xsmall()
                                        .ghost()
                                        .label("End poll")
                                        .disabled(busy)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.engagement_job(Job::EndPoll { id: id.clone() }, cx)
                                        })),
                                )
                            })
                            .when(!running, |el| {
                                el.child(
                                    Button::new("poll-new")
                                        .xsmall()
                                        .ghost()
                                        .label("New poll")
                                        .disabled(busy)
                                        .on_click(
                                            cx.listener(|this, _, w, cx| this.open_new_poll(w, cx)),
                                        ),
                                )
                            }),
                    )
            }
            None => Self::tool_row(Kit::ChartBar, "Poll", "Ask chat a question").child(
                Button::new("poll-new")
                    .xsmall()
                    .ghost()
                    .label("New…")
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, w, cx| this.open_new_poll(w, cx))),
            ),
        };
        section = section.child(poll_card);
        // Prediction
        let prediction = self.engagement.prediction.clone();
        let prediction_card = match prediction.filter(|p| p.open() || p.status == "RESOLVED") {
            Some(p) => {
                let (active, locked) = (p.status == "ACTIVE", p.status == "LOCKED");
                let (lock_id, cancel_id) = (p.id.clone(), p.id.clone());
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
                            .h_flex()
                            .gap_2()
                            .items_center()
                            .child(gpui_kit::component::Icon::new(Kit::Trophy).xsmall())
                            .child(div().flex_1().min_w_0().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(p.title.clone()))
                            .child(div().text_size(px(10.)).text_color(rgb(if active { theme::STREAM_LIVE } else { theme::MUTED })).child(match p.status.as_str() {
                                "ACTIVE" => remaining(p.locks).unwrap_or_else(|| "Locking…".into()),
                                "LOCKED" => "Locked · pick the winner".into(),
                                _ => "Resolved".into(),
                            })),
                    )
                    .child(self.result_bars(&p.id, &p.outcomes, true, p.winner.as_deref(), window, cx))
                    // Locked: each outcome can be confirmed as the winner.
                    .when(locked, |el| {
                        el.child(div().h_flex().flex_wrap().gap_1().children(p.outcomes.iter().enumerate().map(|(index, outcome)| {
                            let (id, winner, title) = (p.id.clone(), outcome.id.clone(), outcome.title.clone());
                            let owner = cx.entity().downgrade();
                            Button::new(("prediction-win", index)).xsmall().ghost().icon(gpui_kit::component::Icon::new(Kit::Trophy)).label(outcome.title.clone()).disabled(busy).on_click(move |_, window, cx| {
                                let (owner, id, winner) = (owner.clone(), id.clone(), winner.clone());
                                Self::studio_confirm(
                                    format!("“{title}” wins?"),
                                    "Channel points are paid out to everyone who predicted it. This can't be undone.",
                                    "Pay out",
                                    move |_, cx| {
                                        let _ = owner.update(cx, |this, cx| this.engagement_job(Job::ResolvePrediction { id: id.clone(), winner: winner.clone() }, cx));
                                    },
                                    window,
                                    cx,
                                );
                            })
                        })))
                    })
                    .child(
                        div()
                            .h_flex()
                            .justify_end()
                            .gap_2()
                            .when(active, |el| {
                                el.child(Button::new("prediction-lock").xsmall().ghost().icon(gpui_kit::component::Icon::new(Kit::Lock)).label("Lock").disabled(busy).on_click(cx.listener(move |this, _, _, cx| {
                                    this.engagement_job(Job::LockPrediction { id: lock_id.clone() }, cx)
                                })))
                            })
                            .when(active || locked, |el| {
                                let owner = cx.entity().downgrade();
                                el.child(Button::new("prediction-cancel").xsmall().ghost().label("Cancel & refund").disabled(busy).on_click(move |_, window, cx| {
                                    let (owner, id) = (owner.clone(), cancel_id.clone());
                                    Self::studio_confirm(
                                        "Cancel this prediction?".into(),
                                        "Everyone gets their channel points back.",
                                        "Cancel & refund",
                                        move |_, cx| {
                                            let _ = owner.update(cx, |this, cx| this.engagement_job(Job::CancelPrediction { id: id.clone() }, cx));
                                        },
                                        window,
                                        cx,
                                    );
                                }))
                            })
                            .when(!active && !locked, |el| el.child(Button::new("prediction-new").xsmall().ghost().label("New prediction").disabled(busy).on_click(cx.listener(|this, _, w, cx| this.open_new_prediction(w, cx))))),
                    )
            }
            None => Self::tool_row(Kit::Trophy, "Prediction", "Let chat bet channel points").child(
                Button::new("prediction-new")
                    .xsmall()
                    .ghost()
                    .label("New…")
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, w, cx| this.open_new_prediction(w, cx))),
            ),
        };
        section.child(prediction_card)
    }
    pub(super) fn tool_row(icon: Kit, title: &'static str, detail: &'static str) -> Div {
        div()
            .h_flex()
            .items_center()
            .gap_3()
            .py_2()
            .child(
                div()
                    .size(px(28.))
                    .flex_shrink_0()
                    .rounded(px(6.))
                    .bg(rgb(theme::CONTROL))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0xA99CF4))
                    .child(gpui_kit::component::Icon::new(icon).xsmall()),
            )
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_size(px(12.)).child(title))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(theme::MUTED))
                            .child(detail),
                    ),
            )
    }
    /// Create dialog shared by polls and predictions: a title, options and a duration.
    fn open_engagement_form(
        &mut self,
        prediction: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (title_limit, max_options) = if prediction { (45, 4) } else { (60, 5) };
        let title = cx.new(|cx| {
            InputState::new(window, cx).placeholder(if prediction {
                "What will happen?"
            } else {
                "Question"
            })
        });
        let options: Vec<Entity<InputState>> = (0..max_options)
            .map(|i| {
                cx.new(|cx| {
                    InputState::new(window, cx).placeholder(if i < 2 {
                        format!("Option {}", i + 1)
                    } else {
                        format!("Option {} (optional)", i + 1)
                    })
                })
            })
            .collect();
        let duration = std::rc::Rc::new(std::cell::Cell::new(120u32));
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(220., 400.);
        window.open_dialog(cx, move |dialog, _, _| {
            let (title, options, duration, owner) = (
                title.clone(),
                options.clone(),
                duration.clone(),
                owner.clone(),
            );
            let chosen = duration.get();
            dialog
                .w(px(width))
                .title(if prediction {
                    "New prediction"
                } else {
                    "New poll"
                })
                // Enter belongs to the fields inside; it must not close the dialog.
                .on_ok(|_, _, _| false)
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(Input::new(&title).small())
                        .children(options.iter().map(|o| Input::new(o).small()))
                        .child(
                            div()
                                .pt_1()
                                .text_size(px(11.))
                                .text_color(rgb(theme::MUTED))
                                .child(if prediction {
                                    "Predictions open for"
                                } else {
                                    "Voting lasts"
                                }),
                        )
                        .child(div().h_flex().gap_1().children(
                            [60u32, 120, 300, 600].into_iter().map(|seconds| {
                                let duration = duration.clone();
                                let selected = chosen == seconds;
                                Button::new(("duration", seconds as usize))
                                    .xsmall()
                                    .ghost()
                                    .selected(selected)
                                    .when(selected, |b| b.primary())
                                    .label(format!("{} min", seconds / 60))
                                    .on_click(move |_, window, _| {
                                        duration.set(seconds);
                                        window.refresh();
                                    })
                            }),
                        )),
                )
                .footer(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("engagement-cancel")
                                .small()
                                .ghost()
                                .label("Cancel")
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(
                            Button::new("engagement-start")
                                .small()
                                .primary()
                                .label("Start")
                                .on_click(move |_, w, cx| {
                                    let question = title.read(cx).value().trim().to_owned();
                                    let choices: Vec<String> = options
                                        .iter()
                                        .map(|o| o.read(cx).value().trim().to_owned())
                                        .filter(|o| !o.is_empty())
                                        .collect();
                                    let problem = if question.is_empty()
                                        || question.chars().count() > title_limit
                                    {
                                        Some(format!(
                                            "Enter a title of up to {title_limit} characters."
                                        ))
                                    } else if choices.len() < 2 {
                                        Some("Enter at least two options.".into())
                                    } else if choices.iter().any(|c| c.chars().count() > 25) {
                                        Some("Options can be up to 25 characters.".into())
                                    } else {
                                        None
                                    };
                                    if let Some(problem) = problem {
                                        w.push_notification(Notification::info(problem), cx);
                                        return;
                                    }
                                    w.close_dialog(cx);
                                    let seconds = duration.get();
                                    let job = if prediction {
                                        Job::CreatePrediction {
                                            title: question,
                                            outcomes: choices,
                                            seconds,
                                        }
                                    } else {
                                        Job::CreatePoll {
                                            title: question,
                                            choices,
                                            seconds,
                                        }
                                    };
                                    let _ =
                                        owner.update(cx, |this, cx| this.engagement_job(job, cx));
                                }),
                        ),
                )
        });
    }
    fn open_new_poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_engagement_form(false, window, cx);
    }
    fn open_new_prediction(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_engagement_form(true, window, cx);
    }
}
