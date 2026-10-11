//! Local message actions. Re-resolve retained data at activation, never stale body snapshots.
use crate::{ChannelPane, PaneEvent, theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Sizable, StyledExt, WindowExt,
        button::{Button, ButtonVariants},
        menu::{PopupMenu, PopupMenuItem},
        notification::Notification,
    },
    *,
};
use std::ops::Range;

#[derive(Clone)]
pub struct Link {
    pub range: Range<usize>,
    pub url: String,
}
pub fn links(text: &str) -> Vec<Link> {
    let mut result = Vec::new();
    let mut offset = 0;
    for raw in text.split_whitespace() {
        let Some(relative) = text[offset..].find(raw) else {
            break;
        };
        let start = offset + relative;
        offset = start + raw.len();
        let token = raw.trim_start_matches(['<', '(', '[', '{', '"', '\'']);
        let start = start + raw.len() - token.len();
        let mut token = token.trim_end_matches(['>', '"', '\'', '.', ',', '!', '?', ';', ':']);
        for (close, open) in [(')', '('), (']', '['), ('}', '{')] {
            while token.ends_with(close)
                && token.chars().filter(|c| *c == close).count()
                    > token.chars().filter(|c| *c == open).count()
            {
                token = &token[..token.len() - close.len_utf8()];
            }
        }
        if token.len() > 2048 {
            continue;
        }
        let lower = token.to_ascii_lowercase();
        let candidate = if lower.starts_with("https://") || lower.starts_with("http://") {
            token.to_owned()
        } else if lower.starts_with("www.") {
            format!("https://{token}")
        } else {
            continue;
        };
        let Ok(url) = reqwest::Url::parse(&candidate) else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            continue;
        }
        result.push(Link {
            range: start..start + token.len(),
            url: url.to_string(),
        });
        if result.len() == 12 {
            break;
        }
    }
    result
}
pub fn message_links(message: &chat_core::Message) -> Vec<Link> {
    if message.deleted {
        return Vec::new();
    }
    let mut offset = message.author_label().len() + 2;
    let mut result = Vec::new();
    for fragment in &message.fragments {
        if let chat_core::Fragment::Text(text) = fragment {
            for mut link in links(text) {
                link.range.start += offset;
                link.range.end += offset;
                result.push(link);
                if result.len() == 12 {
                    return result;
                }
            }
        }
        offset += fragment.copy_text().len();
    }
    result
}
#[derive(Clone)]
pub struct MentionTarget {
    pub range: Range<usize>,
    pub user_id: String,
}
pub fn message_mentions(message: &chat_core::Message) -> Vec<MentionTarget> {
    if message.deleted {
        return Vec::new();
    }
    let mut offset = message.author_label().len() + 2;
    let mut result = Vec::new();
    for fragment in &message.fragments {
        let end = offset + fragment.copy_text().len();
        if let chat_core::Fragment::Mention { user_id, .. } = fragment
            && offset < end
        {
            result.push(MentionTarget {
                range: offset..end,
                user_id: user_id.clone(),
            });
        }
        offset = end;
    }
    result
}
#[derive(Clone)]
pub struct Press {
    pub message: String,
    pub action: Action,
    pub position: Point<Pixels>,
}
pub type PressState = std::rc::Rc<std::cell::RefCell<Option<Press>>>;
#[derive(Clone)]
pub struct Interaction {
    pub owner: WeakEntity<ChannelPane>,
    pub message: String,
    pub author_len: usize,
    pub links: Vec<Link>,
    pub mentions: Vec<MentionTarget>,
    pub pressed: PressState,
}
impl Interaction {
    pub fn target(&self, byte: usize) -> Option<Action> {
        self.links
            .iter()
            .find(|link| link.range.contains(&byte))
            .map(|link| Action::OpenLink(link.url.clone()))
            .or_else(|| {
                self.mentions
                    .iter()
                    .find(|mention| mention.range.contains(&byte))
                    .map(|mention| Action::InspectMention(mention.user_id.clone()))
            })
            .or_else(|| (byte < self.author_len).then_some(Action::Inspect))
    }
    pub fn down(
        &self,
        event: &MouseDownEvent,
        byte: Option<usize>,
        selection: &chat_core::selection::Selection,
    ) -> bool {
        self.pressed.borrow_mut().take();
        if event.click_count != 1
            || !event.modifiers.control
            || event.modifiers.shift
            || selection.anchor != selection.head
        {
            return false;
        }
        let Some(action) = byte.and_then(|byte| self.target(byte)) else {
            return false;
        };
        *self.pressed.borrow_mut() = Some(Press {
            message: self.message.clone(),
            action,
            position: event.position,
        });
        true
    }
    pub fn up(&self, event: &MouseUpEvent, byte: Option<usize>, window: &mut Window, cx: &mut App) {
        if !self
            .pressed
            .borrow()
            .as_ref()
            .is_some_and(|p| p.message == self.message)
        {
            return;
        }
        let Some(press) = self.pressed.borrow_mut().take() else {
            return;
        };
        if event.button != MouseButton::Left
            || !event.modifiers.control
            || event.modifiers.shift
            || (event.position - press.position).magnitude() > 4.
            || byte.and_then(|b| self.target(b)) != Some(press.action.clone())
        {
            return;
        }
        let _ = self.owner.update(cx, |pane, cx| {
            pane.message_action(&self.message, press.action, window, cx)
        });
        cx.stop_propagation();
    }
}
#[derive(Clone, PartialEq)]
pub enum Action {
    CopyLine,
    CopyBody,
    CopyName,
    CopyId,
    Inspect,
    InspectMention(String),
    Notes,
    Customize,
    SearchAuthor,
    Mention,
    Reply,
    Thread,
    Profile,
    OpenLink(String),
    CopyLink(String),
    /// Moderator actions, checked again by the workspace before dispatch.
    ModDelete,
    ModTimeout(u32),
    ModBan,
}
fn copy(text: String, window: &mut Window, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
    let verified = cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .as_deref()
        == Some(&text);
    window.push_notification(
        Notification::info(if verified {
            "Copied to clipboard"
        } else {
            "Clipboard could not be verified. Try again."
        }),
        cx,
    );
}
pub fn menu(
    owner: WeakEntity<ChannelPane>,
    id: String,
    mut menu: PopupMenu,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let current = owner.upgrade().and_then(|pane| {
        pane.read(cx)
            .timeline
            .borrow()
            .messages()
            .iter()
            .find(|m| m.id == id)
            .cloned()
    });
    let Some(message) = current else {
        return menu.item(PopupMenuItem::new("Message is no longer retained"));
    };
    for (label, action) in [
        ("Reply to message", Action::Reply),
        ("View conversation", Action::Thread),
        ("Inspect chatter", Action::Inspect),
        ("Personalize chatter…", Action::Customize),
        ("Private chatter note…", Action::Notes),
        ("Search this chatter’s messages", Action::SearchAuthor),
        ("Mention in composer", Action::Mention),
        ("Open Twitch profile", Action::Profile),
        ("Copy message", Action::CopyLine),
        ("Copy message text", Action::CopyBody),
        ("Copy username", Action::CopyName),
        ("Copy user ID", Action::CopyId),
    ] {
        if action == Action::Notes && message.user_id.is_empty() {
            continue;
        }
        if action == Action::Customize && !crate::chatter_appearance::valid_id(&message.user_id) {
            continue;
        }
        if action == Action::SearchAuthor && message.user_id.is_empty() {
            continue;
        }
        if action == Action::Reply && (message.deleted || !message.replyable) {
            continue;
        }
        if matches!(action, Action::Mention | Action::Profile | Action::CopyName)
            && message.login.is_none()
        {
            continue;
        }
        let owner = owner.clone();
        let id = id.clone();
        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, w, cx| {
            let _ = owner.update(cx, |p, cx| p.message_action(&id, action.clone(), w, cx));
        }));
    }
    if !message.deleted {
        let mut seen = std::collections::HashSet::new();
        for fragment in message
            .fragments
            .iter()
            .filter(|f| matches!(f, chat_core::Fragment::Mention { .. }))
            .take(8)
        {
            if let chat_core::Fragment::Mention { user_id, login, .. } = fragment {
                if !seen.insert(user_id) {
                    continue;
                }
                let owner = owner.clone();
                let id = id.clone();
                let user_id = user_id.clone();
                menu = menu.item(
                    PopupMenuItem::new(format!("Inspect mentioned @{login}")).on_click(
                        move |_, w, cx| {
                            let _ = owner.update(cx, |p, cx| {
                                p.message_action(
                                    &id,
                                    Action::InspectMention(user_id.clone()),
                                    w,
                                    cx,
                                )
                            });
                        },
                    ),
                );
            }
        }
    }
    // Pinned Streamer.bot actions run with this message's user and text as arguments.
    let bot_actions = owner
        .upgrade()
        .map(|pane| pane.read(cx).bot_actions.clone())
        .unwrap_or_default();
    if !bot_actions.is_empty() && !message.deleted {
        menu = menu.separator();
        for action in bot_actions.iter() {
            let owner = owner.clone();
            let action = action.clone();
            let args = serde_json::json!({
                "source": "jawjack",
                "user": message.login.clone().unwrap_or_default(),
                "userId": message.user_id,
                "userName": message.display_name,
                "message": message.body(),
                "messageId": message.id,
                "channel": message.channel_id,
            });
            menu = menu.item(
                PopupMenuItem::new(format!("Streamer.bot · {}", action.name)).on_click(
                    move |_, _, cx| {
                        let (action, args) = (action.clone(), args.clone());
                        let _ = owner.update(cx, |_, cx| {
                            cx.emit(PaneEvent::RunBotAction { action, args })
                        });
                    },
                ),
            );
        }
    }
    // Moderator actions appear only where the account moderates or owns the channel.
    let moderates = owner.upgrade().is_some_and(|pane| {
        matches!(
            pane.read(cx).role,
            Some(crate::capabilities::Role::Moderator | crate::capabilities::Role::Broadcaster)
        )
    });
    if moderates && message.login.is_some() && !message.user_id.is_empty() {
        menu = menu.separator();
        // The broadcaster can't be timed out or banned in their own channel.
        let broadcaster = owner
            .upgrade()
            .is_some_and(|pane| message.login.as_deref() == Some(pane.read(cx).name.as_ref()));
        let mut actions = vec![
            ("Timeout 1 minute", Action::ModTimeout(60)),
            ("Timeout 10 minutes", Action::ModTimeout(600)),
            ("Timeout 1 hour", Action::ModTimeout(3600)),
            ("Timeout 1 day", Action::ModTimeout(86_400)),
            ("Ban…", Action::ModBan),
        ];
        if broadcaster {
            actions.clear();
        }
        if !message.deleted {
            actions.insert(0, ("Delete message", Action::ModDelete));
        }
        for (label, action) in actions {
            let owner = owner.clone();
            let id = id.clone();
            menu = menu.item(PopupMenuItem::new(label).on_click(move |_, w, cx| {
                let _ = owner.update(cx, |p, cx| p.message_action(&id, action.clone(), w, cx));
            }));
        }
    }
    for link in message_links(&message).into_iter().take(3) {
        let host = reqwest::Url::parse(&link.url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_default();
        let open = owner.clone();
        let open_id = id.clone();
        let url = link.url.clone();
        let copy = owner.clone();
        let copy_id = id.clone();
        menu = menu
            .separator()
            .item(
                PopupMenuItem::new(format!("Open link · {host}")).on_click(move |_, w, cx| {
                    let _ = open.update(cx, |p, cx| {
                        p.message_action(&open_id, Action::OpenLink(url.clone()), w, cx)
                    });
                }),
            )
            .item(
                PopupMenuItem::new(format!("Copy link · {host}")).on_click(move |_, w, cx| {
                    let _ = copy.update(cx, |p, cx| {
                        p.message_action(&copy_id, Action::CopyLink(link.url.clone()), w, cx)
                    });
                }),
            );
    }
    menu
}
impl ChannelPane {
    pub fn message_action(
        &mut self,
        id: &str,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let message = self
            .timeline
            .borrow()
            .messages()
            .iter()
            .find(|m| m.id == id)
            .cloned();
        let Some(message) = message else {
            window.push_notification(
                Notification::info("That message is no longer in retained history"),
                cx,
            );
            return;
        };
        let opening = matches!(&action, Action::OpenLink(_));
        let login = message.login.clone().unwrap_or_default();
        let moderate = |request| PaneEvent::TwitchCommand {
            request,
            draft: String::new(),
        };
        match action {
            Action::ModDelete => cx.emit(moderate(crate::twitch_actions::Request::Delete {
                message_id: message.id.clone(),
            })),
            Action::ModTimeout(seconds) => {
                cx.emit(moderate(crate::twitch_actions::Request::Timeout {
                    login,
                    seconds,
                    reason: String::new(),
                }))
            }
            Action::ModBan => {
                // A ban is high impact: name the target and channel before acting.
                let owner = cx.entity().downgrade();
                let channel = self.name.to_string();
                let label = message.author_label().to_owned();
                window.open_dialog(cx, move |dialog, _, _| {
                    let owner = owner.clone();
                    let login = login.clone();
                    dialog
                        .title(format!("Ban {label} from #{channel}?"))
                        .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child(format!("@{login} will be banned until a moderator unbans them (/unban {login}).")))
                        .footer(div().h_flex().justify_end().gap_2()
                            .child(Button::new("ban-cancel").small().label("Cancel").on_click(|_, w, cx| w.close_dialog(cx)))
                            .child(Button::new("ban-confirm").small().danger().label("Ban").on_click(move |_, w, cx| {
                                w.close_dialog(cx);
                                let login = login.clone();
                                let _ = owner.update(cx, |_, cx| cx.emit(PaneEvent::TwitchCommand {
                                    request: crate::twitch_actions::Request::Ban { login, reason: String::new() },
                                    draft: String::new(),
                                }));
                            })))
                });
            }
            Action::Notes => {
                if !message.user_id.is_empty() {
                    cx.emit(PaneEvent::EditChatterNote(
                        crate::chatter_appearance::Target {
                            id: message.user_id,
                            login: message.login.unwrap_or_default(),
                            name: message.display_name,
                        },
                    ));
                }
            }
            Action::Customize => {
                if crate::chatter_appearance::valid_id(&message.user_id) {
                    cx.emit(PaneEvent::EditChatter(crate::chatter_appearance::Target {
                        id: message.user_id,
                        login: message.login.unwrap_or_default(),
                        name: message.display_name,
                    }));
                }
            }
            Action::SearchAuthor => {
                if !message.user_id.is_empty() {
                    self.search_for_author(&message.user_id, window, cx)
                }
            }
            Action::Reply => self.begin_reply(&message, window, cx),
            Action::Thread => self.open_conversation(&message, window, cx),
            Action::CopyLine => copy(message.copy_line(), window, cx),
            Action::CopyBody => copy(message.body(), window, cx),
            Action::CopyName => {
                if let Some(login) = message.login {
                    copy(login, window, cx)
                }
            }
            Action::CopyId => copy(message.user_id, window, cx),
            Action::Profile => {
                if let Some(login) = message.login {
                    cx.open_url(&format!("https://www.twitch.tv/{login}"));
                }
            }
            Action::OpenLink(url) | Action::CopyLink(url) => {
                if message.deleted || !message_links(&message).iter().any(|link| link.url == url) {
                    window.push_notification(
                        Notification::info("That link is no longer in the message"),
                        cx,
                    );
                    return;
                }
                if opening {
                    cx.open_url(&url);
                } else {
                    copy(url, window, cx);
                }
            }
            Action::Mention => {
                if let Some(login) = message.login {
                    self.insert_mention(&login, window, cx)
                }
            }
            Action::Inspect => self.inspect_chatter(
                message.user_id.clone(),
                message.author_label().to_owned(),
                message.login.clone(),
                window,
                cx,
            ),
            Action::InspectMention(user_id) => {
                if message.deleted {
                    return;
                }
                if let Some(chat_core::Fragment::Mention {
                    user_id,
                    login,
                    display_name,
                    ..
                }) = message.fragments.iter().find(
                    |f| matches!(f,chat_core::Fragment::Mention{user_id:id,..} if id==&user_id),
                ) {
                    self.inspect_chatter(
                        user_id.clone(),
                        display_name.clone(),
                        Some(login.clone()),
                        window,
                        cx,
                    );
                }
            }
        }
    }
    pub(crate) fn inspect_chatter(
        &mut self,
        user_id: String,
        clicked_name: String,
        clicked_login: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if user_id.is_empty() {
            return;
        }
        self.catalog.borrow_mut().profiles.request(&user_id);
        let owner = cx.entity().downgrade();
        window.open_dialog(cx,move|dialog,window,cx|{
                    let current=owner.upgrade().map(|pane|{
                        let pane=pane.read(cx);let timeline=pane.timeline.borrow();
                        let messages=timeline.messages().iter().filter(|m|m.user_id==user_id).collect::<Vec<_>>();
                        let last=messages.last().copied();
                        (pane.name.to_string(),messages.len(),last.map(|m|m.author_label().to_owned()).unwrap_or_else(||clicked_name.clone()),last.and_then(|m|m.login.clone()).or_else(||clicked_login.clone()),messages.iter().rev().take(20).map(|m|m.body()).collect::<Vec<_>>(),last.map(|m|{
                            // Show the provider's badge title and art, not internal set IDs.
                            let catalog=pane.catalog.borrow();
                            m.badges.iter().map(|b|match catalog.twitch.badge(&m.channel_id,&b.set_id,&b.id){
                                Some(badge)=>(badge.title.clone(),Some(badge.key.clone())),
                                None=>(b.set_id.replace('_'," "),None),
                            }).collect::<Vec<_>>()
                        }).unwrap_or_default())
                    });
                    let Some((channel,count,name,login,recent,badges))=current else{return dialog.title("Channel closed");};
                    if let Some(pane)=owner.upgrade(){pane.read(cx).catalog.borrow_mut().profiles.request(&user_id);}
                    let profile=owner.upgrade().and_then(|pane|pane.read(cx).catalog.borrow().profiles.get(&user_id).cloned());
                    let avatar=profile.as_ref().and_then(|p|p.avatar.as_ref()).and_then(|key|owner.upgrade().and_then(|pane|{let media=pane.read(cx).media.clone();media.borrow_mut().get(key,cx)}));
                    let cosmetics=owner.upgrade().map(|pane|{
                        let pane=pane.read(cx);let catalog=pane.catalog.borrow();
                        (catalog.paints.for_user(&user_id).cloned(),catalog.paints.preview_for_user(&user_id),catalog.paints.pending_for_user(&user_id),catalog.badges.for_user(&user_id).cloned())
                    });
                    let cosmetic_panel=cosmetics.and_then(|(paint,preview,pending,badge)|{
                        if paint.is_none()&&!pending&&badge.is_none(){return None;}
                        let badge_image=badge.as_ref().and_then(|b|owner.upgrade().and_then(|pane|{let media=pane.read(cx).media.clone();media.borrow_mut().get(&b.key,cx)}));
                        Some(div().v_flex().gap_1().p_2().rounded(px(4.)).bg(rgb(theme::CONTROL))
                            .child(div().text_size(px(11.)).text_color(rgb(0xB6A9E6)).child("7TV cosmetics · observed active grants"))
                            .when_some(badge,|el,b|el.child(div().h_flex().gap_2()
                                .child(crate::emote_picker::icon(badge_image,String::new(),28.,28.))
                                .child(div().text_size(px(12.)).child(b.title))))
                            .when_some(paint,|el,p|el
                                .child(div().text_size(px(12.)).child(p.name))
                                .when_some(preview,|el,image|el.child(img(image).w_full().h(px(32.)).object_fit(ObjectFit::Fill)))
                                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                                    p.limitation.map(|reason|format!("{reason} · preview unavailable")).or(p.approximation).unwrap_or_else(||"Static username paint · local color overrides take priority".into()))))
                            .when(pending,|el|el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Paint grant received; waiting for its definition")))
                            .into_any_element())
                    });
                    let width=(f32::from(window.viewport_size().width)-24.).clamp(180., 420.);
                    let body_height=(f32::from(window.viewport_size().height)-160.).clamp(60.,480.);
                    let mention_owner=owner.clone();let mention_login=login.clone();let profile_login=login.clone();let id=user_id.clone();let search_id=user_id.clone();let search_owner=owner.clone();let note_owner=owner.clone();let note_target=crate::chatter_appearance::Target{id:user_id.clone(),login:login.clone().unwrap_or_default(),name:name.clone()};
                    dialog.w(px(width)).title(name).child(div().id("chatter-body").v_flex().gap_2().min_w_0().max_h(px(body_height)).overflow_y_scroll()
                        .when_some(profile,|el,profile|el.child(div().h_flex().items_start().gap_2()
                            .child(crate::emote_picker::icon(avatar,String::new(),48.,48.))
                            .child(div().v_flex().gap_1().flex_1().min_w_0().child(profile.display_name)
                                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("Created · {}",created_label(&profile.created))))
                                .when(!profile.role.is_empty(),|el|el.child(div().text_size(px(11.)).text_color(rgb(0xC5B8E6)).child(profile.role)))))
                            .when(!profile.description.is_empty(),|el|el.child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child(profile.description))))
                        .child(login.clone().map(|login|format!("@{login}" )).unwrap_or_else(||"Login unavailable".into()))
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("{count} retained messages in #{channel}")))
                        .when(!badges.is_empty(),|el|el.child(div().h_flex().flex_wrap().gap_x_3().gap_y_1().text_size(px(11.)).children(badges.into_iter().map(|(title,key)|{
                            let image=key.and_then(|key|owner.upgrade().and_then(|pane|{let media=pane.read(cx).media.clone();media.borrow_mut().get(&key,cx)}));
                            div().h_flex().gap_1().items_center().when_some(image,|el,image|el.child(crate::emote_picker::icon(Some(image),String::new(),18.,18.))).child(title)
                        }))))
                        .children(cosmetic_panel)
                        .child(div().id("chatter-recent").v_flex().gap_2()
                            .children(recent.into_iter().map(|body|div().p_2().rounded(px(4.)).bg(rgb(theme::CONTROL)).min_w_0().child(body))))
                        .child(div().h_flex().flex_wrap().gap_1()
                            .when(login.is_some(),|el|el
                                .child(Button::new("chatter-mention").small().label("Mention").on_click(move|_,w,cx|{if let Some(login)=&mention_login{w.close_dialog(cx);let _=mention_owner.update(cx,|p,cx|p.insert_mention(login,w,cx));}}))
                                .child(Button::new("chatter-profile").small().label("Twitch profile").on_click(move|_,_,cx|{if let Some(login)=&profile_login{cx.open_url(&format!("https://www.twitch.tv/{login}"));}})))
                            .child(Button::new("chatter-search").small().label("Search messages").tooltip("Find this chatter in retained channel history").on_click(move|_,w,cx|{w.close_dialog(cx);let _=search_owner.update(cx,|p,cx|p.search_for_author(&search_id,w,cx));}))
                            .child(Button::new("chatter-private-note").small().label("Private note…").on_click(move|_,w,cx|{w.close_dialog(cx);let _=note_owner.update(cx,|_,cx|cx.emit(PaneEvent::EditChatterNote(note_target.clone())));}))
                            .child(Button::new("chatter-copy-id").small().label("Copy ID").on_click(move|_,w,cx|copy(id.clone(),w,cx)))))
                });
    }
    pub(crate) fn insert_mention(
        &mut self,
        login: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = self.draft.read(cx);
        let value = input.value().to_string();
        let range = input.selected_range();
        let Some(before) = value.get(..range.start) else {
            return;
        };
        let Some(after) = value.get(range.end..) else {
            return;
        };
        let left = if before.chars().last().is_some_and(|c| !c.is_whitespace()) {
            " "
        } else {
            ""
        };
        let right = if after.chars().next().is_none_or(|c| !c.is_whitespace()) {
            " "
        } else {
            ""
        };
        let insertion = format!("{left}@{login}{right}");
        if before.chars().count() + insertion.chars().count() + after.chars().count() > 500 {
            window.push_notification(
                Notification::info("Mention would exceed 500 characters; draft unchanged"),
                cx,
            );
            return;
        }
        self.draft.update(cx, |input, cx| {
            input.set_selected_range(range, cx);
            input.replace(insertion, window, cx);
            input.focus(window, cx);
        });
        self.picker.open = false;
        self.picker.suggestions.clear();
        self.picker.token = None;
        cx.emit(PaneEvent::DraftChanged);
        cx.notify();
    }
}

/// Twitch returns RFC 3339 timestamps; cards show a readable local date.
fn created_label(raw: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%b %-d, %Y")
                .to_string()
        })
        .unwrap_or_else(|_| raw.to_owned())
}
