//! Stream recap: what happened during your last stream, shown when it ends.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(super) struct Recap {
    pub duration: Duration,
    pub peak_viewers: u64,
    pub title: String,
    pub category: String,
    /// Activity-feed counts by kind label ("Follow", "Sub", …).
    pub events: BTreeMap<&'static str, usize>,
    pub messages: usize,
    pub chatters: usize,
    pub top: Vec<(String, usize)>,
}
impl Recap {
    pub fn text(&self) -> String {
        let s = self.duration.as_secs();
        let mut lines = vec![
            format!("Stream recap · {}h {:02}m", s / 3600, s / 60 % 60),
            format!("{} · {}", self.title, self.category),
            format!("Peak viewers: {}", self.peak_viewers),
            format!(
                "Chat: {} messages from {} chatters",
                self.messages, self.chatters
            ),
        ];
        if !self.events.is_empty() {
            lines.push(
                self.events
                    .iter()
                    .map(|(k, n)| format!("{k}s: {n}"))
                    .collect::<Vec<_>>()
                    .join(" · "),
            );
        }
        if !self.top.is_empty() {
            lines.push(format!(
                "Top chatters: {}",
                self.top
                    .iter()
                    .map(|(n, c)| format!("{n} ({c})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        lines.join("\n")
    }
}

impl Workbench {
    /// Track peak viewers while live (called every frame; cheap).
    pub(super) fn track_stream(&mut self, cx: &App) {
        let Some(own) = self.account.read(cx).login() else {
            return;
        };
        if let Some((info, _)) = self.streams.detail(&own)
            && info.live
        {
            self.peak_viewers = self.peak_viewers.max(info.viewers);
            self.last_stream_info = Some((info.title.clone(), info.category.clone()));
        }
    }
    /// Build and show the recap for the stream that just ended.
    pub(super) fn show_recap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(started) = self.went_live_at else {
            return;
        };
        let duration = started.elapsed();
        // Very short "streams" are usually a test or a dropped connection.
        if duration < Duration::from_secs(300) {
            return;
        }
        let since = chrono::Local::now() - chrono::Duration::from_std(duration).unwrap_or_default();
        let mut events: BTreeMap<&'static str, usize> = BTreeMap::new();
        for item in self.feed.items.iter().filter(|i| i.at >= since) {
            if item.kind != crate::activity_feed::Kind::Moderation {
                *events.entry(item.kind.label()).or_default() += 1;
            }
        }
        let own = self.account.read(cx).login().unwrap_or_default();
        let since_ts = since.timestamp();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut messages = 0;
        if let Some(pane) = self
            .tab_panes()
            .into_iter()
            .find(|p| p.read(cx).name.as_ref() == own)
        {
            for m in pane.read(cx).timeline.borrow().messages() {
                if m.sent_at.is_some_and(|t| t >= since_ts)
                    && m.presentation.notice.is_none()
                    && !m.display_name.is_empty()
                {
                    messages += 1;
                    if m.login.as_deref() != Some(own.as_str()) {
                        *counts.entry(m.display_name.clone()).or_default() += 1;
                    }
                }
            }
        }
        let chatters = counts.len();
        let mut top: Vec<(String, usize)> = counts.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        top.truncate(5);
        let (title, category) = self.last_stream_info.clone().unwrap_or_default();
        let recap = Recap {
            duration,
            peak_viewers: self.peak_viewers,
            title,
            category,
            events,
            messages,
            chatters,
            top,
        };
        self.peak_viewers = 0;
        let text = recap.text();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(260., 420.);
        window.open_dialog(cx, move |dialog, _, _| {
            let stat = |label: &str, value: String| {
                div()
                    .v_flex()
                    .p_2()
                    .rounded(px(6.))
                    .bg(rgb(theme::control()))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme::muted()))
                            .child(label.to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(value),
                    )
            };
            let s = recap.duration.as_secs();
            let copy = text.clone();
            dialog
                .w(px(width))
                .title("Stream recap")
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(theme::muted()))
                                .child(format!("{} · {}", recap.title, recap.category)),
                        )
                        .child(
                            div()
                                .h_flex()
                                .flex_wrap()
                                .gap_2()
                                .child(stat("Length", format!("{}h {:02}m", s / 3600, s / 60 % 60)))
                                .child(stat("Peak viewers", recap.peak_viewers.to_string()))
                                .child(stat("Messages", recap.messages.to_string()))
                                .child(stat("Chatters", recap.chatters.to_string()))
                                .children(
                                    recap
                                        .events
                                        .iter()
                                        .map(|(k, n)| stat(&format!("{k}s"), n.to_string())),
                                ),
                        )
                        .when(!recap.top.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::muted()))
                                    .child(format!(
                                        "Top chatters: {}",
                                        recap
                                            .top
                                            .iter()
                                            .map(|(n, c)| format!("{n} ({c})"))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    )),
                            )
                        }),
                )
                .footer(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("recap-copy")
                                .small()
                                .label("Copy as text")
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                                }),
                        )
                        .child(
                            Button::new("recap-close")
                                .small()
                                .primary()
                                .label("Done")
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        ),
                )
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn recap_text() {
        let mut r = super::Recap {
            duration: std::time::Duration::from_secs(3 * 3600 + 5 * 60),
            peak_viewers: 42,
            ..Default::default()
        };
        r.events.insert("Follow", 7);
        r.top = vec![("Bob".into(), 30)];
        let t = r.text();
        assert!(t.starts_with("Stream recap · 3h 05m"));
        assert!(t.contains("Follows: 7") && t.contains("Bob (30)"));
    }
}
