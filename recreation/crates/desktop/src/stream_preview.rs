//! Hover card for a channel header, like Chatterino's split header tooltip:
//! live thumbnail, title, category, viewers and uptime. Middle-click on the
//! header opens the stream in the browser.
use crate::{
    media::{EmoteKey, MediaCache},
    stream_status::StreamInfo,
    theme,
};
use gpui_kit::component::StyledExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::{cell::RefCell, rc::Rc, time::Duration};

pub struct StreamPreview {
    channel: String,
    info: Option<StreamInfo>,
    media: Rc<RefCell<MediaCache>>,
    key: Option<EmoteKey>,
}
impl StreamPreview {
    pub fn new(
        channel: String,
        info: Option<StreamInfo>,
        media: Rc<RefCell<MediaCache>>,
        // Streamer mode: no thumbnail (it could put another stream on yours).
        hide_image: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let key = info
            .as_ref()
            .filter(|i| i.live && !hide_image)
            .and_then(|_| EmoteKey::stream_preview(&channel));
        // The thumbnail may still be downloading; repaint until it arrives.
        if key.is_some() {
            cx.spawn(async move |view, cx| {
                for _ in 0..40 {
                    cx.background_executor()
                        .timer(Duration::from_millis(250))
                        .await;
                    if view.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        Self {
            channel,
            info,
            media,
            key,
        }
    }
}
impl Render for StreamPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let live = self.info.as_ref().is_some_and(|i| i.live);
        let image = self
            .key
            .as_ref()
            .and_then(|key| self.media.borrow_mut().get(key, cx));
        div()
            .v_flex()
            .gap_1()
            .p_2()
            .w(px(336.))
            .bg(rgb(theme::ELEVATED))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .rounded(px(8.))
            .text_size(px(12.))
            .text_color(rgb(theme::TEXT))
            .when(live, |el| {
                el.child(crate::emote_picker::icon(image, String::new(), 320., 180.))
            })
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(13.))
                    .child(format!("#{}", self.channel)),
            )
            .child(match &self.info {
                Some(info) if info.live => div()
                    .v_flex()
                    .gap_1()
                    .child(div().child(info.title.clone()))
                    .child(
                        div()
                            .text_color(rgb(0xC5B8E6))
                            .child(format!("{} · {} viewers", info.category, info.viewers)),
                    )
                    .when_some(info.uptime(), |el, uptime| {
                        el.child(
                            div()
                                .text_color(rgb(theme::STREAM_LIVE))
                                .child(format!("Live for {uptime}")),
                        )
                    }),
                Some(_) => div().text_color(rgb(theme::MUTED)).child("Offline"),
                None => div()
                    .text_color(rgb(theme::MUTED))
                    .child("Stream status loading…"),
            })
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(theme::MUTED))
                    .child("Middle-click to open in browser · Ctrl+I for details"),
            )
    }
}
