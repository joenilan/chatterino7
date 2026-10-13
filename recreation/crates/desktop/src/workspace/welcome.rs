//! First-run welcome: shown once per profile, the first time an account signs
//! in. Pick how Jawjack is set up (Viewer / Moderator / Streamer), then open
//! some chat: your own channel, channels you moderate, followed channels that
//! are live, Mentions, or everything from Chatterino. Reopen it from
//! Settings › General.
use super::*;
use roles::Experience;

/// How many live follows the welcome lists.
const LIVE_SHOWN: usize = 5;

impl Workbench {
    pub(super) fn open_welcome(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(300., 560.);
        let height = (f32::from(window.viewport_size().height) - 180.).clamp(200., 620.);
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(this) = owner.upgrade() else {
                return dialog.title("Welcome to Jawjack");
            };
            let me = this.read(cx);
            let login = me.account.read(cx).login();
            let current = me.experience;
            let open: std::collections::BTreeSet<String> =
                me.tab_panes().iter().map(|p| p.read(cx).name.to_string()).collect();
            let moderated = me.capabilities.moderated();
            let live: Vec<crate::follows_live::LiveFollow> = me
                .follows_live
                .list
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter(|l| !open.contains(&l.login))
                .take(LIVE_SHOWN)
                .collect();
            let chatterino = crate::chatterino_import::locate().is_some();

            // One row: a title, a short line and its action.
            let step = |title: SharedString, detail: SharedString, action: AnyElement| {
                div()
                    .h_flex()
                    .items_center()
                    .gap_3()
                    .py(px(8.))
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(div().text_size(px(13.)).child(title))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::MUTED))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(detail),
                            ),
                    )
                    .child(div().flex_shrink_0().child(action))
            };
            let opener = |id: SharedString, channel: String, label: &'static str| {
                let owner = owner.clone();
                let done = open.contains(&channel);
                Button::new(id)
                    .small()
                    .label(if done { "Opened" } else { label })
                    .when(done, |b| b.icon(Icon::new(IconName::Check)))
                    .disabled(done)
                    .on_click(move |_, w, cx| {
                        let channel = channel.clone();
                        let _ = owner.update(cx, |this, cx| this.open_channel(channel, w, cx));
                    })
                    .into_any_element()
            };
            let heading = |text: &'static str| {
                div()
                    .pt_3()
                    .pb_1()
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::ACCENT))
                    .child(text)
            };

            let mut chat = div().v_flex();
            if let Some(login) = login.clone() {
                chat = chat.child(step(
                    format!("Your channel · #{login}").into(),
                    "Your own chat, with your streamer and moderator tools.".into(),
                    opener("welcome-own".into(), login, "Open"),
                ));
            }
            match &moderated {
                Some(list) if !list.is_empty() => {
                    let owner = owner.clone();
                    chat = chat.child(step(
                        format!("Channels you moderate · {}", list.len()).into(),
                        "Pick the ones to open, live channels first.".into(),
                        Button::new("welcome-moderated")
                            .small()
                            .label("Choose…")
                            .on_click(move |_, w, cx| {
                                let _ = owner.update(cx, |this, cx| this.offer_moderated_channels(w, cx));
                            })
                            .into_any_element(),
                    ));
                }
                None => {
                    chat = chat.child(step(
                        "Channels you moderate".into(),
                        "Checking which channels you moderate…".into(),
                        div().into_any_element(),
                    ));
                }
                _ => {}
            }
            {
                chat = chat.child(step(
                    "Mentions".into(),
                    "One tab collecting @mentions, replies and highlights from every channel.".into(),
                    opener("welcome-mentions".into(), crate::combined::MENTIONS.into(), "Add"),
                ));
            }
            if chatterino {
                let owner = owner.clone();
                chat = chat.child(step(
                    "Import from Chatterino".into(),
                    "Bring in your Chatterino tabs, channels and highlights.".into(),
                    Button::new("welcome-chatterino")
                        .small()
                        .label("Import…")
                        .on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = owner.update(cx, |this, cx| this.open_chatterino_import(w, cx));
                        })
                        .into_any_element(),
                ));
            }
            let live_rows = live.into_iter().enumerate().map(|(index, follow)| {
                let detail = [follow.game.as_str(), follow.title.as_str()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                step(
                    format!(
                        "{} · {} watching",
                        follow.label(),
                        crate::message_actions::group_digits(follow.viewers)
                    )
                    .into(),
                    detail.into(),
                    opener(SharedString::from(format!("welcome-live-{index}")), follow.login.clone(), "Open"),
                )
            });

            let done_owner = owner.clone();
            dialog
                .w(px(width))
                .title(match &login {
                    Some(login) => format!("Welcome to Jawjack, {login}"),
                    None => "Welcome to Jawjack".into(),
                })
                .child(
                    div()
                        .id("welcome-body")
                        .v_flex()
                        .max_h(px(height))
                        .overflow_y_scroll()
                        .child(heading("HOW WILL YOU USE JAWJACK?"))
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).pb_1().child(
                            "Changes what's in reach, not what you're allowed to do. Change it anytime in Settings › General › Layout.",
                        ))
                        .child(div().h_flex().items_stretch().gap_2().children(Experience::ALL.into_iter().map(|experience| {
                            let owner = owner.clone();
                            let selected = experience == current;
                            div()
                                .id(SharedString::from(format!("welcome-setup-{}", experience.key())))
                                .flex_1()
                                .min_w_0()
                                .v_flex()
                                .gap_1()
                                .p_2()
                                .rounded(px(6.))
                                .border_1()
                                .border_color(rgb(if selected { theme::ACCENT } else { theme::BORDER }))
                                .bg(rgb(if selected { 0x272237 } else { theme::CONTROL }))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(theme::HOVER)))
                                .child(
                                    div()
                                        .h_flex()
                                        .gap_1()
                                        .items_center()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .when(selected, |el| {
                                            el.child(Icon::new(IconName::Check).small().text_color(rgb(theme::ACCENT)))
                                        })
                                        .child(experience.label()),
                                )
                                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(experience.description()))
                                .on_click(move |_, _, cx| {
                                    let _ = owner.update(cx, |this, cx| this.set_experience(experience, cx));
                                })
                        })))
                        .child(heading("OPEN SOME CHAT"))
                        .child(chat)
                        .when(live_rows.len() > 0, |el| {
                            el.child(heading("LIVE NOW FROM YOUR FOLLOWS")).children(live_rows)
                        }),
                )
                .footer(
                    div()
                        .h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(11.))
                                .text_color(rgb(theme::MUTED))
                                .child("Add any channel later with + or Ctrl+K."),
                        )
                        .child(
                            Button::new("welcome-done")
                                .small()
                                .primary()
                                .label("Done")
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    let _ = done_owner.update(cx, |_, cx| cx.notify());
                                }),
                        ),
                )
        });
    }
}
