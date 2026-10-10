//! Public chat restrictions, not the signed-in viewer's exemption/entitlement.
use serde_json::Value;
#[derive(Clone,Debug)]
pub struct RoomSettings {pub emote:bool,pub followers:Option<u64>,pub slow:Option<u64>,pub subscribers:bool,pub unique:bool}
impl RoomSettings {
    pub fn parse(v:&Value)->Option<Self>{
        Some(Self{emote:v["emote_mode"].as_bool()?,subscribers:v["subscriber_mode"].as_bool()?,unique:v["unique_chat_mode"].as_bool()?,
            followers:if v["follower_mode"].as_bool()?{Some(v["follower_mode_duration_minutes"].as_u64().or_else(||v["follower_mode_duration"].as_u64()).unwrap_or(0))}else{None},
            slow:if v["slow_mode"].as_bool()?{Some(v["slow_mode_wait_time_seconds"].as_u64().or_else(||v["slow_mode_wait_time"].as_u64()).unwrap_or(0))}else{None}})
    }
    pub fn labels(&self)->Vec<String>{
        let mut labels=vec![];
        if self.emote{labels.push("Emote-only".into());}
        if self.subscribers{labels.push("Subscriber-only".into());}
        if let Some(minutes)=self.followers{labels.push(if minutes==0{"Followers-only".into()}else{format!("Followers · {minutes} min")});}
        if let Some(seconds)=self.slow{labels.push(format!("Slow mode · {seconds}s"));}
        if self.unique{labels.push("Unique messages".into());}
        labels
    }
    pub fn summary(&self)->String{let labels=self.labels();if labels.is_empty(){"No public room restrictions".into()}else{labels.join(" · ")}}
}
