//! Observed 7TV paints: definitions never imply an active sender entitlement.
use crate::seven_entitlements::Change;
use serde_json::Value;
use std::{collections::HashMap,time::{Duration,Instant}};
#[derive(Clone,Debug,PartialEq)]
pub struct Stop{pub at:f64,pub rgba:u32}
#[derive(Clone,Debug,PartialEq)]
pub enum Brush{Linear{angle:f64},Radial{ellipse:bool},Solid,Unsupported}
#[derive(Clone,Debug,PartialEq)]
pub struct Paint{
    pub id:String,pub name:String,pub brush:Brush,pub color:Option<u32>,
    pub stops:Vec<Stop>,pub repeat:bool,pub limitation:Option<String>,
}
fn packed(value:&Value)->Option<u32>{
    value.as_i64().filter(|n|*n>=i32::MIN as i64&&*n<=u32::MAX as i64).map(|n|n as u32)
}
pub fn definition(data:&Value)->Option<Paint>{
    let id=data["id"].as_str().filter(|id|!id.is_empty()&&id.len()<=128&&id.bytes().all(|b|b.is_ascii_alphanumeric()))?.to_owned();
    let name=data["name"].as_str().unwrap_or("Unnamed paint").chars().filter(|c|!c.is_control()).take(80).collect();
    let color=packed(&data["color"]);let repeat=data["repeat"].as_bool().unwrap_or(false);
    let mut paint=Paint{id,name,color,repeat,brush:Brush::Unsupported,stops:Vec::new(),limitation:None};
    let unsupported=|mut p:Paint,reason:&str|{p.brush=Brush::Unsupported;p.limitation=Some(reason.into());p};
    if data["gradients"].as_array().is_some_and(|a|!a.is_empty())||!data["text"].is_null(){
        return Some(unsupported(paint,"Layered or styled-text paint"));
    }
    if data["shadows"].as_array().is_some_and(|a|!a.is_empty()){
        return Some(unsupported(paint,"Paint includes shadows"));
    }
    let function=data["function"].as_str().unwrap_or("");
    paint.brush=match function{
        "LINEAR_GRADIENT"|"linear-gradient"=>{
            let angle=data["angle"].as_f64().unwrap_or(0.);
            if !angle.is_finite(){return Some(unsupported(paint,"Invalid gradient angle"));}
            Brush::Linear{angle:angle.rem_euclid(360.)}
        },
        "RADIAL_GRADIENT"|"radial-gradient"=>Brush::Radial{ellipse:data["shape"].as_str()==Some("ellipse")},
        "URL"|"url"=>return Some(unsupported(paint,"Image or animated paint")),
        _=>return Some(unsupported(paint,"Unknown paint function")),
    };
    if let Some(stops)=data["stops"].as_array(){
        if stops.len()>32{return Some(unsupported(paint,"More than 32 gradient stops"));}
        for stop in stops{
            let Some(at)=stop["at"].as_f64().filter(|at|at.is_finite()&&(0.0..=1.0).contains(at))else{return Some(unsupported(paint,"Invalid gradient stop"));};
            let Some(rgba)=packed(&stop["color"])else{return Some(unsupported(paint,"Invalid gradient color"));};
            if paint.stops.last().is_some_and(|last|last.at>at){return Some(unsupported(paint,"Unordered gradient stops"));}
            paint.stops.push(Stop{at,rgba});
        }
    }
    if paint.stops.is_empty(){
        if color.is_some()&&matches!(paint.brush,Brush::Linear{..}){paint.brush=Brush::Solid;}
        else{return Some(unsupported(paint,"Missing gradient stops"));}
    }
    if repeat&&!matches!(paint.brush,Brush::Solid)&&paint.stops.last().unwrap().at-paint.stops.first().unwrap().at<=f64::EPSILON{
        return Some(unsupported(paint,"Zero-width repeating gradient"));
    }
    Some(paint)
}
#[derive(Default)]
pub struct Paints{
    previews:HashMap<String,std::sync::Arc<gpui_kit::RenderImage>>,
    definitions:HashMap<String,(Paint,Instant)>,users:HashMap<String,(String,Instant)>,last_expiry:Option<Instant>,
}
impl Paints{
    pub fn apply(&mut self,event:&Change)->bool{match event{
        Change::Reset=>{let changed=!self.users.is_empty();self.users.clear();self.definitions.clear();self.previews.clear();changed},
        Change::PaintDefinition(paint)=>{
            if !self.definitions.contains_key(&paint.id)&&self.definitions.len()>=256{
                let victim=self.definitions.iter().filter(|(id,_)|!self.users.values().any(|(current,_)|current==*id)).min_by_key(|(_,(_,at))|*at).map(|(id,_)|id.clone());
                let Some(victim)=victim else{return false;};self.definitions.remove(&victim);self.previews.remove(&victim);
            }
            let changed=self.definitions.get(&paint.id).is_none_or(|(old,_)|old!=paint);
            if changed{self.previews.remove(&paint.id);if let Some(preview)=paint.preview(){self.previews.insert(paint.id.clone(),preview);}}
            self.definitions.insert(paint.id.clone(),(paint.clone(),Instant::now()));
            changed&&self.users.values().any(|(id,_)|id==&paint.id)
        },
        Change::PaintGrant{users,paint,remove}=>{
            let mut changed=false;
            for user in users{
                if *remove{if self.users.get(user).is_some_and(|(id,_)|id==paint){self.users.remove(user);changed=true;}}
                else if self.users.contains_key(user)||self.users.len()<2048{
                    changed|=self.users.get(user).is_none_or(|(id,_)|id!=paint);
                    self.users.insert(user.clone(),(paint.clone(),Instant::now()));
                }
            }changed
        },
        _=>false
    }}
    pub fn expire(&mut self)->bool{
        if self.last_expiry.is_some_and(|at|at.elapsed()<Duration::from_secs(10)){return false;}
        self.last_expiry=Some(Instant::now());let before=self.users.len();
        self.users.retain(|_,(_,at)|at.elapsed()<Duration::from_secs(1800));before!=self.users.len()
    }
    pub fn for_user(&self,user:&str)->Option<&Paint>{self.users.get(user).and_then(|(id,_)|self.definitions.get(id)).map(|(paint,_)|paint)}
    pub fn preview_for_user(&self,user:&str)->Option<std::sync::Arc<gpui_kit::RenderImage>>{self.users.get(user).and_then(|(id,_)|self.previews.get(id)).cloned()}
    pub fn pending_for_user(&self,user:&str)->bool{self.users.contains_key(user)&&self.for_user(user).is_none()}
    pub fn inspection(&self)->Value{serde_json::json!({
        "definitions":self.definitions.len(),"sender_grants":self.users.len(),
        "unresolved_grants":self.users.values().filter(|(id,_)|!self.definitions.contains_key(id)).count(),
        "unsupported_definitions":self.definitions.values().filter(|(p,_)|p.limitation.is_some()).count(),
        "name_rendering":"pending glyph-mask integration","scope":"observed session entitlements only"
    })}
}

impl Paint{
    /// Opaque RGB swatch over a chosen base; glyph coverage is a separate renderer concern.
    pub fn sample(&self,x:f64,y:f64,width:f64,height:f64,base:u32)->Option<u32>{
        if width<=0.||height<=0.||![x,y,width,height].iter().all(|n|n.is_finite()){return None;}
        let overlay=|rgba:u32|{
            let alpha=(rgba&255)as f64/255.;
            let mut rgb=0;
            for shift in [16,8,0]{let fg=((rgba>>(shift+8))&255)as f64;let bg=((base>>shift)&255)as f64;rgb|=((fg*alpha+bg*(1.-alpha)).round()as u32)<<shift;}
            rgb
        };
        let mut t=match self.brush{
            Brush::Solid=>return self.color.map(overlay),
            Brush::Unsupported=>return None,
            Brush::Linear{angle}=>{
                let a=angle.to_radians();let dx=a.sin();let dy=-a.cos();
                let span=width*dx.abs()+height*dy.abs();
                ((x-width/2.)*dx+(y-height/2.)*dy)/span+0.5
            },
            Brush::Radial{ellipse}=>{
                let (rx,ry)=if ellipse{(width/2.,height/2.)}else{let r=width.max(height)/2.;(r,r)};
                (((x-width/2.)/rx).powi(2)+((y-height/2.)/ry).powi(2)).sqrt()
            }
        };
        let first=self.stops.first()?;let last=self.stops.last()?;
        if self.repeat{let span=last.at-first.at;if span<=f64::EPSILON{return None;}t=(t-first.at).rem_euclid(span)+first.at;}
        if t<first.at{return Some(overlay(first.rgba));}
        let right=self.stops.partition_point(|stop|stop.at<=t);
        if right==self.stops.len(){return Some(overlay(last.rgba));}
        let a=&self.stops[right-1];let b=&self.stops[right];let ratio=(t-a.at)/(b.at-a.at);
        let (a,b)=(overlay(a.rgba),overlay(b.rgba));let mut rgb=0;
        for shift in [16,8,0]{let a=((a>>shift)&255)as f64;let b=((b>>shift)&255)as f64;rgb|=((a+(b-a)*ratio).round()as u32)<<shift;}
        Some(rgb)
    }
    fn preview(&self)->Option<std::sync::Arc<gpui_kit::RenderImage>>{
        let (width,height)=(192,32);let mut pixels=Vec::with_capacity(width*height*4);
        for y in 0..height{for x in 0..width{
            let rgb=self.sample(x as f64+0.5,y as f64+0.5,width as f64,height as f64,crate::theme::CANVAS)?;
            pixels.extend_from_slice(&[(rgb&255)as u8,((rgb>>8)&255)as u8,((rgb>>16)&255)as u8,255]);
        }}
        let buffer=image::RgbaImage::from_raw(width as u32,height as u32,pixels)?;
        Some(std::sync::Arc::new(gpui_kit::RenderImage::new(vec![image::Frame::new(buffer)])))
    }
}
