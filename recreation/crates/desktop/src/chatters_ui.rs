//! Chatter list (Chatterino's viewer list): Twitch's full list where you
//! moderate, plus everyone seen talking, grouped by role and searchable.
use crate::{ChannelPane, theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Sizable, StyledExt, WindowExt,
        button::{Button, ButtonVariants},
        input::{Input, InputState},
    },
    *,
};
use std::collections::BTreeMap;

/// Rows drawn at once; search narrows the rest.
const SHOWN: usize = 400;
const GROUPS: [&str; 4] = ["Broadcaster", "Moderators", "VIPs", "Chatters"];

#[derive(Clone)]
struct Entry {
    login: String,
    name: String,
    user_id: String,
    group: usize,
}

impl ChannelPane {
    /// Everyone known in this channel, by login.
    fn known_chatters(&self) -> BTreeMap<String, Entry> {
        let mut known: BTreeMap<String, Entry> = BTreeMap::new();
        for login in self.chatter_list.iter() {
            known.insert(
                login.clone(),
                Entry {
                    login: login.clone(),
                    name: login.clone(),
                    user_id: String::new(),
                    group: 3,
                },
            );
        }
        for message in self.timeline.borrow().messages() {
            let Some(login) = message.login.clone().filter(|l| !l.is_empty()) else {
                continue;
            };
            let badge = |set: &str| message.badges.iter().any(|b| b.set_id == set);
            let group = if badge("broadcaster") {
                0
            } else if badge("moderator") || badge("lead_moderator") {
                1
            } else if badge("vip") {
                2
            } else {
                3
            };
            let entry = known.entry(login.clone()).or_insert(Entry {
                login,
                name: String::new(),
                user_id: String::new(),
                group,
            });
            // Newest message wins: roles change (a VIP made mod, a mod removed).
            entry.group = group;
            entry.name = message.display_name.clone();
            entry.user_id = message.user_id.clone();
        }
        known
    }
    pub fn open_chatter_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search chatters"));
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(260., 380.);
        let height = (f32::from(window.viewport_size().height) - 200.).clamp(200., 520.);
        let focus = search.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(pane) = owner.upgrade() else {
                return dialog.title("Channel closed");
            };
            let (channel, full, known) = {
                let p = pane.read(cx);
                (
                    p.name.to_string(),
                    !p.chatter_list.is_empty(),
                    p.known_chatters(),
                )
            };
            let query = search
                .read(cx)
                .value()
                .trim()
                .trim_start_matches('@')
                .to_lowercase();
            let mut entries: Vec<Entry> = known
                .into_values()
                .filter(|e| {
                    query.is_empty()
                        || e.login.contains(&query)
                        || e.name.to_lowercase().contains(&query)
                })
                .collect();
            let total = entries.len();
            entries.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.login.cmp(&b.login)));
            entries.truncate(SHOWN);
            let summary = if full {
                format!("{total} in chat")
            } else {
                format!("{total} seen talking · Twitch shares the full list only with moderators")
            };
            let mut list = div().v_flex();
            let mut group = usize::MAX;
            for (index, entry) in entries.into_iter().enumerate() {
                if entry.group != group {
                    group = entry.group;
                    list = list.child(
                        div()
                            .pt_2()
                            .pb_1()
                            .text_size(px(10.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::muted()))
                            .child(GROUPS[group].to_uppercase()),
                    );
                }
                let (inspect, mention) = (owner.clone(), owner.clone());
                let (inspect_entry, mention_login) = (entry.clone(), entry.login.clone());
                let label =
                    if entry.name.is_empty() || entry.name.eq_ignore_ascii_case(&entry.login) {
                        if entry.name.is_empty() {
                            entry.login.clone()
                        } else {
                            entry.name.clone()
                        }
                    } else {
                        format!("{} ({})", entry.name, entry.login)
                    };
                list = list.child(
                    div()
                        .id(("chatter-row", index))
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .h(px(26.))
                        .rounded(px(5.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::hover())))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(13.))
                                .child(label),
                        )
                        .child(
                            Button::new(("chatter-mention", index))
                                .xsmall()
                                .ghost()
                                .label("@")
                                .tooltip("Mention in composer")
                                .on_click(move |_, window, cx| {
                                    cx.stop_propagation();
                                    window.close_dialog(cx);
                                    let _ = mention.update(cx, |p, cx| {
                                        p.insert_mention(&mention_login, window, cx)
                                    });
                                }),
                        )
                        .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            let e = inspect_entry.clone();
                            let _ = inspect.update(cx, |p, cx| {
                                if e.user_id.is_empty() {
                                    // Only on Twitch's list so far: no ID to inspect yet.
                                    p.insert_mention(&e.login, window, cx);
                                } else {
                                    p.inspect_chatter(e.user_id, e.name, Some(e.login), window, cx);
                                }
                            });
                        }),
                );
            }
            dialog
                .w(px(width))
                .title(format!("Chatters · #{channel}"))
                .on_ok(|_, _, _| false)
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(Input::new(&search).small())
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(theme::muted()))
                                .child(summary),
                        )
                        .child(
                            div()
                                .id("chatter-list")
                                .h(px(height))
                                .overflow_y_scroll()
                                .child(list)
                                .when(total > SHOWN, |el| {
                                    el.child(
                                        div()
                                            .py_2()
                                            .text_size(px(11.))
                                            .text_color(rgb(theme::muted()))
                                            .child(format!(
                                                "{} more · search to narrow",
                                                total - SHOWN
                                            )),
                                    )
                                }),
                        ),
                )
        });
        focus.update(cx, |input, cx| input.focus(window, cx));
    }
}
