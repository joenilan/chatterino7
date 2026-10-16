//! Several signed-in Twitch accounts: one active (reading, tools), others usable
//! from any composer's "send as" picker. Each account keeps its own vault entry.
use super::*;
use crate::auth::{TwitchAccount, VAULT};

impl Workbench {
    pub(super) fn load_accounts(
        state: &Value,
        cx: &mut Context<Self>,
    ) -> (Vec<Entity<TwitchAccount>>, usize) {
        let mut vaults: Vec<SharedString> = state["accounts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
            .filter(|v| v.starts_with(crate::profile::vault(VAULT)))
            .take(8)
            .map(|v| SharedString::from(v.to_owned()))
            .collect();
        if vaults.is_empty() {
            vaults.push(crate::profile::vault(VAULT).into());
        }
        let active = state["active_account"]
            .as_str()
            .and_then(|a| vaults.iter().position(|v| v.as_ref() == a))
            .unwrap_or(0);
        let accounts = vaults
            .into_iter()
            .map(|vault| cx.new(|cx| TwitchAccount::with_vault(vault, cx)))
            .collect();
        (accounts, active)
    }
    pub(super) fn accounts_json(&self, cx: &App) -> Value {
        json!({
            "accounts": self.accounts.iter().map(|a| a.read(cx).vault().to_string()).collect::<Vec<_>>(),
            "active_account": self.account.read(cx).vault().to_string(),
        })
    }
    /// Small title-bar avatar stack. Each linked account uses its own Twitch
    /// user ID; never show the current account's picture for a secondary login.
    /// Click anywhere to open the existing account selector.
    pub(super) fn render_account_stack(
        &self,
        spacing: f32,
        viewport_width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active_vault = self.account.read(cx).vault().clone();
        let mut accounts = self
            .accounts
            .iter()
            .filter_map(|account| {
                let account = account.read(cx);
                let login = account.login()?;
                let user_id = account.identity().map(|i| i.user_id);
                Some((
                    account.vault().clone(),
                    login,
                    user_id,
                    !account.missing_scopes().is_empty(),
                ))
            })
            .collect::<Vec<_>>();
        accounts.sort_by_key(|(vault, _, _, _)| *vault != active_vault);
        if accounts.is_empty() {
            return Button::new("account-menu")
                .xsmall()
                .when(self.account.read(cx).signed_out_reason().is_some(), |b| {
                    b.primary()
                })
                .label(if self.account.read(cx).restoring() {
                    "Signing in…"
                } else {
                    "Sign in to Twitch"
                })
                .tooltip("Open Twitch accounts")
                .on_click(cx.listener(|this, _, window, cx| this.open_accounts(window, cx)))
                .into_any_element();
        }
        let count = accounts.len();
        // Keep caption/window controls accessible on narrow layouts. The
        // existing account dialog always exposes every linked account.
        let visible = if viewport_width < 450. {
            1
        } else if viewport_width < 650. {
            2
        } else if viewport_width < 900. {
            4
        } else {
            8
        };
        accounts.truncate(visible);
        let hidden = count.saturating_sub(accounts.len());
        let mut avatars = Vec::with_capacity(accounts.len() + usize::from(hidden > 0));
        for (index, (vault, login, user_id, missing_permissions)) in
            accounts.into_iter().enumerate()
        {
            let active = vault == active_vault;
            let image = user_id
                .as_ref()
                .filter(|id| !id.is_empty())
                .and_then(|id| {
                    let mut catalog = self.catalog.borrow_mut();
                    catalog.profiles.request(id);
                    catalog.profiles.get(id).and_then(|p| p.avatar.clone())
                })
                .and_then(|key| self.media.borrow_mut().get(&key, cx));
            let fallback = login
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            let tooltip = format!(
                "{}{}{} · click to manage accounts",
                login,
                if active { " · Active" } else { "" },
                if missing_permissions {
                    " · Permissions need updating"
                } else {
                    ""
                }
            );
            avatars.push(
                div()
                    .id(("linked-account-avatar", index))
                    .ml(px(if index == 0 { 0. } else { spacing }))
                    .size(px(26.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(if missing_permissions {
                        crate::theme::tone(0xE8C468)
                    } else if active {
                        theme::accent()
                    } else {
                        theme::border()
                    }))
                    .bg(rgb(theme::panel()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .tooltip(move |w, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tooltip.clone()).build(w, cx)
                    })
                    .child(match image {
                        Some(image) => {
                            crate::emote_picker::avatar_fade(image, 22., cx.reduce_motion())
                        }
                        None => div()
                            .size(px(22.))
                            .rounded_full()
                            .bg(rgb(crate::theme::tone(0x3A3350)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.))
                            .text_color(rgb(theme::text()))
                            .child(fallback)
                            .into_any_element(),
                    }),
            );
        }
        if hidden > 0 {
            avatars.push(
                div()
                    .id("linked-account-overflow")
                    .ml(px(spacing))
                    .size(px(24.))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(rgb(theme::control()))
                    .border_1()
                    .border_color(rgb(theme::border()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(theme::text()))
                    .text_size(px(10.))
                    .child(format!("+{hidden}")),
            );
        }
        div()
            .id("account-menu")
            .h_flex()
            .flex_shrink_0()
            .items_center()
            .cursor_pointer()
            .tooltip(move |w, cx| {
                gpui_kit::component::tooltip::Tooltip::new(format!(
                    "{count} linked Twitch account{}",
                    if count == 1 { "" } else { "s" }
                ))
                .build(w, cx)
            })
            .on_hover(cx.listener(|this, over: &bool, _, cx| {
                if this.account_stack_hovered != *over {
                    this.account_stack_hovered = *over;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(|this, _, window, cx| this.open_accounts(window, cx)))
            .children(avatars)
            .into_any_element()
    }

    /// Signed-in accounts as (vault, login), active first.
    pub(super) fn send_as_options(&self, cx: &App) -> Vec<(SharedString, String)> {
        let mut options: Vec<(SharedString, String)> = self
            .accounts
            .iter()
            .filter_map(|a| {
                let a = a.read(cx);
                a.login().map(|login| (a.vault().clone(), login))
            })
            .collect();
        let active = self.account.read(cx).vault().clone();
        options.sort_by_key(|(vault, _)| *vault != active);
        options
    }
    /// The identity to send with in `channel`, when it isn't the active account.
    pub(super) fn send_as_identity(
        &self,
        channel: &str,
        cx: &App,
    ) -> Option<crate::live::Identity> {
        let vault = self.send_as.get(channel)?;
        if vault == self.account.read(cx).vault() {
            return None;
        }
        self.accounts
            .iter()
            .find(|a| a.read(cx).vault() == vault)
            .and_then(|a| a.read(cx).identity())
    }
    /// Shown over the workspace while no account is signed in: sign in, pick
    /// another saved account, or read as a guest. Workspaces stay as they are.
    /// The startup splash: logo and a loading bar while the saved sign-in is
    /// restored, then a short fade into whatever comes next.
    pub(super) fn render_splash(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        const FADE: std::time::Duration = std::time::Duration::from_millis(260);
        // With "Only show live channel tabs", wait for the first live check (up
        // to a few seconds) so offline tabs don't show and then vanish.
        let checking_live = self.live_channels
            && self.account.read(cx).login().is_some()
            && self.opened.elapsed() < std::time::Duration::from_secs(4)
            && !self
                .tab_panes()
                .iter()
                .any(|p| self.streams.get(&p.read(cx).name).is_some());
        if checking_live && !self.splash_wake {
            self.splash_wake = true;
            // Re-check when the cap runs out even if nothing else redraws.
            let left = std::time::Duration::from_secs(4).saturating_sub(self.opened.elapsed());
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(left).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        let restoring = self.account.read(cx).restoring() || checking_live;
        let reduced = cx.reduce_motion();
        let fading = match (restoring, self.splash_done) {
            (true, _) => false,
            (false, None) => {
                let now = std::time::Instant::now();
                self.splash_done = Some(now);
                // Drop the splash once the fade has played.
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(FADE).await;
                    let _ = this.update(cx, |_, cx| cx.notify());
                })
                .detach();
                true
            }
            (false, Some(done)) => done.elapsed() < FADE,
        };
        if !(restoring || fading) || (fading && reduced) {
            return None;
        }
        let status = if checking_live {
            "Checking which channels are live…".to_owned()
        } else {
            self.account.read(cx).status().to_owned()
        };
        let bar = div()
            .w(px(56.))
            .h(px(3.))
            .rounded_full()
            .bg(rgb(theme::accent()))
            .absolute()
            .top_0();
        let bar = if reduced {
            bar.left(px(52.)).into_any_element()
        } else {
            bar.with_animation(
                "splash-bar",
                Animation::new(std::time::Duration::from_millis(1100))
                    .repeat()
                    .with_easing(ease_in_out),
                |el, p| el.left(px(-56. + 216. * p)),
            )
            .into_any_element()
        };
        let splash = div()
            .id("startup-splash")
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgb(theme::canvas()))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .v_flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .text_size(px(26.))
                            .child(
                                div()
                                    .text_color(rgb(theme::accent()))
                                    .font_weight(FontWeight::BOLD)
                                    .child("//"),
                            )
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("JAWJACK")),
                    )
                    .child(
                        div()
                            .relative()
                            .w(px(160.))
                            .h(px(3.))
                            .rounded_full()
                            .overflow_hidden()
                            .bg(rgb(theme::control()))
                            .child(bar),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::muted()))
                            .child(status),
                    ),
            );
        Some(if fading {
            splash
                .with_animation(
                    "splash-out",
                    Animation::new(FADE).with_easing(ease_in_out),
                    |el, p| el.opacity(1. - p),
                )
                .into_any_element()
        } else {
            splash.into_any_element()
        })
    }
    pub(super) fn render_sign_in_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let others: Vec<(usize, String)> = self
            .accounts
            .iter()
            .enumerate()
            .filter(|(_, a)| **a != self.account)
            .filter_map(|(i, a)| a.read(cx).login().map(|login| (i, login)))
            .collect();
        let tabs = self.tabs.len();
        div()
            .id("sign-in-screen")
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgb(theme::canvas()))
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .max_w(px(440.))
                    .gap_4()
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .text_size(px(22.))
                            .child(div().text_color(rgb(theme::accent())).font_weight(FontWeight::BOLD).child("//"))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("JAWJACK")),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(div().text_size(px(16.)).text_color(rgb(theme::text())).child("Sign in to Twitch"))
                            .child(div().text_size(px(12.)).text_color(rgb(theme::muted())).child(if tabs > 0 {
                                format!("Your {} and settings are kept and come back as soon as you're signed in.", if tabs == 1 { "workspace".to_owned() } else { format!("{tabs} workspaces") })
                            } else {
                                "Sign in to chat, moderate and use your streamer tools.".to_owned()
                            })),
                    )
                    .child(
                        div()
                            .rounded(px(8.))
                            .overflow_hidden()
                            .border_1()
                            .border_color(rgb(theme::border()))
                            .child(self.account.clone()),
                    )
                    .when(!others.is_empty(), |el| {
                        el.child(
                            div()
                                .v_flex()
                                .gap_2()
                                .child(div().text_size(px(12.)).text_color(rgb(theme::muted())).child("Or continue with a saved account"))
                                .child(div().h_flex().flex_wrap().gap_2().children(others.into_iter().map(|(index, login)| {
                                    Button::new(SharedString::from(format!("continue-as-{index}")))
                                        .small()
                                        .label(login)
                                        .on_click(cx.listener(move |this, _, _, cx| this.switch_account(index, cx)))
                                }))),
                        )
                    })
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .pt_2()
                            .border_t_1()
                            .border_color(rgb(theme::border()))
                            .child(div().text_size(px(11.)).text_color(rgb(theme::muted())).child("Read-only: no chatting, whispers or tools."))
                            .child(
                                Button::new("read-as-guest")
                                    .ghost()
                                    .small()
                                    .label("Read chat as guest")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.guest_reading = true;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .with_animation(
                "sign-in-in",
                Animation::new(std::time::Duration::from_millis(if cx.reduce_motion() { 1 } else { 220 }))
                    .with_easing(ease_in_out),
                |el, p| el.opacity(p),
            )
            .into_any_element()
    }
    fn switch_account(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(account) = self.accounts.get(index).cloned() else {
            return;
        };
        if account == self.account {
            return;
        }
        self.account = account;
        // Tools bound to the previous account start fresh.
        self.studio_editor = None;
        self.send_as.clear();
        self.notice = Some((
            format!(
                "Switched to {}. Chat reconnects as this account.",
                self.account.read(cx).label()
            ),
            std::time::Instant::now(),
        ));
        self.schedule_save(cx);
        cx.notify();
    }
    fn add_account(&mut self, cx: &mut Context<Self>) -> usize {
        let used: Vec<SharedString> = self
            .accounts
            .iter()
            .map(|a| a.read(cx).vault().clone())
            .collect();
        let vault: SharedString = (1..)
            .map(|n| SharedString::from(format!("{}/{n}", crate::profile::vault(VAULT))))
            .find(|v| !used.contains(v))
            .expect("free vault slot");
        self.accounts
            .push(cx.new(|cx| TwitchAccount::with_vault(vault, cx)));
        self.account_focus = Some(self.accounts.len() - 1);
        self.schedule_save(cx);
        cx.notify();
        self.accounts.len() - 1
    }
    /// Forget a non-active account on this device (its vault entry is removed by Sign out).
    fn remove_account(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.accounts.get(index).is_none_or(|a| *a == self.account) {
            return;
        }
        let removed = self.accounts.remove(index);
        let vault = removed.read(cx).vault().clone();
        self.send_as.retain(|_, v| *v != vault);
        let delete = cx.delete_credentials(&vault);
        cx.spawn(async move |_, _| {
            let _ = delete.await;
        })
        .detach();
        self.account_focus = None;
        self.schedule_save(cx);
        cx.notify();
    }
    pub(super) fn open_accounts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.account_focus = None;
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).min(420.);
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(this) = owner.upgrade() else { return dialog.title("Workspace closed") };
            let (accounts, active, focus) = {
                let this = this.read(cx);
                (this.accounts.clone(), this.account.clone(), this.account_focus)
            };
            let focused = focus.and_then(|i| accounts.get(i).cloned()).unwrap_or(active.clone());
            let rows = accounts.iter().enumerate().map(|(index, account)| {
                let is_active = *account == active;
                let (login, signed_in) = {
                    let a = account.read(cx);
                    (a.login(), a.login().is_some())
                };
                let owner = owner.clone();
                let remove_owner = owner.clone();
                let focus_owner = owner.clone();
                div()
                    .id(("account-row", index))
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py(px(6.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(*account == focused, |el| el.bg(rgb(theme::control())))
                    .hover(|s| s.bg(rgb(theme::hover())))
                    .on_click(move |_, _, cx| {
                        let _ = focus_owner.update(cx, |this, cx| {
                            this.account_focus = Some(index);
                            cx.notify();
                        });
                    })
                    .child(div().size(px(7.)).flex_shrink_0().rounded_full().bg(rgb(if signed_in { theme::stream_live() } else { theme::muted() })))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(px(13.)).child(login.unwrap_or_else(|| "Not signed in".into())))
                    .when(is_active, |el| el.child(div().text_size(px(10.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::accent())).child("ACTIVE")))
                    .when(!is_active && signed_in, |el| {
                        el.child(Button::new(("account-switch", index)).xsmall().ghost().label("Switch").on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |this, cx| this.switch_account(index, cx));
                        }))
                    })
                    .when(!is_active, |el| {
                        el.child(Button::new(("account-remove", index)).xsmall().ghost().label("Remove").on_click(move |_, _, cx| {
                            let _ = remove_owner.update(cx, |this, cx| this.remove_account(index, cx));
                        }))
                    })
            });
            dialog
                .w(px(width))
                .title("Twitch accounts")
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .children(rows)
                        .child(
                            div().pt_1().child(Button::new("account-add").xsmall().ghost().icon(IconName::Plus).label("Add account").on_click({
                                let owner = owner.clone();
                                move |_, _, cx| {
                                    let _ = owner.update(cx, |this, cx| {
                                        this.add_account(cx);
                                    });
                                }
                            })),
                        )
                        .child(div().pt_2().text_size(px(11.)).text_color(rgb(theme::muted())).child(
                            "The active account reads chat and runs tools. With more than one account signed in, each message box gets a \"send as\" picker.",
                        ))
                        .child(div().pt_2().child(focused.clone())),
                )
        });
    }
}
