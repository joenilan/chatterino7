//! Ephemeral, sender-scoped grants observed on the anonymous EventAPI session.
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub enum Change {
    Reset,
    Grant {
        users: Vec<String>,
        set: String,
        remove: bool,
    },
    Set {
        id: String,
        removed: bool,
    },
}

#[derive(Default)]
pub struct Entitlements {
    users: HashMap<String, BTreeMap<String, Instant>>,
    pub generation: u64,
}
impl Entitlements {
    pub fn apply(&mut self, change: &Change) -> bool {
        match change {
            Change::Reset => {
                self.generation = self.generation.wrapping_add(1);
                let changed = !self.users.is_empty();
                self.users.clear();
                changed
            }
            Change::Grant { users, set, remove } => {
                let mut changed = false;
                // A bounded set pool also coalesces shared-set fetches across senders.
                let known = self.sets();
                if !remove && !known.contains(set) && known.len() >= 256 {
                    return false;
                }
                for user in users {
                    if *remove {
                        if let Some(sets) = self.users.get_mut(user) {
                            changed |= sets.remove(set).is_some();
                        }
                    } else {
                        if !self.users.contains_key(user) && self.users.len() >= 2048 {
                            continue;
                        }
                        let sets = self.users.entry(user.clone()).or_default();
                        if sets.len() < 8 || sets.contains_key(set) {
                            changed |= sets.insert(set.clone(), Instant::now()).is_none();
                        }
                    }
                }
                self.users.retain(|_, sets| !sets.is_empty());
                changed
            }
            Change::Set { id, removed: true } => {
                let mut changed = false;
                for sets in self.users.values_mut() {
                    changed |= sets.remove(id).is_some();
                }
                self.users.retain(|_, sets| !sets.is_empty());
                changed
            }
            _ => false,
        }
    }
    pub fn expire(&mut self) -> bool {
        let before: usize = self.users.values().map(BTreeMap::len).sum();
        // Passive events are not a roster. Missing revocations must not persist forever.
        self.users.retain(|_, sets| {
            sets.retain(|_, seen| seen.elapsed() < Duration::from_secs(1800));
            !sets.is_empty()
        });
        before != self.users.values().map(BTreeMap::len).sum::<usize>()
    }
    pub fn sets(&self) -> BTreeSet<String> {
        self.users
            .values()
            .flat_map(|sets| sets.keys().cloned())
            .collect()
    }
    pub fn for_user(&self, user: &str) -> impl Iterator<Item = &String> {
        self.users
            .get(user)
            .into_iter()
            .flat_map(|sets| sets.keys())
    }
    pub fn len(&self) -> usize {
        self.users.len()
    }
}
