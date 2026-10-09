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
