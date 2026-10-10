//! Session-local, retention-bounded attention. References never own message bodies.
use std::{collections::{BTreeSet,BTreeMap},rc::Rc,time::Instant};
use chat_core::{Message,Fragment,Timeline};
#[derive(Default)]
pub struct Attention {
    pub unread:BTreeSet<u64>,
    pub highlights:Rc<BTreeSet<u64>>,
    pub arrivals:BTreeMap<u64,Instant>,
    identity:Option<(String,String)>,
    terms:Vec<String>,
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
fn word(c:char)->bool{c.is_alphanumeric()||c=='_'}
fn contains(text:&str,needle:&str)->bool{
    if needle.is_empty(){return false;}
    let text=text.to_lowercase();
    text.match_indices(needle).any(|(at,_)|{
        text[..at].chars().next_back().is_none_or(|c|!word(c))
            && text[at+needle.len()..].chars().next().is_none_or(|c|!word(c))
    })
}
impl Attention {
    pub fn matches(&self,message:&Message)->bool{
        if message.deleted{return false;}
        if let Some((user,_))=&self.identity {
            if &message.user_id==user{return false;}
            if message.mentions.iter().any(|id|id==user)||message.reply.as_ref().is_some_and(|r|&r.parent_user_id==user){return true;}
        }
        let matches=|text:&str|self.identity.as_ref().is_some_and(|(_,login)|contains(text,login))||self.terms.iter().any(|term|contains(text,term));
        let mut text=String::new();
        for fragment in &message.fragments {
            match fragment {Fragment::Text(part)=>text.push_str(part),_=>{if matches(&text){return true;}text.clear();}}
        }
        matches(&text)
    }
    pub fn configure(&mut self,identity:Option<(String,String)>,terms:&[String],timeline:&Timeline,first:u64,next:u64)->bool{
        let changed=self.identity!=identity||self.terms!=terms;
        if self.identity!=identity {self.identity=identity;self.since=next;self.unread.clear();Rc::make_mut(&mut self.highlights).clear();}
        if changed{self.terms=terms.to_vec();self.reconcile(timeline,first);}
        changed
    }
    pub fn appended(&mut self,message:&Message,row:u64,first:u64){
        self.unread.insert(row);self.arrivals.insert(row,Instant::now());
        if row>=self.since&&self.matches(message){Rc::make_mut(&mut self.highlights).insert(row);}
        self.prune(first);
    }
    fn prune(&mut self,first:u64){
        while self.arrivals.first_key_value().is_some_and(|(row,_)|*row<first){self.arrivals.pop_first();}
        while self.unread.first().is_some_and(|row|*row<first){self.unread.pop_first();}
        let h=Rc::make_mut(&mut self.highlights);while h.first().is_some_and(|row|*row<first){h.pop_first();}
    }
    pub fn reconcile(&mut self,timeline:&Timeline,first:u64){
        self.prune(first);
        let mut highlights=BTreeSet::new();
        for (index,message) in timeline.messages().iter().enumerate(){
            let row=first+index as u64;
            if message.deleted {self.unread.remove(&row);}
            else if row>=self.since&&self.matches(message){highlights.insert(row);}
        }
        self.highlights=Rc::new(highlights);
    }
    pub fn mark_through(&mut self,row:u64)->bool{
        let before=self.unread.len();self.unread.retain(|r|*r>row);before!=self.unread.len()
    }
    pub fn mark_all(&mut self)->bool{let changed=!self.unread.is_empty();self.unread.clear();changed}
    pub fn counts(&self)->(usize,usize){(self.unread.len(),self.unread.intersection(&self.highlights).count())}
}
pub fn count(value:usize)->String{if value>99{"99+".into()}else{value.to_string()}}
