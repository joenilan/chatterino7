//! Room cosmetics only. Twitch message badges remain the authority for roles.
use crate::{media::EmoteKey, twitch_assets::Badge};
#[derive(Clone, Default)]
pub struct Roles {
    pub moderator: Option<Badge>,
    pub vip: Option<Badge>,
    pub grants: crate::ffz_badges::Grants,
    pub grants_loaded: Option<std::time::Instant>,
}
impl Roles {
    pub fn get(&self, role: &str) -> Option<&Badge> {
        match role {
            "moderator" => self.moderator.as_ref(),
            "vip" => self.vip.as_ref(),
            _ => None,
        }
    }
    pub fn parse(value: &serde_json::Value) -> Self {
        let badge = |field: &str, role: &str, title: &str| {
            let urls = &value["room"][field];
            ["2", "1", "4"].into_iter().find_map(|scale| {
                let raw = urls[scale].as_str()?;
                let url = if raw.starts_with("//") {
                    format!("https:{raw}")
                } else {
                    raw.into()
                };
                Some(Badge {
                    title: format!("{title} · FFZ channel badge"),
                    key: EmoteKey::ffz_role_badge(role, &url)?,
                })
            })
        };
        Self {
            moderator: badge("mod_urls", "mod", "Moderator"),
            vip: badge("vip_badge", "vip", "VIP"),
            grants: crate::ffz_badges::grants(&value["room"]["user_badge_ids"]).unwrap_or_default(),
            grants_loaded: Some(std::time::Instant::now()),
        }
    }
}
