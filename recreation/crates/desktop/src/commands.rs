//! Local composer commands. Unknown slash input never falls through to chat.
use crate::{ChannelPane, ComposerFeedback, PaneEvent, message_actions::Action, theme};
use gpui_kit::{*, component::{button::Button, notification::{Notification, NotificationDelivery}, Sizable, StyledExt, WindowExt}};

pub struct Command { pub name: &'static str, pub usage: &'static str, pub description: &'static str }
pub const COMMANDS: &[Command] = &[
    Command { name: "/commands", usage: "/commands", description: "Edit personal text shortcuts" },
    Command { name: "/help", usage: "/help", description: "Show local commands" },
    Command { name: "/find", usage: "/find query", description: "Search retained messages in this channel" },
    Command { name: "/usercard", usage: "/usercard login", description: "Open a recent chatter's card" },
    Command { name: "/reply", usage: "/reply login message", description: "Prepare a reply; review before sending" },
    Command { name: "/unreply", usage: "/unreply", description: "Remove the current reply target" },
    Command { name: "/latest", usage: "/latest", description: "Return to the live end of chat" },
];
pub fn is_command(value: &str) -> bool { value.trim_start().starts_with('/') }
fn split(value: &str) -> (&str, &str) {
    let value = value.trim();
    value.find(char::is_whitespace).map_or((value, ""), |at| (&value[..at], value[at..].trim()))
}
impl ChannelPane {
    fn command_feedback(&mut self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) {
        let text: SharedString = text.into();
        self.send_status = text.to_string();
        cx.notify();
        window.push_notification(Notification::info(text).id::<ComposerFeedback>().delivery(NotificationDelivery::InApp), cx);
    }
    fn command_draft(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.compose_revision = self.compose_revision.wrapping_add(1);
        self.draft.update(cx, |input, cx| { input.set_value(value.to_owned(), window, cx); input.set_selected_range(value.len()..value.len(),cx); input.focus(window,cx); });
        self.picker.open = false; self.picker.suggestions.clear(); self.picker.token = None;
        self.send_status.clear(); cx.emit(PaneEvent::DraftChanged); cx.notify();
    }
    /// Returns true for every slash-prefixed input, including unsupported commands.
    pub fn route_command(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !is_command(text) { return false; }
        let (name, args) = split(text);
        let name = name.to_ascii_lowercase();
        let name = match name.as_str() { "/usercard" => "/user".to_owned(), "/unreply" => "/cancelreply".to_owned(), _ => name };
        match name.as_str() {
            "/help" | "/commands" | "/latest" | "/cancelreply" if !args.is_empty() => {
                self.command_feedback(format!("{name} takes no arguments. Your draft is kept."),window,cx);
            }
            "/commands" => {self.command_draft("",window,cx);cx.emit(PaneEvent::OpenCommands(self.name.to_string()));}
            "/help" => { self.command_draft("",window,cx); self.open_command_help(window,cx); }
            "/latest" => {
                self.command_draft("",window,cx); self.close_search(window,cx);
                self.scroller.update(cx,|s,cx|s.scroll_to_end(cx));
            }
            "/cancelreply" => { self.command_draft("",window,cx); self.cancel_reply(window,cx); self.command_feedback("Reply target removed",window,cx); }
            "/find" => {
                if args.chars().count() > 256 { self.command_feedback("Search is limited to 256 characters. Your draft is kept.",window,cx); }
                else { self.command_draft("",window,cx); self.set_search_query(args.to_owned(),window,cx); self.open_search(window,cx); }
            }
            "/user" | "/reply" => {
                let (raw_login, body) = split(args);
                let login = chat_core::twitch_login(raw_login.strip_prefix('@').unwrap_or(raw_login));
                if login.is_none() || (name == "/user" && !body.is_empty()) {
                    self.command_feedback(if name == "/user" {"Use /user login. Opens a retained chatter's card."} else {"Use /reply login message. Prepares a reply without sending."},window,cx);
                    return true;
                }
                let login=login.unwrap();
                let message=self.timeline.borrow().messages().iter().rev().find(|m|
                    m.login.as_deref().is_some_and(|s|s.eq_ignore_ascii_case(&login)) && !m.user_id.is_empty()
                    && (name != "/reply" || (!m.deleted && m.replyable))
                ).cloned();
                let Some(message)=message else {
                    self.command_feedback(format!("No {} from @{login} in this channel's retained history. Your draft is kept.",if name=="/reply"{"replyable message"}else{"message"}),window,cx);return true;
                };
                if name == "/reply" {
                    if body.chars().count()>500 {self.command_feedback("Reply must fit 500 characters. Your draft is kept.",window,cx);return true;}
                    self.begin_reply(&message,window,cx); self.command_draft(body,window,cx);
                    self.command_feedback("Reply prepared. Review it, then press Send.",window,cx);
                } else { self.command_draft("",window,cx); self.message_action(&message.id,Action::Inspect,window,cx); }
            }
            _ => {
                let template=self.custom_commands.borrow().get(name.trim_start_matches('/')).cloned();
                if let Some(template)=template {
                    match crate::custom_commands::expand(&template,&self.name,args){
                        Ok(value)=>{self.command_draft(&value,window,cx);self.command_feedback("Message prepared. Review it, then press Send.",window,cx);}
                        Err(error)=>self.command_feedback(error,window,cx),
                    }
                }else{self.command_feedback(format!("{name} isn't supported here yet. Nothing was sent; your draft is kept. Use /help for local commands."),window,cx);}
            },
        }
        true
    }
    pub fn open_command_help(&self, window: &mut Window, cx: &mut Context<Self>) {
        let owner=cx.entity().downgrade();
        window.open_dialog(cx,move|dialog,window,_cx| {
            let width=(f32::from(window.viewport_size().width)-24.).clamp(180.,500.);
            let height=(f32::from(window.viewport_size().height)-160.).clamp(60.,440.);
            dialog.w(px(width)).title("Composer commands")
                .child(div().id("composer-command-list").v_flex().gap_2().max_h(px(height)).overflow_y_scroll()
                    .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("These commands act locally. Choosing one inserts it; Enter runs it. Moderation, whispers and other unsupported commands are blocked rather than posted to chat."))
                    .children(COMMANDS.iter().map(|command| {
                        let owner=owner.clone(); let name=command.name;
                        div().v_flex().gap_1().p_2().bg(rgb(theme::CONTROL)).rounded(px(4.))
                            .child(Button::new(SharedString::from(format!("command-{name}"))).small().label(command.usage).on_click(move|_,w,cx| {
                                w.close_dialog(cx); let _=owner.update(cx,|p,cx|p.command_draft(&format!("{name} "),w,cx));
                            }))
                            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(command.description))
                    })))
        });
    }
}
