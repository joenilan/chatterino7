//! Session-local, retention-bounded attention. References never own message bodies.
use std::{collections::{BTreeSet,BTreeMap},rc::Rc,time::Instant};
use chat_core::{Message,Fragment,Timeline};
#[derive(Default)]
pub struct Attention {
    pub unread:BTreeSet<u64>,
    pub highlights:Rc<BTreeSet<u64>>,
    pub colors:Rc<BTreeMap<u64,usize>>,
    pub arrivals:BTreeMap<u64,Instant>,
    identity:Option<(String,String)>,
    terms:Vec<String>,
    rules:Vec<crate::highlight_rules::Rule>,
    channel:String,
    since:u64,
}
pub fn terms(text:&str)->Vec<String>{
    let mut result=Vec::new();
    for item in text.split(',').map(str::trim).filter(|s|!s.is_empty()) {
        let item=item.chars().take(40).collect::<String>().to_lowercase();
        if !result.contains(&item){result.push(item);}
        if result.len()==8{break;}
    } result
}
impl Attention {
    fn color(&self,message:&Message)->Option<usize>{
        if message.deleted{return None;}
        if let Some((user,_))=&self.identity {
            if &message.user_id==user{return None;}
        }
        if let Some(color)=crate::highlight_rules::first_color(&self.rules,message,&self.channel){return Some(color);}
        if self.identity.as_ref().is_some_and(|(user,_)|message.mentions.iter().any(|id|id==user)||message.reply.as_ref().is_some_and(|r|&r.parent_user_id==user)){return Some(0);}
        let matches=|text:&str|{let folded=text.to_lowercase();self.identity.as_ref().is_some_and(|(_,login)|crate::highlight_rules::normalized_match(&folded,login,true))||self.terms.iter().any(|term|crate::highlight_rules::normalized_match(&folded,term,true))};
        let mut text=String::new();
        for fragment in &message.fragments {
            match fragment {Fragment::Text(part)=>text.push_str(part),_=>{if matches(&text){return Some(0);}text.clear();}}
        }
        matches(&text).then_some(0)
    }
    pub fn configure(&mut self,identity:Option<(String,String)>,terms:&[String],rules:&[crate::highlight_rules::Rule],channel:&str,timeline:&Timeline,first:u64,next:u64)->bool{
        let changed=self.identity!=identity||self.terms!=terms||self.rules!=rules||self.channel!=channel;
        if self.identity!=identity {self.identity=identity;self.since=next;self.unread.clear();Rc::make_mut(&mut self.highlights).clear();Rc::make_mut(&mut self.colors).clear();}
        if changed{self.terms=terms.to_vec();self.rules=rules.to_vec();self.channel=channel.into();self.reconcile(timeline,first);}
        changed
    }
    pub fn appended(&mut self,message:&Message,row:u64,first:u64){
        self.unread.insert(row);self.arrivals.insert(row,Instant::now());
        if row>=self.since {if let Some(color)=self.color(message){Rc::make_mut(&mut self.highlights).insert(row);Rc::make_mut(&mut self.colors).insert(row,color);}}
        self.prune(first);
    }
    fn prune(&mut self,first:u64){
        while self.arrivals.first_key_value().is_some_and(|(row,_)|*row<first){self.arrivals.pop_first();}
        while self.unread.first().is_some_and(|row|*row<first){self.unread.pop_first();}
        let colors=Rc::make_mut(&mut self.colors);while colors.first_key_value().is_some_and(|(row,_)|*row<first){colors.pop_first();}
        let h=Rc::make_mut(&mut self.highlights);while h.first().is_some_and(|row|*row<first){h.pop_first();}
    }
    pub fn reconcile(&mut self,timeline:&Timeline,first:u64){
        self.prune(first);
        let mut highlights=BTreeSet::new();let mut colors=BTreeMap::new();
        for (index,message) in timeline.messages().iter().enumerate(){
            let row=first+index as u64;
            if message.deleted {self.unread.remove(&row);}
            else if row>=self.since {if let Some(color)=self.color(message){highlights.insert(row);colors.insert(row,color);}}
        }
        self.highlights=Rc::new(highlights);self.colors=Rc::new(colors);
    }
    pub fn mark_through(&mut self,row:u64)->bool{
        let before=self.unread.len();self.unread.retain(|r|*r>row);before!=self.unread.len()
    }
    pub fn mark_all(&mut self)->bool{let changed=!self.unread.is_empty();self.unread.clear();changed}
    pub fn counts(&self)->(usize,usize){(self.unread.len(),self.unread.intersection(&self.highlights).count())}
}
pub fn count(value:usize)->String{if value>99{"99+".into()}else{value.to_string()}}
