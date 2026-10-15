//! Channel point rewards (Helix custom rewards and redemptions). Twitch lets an
//! app change only the rewards it created, so dashboard rewards are read-only
//! here and new rewards made in Jawjack stay manageable, with a redemption queue.
use crate::{auth::CLIENT_ID, live::Identity};
use serde_json::{Value, json};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

const REFRESH: Duration = Duration::from_secs(60);
const URL: &str = "https://api.twitch.tv/helix/channel_points/custom_rewards";

#[derive(Clone, Debug, PartialEq)]
pub struct Reward {
    pub id: String,
    pub title: String,
    pub cost: u64,
    pub paused: bool,
    pub enabled: bool,
    /// Created by Jawjack, so it can be changed and its redemptions handled.
    pub manageable: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Redemption {
    pub id: String,
    pub reward_id: String,
    pub reward: String,
    pub user: String,
    pub input: String,
}
pub enum Job {
    Refresh,
    Create {
        title: String,
        cost: u64,
        prompt: String,
        input: bool,
    },
    Pause {
        id: String,
        paused: bool,
    },
    Delete {
        id: String,
    },
    Resolve {
        reward_id: String,
        id: String,
        fulfil: bool,
    },
}
pub struct Rewards {
    pub list: Option<Vec<Reward>>,
    pub queue: Vec<Redemption>,
    /// Last outcome to show ("Created …", "Couldn't …").
    pub notice: Option<String>,
    last: Option<Instant>,
    pending: bool,
    tx: mpsc::SyncSender<(Identity, Job)>,
    rx: mpsc::Receiver<Reply>,
}
struct Reply {
    list: Option<Vec<Reward>>,
    queue: Option<Vec<Redemption>>,
    notice: Option<String>,
}

fn text(v: &Value, max: usize) -> String {
    v.as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect()
}
fn call(
    client: &reqwest::blocking::Client,
    identity: &Identity,
    request: reqwest::blocking::RequestBuilder,
) -> Option<(u16, Value)> {
    let _ = client;
    request
        .header("Client-Id", CLIENT_ID)
        .bearer_auth(&identity.access)
        .send()
        .ok()
        .and_then(|r| crate::live::response_json(r).ok())
}
fn rewards(client: &reqwest::blocking::Client, identity: &Identity) -> Option<Vec<Reward>> {
    let me = identity.user_id.as_str();
    let (status, all) = call(
        client,
        identity,
        client.get(URL).query(&[("broadcaster_id", me)]),
    )?;
    if status != 200 {
        return None;
    }
    let mine: Vec<String> = call(
        client,
        identity,
        client
            .get(URL)
            .query(&[("broadcaster_id", me), ("only_manageable_rewards", "true")]),
    )
    .filter(|(s, _)| *s == 200)
    .map(|(_, v)| {
        v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["id"].as_str().map(str::to_owned))
            .collect()
    })
    .unwrap_or_default();
    let mut list: Vec<Reward> = all["data"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            let id = text(&r["id"], 64);
            Reward {
                manageable: mine.contains(&id),
                id,
                title: text(&r["title"], 45),
                cost: r["cost"].as_u64().unwrap_or(0),
                paused: r["is_paused"].as_bool().unwrap_or(false),
                enabled: r["is_enabled"].as_bool().unwrap_or(true),
            }
        })
        .collect();
    list.sort_by(|a, b| b.manageable.cmp(&a.manageable).then(a.cost.cmp(&b.cost)));
    Some(list)
}
fn queue(
    client: &reqwest::blocking::Client,
    identity: &Identity,
    list: &[Reward],
) -> Vec<Redemption> {
    let mut out = Vec::new();
    for reward in list.iter().filter(|r| r.manageable) {
        let mut cursor: Option<String> = None;
        // Paginate rather than silently dropping redemptions beyond the first
        // ten rewards or the first 50 queued entries per reward. Bound the
        // backlog to avoid indefinitely monopolizing the API worker.
        for _ in 0..20 {
            let request = client.get(format!("{URL}/redemptions")).query(&[
                ("broadcaster_id", identity.user_id.as_str()),
                ("reward_id", reward.id.as_str()),
                ("status", "UNFULFILLED"),
                ("first", "50"),
            ]);
            let request = if let Some(after) = &cursor {
                request.query(&[("after", after.as_str())])
            } else {
                request
            };
            let Some((200, v)) = call(client, identity, request) else {
                break;
            };
            let Some(entries) = v["data"].as_array() else {
                break;
            };
            if entries.is_empty() {
                break;
            }
            out.extend(entries.iter().map(|r| Redemption {
                id: text(&r["id"], 64),
                reward_id: reward.id.clone(),
                reward: reward.title.clone(),
                user: text(&r["user_name"], 60),
                input: text(&r["user_input"], 200),
            }));
            let Some(next) = v["pagination"]["cursor"].as_str().filter(|s| !s.is_empty()) else {
                break;
            };
            if cursor.as_deref() == Some(next) {
                break;
            }
            cursor = Some(next.to_owned());
        }
    }
    out
}
fn refused(action: &str, status: u16, v: &Value) -> String {
    format!(
        "Couldn't {action} ({status}): {}",
        v["message"]
            .as_str()
            .unwrap_or("Twitch refused")
            .chars()
            .take(120)
            .collect::<String>()
    )
}
fn run(client: &reqwest::blocking::Client, identity: &Identity, job: Job) -> Reply {
    let me = identity.user_id.as_str();
    let notice = match job {
        Job::Refresh => None,
        Job::Create {
            title,
            cost,
            prompt,
            input,
        } => {
            let body = json!({"title": title, "cost": cost, "prompt": prompt, "is_user_input_required": input});
            match call(
                client,
                identity,
                client
                    .post(URL)
                    .query(&[("broadcaster_id", me)])
                    .header("Content-Type", "application/json")
                    .body(body.to_string()),
            ) {
                Some((200, _)) => Some(format!("Created “{title}”.")),
                Some((status, v)) => Some(refused("create the reward", status, &v)),
                None => Some("Couldn't reach Twitch.".into()),
            }
        }
        Job::Pause { id, paused } => {
            let body = json!({"is_paused": paused});
            match call(
                client,
                identity,
                client
                    .patch(URL)
                    .query(&[("broadcaster_id", me), ("id", id.as_str())])
                    .header("Content-Type", "application/json")
                    .body(body.to_string()),
            ) {
                Some((200, _)) => None,
                Some((status, v)) => Some(refused("change the reward", status, &v)),
                None => Some("Couldn't reach Twitch.".into()),
            }
        }
        Job::Delete { id } => match call(
            client,
            identity,
            client
                .delete(URL)
                .query(&[("broadcaster_id", me), ("id", id.as_str())]),
        ) {
            Some((204 | 200, _)) => Some("Reward deleted.".into()),
            Some((status, v)) => Some(refused("delete the reward", status, &v)),
            None => Some("Couldn't reach Twitch.".into()),
        },
        Job::Resolve {
            reward_id,
            id,
            fulfil,
        } => {
            let body = json!({"status": if fulfil { "FULFILLED" } else { "CANCELED" }});
            match call(
                client,
                identity,
                client
                    .patch(format!("{URL}/redemptions"))
                    .query(&[
                        ("broadcaster_id", me),
                        ("reward_id", reward_id.as_str()),
                        ("id", id.as_str()),
                    ])
                    .header("Content-Type", "application/json")
                    .body(body.to_string()),
            ) {
                Some((200, _)) => None,
                Some((status, v)) => Some(refused(
                    if fulfil { "complete it" } else { "refund it" },
                    status,
                    &v,
                )),
                None => Some("Couldn't reach Twitch.".into()),
            }
        }
    };
    // Every job ends with a fresh list and queue.
    let list = rewards(client, identity);
    let queue = list.as_ref().map(|l| queue(client, identity, l));
    Reply {
        list,
        queue,
        notice,
    }
}

impl Rewards {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<(Identity, Job)>(4);
        let (results, rx) = mpsc::sync_channel(4);
        let _ = std::thread::Builder::new()
            .name("jawjack-rewards".into())
            .spawn(move || {
                let Ok(client) = crate::live::client() else {
                    return;
                };
                while let Ok((identity, job)) = jobs.recv() {
                    if results.send(run(&client, &identity, job)).is_err() {
                        break;
                    }
                }
            });
        Self {
            list: None,
            queue: Vec::new(),
            notice: None,
            last: None,
            pending: false,
            tx,
            rx,
        }
    }
    pub fn send(&mut self, identity: &Identity, job: Job) {
        if self.tx.try_send((identity.clone(), job)).is_ok() {
            self.pending = true;
            self.last = Some(Instant::now());
        }
    }
    /// Refetch soon (a redemption just arrived).
    pub fn refresh(&mut self) {
        self.last = None;
    }
    /// `active`: the Studio Tools tab is showing. Returns true on change.
    pub fn pump(&mut self, identity: Option<&Identity>, active: bool) -> bool {
        let mut changed = false;
        while let Ok(reply) = self.rx.try_recv() {
            self.pending = false;
            if let Some(list) = reply.list {
                changed |= self.list.as_ref() != Some(&list);
                self.list = Some(list);
            }
            if let Some(queue) = reply.queue {
                changed |= self.queue != queue;
                self.queue = queue;
            }
            if reply.notice.is_some() {
                self.notice = reply.notice;
                changed = true;
            }
        }
        if active
            && let Some(identity) = identity.filter(|i| {
                i.has("channel:read:redemptions") || i.has("channel:manage:redemptions")
            })
            && !self.pending
            && self.last.is_none_or(|t| t.elapsed() >= REFRESH)
        {
            self.send(identity, Job::Refresh);
        }
        changed
    }
}
