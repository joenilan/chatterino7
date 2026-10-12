//! Ping sounds: when they play, and the Sounds rows of Settings → Notifications.
use super::settings_ui::{heading, row};
use super::*;
use gpui_kit::component::switch::Switch;

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
        use crate::attention::Ping;
        // A highlight rule can pick its own tone or stay silent.
        let tone = match ping {
            Ping::Mention => self.sound_mentions.then(|| self.sound_tone.clone()),
            Ping::Highlight | Ping::Rule(0) => {
                self.sound_highlights.then(|| self.sound_tone.clone())
            }
            Ping::Rule(2) => Some(crate::sound::Tone::Chime),
            Ping::Rule(3) => Some(crate::sound::Tone::Soft),
            Ping::Rule(_) => None,
        };
        let muted = self.streamer_live && self.streamer_mute;
        if let Some(tone) = tone
            && !muted
            && (self.sound_focused || !window.is_window_active())
        {
            crate::sound::play(&tone, self.sound_volume);
        }
        // Like Chatterino: the taskbar button flashes until you come back.
        if self.flash_taskbar && !window.is_window_active() {
            flash_taskbar(window);
        }
    }
    /// After a live alert: your ping sound and a taskbar flash, when turned on.
    pub(super) fn live_alert_extras(&self, window: &Window) {
        if self.live_alert_sound && !(self.streamer_live && self.streamer_mute) {
            crate::sound::play(&self.sound_tone, self.sound_volume);
        }
        if self.live_alert_flash && !window.is_window_active() {
            flash_taskbar(window);
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
                "Flash the taskbar",
                "On a mention or highlight while Jawjack is in the background, its taskbar button flashes until you return.",
                Switch::new("settings-flash-taskbar")
                    .checked(self.flash_taskbar)
                    .on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.flash_taskbar = *on;
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
                crate::theme::Segmented::new("settings-tone")
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
                    .children(["Chime", "Soft", "Your .wav…"].map(|l| crate::theme::segment(l))),
            ))
            .child(row(
                "Volume",
                "Relative to your system volume.",
                crate::theme::Segmented::new("settings-volume")
                    .selected_index(volume_index)
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.sound_volume = VOLUMES[*index];
                        crate::sound::play(&this.sound_tone, this.sound_volume);
                        this.schedule_save(cx);
                        cx.notify();
                    }))
                    .children(["Quiet", "Normal", "Loud"].map(|l| crate::theme::segment(l))),
            ))
    }
}

impl Workbench {
    pub(super) fn streamer_rows(&mut self, page: Div, cx: &mut Context<Self>) -> Div {
        let mode = self.streamer_mode;
        let status = match (mode, self.streamer_live) {
            (crate::streamer_mode::Mode::Automatic, true) => {
                "On now: broadcasting software is running."
            }
            (crate::streamer_mode::Mode::Automatic, false) => {
                "Turns on while OBS, Streamlabs, XSplit, vMix, Twitch Studio, PRISM or Meld is running."
            }
            (_, true) => "On now.",
            (_, false) => "Off.",
        };
        page.child(row(
            "Streamer mode",
            status,
            crate::theme::Segmented::new("settings-streamer-mode")
                .selected_index(
                    crate::streamer_mode::Mode::ALL
                        .iter()
                        .position(|m| *m == mode)
                        .unwrap_or(2),
                )
                .on_click(cx.listener(|this, index: &usize, _, cx| {
                    this.streamer_mode = crate::streamer_mode::Mode::ALL[*index];
                    this.schedule_save(cx);
                    cx.notify();
                }))
                .children(
                    crate::streamer_mode::Mode::ALL.map(|m| crate::theme::segment(m.label())),
                ),
        ))
        .child(row(
            "Mute ping sounds",
            "Keeps mention pings off your stream's desktop audio.",
            Switch::new("settings-streamer-mute")
                .checked(self.streamer_mute)
                .on_click(cx.listener(|this, on: &bool, _, cx| {
                    this.streamer_mute = *on;
                    this.schedule_save(cx);
                    cx.notify();
                })),
        ))
        .child(heading("While it's on"))
        .child(row(
            "Private chatter notes",
            "The notes editor won't open.",
            div()
                .text_size(px(11.))
                .text_color(rgb(theme::MUTED))
                .child("Hidden"),
        ))
        .child(row(
            "Whisper notifications",
            "Say a whisper arrived, not who sent it or what it says.",
            div()
                .text_size(px(11.))
                .text_color(rgb(theme::MUTED))
                .child("Hidden"),
        ))
        .child(row(
            "Stream thumbnails",
            "Hovering a channel name shows its info without the preview image.",
            div()
                .text_size(px(11.))
                .text_color(rgb(theme::MUTED))
                .child("Hidden"),
        ))
        .children(crate::streamer_mode::OPTIONS.iter().enumerate().map(
            |(index, (_, label, detail, _))| {
                row(
                    *label,
                    *detail,
                    Switch::new(("settings-streamer-hide", index))
                        .checked(
                            crate::streamer_mode::HIDE[index]
                                .load(std::sync::atomic::Ordering::Relaxed),
                        )
                        .on_click(cx.listener(move |this, on: &bool, _, cx| {
                            crate::streamer_mode::HIDE[index]
                                .store(*on, std::sync::atomic::Ordering::Relaxed);
                            this.remeasure_all(cx);
                        })),
                )
            },
        ))
    }
}

impl Workbench {
    /// One log line for a chat event in `channel`.
    pub(super) fn log_event(&self, channel: &str, event: &chat_core::Event, cx: &App) {
        // A deleted or cleared message's author comes from what's on screen.
        let find = |pred: &dyn Fn(&chat_core::Message) -> bool| {
            self.panes()
                .iter()
                .filter(|p| p.read(cx).name.as_ref() == channel)
                .find_map(|p| {
                    p.read(cx)
                        .timeline
                        .borrow()
                        .messages()
                        .iter()
                        .rev()
                        .find(|m| pred(m))
                        .cloned()
                })
        };
        let text = match event {
            chat_core::Event::Message(message) => crate::chat_logs::message_text(message),
            chat_core::Event::DeleteMessage { message_id, .. } => {
                match find(&|m| &m.id == message_id) {
                    Some(m) => format!(
                        "A moderator deleted {}'s message: {}",
                        m.display_name,
                        m.body()
                    ),
                    None => "A moderator deleted a message.".into(),
                }
            }
            chat_core::Event::ClearUser { user_id, .. } => match find(&|m| &m.user_id == user_id) {
                Some(m) => format!(
                    "{} was timed out or banned; their messages were cleared.",
                    m.display_name
                ),
                None => "A chatter was timed out or banned.".into(),
            },
            chat_core::Event::ClearChannel { .. } => "Chat was cleared by a moderator.".into(),
            // The vague line duplicates ClearUser above; log the detailed ones.
            chat_core::Event::Moderation { text, detail, .. }
                if *detail > chat_core::Detail::Vague =>
            {
                text.clone()
            }
            chat_core::Event::Moderation { .. } => return,
        };
        if crate::chat_logs::wanted(channel) {
            self.logs.write(&self.logs_folder, channel, text);
        }
    }
    pub(super) fn choose_logs_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Save logs here".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = view.update(cx, |this, cx| {
                this.logs_folder = path;
                this.schedule_save(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

impl Workbench {
    /// Share the block list with the message filter (or clear it when turned off).
    pub(super) fn apply_blocks(&mut self) {
        let blocked = if self.hide_blocked {
            self.blocks.ids.clone().unwrap_or_default()
        } else {
            Default::default()
        };
        self.ignores.borrow_mut().blocked = blocked;
    }
}

/// Keep this window above other windows, or stop (Windows; no-op elsewhere).
pub(crate) fn set_on_top(window: &Window, on: bool) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = HasWindowHandle::window_handle(window)
        && let RawWindowHandle::Win32(win32) = handle.as_raw()
    {
        return jawjack_taskbar::set_topmost(win32.hwnd.get(), on);
    }
    false
}
/// Flash this window's taskbar button (Windows; no-op elsewhere).
fn flash_taskbar(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = HasWindowHandle::window_handle(window)
        && let RawWindowHandle::Win32(win32) = handle.as_raw()
    {
        jawjack_taskbar::flash(win32.hwnd.get());
    }
}
