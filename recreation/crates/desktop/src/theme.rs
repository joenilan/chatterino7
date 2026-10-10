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
    });
}

// Resolve once on receipt, not during each animation frame.
pub fn readable_name_color(user_id: &str, color: Option<u32>) -> u32 {
        let palette = [0xA99CF4, 0x78CEBD, 0xF2AF87, 0x86B8EB, 0xE69DCD, 0xD3CF8B];
        let hash = user_id.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
        let rgb_value = color.unwrap_or(palette[hash as usize % palette.len()]);
        let mut channels = [((rgb_value >> 16) & 255) as f32 / 255., ((rgb_value >> 8) & 255) as f32 / 255., (rgb_value & 255) as f32 / 255.];
        let luminance = |c: [f32; 3]| {
            let linear = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
            0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
        };
        let background = luminance([10. / 255., 11. / 255., 13. / 255.]);
        // Preserve the chosen hue where possible, lifting unreadable dark names.
        for _ in 0..32 {
            if (luminance(channels) + 0.05) / (background + 0.05) >= 4.5 { break; }
            channels = channels.map(|v| v + (1. - v) * 0.08);
        }
        let visible = ((channels[0] * 255.).round() as u32) << 16
            | ((channels[1] * 255.).round() as u32) << 8
            | (channels[2] * 255.).round() as u32;
    visible
}
