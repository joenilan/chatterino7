//! GPUI Kit's default component icons plus the few extra kit icons Jawjack uses,
//! selected with the kit's `icon_assets!` so only those SVGs are embedded, plus
//! the chat services' own marks (Lucide has no brand icons): Twitch, YouTube and
//! Kick from Simple Icons (CC0), under `assets/brand-icons/`.
use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

gpui_kit::assets::icon_assets!(
    ExtraIcons,
    [
        Plus,
        Reply,
        Pencil,
        Shield,
        RotateCcwClock,
        Sticker,
        Pin,
        UserPlus,
        Gem,
        Rocket,
        Sparkles,
        HandCoins,
        Activity,
        BookmarkPlus,
        Tv,
        ChartBar,
        Trophy,
        Lock,
        MessageCircle,
        SlidersHorizontal,
        MessageSquareText,
        Highlighter,
        BellRing,
        Plug,
        Keyboard,
        Download,
        Palette,
        Users,
        Swords,
        Trash,
        Clock,
        Ban,
        Mic,
        MicOff,
        ListFilter,
        ArrowDownWideNarrow
    ]
);

/// Brand marks by asset path; drawn as single-colour icons like the rest.
const BRANDS: [(&str, &[u8]); 3] = [
    (
        "icons/brand-twitch.svg",
        include_bytes!("../assets/brand-icons/twitch.svg"),
    ),
    (
        "icons/brand-youtube.svg",
        include_bytes!("../assets/brand-icons/youtube.svg"),
    ),
    (
        "icons/brand-kick.svg",
        include_bytes!("../assets/brand-icons/kick.svg"),
    ),
];

/// A service's mark in its brand colour.
pub fn platform_icon(platform: crate::combined::Platform) -> gpui_kit::component::Icon {
    use crate::combined::Platform;
    use gpui_kit::{Styled, rgb};
    let (path, color) = match platform {
        Platform::Twitch => ("icons/brand-twitch.svg", 0xA970FF),
        Platform::YouTube => ("icons/brand-youtube.svg", 0xFF3B4E),
    };
    gpui_kit::component::Icon::empty()
        .path(path)
        .text_color(rgb(color))
}

/// Why YouTube carries a Beta label.
pub const YOUTUBE_BETA: &str = "YouTube support is in beta. Google is still reviewing Jawjack, so all users share a limited daily YouTube allowance (it resets at midnight Pacific).";

/// A small "Beta" pill with its explanation on hover.
pub fn beta_badge(id: &'static str, why: &'static str) -> gpui_kit::AnyElement {
    use gpui_kit::{
        InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div,
        px, rgb,
    };
    div()
        .id(id)
        .flex_shrink_0()
        .px(px(5.))
        .rounded(px(4.))
        .border_1()
        .border_color(rgb(0x6D5BC7))
        .bg(rgb(0x241D33))
        .text_size(px(9.))
        .font_weight(gpui_kit::FontWeight::BOLD)
        .text_color(rgb(crate::theme::ACCENT))
        .child("BETA")
        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(why).build(w, cx))
        .into_any_element()
}

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = BRANDS.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut list = gpui_kit::assets::Assets.list(path)?;
        list.extend(ExtraIcons.list(path)?);
        list.extend(
            BRANDS
                .iter()
                .filter(|(p, _)| p.starts_with(path))
                .map(|(p, _)| SharedString::from(*p)),
        );
        list.sort();
        list.dedup();
        Ok(list)
    }
}
