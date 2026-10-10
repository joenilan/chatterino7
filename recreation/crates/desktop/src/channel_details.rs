//! Compact on-demand metadata. Shares the existing stream-status request/cache.
use super::*;
impl Workbench {
    pub(super) fn open_focused_channel_details(&mut self,window:&mut Window,cx:&mut Context<Self>){
        let visible=self.visible_panes(cx);
        let focused=visible.iter().find(|p|{let p=p.read(cx);p.focus.contains_focused(window,cx)||p.draft.read(cx).focus_handle(cx).is_focused(window)||p.search.input.read(cx).focus_handle(cx).is_focused(window)}).map(|p|p.read(cx).name.to_string());
        let channel=focused.or_else(||self.selected_channel.clone().filter(|n|visible.iter().any(|p|p.read(cx).name.as_ref()==n))).or_else(||visible.first().map(|p|p.read(cx).name.to_string()));
        if let Some(channel)=channel{self.open_channel_details(channel,window,cx);}
    }

    pub(super) fn open_channel_details(&mut self,channel:String,window:&mut Window,cx:&mut Context<Self>){
        let owner=cx.entity().downgrade();
        window.open_dialog(cx,move|dialog,window,cx|{
            let Some(view)=owner.upgrade()else{return dialog.title("Workspace closed");};
            let this=view.read(cx);
            let detail=this.streams.detail(&channel);
            let live=detail.map(|(info,_)|info.live);
            let chat=this.panes().into_iter().find(|p|p.read(cx).name.as_ref()==channel).map(|p|p.read(cx).connection.clone()).unwrap_or_else(||"Channel is no longer open".into());
            let room=this.panes().into_iter().find(|p|p.read(cx).name.as_ref()==channel).and_then(|p|p.read(cx).room_settings.clone());
            let width=(f32::from(window.viewport_size().width)-24.).min(410.);
            let max_height=(f32::from(window.viewport_size().height)-150.).max(72.);
            let mut body=div().id("channel-details-body").v_flex().gap_3().min_w_0().max_h(px(max_height)).overflow_y_scroll()
                .child(div().h_flex().gap_2().child(stream_marker(&channel,live)).child(match live{Some(true)=>"Live broadcast",Some(false)=>"Broadcast offline",None=>"Broadcast status unknown"}))
                .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child(format!("Chat · {chat}")));
            if let Some((info,age))=detail {
                if info.live {
                    body=body.child(div().font_weight(FontWeight::SEMIBOLD).child(info.title.clone()))
                        .child(div().h_flex().flex_wrap().gap_2().text_color(rgb(0xC5B8E6)).child(info.category.clone()).child(format!("{} viewers",info.viewers)))
                        .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child(format!("Language · {}",info.language)))
                        .child(div().text_size(px(12.)).text_color(rgb(0xB3DCC7)).child(info.uptime().map(|value|format!("Live for {value}")).unwrap_or_else(||"Uptime unavailable".into())))
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(info.local_start().map(|value|format!("Started · {value}")).unwrap_or_else(||"Start time unavailable".into())))
                        .child(Button::new("copy-stream-title").ghost().small().label("Copy stream title").on_click({let title=info.title.clone();move|_,window,cx|{cx.write_to_clipboard(ClipboardItem::new_string(title.clone()));window.push_notification(Notification::info("Stream title copied"),cx);}}));
                }
                body=body.child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).child(format!("Snapshot {age}s ago · refreshes about every minute")));
            }else{body=body.child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Waiting for current Twitch metadata. An unavailable status does not mean chat is disconnected."));}
            body=body.child(div().border_t_1().border_color(rgb(theme::BORDER)).pt_2().text_size(px(12.)).child(room.as_ref().map(|r|r.summary()).unwrap_or_else(||"Room settings not available yet".into())))
                .child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).child("Room modes are public restrictions. Your role or subscription may grant exemptions; Twitch validates each send."));
            dialog.w(px(width)).title(format!("#{channel}")).child(body)
                .footer(div().h_flex().justify_end().child(Button::new("channel-details-done").label("Done").on_click(|_,window,cx|window.close_dialog(cx))))
        });
    }
}
