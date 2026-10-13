use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

/// Settings › Appearance › Theme: Dark, Light, or follow Windows.
pub const THEMES: [&str; 3] = ["Dark", "Light", "System"];
pub static THEME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// The resolved mode (System follows Windows' app mode).
static LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn is_light() -> bool {
    LIGHT.load(std::sync::atomic::Ordering::Relaxed)
}
/// Dark value, light value.
const fn pick(dark: u32, light: u32) -> [u32; 2] {
    [dark, light]
}
macro_rules! colors {
    ($($(#[$doc:meta])* $name:ident = $value:expr;)*) => {$(
        $(#[$doc])*
        pub fn $name() -> u32 {
            $value[is_light() as usize]
        }
    )*};
}
colors! {
    shell = pick(0x141518, 0xECEEF2);
    canvas = pick(0x0A0B0D, 0xFFFFFF);
    panel = pick(0x0F1012, 0xF6F7F9);
    elevated = pick(0x1A1B1E, 0xFFFFFF);
    control = pick(0x1F2024, 0xEAEBEF);
    hover = pick(0x27292D, 0xE0E2E7);
    border = pick(0x222428, 0xD9DBE1);
    text = pick(0xECEEF1, 0x17181C);
    muted = pick(0xA4A9B1, 0x5B606A);
    stream_live = pick(0x91D7BA, 0x15805A);
    stream_offline = pick(0xB4ACC6, 0x6C6488);
    stream_unknown = pick(0xE8C68A, 0x8F6212);
    /// Jawjack's lilac accent: primary buttons, switches and selected states.
    accent = pick(0xA99CF4, 0x6A55D8);
}
/// A fixed colour written for the dark theme, in its light-theme form: pale
/// text gets deeper, dark tints get light. Unknown colours stay as they are.
pub fn tone(dark: u32) -> u32 {
    if !is_light() {
        return dark;
    }
    match dark {
        // Text and accents.
        0xA99CF4 | 0xB9A1FF => 0x6A55D8,
        0xC5B8E6 => 0x5E4FA8,
        0xB6A9E6 => 0x5B4BB0,
        0xC9BAF7 => 0x5A47C0,
        0xD9D2FA => 0x4A3AA8,
        0xBAAFF8 => 0x7A67E0,
        0x968AE6 => 0x5944C2,
        0xE9C99C => 0x8A5A12,
        0xE8C468 | 0xE8C68A => 0x8F6212,
        0xF29D9D => 0xB3261E,
        0xF2B8B8 => 0x9B1C1C,
        0xF07A7A => 0xD14343,
        0x91C7E8 => 0x1F6FB0,
        0xB3DCC7 | 0x91D7BA => 0x15805A,
        0x9DBFCE => 0x3C7088,
        // Dark text on the accent.
        0x16121F => 0xFFFFFF,
        // Tinted backgrounds.
        0x241D33 => 0xEEE9FB,
        0x272237 => 0xECE7FA,
        0x2E2640 => 0xE4DCF8,
        0x3A3350 => 0xDCD3F3,
        0x2A2340 => 0xE6E0F8,
        0x2A1F3D => 0xEFE9FC,
        0x282333 => 0xF1EEF9,
        0x272331 => 0xFFF1BF,
        0x1E1A10 => 0xFFF6DF,
        0x3A2328 => 0xFBE3E3,
        0x3A1D20 => 0xFDECEC,
        0x192D27 => 0xE5F5EE,
        0x25272B => 0xD5D8DE,
        0x315166 => 0xC6DAF2,
        // Borders.
        0x6D5BC7 => 0xB3A6EE,
        0x5A4A1E => 0xE3C77E,
        0x4A3D73 => 0xC4B8EE,
        0x2B3038 => 0xD3D6DC,
        0x40535E => 0xB9C6CE,
        other => other,
    }
}
/// `tone` for an RRGGBBAA colour (a white wash becomes a dark one).
pub fn tone_a(dark: u32) -> u32 {
    let (color, alpha) = (dark >> 8, dark & 0xFF);
    let color = if is_light() && color == 0xFFFFFF {
        0
    } else {
        tone(color)
    };
    (color << 8) | alpha
}
/// Resolve Settings › Theme, restyle the UI kit and redraw every window.
/// Returns true when the mode changed.
pub fn apply(cx: &mut App) -> bool {
    let light = match THEME.load(std::sync::atomic::Ordering::Relaxed) {
        0 => false,
        1 => true,
        _ => matches!(
            cx.window_appearance(),
            WindowAppearance::Light | WindowAppearance::VibrantLight
        ),
    };
    let changed = LIGHT.swap(light, std::sync::atomic::Ordering::Relaxed) != light;
    install(cx);
    cx.refresh_windows();
    changed
}

pub fn install(cx: &mut App) {
    Theme::change(
        if is_light() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        },
        None,
        cx,
    );
    let light = is_light();
    Theme::update(cx, |theme| {
        theme.font_family = "Segoe UI".into();
        theme.font_size = px(14.);
        theme.radius = px(6.);
        theme.radius_lg = px(10.);
        theme.background = rgb(canvas()).into();
        theme.foreground = rgb(text()).into();
        theme.border = rgb(border()).into();
        theme.title_bar = rgb(shell()).into();
        theme.title_bar_border = rgb(border()).into();
        theme.muted = rgb(panel()).into();
        theme.muted_foreground = rgb(muted()).into();
        theme.popover = rgb(elevated()).into();
        theme.popover_foreground = rgb(text()).into();
        theme.button = rgb(control()).into();
        theme.button_hover = rgb(hover()).into();
        theme.button_active = rgb(if light { 0xD5D8DE } else { 0x25272B }).into();
        theme.button_foreground = rgb(text()).into();
        theme.selection = rgb(if light { 0xC6DAF2 } else { 0x315166 }).into();
        // Primary actions use the lilac accent with dark text, not the kit's white.
        theme.primary = rgb(accent()).into();
        theme.primary_hover = rgb(if light { 0x7A67E0 } else { 0xBAAFF8 }).into();
        theme.primary_active = rgb(if light { 0x5944C2 } else { 0x968AE6 }).into();
        theme.primary_foreground = rgb(if light { 0xFFFFFF } else { 0x16121F }).into();
        theme.button_primary = rgb(accent()).into();
        theme.button_primary_hover = rgb(if light { 0x7A67E0 } else { 0xBAAFF8 }).into();
        theme.button_primary_active = rgb(if light { 0x5944C2 } else { 0x968AE6 }).into();
        theme.button_primary_foreground = rgb(if light { 0xFFFFFF } else { 0x16121F }).into();
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
    // Preserve the chosen hue where possible, lifting unreadable dark names
    // (or darkening unreadable light ones on the light theme).
    let light = is_light();
    let background = if light {
        1.
    } else {
        luminance([10. / 255., 11. / 255., 13. / 255.])
    };
    for _ in 0..32 {
        let name = luminance(channels);
        let contrast = if light {
            (background + 0.05) / (name + 0.05)
        } else {
            (name + 0.05) / (background + 0.05)
        };
        if contrast >= 4.5 {
            break;
        }
        channels = if light {
            channels.map(|v| v * 0.9)
        } else {
            channels.map(|v| v + (1. - v) * 0.08)
        };
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
            .bg(rgb(control()))
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
                                el.bg(rgb(accent()))
                                    .text_color(rgb(0x16121F))
                                    .font_weight(FontWeight::SEMIBOLD)
                            })
                            .when(!chosen, |el| {
                                el.text_color(rgb(muted()))
                                    .hover(|s| s.bg(rgb(hover())).text_color(rgb(text())))
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
