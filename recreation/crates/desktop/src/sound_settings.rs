//! Ping sounds: when they play, and the Sounds rows of Settings → Notifications.
use super::settings_ui::{heading, row};
use super::*;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};

const VOLUMES: [f32; 3] = [0.3, 0.6, 1.0];

impl Workbench {
    pub(super) fn ping(&mut self, ping: crate::attention::Ping, id: &str, window: &Window) {
        if self.pinged.iter().any(|p| p == id) {
            return;
        }
        if self.pinged.len() >= 64 {
            self.pinged.pop_front();
        }
        self.pinged.push_back(id.to_owned());
        let wanted = match ping {
            crate::attention::Ping::Mention => self.sound_mentions,
            crate::attention::Ping::Highlight => self.sound_highlights,
        };
        if wanted && (self.sound_focused || !window.is_window_active()) {
            crate::sound::play(&self.sound_tone, self.sound_volume);
        }
    }
    fn choose_sound(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use this sound".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = view.update_in(cx, |this, window, cx| {
                match crate::sound::check_custom(&path) {
                    Ok(()) => {
                        this.sound_tone = crate::sound::Tone::Custom(path);
                        crate::sound::play(&this.sound_tone, this.sound_volume);
                        this.schedule_save(cx);
                    }
                    Err(error) => window.push_notification(Notification::info(error), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn sound_rows(&mut self, page: Div, cx: &mut Context<Self>) -> Div {
        let custom = match &self.sound_tone {
            crate::sound::Tone::Custom(path) => {
                path.file_name().map(|n| n.to_string_lossy().to_string())
            }
            _ => None,
        };
        let tone_index = match self.sound_tone {
            crate::sound::Tone::Chime => 0,
            crate::sound::Tone::Soft => 1,
            crate::sound::Tone::Custom(_) => 2,
        };
        let volume_index = VOLUMES
            .iter()
            .position(|v| (v - self.sound_volume).abs() < 0.01)
            .unwrap_or(1);
        page.child(heading("Sounds"))
            .child(row(
                "Sound when you're mentioned",
                "Your name, @you, or a reply to you.",
                Switch::new("settings-sound-mentions")
                    .checked(self.sound_mentions)
                    .on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.sound_mentions = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
            ))
            .child(row(
                "Sound for highlight words and rules",
                "Subs, raids and other event colours stay silent.",
                Switch::new("settings-sound-highlights")
                    .checked(self.sound_highlights)
                    .on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.sound_highlights = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
            ))
            .child(row(
                "Also while Jawjack is in front",
                "Off: sounds only play while you're in another window.",
                Switch::new("settings-sound-focused")
                    .checked(self.sound_focused)
                    .on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.sound_focused = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
            ))
            .child(row(
                "Sound",
                custom
                    .map(|name| format!("Your file: {name}"))
                    .unwrap_or_else(|| "Choosing one plays it once.".into()),
                TabBar::new("settings-tone")
                    .segmented()
                    .selected_index(tone_index)
                    .on_click(cx.listener(|this, index: &usize, window, cx| {
                        match index {
                            0 => this.sound_tone = crate::sound::Tone::Chime,
                            1 => this.sound_tone = crate::sound::Tone::Soft,
                            _ => return this.choose_sound(window, cx),
                        }
                        crate::sound::play(&this.sound_tone, this.sound_volume);
                        this.schedule_save(cx);
                        cx.notify();
                    }))
                    .children(["Chime", "Soft", "Your .wav…"].map(|l| Tab::new().label(l))),
            ))
            .child(row(
                "Volume",
                "Relative to your system volume.",
                TabBar::new("settings-volume")
                    .segmented()
                    .selected_index(volume_index)
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.sound_volume = VOLUMES[*index];
                        crate::sound::play(&this.sound_tone, this.sound_volume);
                        this.schedule_save(cx);
                        cx.notify();
                    }))
                    .children(["Quiet", "Normal", "Loud"].map(|l| Tab::new().label(l))),
            ))
    }
}
