//! Content-sized hover targets and cached emote previews; no hover-time downloads.
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::component::StyledExt;
use std::sync::Arc;
use crate::{media::DecodedMedia,theme};
#[derive(Clone)]
pub enum Content {
    Hint(String),
    Emote {label:String,source:String,images:Vec<Arc<DecodedMedia>>},
    Gif {label:String,source:String,images:Vec<Arc<DecodedMedia>>},
}
pub struct Card(pub Content);
impl Render for Card {
    fn render(&mut self,window:&mut Window,_cx:&mut Context<Self>)->impl IntoElement{
        match &self.0 {
            Content::Hint(text)=>div().p_2().max_w(px(300.)).font_family("Segoe UI").text_color(rgb(theme::TEXT)).text_size(px(11.)).bg(rgb(theme::ELEVATED)).border_1().border_color(rgb(theme::BORDER)).rounded(px(5.)).child(text.clone()).into_any_element(),
            Content::Emote{label,source,images}|Content::Gif{label,source,images}=>{
                let gif=matches!(&self.0,Content::Gif{..});
                let width=(f32::from(window.viewport_size().width)-24.).clamp(80.,if gif{336.}else{180.});
                let height=(f32::from(window.viewport_size().height)-100.).clamp(48.,if gif{224.}else{112.});
                let layers=images.clone();
                let preview=canvas(|bounds,_,_|bounds,move|bounds,_,window,cx|{
                    for image in &layers {
                        let frame=image.frame(cx.reduce_motion());
                        if image.image.frame_count()>1 && !cx.reduce_motion(){window.request_animation_frame();}
                        let fitted=ObjectFit::Contain.get_bounds(bounds,image.image.size(frame));
                        let _=window.paint_image(bounds,fitted,Corners::all(px(0.)),image.image.clone(),frame,false);
                    }
                }).w(px(width-16.)).h(px(height)).flex_shrink_0();
                div().v_flex().items_center().gap_1().p_2().w(px(width)).font_family("Segoe UI").text_color(rgb(theme::TEXT)).bg(rgb(theme::ELEVATED)).border_1().border_color(rgb(theme::BORDER)).rounded(px(5.))
                    .child(div().relative().w(px(width-16.)).h(px(height)).flex_shrink_0().child(preview).when(images.is_empty(),|el|el.child(div().absolute().inset_0().flex().items_center().justify_center().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Preview not loaded"))))
                    .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(label.clone()))
                    .child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).child(source.clone())).into_any_element()
            }
        }
    }
}
pub fn region(id:String,bounds:Bounds<Pixels>,content:Content,window:&mut Window,cx:&mut App)->AnyElement{
    let mut element=div().id(SharedString::from(id)).w(bounds.size.width).h(bounds.size.height)
        .tooltip(move|_,cx|cx.new(|_|Card(content.clone())).into()).into_any_element();
    element.prepaint_as_root(bounds.origin,size(AvailableSpace::Definite(bounds.size.width),AvailableSpace::Definite(bounds.size.height)),window,cx);
    element
}
pub fn source(provider:&str)->&'static str {match provider{"twitch"=>"Twitch Emote","7tv"=>"7TV Emote","bttv"=>"BetterTTV Emote","ffz"=>"FrankerFaceZ Emote","twitch_cheer"=>"Twitch Cheermote",_=>"Emote"}}
pub struct HoverState {pub hitbox:Hitbox,pub regions:Vec<AnyElement>}
