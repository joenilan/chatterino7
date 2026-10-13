//! AutoMod queue state per channel and the Allow/Deny round trip.
use super::*;

impl Workbench {
    pub(super) fn share_automod(
        &mut self,
        channel: &str,
        queue: std::rc::Rc<Vec<crate::automod::Held>>,
        cx: &mut Context<Self>,
    ) {
        for pane in self.panes() {
            if pane.read(cx).name.as_ref() == channel {
                let queue = queue.clone();
                pane.update(cx, |p, cx| {
                    p.automod = queue;
                    cx.notify();
                });
            }
        }
    }
    /// Remember what happened to a held message, for its inline chat line.
    pub(super) fn automod_outcome(
        &mut self,
        channel: &str,
        id: String,
        outcome: String,
        cx: &mut Context<Self>,
    ) {
        for pane in self.panes() {
            if pane.read(cx).name.as_ref() == channel {
                let (id, outcome) = (id.clone(), outcome.clone());
                pane.update(cx, |p, cx| {
                    std::rc::Rc::make_mut(&mut p.automod_outcomes).insert(id, outcome);
                    cx.notify();
                });
            }
        }
    }
    fn set_automod_pending(
        &mut self,
        channel: &str,
        id: &str,
        pending: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(queue) = self.automod.get_mut(channel) else {
            return;
        };
        if let Some(held) = std::rc::Rc::make_mut(queue).iter_mut().find(|h| h.id == id) {
            held.pending = pending;
        }
        let queue = queue.clone();
        self.share_automod(channel, queue, cx);
    }
    pub(super) fn resolve_automod(
        &mut self,
        channel: String,
        id: String,
        allow: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(identity) = self.account.read(cx).identity() else {
            window.push_notification(
                Notification::info("Sign in with Twitch to review AutoMod."),
                cx,
            );
            return;
        };
        if !identity.has("moderator:manage:automod") {
            window.push_notification(
                Notification::info(
                    "AutoMod review needs updated Twitch permissions (account menu).",
                ),
                cx,
            );
            return;
        }
        self.set_automod_pending(&channel, &id, true, cx);
        let message = id.clone();
        let task =
            cx.background_spawn(async move { crate::automod::resolve(&identity, &message, allow) });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, window, cx| {
                match result {
                    // Twitch confirms with automod.message.update, which removes it;
                    // remove it now too so the strip responds immediately.
                    Ok(()) => {
                        let update = crate::automod::Update::Resolved {
                            id,
                            status: if allow { "approved" } else { "denied" }.into(),
                            by: "you".into(),
                        };
                        if let Some(queue) = this.automod.get_mut(&channel) {
                            if let Some(notice) =
                                crate::automod::apply(std::rc::Rc::make_mut(queue), update)
                            {
                                this.notice = Some((notice, std::time::Instant::now()));
                            }
                            let queue = queue.clone();
                            this.share_automod(&channel, queue, cx);
                        }
                    }
                    Err(error) => {
                        // Gone already (handled elsewhere or expired): drop it from the strip.
                        if error.contains("already") || error.contains("expired") {
                            if let Some(queue) = this.automod.get_mut(&channel) {
                                std::rc::Rc::make_mut(queue).retain(|h| h.id != id);
                                let queue = queue.clone();
                                this.share_automod(&channel, queue, cx);
                            }
                        } else {
                            this.set_automod_pending(&channel, &id, false, cx);
                        }
                        window.push_notification(Notification::info(error), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
