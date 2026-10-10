//! Typed Twitch presentation. No purchases, private APIs or credentialed media.
use crate::{media::{EmoteKey,MediaCache}, theme};
use chat_core::{Fragment,Message};
use gpui_kit::{component::StyledExt, prelude::FluentBuilder, *};

pub fn accent(message:&Message)->Option<u32>{
    if message.deleted{return None;}
    match message.presentation.kind.as_str(){
        "channel_points_highlighted"|"channel_points_sub_only"=>Some(0xA99CF4),
        "user_intro"=>Some(0x91D7BA),
        "power_ups_message_effect"|"power_ups_gigantified_emote"=>Some(0xE8C68A),
        _ if message.presentation.bits>0=>Some(0xCAB3ED),
        _=>None,
    }
}
pub fn heading(message:&Message)->Option<AnyElement>{
    if message.deleted{return None;}
    let p=&message.presentation;
    let mut labels=Vec::new();
    match p.kind.as_str(){
        "channel_points_highlighted"=>labels.push("Highlighted reward".to_owned()),
        "channel_points_sub_only"=>labels.push("Subscriber reward".to_owned()),
        "user_intro"=>labels.push("Introduction".to_owned()),
        "power_ups_message_effect"=>labels.push("Bits · Message effect".to_owned()),
        "power_ups_gigantified_emote"=>labels.push("Bits · Gigantified emote".to_owned()),
        ""|"text"=>{},
        _=>labels.push("Twitch special message".to_owned()),
    }
    if let Some(kind)=&p.notice_type {labels.push(kind.replace('_'," "));}
    if p.bits>0 {labels.push(format!("{} Bits",p.bits));}
    if p.reward_id.is_some() && !p.kind.starts_with("channel_points_"){labels.push("Channel reward".into());}
    if let Some(source)=&p.source{labels.push(format!("From #{source}"));}
    if labels.is_empty()&&p.notice.is_none(){return None;}
    Some(div().v_flex().min_w_0().gap_1().py_1()
        .when(!labels.is_empty(),|el|el.child(div().text_size(px(10.)).text_color(rgb(accent(message).unwrap_or(theme::MUTED))).child(labels.join(" · "))))
        .when_some(p.notice.clone().filter(|_|!message.fragments.is_empty()),|el,text|el.child(div().text_size(px(12.)).text_color(rgb(0xC5B8E6)).child(text)))
        .into_any_element())
}
pub fn attachments(message:&Message,media:&mut MediaCache,cx:&mut App)->Option<AnyElement>{
    if message.deleted{return None;}
    let gifs:Vec<_>=message.fragments.iter().filter_map(|f|if let Fragment::Gif{id,url,..}=f{Some((id,url))}else{None}).take(4).collect();
    if gifs.is_empty(){return None;}
    let cards=gifs.into_iter().enumerate().map(|(i,(id,url))|{
        let mut key=EmoteKey::gif(id,url,!cx.reduce_motion());
        let mut image=key.as_ref().and_then(|k|media.get(k,cx));
        let animation_failure=key.as_ref().and_then(|k|media.failure_label(k));
        if key.as_ref().is_some_and(|k|k.animated&&media.failed(k)){
            if let Some(k)=key.as_mut(){k.animated=false;image=media.get(k,cx);}
        }
        let label=if key.is_none(){"GIF unavailable · unsupported source"}else if key.as_ref().is_some_and(|k|media.failed(k)){"GIF unavailable · size or download limit"}else{"Loading GIF…"};
        let status=if key.is_none(){"GIF unavailable · unsupported source".to_owned()}
            else if cx.reduce_motion(){"Animation paused · reduced motion".to_owned()}
            else if let Some(reason)=animation_failure{format!("Static fallback · {}",reason.replace('_'," "))}
            else if image.as_ref().is_some_and(|i|i.image.frame_count()==1){"Source contains one decoded frame".into()}
            else{"Twitch GIF · GIPHY".into()};
        let hover=crate::media_hover::Content::Gif{label:"Chat GIF".into(),source:status.clone(),images:image.iter().cloned().collect()};
        div().id(SharedString::from(format!("gif-{}-{i}",message.id))).tooltip(move|_,cx|cx.new(|_|crate::media_hover::Card(hover.clone())).into()).w(px(160.)).max_w_full().h(px(112.)).flex_shrink_0().overflow_hidden().rounded(px(4.)).border_1().border_color(rgb(theme::BORDER)).bg(rgb(theme::ELEVATED))
            .child(if let Some(image)=image{
                canvas(|bounds,_,_|bounds,move|bounds,_,window,cx|{
                    let clip=bounds.intersect(&window.content_mask().bounds);
                    if clip.size.width<=px(0.)||clip.size.height<=px(0.){return;}
                    let frame=image.frame(cx.reduce_motion());
                    if image.image.frame_count()>1&&!cx.reduce_motion(){window.request_animation_frame();}
                    let fit=ObjectFit::Contain.get_bounds(bounds,image.image.size(frame));
                    let _=window.paint_image(clip,fit,Corners::all(px(0.)),image.image.clone(),frame,false);
                }).w_full().h(px(110.)).into_any_element()
            }else{div().size_full().flex().items_center().justify_center().p_2().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(label).into_any_element()})
    }).collect::<Vec<_>>();
    Some(div().h_flex().flex_wrap().gap_2().py_1().min_w_0().children(cards).into_any_element())
}

/// Bounded summaries support real-app diagnosis without exposing GIF query URLs.
pub fn inspection(timeline:&chat_core::Timeline)->serde_json::Value {
    let messages=timeline.messages();
    let mut gifs=0;let mut cheers=0;let mut unknown=0;let mut special=0;let mut notices=0;
    for message in messages.iter().filter(|m|!m.deleted){
        special+=usize::from(!matches!(message.presentation.kind.as_str(),""|"text"));
        notices+=usize::from(message.presentation.notice.is_some());
        for fragment in &message.fragments{match fragment{Fragment::Gif{..}=>gifs+=1,Fragment::Cheer{..}=>cheers+=1,Fragment::Unknown{..}=>unknown+=1,_=>{}}}
    }
    serde_json::json!({"gifs":gifs,"cheermotes":cheers,"unknown_fragments":unknown,"special":special,"notices":notices})
}
