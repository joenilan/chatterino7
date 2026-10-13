use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

pub const SHELL: u32 = 0x141518;
pub const CANVAS: u32 = 0x0A0B0D;
pub const PANEL: u32 = 0x0F1012;
pub const ELEVATED: u32 = 0x1A1B1E;
pub const CONTROL: u32 = 0x1F2024;
pub const HOVER: u32 = 0x27292D;
pub const BORDER: u32 = 0x222428;
pub const TEXT: u32 = 0xECEEF1;
pub const MUTED: u32 = 0xA4A9B1;
pub const STREAM_LIVE: u32 = 0x91D7BA;
pub const STREAM_OFFLINE: u32 = 0xB4ACC6;
pub const STREAM_UNKNOWN: u32 = 0xE8C68A;
/// Jawjack's lilac accent: primary buttons, switches and selected states.
pub const ACCENT: u32 = 0xA99CF4;

pub fn install(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = "Segoe UI".into();
        theme.font_size = px(14.);
        theme.radius = px(6.);
        theme.radius_lg = px(10.);
        theme.background = rgb(CANVAS).into();
        theme.foreground = rgb(TEXT).into();
        theme.border = rgb(BORDER).into();
        theme.title_bar = rgb(SHELL).into();
        theme.title_bar_border = rgb(BORDER).into();
        theme.muted = rgb(PANEL).into();
        theme.muted_foreground = rgb(MUTED).into();
        theme.popover = rgb(ELEVATED).into();
        theme.popover_foreground = rgb(TEXT).into();
        theme.button = rgb(CONTROL).into();
        theme.button_hover = rgb(HOVER).into();
        theme.button_active = rgb(0x25272B).into();
        theme.button_foreground = rgb(TEXT).into();
        theme.selection = rgb(0x315166).into();
        // Primary actions use the lilac accent with dark text, not the kit's white.
        theme.primary = rgb(ACCENT).into();
        theme.primary_hover = rgb(0xBAAFF8).into();
        theme.primary_active = rgb(0x968AE6).into();
        theme.primary_foreground = rgb(0x16121F).into();
        theme.button_primary = rgb(ACCENT).into();
        theme.button_primary_hover = rgb(0xBAAFF8).into();
        theme.button_primary_active = rgb(0x968AE6).into();
        theme.button_primary_foreground = rgb(0x16121F).into();
        // Buttons read the resolved tokens; rebuild them from the edited colors.
        theme.tokens = (&theme.colors).into();
    });
}

// Resolve once on receipt, not during each animation frame.
pub fn readable_name_color(user_id: &str, color: Option<u32>) -> u32 {
    let palette = [0xA99CF4, 0x78CEBD, 0xF2AF87, 0x86B8EB, 0xE69DCD, 0xD3CF8B];
    let hash = user_id
        .bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let rgb_value = color.unwrap_or(palette[hash as usize % palette.len()]);
    let mut channels = [
        ((rgb_value >> 16) & 255) as f32 / 255.,
        ((rgb_value >> 8) & 255) as f32 / 255.,
        (rgb_value & 255) as f32 / 255.,
    ];
    let luminance = |c: [f32; 3]| {
        let linear = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
    };
    let background = luminance([10. / 255., 11. / 255., 13. / 255.]);
    // Preserve the chosen hue where possible, lifting unreadable dark names.
    for _ in 0..32 {
        if (luminance(channels) + 0.05) / (background + 0.05) >= 4.5 {
            break;
        }
        channels = channels.map(|v| v + (1. - v) * 0.08);
    }

    ((channels[0] * 255.).round() as u32) << 16
        | ((channels[1] * 255.).round() as u32) << 8
        | (channels[2] * 255.).round() as u32
}

/// Settings → Appearance → Alternate row backgrounds.
pub static ALTERNATE_ROWS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub fn alternate_rows() -> bool {
    ALTERNATE_ROWS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Settings → Appearance → Lines between messages.
pub static SEPARATORS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Settings → Appearance → Deleted messages: hide instead of showing them greyed.
pub static HIDE_DELETED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn flag(flag: &std::sync::atomic::AtomicBool) -> bool {
    flag.load(std::sync::atomic::Ordering::Relaxed)
}
/// Settings → Appearance → Chat font (None: the app font).
pub static CHAT_FONT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// Settings › Appearance › UI scale: the whole window, separate from the
/// chat text size. Percentages.
pub const UI_SCALES: [u32; 6] = [75, 90, 100, 110, 125, 150];
pub static UI_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);
pub fn ui_zoom() -> f32 {
    UI_SCALE
        .load(std::sync::atomic::Ordering::Relaxed)
        .clamp(50, 300) as f32
        / 100.
}
/// Settings › Appearance › Tab style: Normal, or Compact (shorter tabs with
/// smaller text, Chatterino's compact tabs).
pub const TAB_STYLES: [&str; 2] = ["Normal", "Compact"];
pub static TAB_STYLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn compact_tabs() -> bool {
    TAB_STYLE.load(std::sync::atomic::Ordering::Relaxed) == 1
}
/// Settings › Appearance › Tab position (Chatterino's): where each split's
/// channel tabs sit. 0 top, 1 bottom, 2 left, 3 right.
pub const TAB_POSITIONS: [&str; 4] = ["Top", "Bottom", "Left", "Right"];
pub static TAB_POSITION: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn tab_position() -> usize {
    TAB_POSITION
        .load(std::sync::atomic::Ordering::Relaxed)
        .min(3)
}
pub fn chat_font() -> Option<String> {
    CHAT_FONT.lock().ok().and_then(|f| f.clone())
}

/// Settings → Appearance → Badges: which kinds show (Chatterino's categories).
pub const BADGE_KINDS: [&str; 8] = [
    "Staff & admin",
    "Broadcaster, mod & VIP",
    "Predictions",
    "Subscriber",
    "Other Twitch",
    "FFZ",
    "7TV",
    "Chatterino",
];
pub static BADGES: [std::sync::atomic::AtomicBool; 8] = [
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(true),
];
pub const FFZ_BADGES: usize = 5;
pub const SEVEN_TV_BADGES: usize = 6;
pub const CHATTERINO_BADGES: usize = 7;
/// The kind of a Twitch badge set (index into BADGE_KINDS).
pub fn badge_kind(set_id: &str) -> usize {
    match set_id {
        "staff" | "admin" | "global_mod" => 0,
        "broadcaster" | "lead_moderator" | "moderator" | "vip" => 1,
        "predictions" => 2,
        "subscriber" | "founder" => 3,
        _ => 4,
    }
}
pub fn badge_shown(kind: usize) -> bool {
    BADGES
        .get(kind)
        .is_none_or(|b| b.load(std::sync::atomic::Ordering::Relaxed))
}

/// Jawjack's segmented control (Settings choices, Studio tabs): the selected
/// segment is filled with the lilac accent, like other selected states.
#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    labels: Vec<SharedString>,
    selected: usize,
    full: bool,
    on_click: Option<std::rc::Rc<dyn Fn(&usize, &mut Window, &mut App)>>,
}
/// One segment's label (reads like the kit's `Tab::new().label(..)`).
pub fn segment(label: impl Into<SharedString>) -> SharedString {
    label.into()
}
impl Segmented {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            labels: Vec::new(),
            selected: 0,
            full: false,
            on_click: None,
        }
    }
    /// Stretch across the available width, segments sharing it equally.
    pub fn full_width(mut self) -> Self {
        self.full = true;
        self
    }
    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }
    pub fn on_click(mut self, f: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(std::rc::Rc::new(f));
        self
    }
    pub fn child(mut self, label: impl Into<SharedString>) -> Self {
        self.labels.push(label.into());
        self
    }
    pub fn children(mut self, labels: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.labels.extend(labels.into_iter().map(Into::into));
        self
    }
}
impl RenderOnce for Segmented {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        use gpui_kit::prelude::FluentBuilder;
        let selected = self.selected;
        let on_click = self.on_click;
        let full = self.full;
        div()
            .id(self.id)
            .when(full, |el| el.w_full())
            .flex()
            .flex_row()
            .items_center()
            .flex_shrink_0()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(7.))
            .bg(rgb(CONTROL))
            .children(
                self.labels
                    .into_iter()
                    .enumerate()
                    .map(move |(index, label)| {
                        let chosen = index == selected;
                        let on_click = on_click.clone();
                        div()
                            .id(("segment", index))
                            .px(px(10.))
                            .py(px(3.))
                            .rounded(px(5.))
                            .when(full, |el| el.flex_1().flex().justify_center())
                            .text_size(px(12.))
                            .cursor_pointer()
                            .when(chosen, |el| {
                                el.bg(rgb(ACCENT))
                                    .text_color(rgb(0x16121F))
                                    .font_weight(FontWeight::SEMIBOLD)
                            })
                            .when(!chosen, |el| {
                                el.text_color(rgb(MUTED))
                                    .hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT)))
                            })
                            .child(label)
                            .when_some(on_click, |el, f| {
                                el.on_click(move |_, window, cx| f(&index, window, cx))
                            })
                    }),
            )
    }
}

/// Settings → Appearance → Channel header: what shows beside the channel name
/// while it's live (Chatterino's header options).
pub const HEADER_PARTS: [&str; 4] = ["Viewers", "Uptime", "Category", "Title"];
pub static HEADER: [std::sync::atomic::AtomicBool; 4] = [
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(false),
    std::sync::atomic::AtomicBool::new(false),
];
pub fn header_shows(part: usize) -> bool {
    HEADER[part].load(std::sync::atomic::Ordering::Relaxed)
}
