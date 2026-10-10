//! Held pause changes scrolling only. Receipt, moderation updates and retention continue.
use crate::ChannelPane;
use gpui_kit::*;
impl ChannelPane {
    pub fn update_hover_pause(&mut self, window: &Window, cx: &mut Context<Self>) {
        let hold = self.transcript_available
            && self.transcript_hover
            && window.is_window_active()
            && match self.pause_mode {
                1 => true,
                2 => window.modifiers().alt,
                _ => false,
            };
        let selected = {
            let s = self.selection.borrow();
            s.dragging || s.anchor != s.head
        };
        self.scroller.update(cx, |s, cx| {
            if selected {
                s.cancel_auto_resume();
            }
            s.set_hover_pause(hold, cx);
        });
    }
}
