//! Channel points in Studio > Tools: rewards (pause/resume the ones Jawjack
//! made), a New reward form, and the redemption queue for Jawjack rewards.
use super::*;
use crate::rewards::Job;

impl Workbench {
    fn rewards_job(&mut self, job: Job, cx: &mut Context<Self>) {
        let Some(identity) = self.account.read(cx).identity() else {
            return;
        };
        if !identity.has("channel:manage:redemptions") {
            self.rewards.notice = Some("Managing rewards needs updated Twitch permissions.".into());
            cx.notify();
            return;
        }
        self.rewards.send(&identity, job);
        cx.notify();
    }
    pub(super) fn pump_rewards(
        &mut self,
        identity: Option<&crate::live::Identity>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active = self.studio_open && self.studio_tab == 3;
        if self.rewards.pump(identity, active) {
            cx.notify();
        }
        if let Some(notice) = self.rewards.notice.take() {
            window.push_notification(Notification::info(notice), cx);
        }
    }
    fn open_reward_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Reward name (up to 45)"));
        let cost = cx.new(|cx| InputState::new(window, cx).placeholder("Cost in points"));
        let prompt = cx.new(|cx| InputState::new(window, cx).placeholder("Description (optional)"));
        let needs_input = std::rc::Rc::new(std::cell::Cell::new(false));
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(220., 380.);
        window.open_dialog(cx, move |dialog, _, _| {
            let (title, cost, prompt, needs_input, owner) = (title.clone(), cost.clone(), prompt.clone(), needs_input.clone(), owner.clone());
            let toggle = needs_input.clone();
            dialog
                .w(px(width))
                .title("New channel point reward")
                .on_ok(|_, _, _| false)
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                            "Made from Jawjack, so you can pause it and handle its redemptions here.",
                        ))
                        .child(Input::new(&title).small())
                        .child(Input::new(&cost).small())
                        .child(Input::new(&prompt).small())
                        .child(
                            div()
                                .h_flex()
                                .items_center()
                                .justify_between()
                                .text_size(px(12.))
                                .child("Viewer must type a message")
                                .child(gpui_kit::component::switch::Switch::new("reward-input").checked(needs_input.get()).on_click(move |on: &bool, window, _| {
                                    toggle.set(*on);
                                    window.refresh();
                                })),
                        ),
                )
                .footer(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap_2()
                        .child(Button::new("reward-cancel").small().ghost().label("Cancel").on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(Button::new("reward-create").small().primary().label("Create").on_click(move |_, w, cx| {
                            let name = title.read(cx).value().trim().to_owned();
                            let points = cost.read(cx).value().trim().replace(',', "").parse::<u64>().ok().filter(|c| *c >= 1);
                            let description: String = prompt.read(cx).value().trim().chars().take(200).collect();
                            let (Some(points), false) = (points, name.is_empty() || name.chars().count() > 45) else {
                                w.push_notification(Notification::info("Enter a name (up to 45 characters) and a cost of at least 1."), cx);
                                return;
                            };
                            w.close_dialog(cx);
                            let input = needs_input.get();
                            let _ = owner.update(cx, |this, cx| {
                                this.rewards_job(Job::Create { title: name, cost: points, prompt: description, input }, cx)
                            });
                        })),
                )
        });
    }
    pub(super) fn render_rewards(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(list) = self.rewards.list.clone() else {
            return div()
                .text_size(px(11.))
                .text_color(rgb(theme::MUTED))
                .child("Loading your rewards…");
        };
        let queue = self.rewards.queue.clone();
        div()
            .v_flex()
            .gap_1()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .child(div().flex_1().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!(
                        "{} rewards{}",
                        list.len(),
                        if queue.is_empty() { String::new() } else { format!(" · {} waiting", queue.len()) }
                    )))
                    .child(Button::new("reward-new").xsmall().ghost().icon(IconName::Plus).label("New reward").on_click(cx.listener(|this, _, w, cx| this.open_reward_form(w, cx)))),
            )
            // The queue first: these are people waiting on you.
            .children(queue.into_iter().enumerate().map(|(index, r)| {
                let (done, refund) = ((r.reward_id.clone(), r.id.clone()), (r.reward_id.clone(), r.id.clone()));
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .p_1()
                    .rounded(px(6.))
                    .bg(rgb(0x272237))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(px(12.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(format!("{} · {}", r.user, r.reward)))
                            .when(!r.input.is_empty(), |el| {
                                el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(r.input.clone()))
                            }),
                    )
                    .child(Button::new(("redeem-done", index)).xsmall().primary().label("Complete").on_click(cx.listener(move |this, _, _, cx| {
                        let (reward_id, id) = done.clone();
                        this.rewards_job(Job::Resolve { reward_id, id, fulfil: true }, cx)
                    })))
                    .child(Button::new(("redeem-refund", index)).xsmall().ghost().label("Refund").on_click(cx.listener(move |this, _, _, cx| {
                        let (reward_id, id) = refund.clone();
                        this.rewards_job(Job::Resolve { reward_id, id, fulfil: false }, cx)
                    })))
            }))
            .children(list.into_iter().enumerate().map(|(index, reward)| {
                let (pause_id, delete_id, paused) = (reward.id.clone(), reward.id.clone(), reward.paused);
                div()
                    .id(("reward-row", index))
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .py(px(3.))
                    .text_size(px(12.))
                    .when(!reward.enabled || reward.paused, |el| el.text_color(rgb(theme::MUTED)))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(reward.title.clone()))
                    .child(div().flex_shrink_0().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(reward.cost.to_string()))
                    .when(reward.manageable, |el| {
                        el.child(
                            gpui_kit::component::switch::Switch::new(("reward-active", index))
                                .checked(!paused)
                                .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                    this.rewards_job(Job::Pause { id: pause_id.clone(), paused: !*on }, cx)
                                })),
                        )
                        .child(Button::new(("reward-delete", index)).xsmall().ghost().icon(IconName::Close).tooltip("Delete this reward").on_click(cx.listener(move |this, _, _, cx| {
                            this.rewards_job(Job::Delete { id: delete_id.clone() }, cx)
                        })))
                    })
                    .when(!reward.manageable, |el| {
                        el.child(
                            div()
                                .id(("reward-locked", index))
                                .text_color(rgb(theme::MUTED))
                                .child(Icon::new(gpui_kit::assets::IconName::Lock).xsmall())
                                .tooltip(|w, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new(
                                        "Made on the Twitch dashboard. Twitch only lets the app that made a reward change it.",
                                    )
                                    .build(w, cx)
                                }),
                        )
                    })
            }))
    }
}
