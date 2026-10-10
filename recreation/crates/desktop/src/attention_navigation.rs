//! Navigate retained references without treating navigation as acknowledgment.
use crate::ChannelPane;
use gpui_kit::component::{WindowExt, notification::Notification};
use gpui_kit::*;
struct NavigationFeedback;
impl ChannelPane {
    pub fn navigate_attention(
        &mut self,
        direction: i8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = {
            let timeline = self.timeline.borrow();
            let first = (self.next_id - timeline.messages().len()) as u64;
            let retained = |row: u64| {
                row.checked_sub(first)
                    .and_then(|i| timeline.messages().get(i as usize))
                    .filter(|m| !m.deleted)
            };
            if direction == 0 {
                self.attention
                    .unread
                    .iter()
                    .copied()
                    .find(|row| retained(*row).is_some())
                    .or_else(|| {
                        self.attention.read_boundary.and_then(|boundary| {
                            (boundary.max(first)..self.next_id as u64)
                                .find(|row| retained(*row).is_some())
                        })
                    })
                    .and_then(|row| retained(row).map(|m| (m.id.clone(), None)))
            } else {
                let rows = self
                    .attention
                    .highlights
                    .iter()
                    .copied()
                    .filter(|row| retained(*row).is_some())
                    .collect::<Vec<_>>();
                let chosen = if direction > 0 {
                    self.attention_cursor
                        .and_then(|at| rows.iter().copied().find(|row| *row > at))
                        .or_else(|| rows.first().copied())
                } else {
                    self.attention_cursor
                        .and_then(|at| rows.iter().rev().copied().find(|row| *row < at))
                        .or_else(|| rows.last().copied())
                };
                chosen.and_then(|row| {
                    retained(row).map(|m| {
                        (
                            m.id.clone(),
                            rows.iter()
                                .position(|r| *r == row)
                                .map(|at| format!("Highlight {} of {}", at + 1, rows.len())),
                        )
                    })
                })
            }
        };
        if let Some((id, label)) = result {
            self.scroller.update(cx, |s, cx| s.hold_for_navigation(cx));
            self.jump_message(&id, window, cx);
            if let Some(label) = label {
                window.push_notification(Notification::info(label).id::<NavigationFeedback>(), cx);
            }
        } else {
            window.push_notification(
                Notification::info(if direction == 0 {
                    "No unread messages or last-read position remain in this channel"
                } else {
                    "No highlighted messages remain in this channel"
                })
                .id::<NavigationFeedback>(),
                cx,
            );
        }
    }
}
