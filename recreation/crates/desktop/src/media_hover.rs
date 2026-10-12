//! Content-sized hover targets and cached emote previews; no hover-time downloads.
use crate::{media::DecodedMedia, theme};
use gpui_kit::component::StyledExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::sync::Arc;
#[derive(Clone)]
pub enum Content {
    Hint(String),
    /// A link: its preview (title, site, where it redirects, warnings).
    Link(String),
    Emote {
        label: String,
        source: String,
        images: Vec<Arc<DecodedMedia>>,
    },
    Gif {
        label: String,
        source: String,
        images: Vec<Arc<DecodedMedia>>,
    },
    /// A live channel: avatar, name, stream title, and "category · viewers · uptime".
    Stream {
        name: String,
        title: String,
        detail: String,
        avatar: Option<Arc<DecodedMedia>>,
    },
}
pub struct Card(pub Content);
impl Render for Card {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match &self.0 {
            Content::Link(url) => link_card(url, window, _cx),
            Content::Stream {
                name,
                title,
                detail,
                avatar,
            } => div()
                .h_flex()
                .items_start()
                .gap_2()
                .p_2()
                .max_w(px(320.))
                .font_family("Segoe UI")
                .text_color(rgb(theme::TEXT))
                .bg(rgb(theme::ELEVATED))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .rounded(px(6.))
                .when_some(avatar.clone(), |el, avatar| {
                    el.child(crate::emote_picker::avatar(avatar, 40.))
                })
                .child(
                    div()
                        .v_flex()
                        .gap(px(2.))
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name.clone()),
                        )
                        .when(!title.is_empty(), |el| {
                            el.child(div().text_size(px(12.)).child(title.clone()))
                        })
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(0xC5B8E6))
                                .child(detail.clone()),
                        ),
                )
                .into_any_element(),
            Content::Hint(text) => div()
                .p_2()
                .max_w(px(300.))
                .font_family("Segoe UI")
                .text_color(rgb(theme::TEXT))
                .text_size(px(11.))
                .bg(rgb(theme::ELEVATED))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .rounded(px(5.))
                .child(text.clone())
                .into_any_element(),
            Content::Emote {
                label,
                source,
                images,
            }
            | Content::Gif {
                label,
                source,
                images,
            } => {
                let gif = matches!(&self.0, Content::Gif { .. });
                let width = (f32::from(window.viewport_size().width) - 24.)
                    .clamp(80., if gif { 336. } else { 180. });
                let height = (f32::from(window.viewport_size().height) - 100.)
                    .clamp(48., if gif { 224. } else { 112. });
                let layers = images.clone();
                let preview = canvas(
                    |bounds, _, _| bounds,
                    move |bounds, _, window, cx| {
                        for image in &layers {
                            let frame = image.frame(cx.reduce_motion());
                            if image.image.frame_count() > 1 && !cx.reduce_motion() {
                                window.request_animation_frame();
                            }
                            let fitted =
                                ObjectFit::Contain.get_bounds(bounds, image.image.size(frame));
                            let _ = window.paint_image(
                                bounds,
                                fitted,
                                Corners::all(px(0.)),
                                image.image.clone(),
                                frame,
                                false,
                            );
                        }
                    },
                )
                .w(px(width - 16.))
                .h(px(height))
                .flex_shrink_0();
                div()
                    .v_flex()
                    .items_center()
                    .gap_1()
                    .p_2()
                    .w(px(width))
                    .font_family("Segoe UI")
                    .text_color(rgb(theme::TEXT))
                    .bg(rgb(theme::ELEVATED))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .rounded(px(5.))
                    .child(
                        div()
                            .relative()
                            .w(px(width - 16.))
                            .h(px(height))
                            .flex_shrink_0()
                            .child(preview)
                            .when(images.is_empty(), |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(11.))
                                        .text_color(rgb(theme::MUTED))
                                        .child("Preview not loaded"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(label.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme::MUTED))
                            .child(source.clone()),
                    )
                    .into_any_element()
            }
        }
    }
}
pub fn region(
    id: String,
    bounds: Bounds<Pixels>,
    content: Content,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let mut element = div()
        .id(SharedString::from(id))
        .w(bounds.size.width)
        .h(bounds.size.height)
        .tooltip(move |_, cx| cx.new(|_| Card(content.clone())).into())
        .into_any_element();
    element.prepaint_as_root(
        bounds.origin,
        size(
            AvailableSpace::Definite(bounds.size.width),
            AvailableSpace::Definite(bounds.size.height),
        ),
        window,
        cx,
    );
    element
}
/// A hover card for any image: picture, name and where it's from.
pub fn image_card(
    label: String,
    source: String,
    image: Option<Arc<DecodedMedia>>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let content = Content::Emote {
        label,
        source,
        images: image.into_iter().collect(),
    };
    move |_, cx| cx.new(|_| Card(content.clone())).into()
}
pub fn source(provider: &str) -> &'static str {
    match provider {
        "twitch_badge" => "Twitch Badge",
        "ffz_badge" => "FrankerFaceZ Badge",
        "ffz_role_badge" => "FrankerFaceZ Channel Badge",
        "7tv_badge" => "7TV Badge",
        "twitch_avatar" => "Twitch Channel",
        "twitch" => "Twitch Emote",
        "7tv" => "7TV Emote",
        "bttv" => "BetterTTV Emote",
        "ffz" => "FrankerFaceZ Emote",
        "twitch_cheer" => "Twitch Cheermote",
        _ => "Emote",
    }
}
pub struct HoverState {
    pub hitbox: Hitbox,
    pub regions: Vec<AnyElement>,
}

/// The hover card for a link: a preview once it has loaded (Chatterino-style).
fn link_card(url: &str, _window: &mut Window, cx: &mut Context<Card>) -> AnyElement {
    use crate::link_preview::{self, State};
    let state = if link_preview::enabled()
        && !crate::streamer_mode::hides(crate::streamer_mode::LINK_PREVIEWS)
    {
        Some(link_preview::get(url))
    } else {
        None
    };
    if state == Some(State::Loading) {
        // Re-render until the preview arrives (the card closes when the mouse leaves).
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(200))
                .await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    let shown: String = url.chars().take(120).collect();
    let small =
        |text: String, color: u32| div().text_size(px(11.)).text_color(rgb(color)).child(text);
    div()
        .p_2()
        .w(px(340.))
        .v_flex()
        .gap_1()
        .font_family("Segoe UI")
        .text_color(rgb(theme::TEXT))
        .text_size(px(12.))
        .bg(rgb(theme::ELEVATED))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .rounded(px(5.))
        .map(|el| match state {
            Some(State::Ready(p)) => el
                .when(!p.site.is_empty(), |el| {
                    el.child(small(p.site.clone(), theme::MUTED))
                })
                .when(!p.title.is_empty(), |el| {
                    el.child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(p.title.clone()),
                    )
                })
                .when(!p.description.is_empty(), |el| {
                    el.child(small(p.description.clone(), 0xC0C4CE))
                })
                .when_some(p.redirected_to.clone(), |el, to| {
                    el.child(small(
                        format!("Goes to: {}", to.chars().take(120).collect::<String>()),
                        0xE8C68A,
                    ))
                })
                .children(
                    p.warnings
                        .iter()
                        .map(|w| small(format!("Warning: {w}"), 0xF29D9D)),
                ),
            Some(State::Loading) => el.child(small("Loading preview…".into(), theme::MUTED)),
            Some(State::Failed(why)) => el.child(small(format!("No preview: {why}"), theme::MUTED)),
            None => el,
        })
        .child(small(shown, 0x91C7E8))
        .child(small(
            "Ctrl+click to open · Right-click for actions".into(),
            theme::MUTED,
        ))
        .into_any_element()
}
