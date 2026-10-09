//! Persisted recursive docking geometry. Channel entities and credentials live elsewhere.
use serde_json::{Value, json};
#[derive(Clone)]
pub enum Dock {
    Leaf(String),
    Deck { channels: Vec<String>, active: String },
    Split { vertical: bool, weights: Vec<f32>, children: Vec<Dock> },
}
impl Dock {
    pub fn legacy(channels: &[String], vertical: bool, sizes: &[f32]) -> Option<Self> {
        match channels.len() {
            0 => None,
            1 => Some(Self::Leaf(channels[0].clone())),
            _ => Some(Self::Split { vertical, weights: sizes.to_vec(), children: channels.iter().cloned().map(Self::Leaf).collect() }),
        }
    }
    pub fn names(&self, names: &mut Vec<String>) {
        match self { Self::Leaf(name) => names.push(name.clone()), Self::Deck {channels,..}=>names.extend(channels.iter().cloned()), Self::Split {children,..} => for child in children {child.names(names);} }
    }
    pub fn from_json(value: &Value) -> Option<Self> {
        if let Some(name)=value["channel"].as_str() {return Some(Self::Leaf(name.to_owned()));}
        if let Some(items)=value["tabs"].as_array(){let channels=items.iter().map(|v|v.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>()?;let active=value["active"].as_str().filter(|s|channels.iter().any(|n|n==s)).map(str::to_owned).or_else(||channels.first().cloned())?;return Some(Self::Deck{channels,active});}
        let children=value["children"].as_array()?.iter().map(Self::from_json).collect::<Option<Vec<_>>>()?;
        if children.is_empty() {return None;}
        let weights=value["weights"].as_array().map(|a|a.iter().map(|n|n.as_f64().filter(|n|n.is_finite() && *n>0.).unwrap_or(1.) as f32).collect()).unwrap_or_default();
        Some(Self::Split {vertical:value["vertical"].as_bool().unwrap_or(false),weights,children})
    }
    pub fn json(&self) -> Value {
        match self { Self::Leaf(name)=>json!({"channel":name}), Self::Deck{channels,active}=>json!({"tabs":channels,"active":active}), Self::Split{vertical,weights,children}=>json!({"vertical":vertical,"weights":weights,"children":children.iter().map(Self::json).collect::<Vec<_>>()}) }
    }
    pub fn remove(self, name: &str) -> Option<Self> {
        match self {
            Self::Leaf(n)=> (n!=name).then_some(Self::Leaf(n)),
            Self::Deck {mut channels,mut active}=>{channels.retain(|n|n!=name);if active==name {active=channels.first().cloned().unwrap_or_default();}match channels.len(){0=>None,1=>Some(Self::Leaf(channels.remove(0))),_=>Some(Self::Deck{channels,active})}},
            Self::Split {vertical,weights,children}=> {
                let mut kept=Vec::new();let mut sizes=Vec::new();
                for (ix,child) in children.into_iter().enumerate() {if let Some(child)=child.remove(name) {kept.push(child);sizes.push(weights.get(ix).copied().unwrap_or(1.));}}
                match kept.len() {0=>None,1=>kept.pop(),_=>Some(Self::Split{vertical,weights:sizes,children:kept})}
            }
        }
    }
    pub fn insert(&mut self, target: &str, name: String, vertical: bool, first: bool) -> bool {
        match self {
            node if node.contains_direct(target) => {
                let old=node.clone();let new=Self::Leaf(name);
                *node=Self::Split {vertical,weights:vec![1.,1.],children:if first {vec![new,old]}else{vec![old,new]}};true
            }
            Self::Leaf(_)|Self::Deck{..}=>false,
            Self::Split {vertical:axis,weights,children}=> {
                if *axis==vertical {
                    if let Some(ix)=children.iter().position(|c|c.contains_direct(target)) {
                        let at=ix+usize::from(!first);
                        while weights.len()<children.len(){weights.push(1.);}
                        let share=weights[ix]/2.;weights[ix]=share;
                        children.insert(at,Self::Leaf(name));weights.insert(at,share);return true;
                    }
                }
                children.iter_mut().any(|c|c.insert(target,name.clone(),vertical,first))
            }
        }
    }
    pub fn append(self, name: String, vertical: bool, first: bool) -> Self {
        match self {
            Self::Split {vertical:axis,mut weights,mut children} if axis==vertical=>{
                while weights.len()<children.len(){weights.push(1.);}
                let weight=weights.iter().sum::<f32>()/weights.len().max(1) as f32;
                let at=if first{0}else{children.len()};children.insert(at,Self::Leaf(name));weights.insert(at,weight);
                Self::Split{vertical,weights,children}
            }
            old=>Self::Split{vertical,weights:vec![1.,1.],children:if first{vec![Self::Leaf(name),old]}else{vec![old,Self::Leaf(name)]}},
        }
    }
    pub fn resize(&mut self,path:&[usize],sizes:Vec<f32>) {
        if let Self::Split {weights,children,..}=self {
            if let Some((ix,rest))=path.split_first() {if let Some(child)=children.get_mut(*ix){child.resize(rest,sizes);}}
            else if sizes.len()==children.len() && sizes.iter().all(|n|n.is_finite() && *n>0.) {*weights=sizes;}
        }
    }
    pub fn rotate(&mut self) {if let Self::Split {vertical,..}=self {*vertical=!*vertical;}}
    fn contains_direct(&self,name:&str)->bool {match self{Self::Leaf(n)=>n==name,Self::Deck{channels,..}=>channels.iter().any(|n|n==name),_=>false}}
    pub fn companion(&self,name:&str)->Option<String>{match self{Self::Deck{channels,..} if channels.iter().any(|n|n==name)=>channels.iter().find(|n|n.as_str()!=name).cloned(),Self::Split{children,..}=>children.iter().find_map(|c|c.companion(name)),_=>None}}
    pub fn active_names(&self,names:&mut Vec<String>){match self{Self::Leaf(n)=>names.push(n.clone()),Self::Deck{active,..}=>names.push(active.clone()),Self::Split{children,..}=>for c in children{c.active_names(names);}}}
    pub fn select(&mut self,name:&str)->bool {match self{Self::Deck{channels,active} if channels.iter().any(|n|n==name)=>{*active=name.to_owned();true},Self::Split{children,..}=>children.iter_mut().any(|c|c.select(name)),_=>false}}
    pub fn tabify(&mut self,target:&str,name:String)->bool {
        match self {
            Self::Leaf(n) if n==target=>{*self=Self::Deck{channels:vec![n.clone(),name.clone()],active:name};true},
            Self::Deck{channels,active} if channels.iter().any(|n|n==target)=>{channels.push(name.clone());*active=name;true},
            Self::Split{children,..}=>children.iter_mut().any(|c|c.tabify(target,name.clone())),_=>false,
        }
    }
    pub fn minimum(&self)->(f32,f32) {
        match self {
            Self::Leaf(_)|Self::Deck{..}=>(240.,212.),
            Self::Split {vertical,children,..}=>children.iter().map(Self::minimum).fold((0.,0.),|(w,h),(cw,ch)|if *vertical {(w.max(cw),h+ch)}else{(w+cw,h.max(ch))}),
        }
    }
}
