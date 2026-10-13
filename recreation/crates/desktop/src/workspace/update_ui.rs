//! The update control in the workspaces sidebar, the release-notes dialog, the
//! install confirmation and the update notifications (the flow in
//! `crate::updates`; Haiwire's / T3 Code's notify → click to download →
//! confirm to install). Nothing downloads or installs without a click, and
//! quitting never installs.
use super::*;
use crate::updates::{Announcement, ErrorContext, NoteLine, Phase};

impl Workbench {
    /// Turn the updater's once-per-session announcements into notifications:
    /// in the app, plus Windows while Jawjack is in the background.
    pub(super) fn announce_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(announcement) = self.updates.take_announcement() {
            let owner = cx.entity().downgrade();
            let note = match announcement {
                Announcement::Available(version) => Notification::new()
                    .title(format!("Jawjack {version} is available"))
                    .message("Open Jawjack to see what's changed and download it.")
                    .in_app_and_system()
                    .on_click(move |_, window, cx| {
                        let _ = owner.update(cx, |this, cx| this.open_release_notes(window, cx));
                    }),
                Announcement::Downloaded(version) => Notification::new()
                    .title(format!("Update {version} downloaded"))
                    .message("Restart Jawjack from the update button to install it.")
                    .in_app_and_system()
                    .on_click(move |_, window, cx| {
                        let _ = owner.update(cx, |this, cx| this.confirm_install(window, cx));
                    }),
            };
            window.push_notification(note, cx);
        }
        if let Some(failure) = self.updates.take_last_install_failure() {
            window.push_notification(Notification::info(failure), cx);
        }
    }

    /// The sidebar's update button: what it does depends on the update state.
    pub(super) fn update_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match (self.updates.phase(), self.updates.error_context()) {
            (Phase::Available, Some(ErrorContext::Download)) => {
                self.updates.start_download();
            }
            (Phase::Available, _) => self.open_release_notes(window, cx),
            (Phase::Downloaded, _) => self.confirm_install(window, cx),
            (Phase::Downloading | Phase::Installing | Phase::Checking, _) => {}
            _ => {
                self.updates.check_now();
            }
        }
        cx.notify();
    }

    /// Release notes before anything downloads, with Download / Restart and install.
    pub(super) fn open_release_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(280., 520.);
        let height = (f32::from(window.viewport_size().height) - 220.).clamp(120., 420.);
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(this) = owner.upgrade() else {
                return dialog.title("Update");
            };
            let updates = &this.read(cx).updates;
            let version = updates
                .available_version()
                .or(updates.downloaded_version())
                .unwrap_or("")
                .to_owned();
            let can_download = updates.can_download();
            let subtitle = match updates.size_label() {
                Some(size) => format!("Jawjack {version} · {size}"),
                None => format!("Jawjack {version}"),
            };
            let notes = updates.notes().to_vec();
            let reason = updates
                .download_unavailable_reason()
                .or(updates.ineligibility_reason());
            let phase = updates.phase();
            let percent = updates.percent().unwrap_or(0);
            let failed = updates.error_context() == Some(ErrorContext::Download);
            let primary: Option<(String, bool)> = match phase {
                _ if !can_download => None,
                Phase::Downloading => Some((format!("Downloading… {percent}%"), false)),
                Phase::Downloaded => Some(("Restart and install".into(), true)),
                Phase::Available if failed => Some(("Retry download".into(), true)),
                Phase::Available => Some(("Download".into(), true)),
                _ => None,
            };
            let (later, act) = (owner.clone(), owner.clone());
            dialog
                .w(px(width))
                .title(if can_download {
                    "Update ready to download"
                } else {
                    "Update available"
                })
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(div().text_size(px(12.)).text_color(rgb(theme::muted())).child(subtitle))
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(theme::accent()))
                                .child("WHAT'S CHANGED"),
                        )
                        .child(
                            div()
                                .id("update-notes")
                                .v_flex()
                                .gap_1()
                                .max_h(px(height))
                                .overflow_y_scroll()
                                .text_size(px(12.))
                                .when(notes.is_empty(), |el| {
                                    el.child(
                                        div()
                                            .text_color(rgb(theme::muted()))
                                            .child("No release notes were published."),
                                    )
                                })
                                .children(notes.into_iter().map(|line| match line {
                                    NoteLine::Heading(text) => div()
                                        .pt_1()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(text),
                                    NoteLine::Bullet(text) => div()
                                        .h_flex()
                                        .items_start()
                                        .gap_1()
                                        .child(div().text_color(rgb(theme::muted())).child("•"))
                                        .child(div().flex_1().min_w_0().child(text)),
                                    NoteLine::Text(text) => div().child(text),
                                })),
                        )
                        .when(failed, |el| {
                            el.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(crate::theme::tone(0xF29D9D)))
                                    .child("The download failed or didn't match its published checksum. Nothing was installed."),
                            )
                        })
                        .when_some(reason.filter(|_| !can_download), |el, reason| {
                            el.child(div().text_size(px(11.)).text_color(rgb(theme::muted())).child(reason))
                        }),
                )
                .footer(
                    div()
                        .h_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            Button::new("update-release-page")
                                .small()
                                .ghost()
                                .label("Release page")
                                .on_click(|_, _, cx| cx.open_url(crate::updates::RELEASE_PAGE)),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("update-later")
                                .small()
                                .label("Later")
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    let _ = later.update(cx, |_, cx| cx.notify());
                                }),
                        )
                        .when_some(primary, |el, (label, enabled)| {
                            el.child(
                                Button::new("update-primary")
                                    .small()
                                    .primary()
                                    .label(label)
                                    .disabled(!enabled)
                                    .on_click(move |_, w, cx| {
                                        let _ = act.update(cx, |this, cx| {
                                            if this.updates.phase() == Phase::Downloaded {
                                                w.close_dialog(cx);
                                                this.confirm_install(w, cx);
                                            } else {
                                                this.updates.start_download();
                                            }
                                            cx.notify();
                                        });
                                    }),
                            )
                        }),
                )
        });
    }

    /// "Install update {v} and restart Jawjack?" — always asked first.
    pub(super) fn confirm_install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(version) = self.updates.downloaded_version().map(str::to_owned) else {
            return;
        };
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(280., 460.);
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            dialog
                .w(px(width))
                .title(format!("Install update {version} and restart Jawjack?"))
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .text_size(px(12.))
                        .child("Any running work will be interrupted. Make sure you're ready before continuing.")
                        .child(div().text_color(rgb(theme::muted())).child(
                            "Jawjack saves your workspace and closes; the installer then runs quietly, keeps your settings and sign-ins, and starts Jawjack again.",
                        )),
                )
                .footer(
                    div()
                        .h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("install-cancel")
                                .small()
                                .label("Cancel")
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(
                            Button::new("install-confirm")
                                .small()
                                .primary()
                                .label("Restart and install")
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    let _ = owner.update(cx, |this, cx| this.install_now(w, cx));
                                }),
                        ),
                )
        });
    }

    /// Re-check the download, save, start setup and close. Setup waits for
    /// Jawjack to exit and starts it again; if it can't start, Jawjack stays open.
    fn install_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(problem) = self.updates.request_install() {
            window.push_notification(Notification::info(problem), cx);
            cx.notify();
            return;
        }
        let _ = storage::save(&self.snapshot(cx));
        match self.updates.launch_installer() {
            Ok(_) => cx.quit(),
            Err(problem) => window.push_notification(Notification::info(problem), cx),
        }
        cx.notify();
    }

    /// A title-bar chip while an update is known, so it's visible even with the
    /// workspaces sidebar hidden. Clicking does what the sidebar button does.
    pub(super) fn render_update_chip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let updates = &self.updates;
        let version = updates
            .available_version()
            .or(updates.downloaded_version())?
            .to_owned();
        let (icon, label) = match updates.phase() {
            Phase::Available if updates.error_context() == Some(ErrorContext::Download) => (
                Icon::new(IconName::CircleAlert),
                "Download failed · Retry".to_owned(),
            ),
            Phase::Available => (
                Icon::new(gpui_kit::assets::IconName::Download),
                format!("Update {version}"),
            ),
            Phase::Downloading => (
                Icon::new(IconName::LoaderCircle),
                format!("Downloading {}%", updates.percent().unwrap_or(0)),
            ),
            Phase::Downloaded => (
                Icon::new(gpui_kit::assets::IconName::RotateCcw),
                "Restart to update".to_owned(),
            ),
            Phase::Installing => (Icon::new(IconName::LoaderCircle), "Installing…".to_owned()),
            _ => return None,
        };
        Some(
            Button::new("update-chip")
                .ghost()
                .xsmall()
                .icon(icon.text_color(rgb(theme::accent())))
                .label(label)
                .tooltip(format!("Jawjack {version}"))
                .disabled(matches!(
                    updates.phase(),
                    Phase::Downloading | Phase::Installing
                ))
                .on_click(cx.listener(|this, _, w, cx| this.update_click(w, cx)))
                .into_any_element(),
        )
    }

    /// The version and one update button at the bottom of the workspaces sidebar.
    pub(super) fn render_updates(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let updates = &self.updates;
        let version = updates
            .available_version()
            .or(updates.downloaded_version())
            .unwrap_or("")
            .to_owned();
        let failed = |context| updates.error_context() == Some(context);
        // (icon, label shown beside it, tooltip, accent)
        let (icon, label, tip, accent): (Icon, Option<String>, String, Option<u32>) =
            match updates.phase() {
                Phase::Checking => (
                    Icon::new(IconName::LoaderCircle),
                    None,
                    "Checking for updates…".into(),
                    None,
                ),
                Phase::UpToDate => (
                    Icon::new(IconName::RefreshCw),
                    Some("Up to date".into()),
                    "Up to date. Click to check again.".into(),
                    None,
                ),
                Phase::Error => (
                    Icon::new(IconName::CircleAlert),
                    Some("Update check failed".into()),
                    "Update check failed. Click to try again.".into(),
                    None,
                ),
                Phase::Available if failed(ErrorContext::Download) => (
                    Icon::new(IconName::CircleAlert),
                    Some("Download failed · Retry".into()),
                    format!("Download failed for {version}. Click to retry."),
                    Some(0xF29D9D),
                ),
                Phase::Available => (
                    Icon::new(gpui_kit::assets::IconName::Download),
                    Some(format!("Jawjack {version} is available")),
                    format!("Update {version} ready to download"),
                    Some(theme::accent()),
                ),
                Phase::Downloading => (
                    Icon::new(IconName::LoaderCircle),
                    Some(format!(
                        "Downloading update ({}%)",
                        updates.percent().unwrap_or(0)
                    )),
                    String::new(),
                    Some(theme::accent()),
                ),
                Phase::Downloaded if failed(ErrorContext::Install) => (
                    Icon::new(gpui_kit::assets::IconName::RotateCcw),
                    Some("Install failed · Retry".into()),
                    format!("Install failed for {version}. Click to retry."),
                    Some(0xF29D9D),
                ),
                Phase::Downloaded => (
                    Icon::new(gpui_kit::assets::IconName::RotateCcw),
                    Some(format!("Restart to install {version}")),
                    format!("Update {version} downloaded. Click to restart and install."),
                    Some(theme::stream_live()),
                ),
                Phase::Installing => (
                    Icon::new(IconName::LoaderCircle),
                    Some(format!("Installing {version}…")),
                    String::new(),
                    Some(theme::accent()),
                ),
                Phase::Unpublished => (
                    Icon::new(IconName::RefreshCw),
                    None,
                    "No release published yet. Click to check again.".into(),
                    None,
                ),
                Phase::Idle => match updates.last_install_failure() {
                    Some(_) => (
                        Icon::new(IconName::CircleAlert),
                        Some("Update not installed".into()),
                        "The last update was not installed. Click to check again.".into(),
                        None,
                    ),
                    None => (
                        Icon::new(IconName::RefreshCw),
                        None,
                        "Check for updates".into(),
                        None,
                    ),
                },
            };
        let busy = matches!(
            updates.phase(),
            Phase::Checking | Phase::Downloading | Phase::Installing
        );
        let known = accent.is_some();
        div()
            .id(SharedString::from(format!(
                "update-control-{:?}",
                updates.phase()
            )))
            .h_flex()
            .gap_1()
            .items_center()
            .min_w_0()
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(rgb(theme::muted()))
                    .child(format!("v{}", updates.current_version())),
            )
            .child(div().flex_1().min_w_0())
            .child(
                div()
                    .id("update-button")
                    .h_flex()
                    .gap_1()
                    .items_center()
                    .min_w_0()
                    .px_1()
                    .py(px(2.))
                    .rounded(px(4.))
                    .when(known, |el| {
                        el.bg(rgb(theme::control()))
                            .border_1()
                            .border_color(rgb(accent.unwrap_or(theme::border())))
                    })
                    .text_color(rgb(accent.unwrap_or(theme::muted())))
                    .when(!busy, |el| {
                        el.cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::hover())).text_color(rgb(theme::text())))
                            .on_click(cx.listener(|this, _, w, cx| this.update_click(w, cx)))
                    })
                    .when(!tip.is_empty(), |el| {
                        let tip = tip.clone();
                        el.tooltip(move |w, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx)
                        })
                    })
                    .child(icon.xsmall())
                    .when_some(label, |el, label| {
                        el.child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(label),
                        )
                    }),
            )
    }
}
