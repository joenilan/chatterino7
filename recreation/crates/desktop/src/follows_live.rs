//! Followed channels that are live (Helix Get Followed Streams,
//! `user:read:follows`), refreshed every two minutes for the sidebar.
use crate::{auth::CLIENT_ID, live::Identity};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

const REFRESH: Duration = Duration::from_secs(120);
const MAX_PAGES: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct LiveFollow {
    pub login: String,
    pub name: String,
    pub game: String,
    pub title: String,
    pub viewers: u64,
    /// Twitch user ID (for the avatar) and when the stream started (RFC 3339).
    pub user_id: String,
    pub started_at: String,
}
impl LiveFollow {
    pub fn label(&self) -> &str {
        if self.name.is_empty() {
            &self.login
        } else {
            &self.name
        }
    }
    /// Seconds live, if the start time is known.
    pub fn live_seconds(&self) -> Option<i64> {
        let started = chrono::DateTime::parse_from_rfc3339(&self.started_at).ok()?;
        Some(
            (chrono::Utc::now() - started.with_timezone(&chrono::Utc))
                .num_seconds()
                .max(0),
        )
    }
    /// "2h 05m" / "14m".
    pub fn uptime(&self) -> Option<String> {
        let s = self.live_seconds()?;
        Some(if s >= 3600 {
            format!("{}h {:02}m", s / 3600, s % 3600 / 60)
        } else {
            format!("{}m", s / 60)
        })
    }
}

/// How a list of live channels is ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    MostViewers,
    FewestViewers,
    LongestLive,
    JustWentLive,
    Name,
    /// Grouped by category (biggest category first), viewers within.
    Category,
    /// Raids: your current category first, then by viewers.
    SameCategory,
}
impl Sort {
    /// Choices in the sidebar, and in the raid finder.
    pub const SIDEBAR: [Sort; 6] = [
        Sort::MostViewers,
        Sort::FewestViewers,
        Sort::LongestLive,
        Sort::JustWentLive,
        Sort::Name,
        Sort::Category,
    ];
    pub const RAID: [Sort; 4] = [
        Sort::SameCategory,
        Sort::FewestViewers,
        Sort::MostViewers,
        Sort::Name,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Sort::MostViewers => "Most viewers",
            Sort::FewestViewers => "Fewest viewers",
            Sort::LongestLive => "Live longest",
            Sort::JustWentLive => "Just went live",
            Sort::Name => "Name A–Z",
            Sort::Category => "By category",
            Sort::SameCategory => "Your category first",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Sort::MostViewers => "viewers",
            Sort::FewestViewers => "fewest",
            Sort::LongestLive => "longest",
            Sort::JustWentLive => "newest",
            Sort::Name => "name",
            Sort::Category => "category",
            Sort::SameCategory => "same_category",
        }
    }
    pub fn from_key(key: &str, fallback: Sort) -> Sort {
        [
            Sort::MostViewers,
            Sort::FewestViewers,
            Sort::LongestLive,
            Sort::JustWentLive,
            Sort::Name,
            Sort::Category,
            Sort::SameCategory,
        ]
        .into_iter()
        .find(|s| s.key() == key)
        .unwrap_or(fallback)
    }
}

/// Filter by `query` (name, login, category or title) and order by `sort`.
/// `own_game` is your current category, for `Sort::SameCategory`.
pub fn arrange(
    list: &[LiveFollow],
    sort: Sort,
    query: &str,
    own_game: Option<&str>,
) -> Vec<LiveFollow> {
    let query = query.trim().to_lowercase();
    let mut out: Vec<LiveFollow> = list
        .iter()
        .filter(|l| {
            query.is_empty()
                || [&l.login, &l.name, &l.game, &l.title]
                    .iter()
                    .any(|field| field.to_lowercase().contains(&query))
        })
        .cloned()
        .collect();
    let by_viewers = |a: &LiveFollow, b: &LiveFollow| b.viewers.cmp(&a.viewers);
    match sort {
        Sort::MostViewers => out.sort_by(by_viewers),
        Sort::FewestViewers => out.sort_by(|a, b| a.viewers.cmp(&b.viewers)),
        Sort::LongestLive => out.sort_by_key(|l| std::cmp::Reverse(l.live_seconds().unwrap_or(0))),
        Sort::JustWentLive => out.sort_by_key(|l| l.live_seconds().unwrap_or(i64::MAX)),
        Sort::Name => out.sort_by_key(|l| l.label().to_lowercase()),
        Sort::Category => {
            let mut totals = std::collections::HashMap::<String, u64>::new();
            for l in &out {
                *totals.entry(l.game.clone()).or_default() += l.viewers;
            }
            out.sort_by(|a, b| {
                totals[&b.game]
                    .cmp(&totals[&a.game])
                    .then_with(|| a.game.cmp(&b.game))
                    .then_with(|| by_viewers(a, b))
            });
        }
        Sort::SameCategory => {
            let own = own_game.unwrap_or("").to_lowercase();
            out.sort_by(|a, b| {
                let same = |l: &LiveFollow| !own.is_empty() && l.game.to_lowercase() == own;
                same(b).cmp(&same(a)).then_with(|| by_viewers(a, b))
            });
        }
    }
    out
}

pub struct FollowsLive {
    /// Live followed channels, most viewers first; None until loaded.
    pub list: Option<Vec<LiveFollow>>,
    user: Option<String>,
    last: Option<Instant>,
    pending: bool,
    tx: mpsc::SyncSender<Identity>,
    rx: mpsc::Receiver<(String, Option<Vec<LiveFollow>>)>,
}
impl FollowsLive {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Identity>(1);
        let (results, rx) = mpsc::sync_channel(1);
        let _ = std::thread::Builder::new()
            .name("jawjack-follows-live".into())
            .spawn(move || {
                let Ok(client) = crate::live::client() else {
                    return;
                };
                while let Ok(identity) = jobs.recv() {
                    let list = fetch(&client, &identity);
                    if results.send((identity.user_id.clone(), list)).is_err() {
                        break;
                    }
                }
            });
        Self {
            list: None,
            user: None,
            last: None,
            pending: false,
            tx,
            rx,
        }
    }
    /// Returns true when the list changed.
    pub fn pump(&mut self, identity: Option<&Identity>) -> bool {
        let user = identity.map(|i| i.user_id.clone());
        let mut changed = false;
        if user != self.user {
            self.user = user;
            self.last = None;
            changed = self.list.take().is_some();
        }
        while let Ok((user, list)) = self.rx.try_recv() {
            self.pending = false;
            if Some(&user) == self.user.as_ref()
                && let Some(list) = list
                && self.list.as_ref() != Some(&list)
            {
                self.list = Some(list);
                changed = true;
            }
        }
        if let Some(identity) = identity.filter(|i| i.has("user:read:follows"))
            && !self.pending
            && self.last.is_none_or(|t| t.elapsed() >= REFRESH)
            && self.tx.try_send(identity.clone()).is_ok()
        {
            self.pending = true;
            self.last = Some(Instant::now());
        }
        changed
    }
}

fn fetch(client: &reqwest::blocking::Client, identity: &Identity) -> Option<Vec<LiveFollow>> {
    let text = |v: &serde_json::Value, max: usize| {
        v.as_str()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_control())
            .take(max)
            .collect::<String>()
    };
    let mut list = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut query = vec![("user_id", identity.user_id.as_str()), ("first", "100")];
        if let Some(after) = &cursor {
            query.push(("after", after.as_str()));
        }
        let response = client
            .get("https://api.twitch.tv/helix/streams/followed")
            .query(&query)
            .header("Client-Id", CLIENT_ID)
            .bearer_auth(&identity.access)
            .send()
            .ok()?;
        let Ok((200, page)) = crate::live::response_json(response) else {
            return None;
        };
        for s in page["data"].as_array().into_iter().flatten() {
            let Some(login) = chat_core::twitch_login(&text(&s["user_login"], 64)) else {
                continue;
            };
            list.push(LiveFollow {
                user_id: text(&s["user_id"], 32),
                started_at: text(&s["started_at"], 40),
                name: text(&s["user_name"], 64),
                login,
                game: text(&s["game_name"], 80),
                title: text(&s["title"], 140),
                viewers: s["viewer_count"].as_u64().unwrap_or(0),
            });
        }
        cursor = page["pagination"]["cursor"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    list.sort_by(|a, b| {
        b.viewers
            .cmp(&a.viewers)
            .then_with(|| a.login.cmp(&b.login))
    });
    Some(list)
}

/// "842", "12.4k", "1.2M".
pub fn viewers(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}k", n as f64 / 1_000.).replace(".0k", "k"),
        _ => format!("{:.1}M", n as f64 / 1_000_000.).replace(".0M", "M"),
    }
}

#[cfg(test)]
mod arrange_tests {
    use super::{LiveFollow, Sort, arrange};
    fn live(login: &str, game: &str, viewers: u64) -> LiveFollow {
        LiveFollow {
            login: login.into(),
            name: String::new(),
            game: game.into(),
            title: format!("{login} plays {game}"),
            viewers,
            user_id: String::new(),
            started_at: String::new(),
        }
    }
    #[test]
    fn sorts_and_filters() {
        let list = [
            live("a", "Art", 10),
            live("b", "Chess", 500),
            live("c", "Art", 90),
        ];
        let logins = |v: Vec<LiveFollow>| v.into_iter().map(|l| l.login).collect::<Vec<_>>();
        assert_eq!(
            logins(arrange(&list, Sort::MostViewers, "", None)),
            ["b", "c", "a"]
        );
        assert_eq!(
            logins(arrange(&list, Sort::FewestViewers, "", None)),
            ["a", "c", "b"]
        );
        assert_eq!(
            logins(arrange(&list, Sort::Category, "", None)),
            ["b", "c", "a"]
        );
        assert_eq!(
            logins(arrange(&list, Sort::SameCategory, "", Some("art"))),
            ["c", "a", "b"]
        );
        assert_eq!(
            logins(arrange(&list, Sort::MostViewers, "chess", None)),
            ["b"]
        );
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn viewer_counts() {
        assert_eq!(super::viewers(842), "842");
        assert_eq!(super::viewers(12_400), "12.4k");
        assert_eq!(super::viewers(3_000), "3k");
        assert_eq!(super::viewers(1_250_000), "1.2M");
    }
}
