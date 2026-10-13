//! Live stats card at the top of Studio while you're live: viewers, uptime,
//! chat pace with a 30-minute sparkline, this stream's activity, OBS health.
use super::*;

/// Messages per minute for the last `minutes` minutes (oldest first).
pub(super) fn chat_pace(
    messages: &std::collections::VecDeque<chat_core::Message>,
    now: i64,
    minutes: usize,
) -> Vec<usize> {
    let mut buckets = vec![0usize; minutes];
    for m in messages.iter().rev() {
        let Some(t) = m.sent_at else { continue };
        let age = now - t;
        if age < 0 {
            continue;
        }
        let bucket = (age / 60) as usize;
        if bucket >= minutes {
            break;
        }
        if m.presentation.notice.is_none() && !m.presentation.historical {
            buckets[minutes - 1 - bucket] += 1;
        }
    }
    buckets
}

impl Workbench {
    pub(super) fn render_live_stats(&self, cx: &App) -> Option<Div> {
        let own = self.account.read(cx).login()?;
        let info = self
            .streams
            .detail(&own)
            .map(|(i, _)| i.clone())
            .filter(|i| i.live);
        if info.is_none() && !self.obs.state.streaming {
            return None;
        }
        let pace = self
            .tab_panes()
            .into_iter()
            .find(|p| p.read(cx).name.as_ref() == own)
            .map(|p| {
                chat_pace(
                    p.read(cx).timeline.borrow().messages(),
                    chrono::Utc::now().timestamp(),
                    30,
                )
            })
            .unwrap_or_else(|| vec![0; 30]);
        let now_pace = pace.last().copied().unwrap_or(0);
        let peak_pace = pace.iter().copied().max().unwrap_or(0).max(1);
        let since = self.went_live_at.map(|t| {
            chrono::Local::now() - chrono::Duration::from_std(t.elapsed()).unwrap_or_default()
        });
        let count = |kind: crate::activity_feed::Kind| {
            self.feed
                .items
                .iter()
                .filter(|i| i.kind == kind && since.is_some_and(|s| i.at >= s))
                .count()
        };
        let tile = |label: &'static str, value: String, sub: Option<String>| {
            div()
                .v_flex()
                .flex_1()
                .min_w(px(78.))
                .px_2()
                .py_1()
                .rounded(px(6.))
                .bg(rgb(theme::control()))
                .child(
                    div()
                        .text_size(px(9.))
                        .text_color(rgb(theme::muted()))
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(value),
                )
                .when_some(sub, |el, s| {
                    el.child(
                        div()
                            .text_size(px(9.))
                            .text_color(rgb(theme::muted()))
                            .child(s),
                    )
                })
        };
        let uptime = info
            .as_ref()
            .and_then(|i| i.uptime())
            .or_else(|| {
                self.obs
                    .uptime()
                    .map(|d| format!("{}h {}m", d.as_secs() / 3600, d.as_secs() / 60 % 60))
            })
            .unwrap_or_else(|| "—".into());
        let viewers = info.as_ref().map(|i| i.viewers);
        // Sparkline: one bar per minute, height relative to the busiest minute.
        let spark = div()
            .h_flex()
            .items_end()
            .gap(px(1.))
            .h(px(22.))
            .children(pace.iter().map(|n| {
                div()
                    .w(px(3.))
                    .h(px(2. + 20. * *n as f32 / peak_pace as f32))
                    .rounded(px(1.))
                    .bg(rgb(if *n == 0 {
                        theme::border()
                    } else {
                        theme::accent()
                    }))
            }));
        Some(
            div()
                .v_flex()
                .gap_2()
                .mx_3()
                .mb_2()
                .p_2()
                .rounded(px(8.))
                .bg(rgb(theme::elevated()))
                .border_1()
                .border_color(rgb(theme::border()))
                .child(
                    div()
                        .h_flex()
                        .flex_wrap()
                        .gap_1()
                        .child(tile(
                            "VIEWERS",
                            viewers
                                .map(crate::follows_live::viewers)
                                .unwrap_or_else(|| "—".into()),
                            (self.peak_viewers > 0).then(|| {
                                format!("peak {}", crate::follows_live::viewers(self.peak_viewers))
                            }),
                        ))
                        .child(tile("UPTIME", uptime, None))
                        .child(tile(
                            "CHAT / MIN",
                            now_pace.to_string(),
                            Some(format!("peak {peak_pace}")),
                        )),
                )
                .child(
                    div().h_flex().items_end().gap_2().child(spark).child(
                        div()
                            .text_size(px(9.))
                            .text_color(rgb(theme::muted()))
                            .child("last 30 min"),
                    ),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme::muted()))
                        .child(format!(
                            "This stream: {} follows · {} subs · {} Bits events · {} raids",
                            count(crate::activity_feed::Kind::Follow),
                            count(crate::activity_feed::Kind::Subscription),
                            count(crate::activity_feed::Kind::Bits),
                            count(crate::activity_feed::Kind::Raid),
                        )),
                )
                .when_some(
                    self.obs
                        .state
                        .health
                        .clone()
                        .filter(|_| self.obs.state.streaming),
                    |el, h| {
                        el.child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(if h.dropped >= 1. {
                                    0xE8C468
                                } else {
                                    theme::muted()
                                }))
                                .child(format!(
                                    "OBS: {} kbps · {:.1}% dropped · {:.0} fps",
                                    h.kbps, h.dropped, h.fps
                                )),
                        )
                    },
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pace_buckets() {
        let mut messages = std::collections::VecDeque::new();
        for (i, t) in [100i64, 130, 150, 170, 175].into_iter().enumerate() {
            messages.push_back(chat_core::Message {
                sent_at: Some(t),
                id: i.to_string(),
                channel_id: "c".into(),
                user_id: "u".into(),
                display_name: "A".into(),
                login: Some("a".into()),
                reply: None,
                mentions: vec![],
                replyable: true,
                name_color: None,
                badges: vec![],
                fragments: vec![],
                presentation: Default::default(),
                deleted: false,
            });
        }
        // now = 180: ages 80,50,30,10,5 -> minute buckets 1,0,0,0,0
        assert_eq!(super::chat_pace(&messages, 180, 3), vec![0, 1, 4]);
    }
}
