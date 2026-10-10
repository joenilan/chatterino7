//! Public 7TV badge definitions and ephemeral sender grants, never channel-wide.
use crate::{media::EmoteKey, seven_entitlements::Change, twitch_assets::Badge};
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Badges {
    last_expiry: Option<Instant>,
    definitions: HashMap<String, (Badge, Instant)>,
    users: HashMap<String, (String, Instant)>,
}
impl Badges {
    pub fn apply(&mut self, event: &Change) -> bool {
        match event {
            Change::Reset => {
                let changed = !self.users.is_empty();
                self.users.clear();
                self.definitions.clear();
                changed
            }
            Change::BadgeDefinition { id, badge } => {
                if !self.definitions.contains_key(id) && self.definitions.len() >= 256 {
                    let evict = self
                        .definitions
                        .iter()
                        .filter(|(id, _)| !self.users.values().any(|(b, _)| b == *id))
                        .min_by_key(|(_, (_, at))| *at)
                        .map(|(id, _)| id.clone());
                    let Some(evict) = evict else {
                        return false;
                    };
                    self.definitions.remove(&evict);
                }
                let changed = self
                    .definitions
                    .get(id)
                    .is_none_or(|(old, _)| old.title != badge.title || old.key != badge.key);
                self.definitions
                    .insert(id.clone(), (badge.clone(), Instant::now()));
                changed && self.users.values().any(|(b, _)| b == id)
            }
            Change::BadgeGrant {
                users,
                badge,
                remove,
            } => {
                let mut changed = false;
                for user in users {
                    if *remove {
                        if self
                            .users
                            .get(user)
                            .is_some_and(|(current, _)| current == badge)
                        {
                            self.users.remove(user);
                            changed = true;
                        }
                    } else if self.users.contains_key(user) || self.users.len() < 2048 {
                        changed |= self
                            .users
                            .get(user)
                            .is_none_or(|(current, _)| current != badge);
                        self.users
                            .insert(user.clone(), (badge.clone(), Instant::now()));
                    }
                }
                changed
            }
            _ => false,
        }
    }
    pub fn expire(&mut self) -> bool {
        if self
            .last_expiry
            .is_some_and(|at| at.elapsed() < Duration::from_secs(10))
        {
            return false;
        }
        self.last_expiry = Some(Instant::now());
        let before = self.users.len();
        self.users
            .retain(|_, (_, at)| at.elapsed() < Duration::from_secs(1800));
        // The server may not resend known definitions for a later sender grant.
        before != self.users.len()
    }
    pub fn for_user(&self, user: &str) -> Option<&Badge> {
        self.users
            .get(user)
            .and_then(|(id, _)| self.definitions.get(id))
            .map(|(badge, _)| badge)
    }
    pub fn inspection(&self) -> Value {
        serde_json::json!({"definitions":self.definitions.len(),"sender_grants":self.users.len()})
    }
}

pub fn definition(data: &Value) -> Option<(String, Badge)> {
    let id = data["id"].as_str().filter(|s| {
        !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric())
    })?;
    let host = &data["host"];
    let base = host["url"].as_str()?;
    if base.len() > 1024 {
        return None;
    }
    let base = if base.starts_with("//") {
        format!("https:{base}")
    } else {
        base.into()
    };
    let file = host["files"]
        .as_array()?
        .iter()
        .take(128)
        .filter(|file| {
            file["format"]
                .as_str()
                .is_some_and(|f| f.eq_ignore_ascii_case("WEBP"))
                && file["width"].as_u64().is_some_and(|n| n > 0 && n <= 256)
                && file["height"].as_u64().is_some_and(|n| n > 0 && n <= 256)
                && file["size"].as_u64().is_none_or(|n| n <= 2 * 1024 * 1024)
        })
        .min_by_key(|file| file["height"].as_u64().unwrap_or(0).abs_diff(36))?;
    let valid_file = |s: &&str| {
        !s.is_empty()
            && s.len() <= 128
            && !s.contains("..")
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    let name = file["name"].as_str().filter(valid_file)?;
    let static_name = file["static_name"]
        .as_str()
        .filter(valid_file)
        .unwrap_or(name);
    let asset = chat_core::EmoteAsset {
        url: format!("{}/{name}", base.trim_end_matches('/')),
        static_url: format!("{}/{static_name}", base.trim_end_matches('/')),
        width: file["width"].as_u64()? as u16,
        height: file["height"].as_u64()? as u16,
    };
    let key = EmoteKey::seven_badge(id, file["frame_count"].as_u64().unwrap_or(1) > 1, &asset)?;
    let title = data["tooltip"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| data["name"].as_str())
        .unwrap_or("Badge")
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect::<String>();
    Some((
        id.into(),
        Badge {
            title: format!("7TV · {title}"),
            key,
        },
    ))
}
