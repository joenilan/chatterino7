//! Twitch polls and predictions for the signed-in broadcaster (Studio → Tools).
//! The current poll/prediction is re-read every few seconds while one is
//! running (every 30 s otherwise); every write is confirmed by Twitch's reply.
use crate::{auth::CLIENT_ID, live::Identity};
use serde_json::{Value, json};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub id: String,
    pub title: String,
    pub votes: u64,
    /// Predictions: channel points and number of predictors.
    pub points: u64,
    pub users: u64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Poll {
    pub id: String,
    pub title: String,
    pub status: String,
    pub choices: Vec<Choice>,
    pub ends: Option<chrono::DateTime<chrono::Utc>>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Prediction {
    pub id: String,
    pub title: String,
    /// ACTIVE, LOCKED, RESOLVED or CANCELED.
    pub status: String,
    pub outcomes: Vec<Choice>,
    pub winner: Option<String>,
    pub locks: Option<chrono::DateTime<chrono::Utc>>,
}
impl Poll {
    pub fn running(&self) -> bool {
        self.status == "ACTIVE"
    }
}
impl Prediction {
    pub fn open(&self) -> bool {
        matches!(self.status.as_str(), "ACTIVE" | "LOCKED")
    }
}

pub enum Job {
    Refresh,
    CreatePoll {
        title: String,
        choices: Vec<String>,
        seconds: u32,
    },
    EndPoll {
        id: String,
    },
    CreatePrediction {
        title: String,
        outcomes: Vec<String>,
        seconds: u32,
    },
    LockPrediction {
        id: String,
    },
    ResolvePrediction {
        id: String,
        winner: String,
    },
    CancelPrediction {
        id: String,
    },
}
enum Reply {
    State(Option<Poll>, Option<Prediction>),
    /// Reading the current state failed.
    Unavailable(String),
    /// The result of a write.
    Done(Result<String, String>),
}

pub struct Engagement {
    pub poll: Option<Poll>,
    pub prediction: Option<Prediction>,
    /// Why polls/predictions can't be shown (eligibility, permission, network).
    pub problem: Option<String>,
    pub busy: bool,
    next: Instant,
    pending_refresh: bool,
    user: Option<String>,
    tx: mpsc::Sender<(Identity, Job)>,
    rx: mpsc::Receiver<Reply>,
}
impl Engagement {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::channel::<(Identity, Job)>();
        let (replies, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok(client) = crate::live::client() else {
                return;
            };
            while let Ok((identity, job)) = jobs.recv() {
                let refresh = matches!(job, Job::Refresh);
                if !refresh {
                    let _ = replies.send(Reply::Done(run(&client, &identity, job)));
                }
                // Every job is followed by a fresh read, so the UI shows Twitch's state.
                let state = read(&client, &identity);
                let reply = match state {
                    Ok((poll, prediction)) => Reply::State(poll, prediction),
                    Err(e) => Reply::Unavailable(e),
                };
                if replies.send(reply).is_err() {
                    break;
                }
            }
        });
        Self {
            poll: None,
            prediction: None,
            problem: None,
            busy: false,
            next: Instant::now(),
            pending_refresh: false,
            user: None,
            tx,
            rx,
        }
    }
    /// Refresh while Studio is open; returns (changed, completed write results).
    pub fn pump(
        &mut self,
        identity: Option<&Identity>,
        visible: bool,
    ) -> (bool, Vec<Result<String, String>>) {
        let mut changed = false;
        let mut results = Vec::new();
        let user = identity.map(|i| i.user_id.clone());
        if user != self.user {
            self.user = user;
            self.poll = None;
            self.prediction = None;
            self.problem = None;
            self.next = Instant::now();
            changed = true;
        }
        while let Ok(reply) = self.rx.try_recv() {
            changed = true;
            match reply {
                Reply::State(poll, prediction) => {
                    self.pending_refresh = false;
                    self.busy = false;
                    self.problem = None;
                    self.poll = poll;
                    self.prediction = prediction;
                }
                Reply::Unavailable(e) => {
                    self.pending_refresh = false;
                    self.busy = false;
                    self.problem = Some(e);
                }
                Reply::Done(result) => results.push(result),
            }
        }
        let running = self.poll.as_ref().is_some_and(Poll::running)
            || self.prediction.as_ref().is_some_and(Prediction::open);
        if let Some(identity) = identity
            && visible
            && !self.pending_refresh
            && !self.busy
            && Instant::now() >= self.next
        {
            self.next = Instant::now()
                + if running {
                    Duration::from_secs(3)
                } else {
                    Duration::from_secs(30)
                };
            if self.tx.send((identity.clone(), Job::Refresh)).is_ok() {
                self.pending_refresh = true;
            }
        }
        (changed, results)
    }
    pub fn send(&mut self, identity: &Identity, job: Job) {
        if self.tx.send((identity.clone(), job)).is_ok() {
            self.busy = true;
            self.pending_refresh = true;
            self.next = Instant::now() + Duration::from_secs(3);
        }
    }
}

fn helix(
    client: &reqwest::blocking::Client,
    identity: &Identity,
    method: reqwest::Method,
    path: &str,
    query: &[(&str, &str)],
    body: Option<Value>,
) -> Result<Value, String> {
    let mut request = client
        .request(method, format!("https://api.twitch.tv/helix/{path}"))
        .query(query)
        .header("Client-Id", CLIENT_ID)
        .bearer_auth(&identity.access);
    if let Some(body) = body {
        request = request
            .header("Content-Type", "application/json")
            .body(body.to_string());
    }
    let response = request
        .send()
        .map_err(|_| "Could not reach Twitch.".to_owned())?;
    let status = response.status().as_u16();
    let (_, value) = crate::live::response_json(response)?;
    if (200..300).contains(&status) {
        return Ok(value);
    }
    let message: String = value["message"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(200)
        .collect();
    Err(match status {
        401 => {
            "Twitch rejected the session. Update Twitch permissions from the account menu.".into()
        }
        403 if message.to_lowercase().contains("partner")
            || message.to_lowercase().contains("affiliate") =>
        {
            "Polls and predictions need Twitch Affiliate or Partner.".into()
        }
        _ if !message.is_empty() => format!("Twitch: {message}"),
        _ => format!("Twitch refused (HTTP {status})."),
    })
}
fn text(value: &Value, limit: usize) -> String {
    value.as_str().unwrap_or("").chars().take(limit).collect()
}
fn time(value: &Value) -> Option<chrono::DateTime<chrono::Utc>> {
    value
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&chrono::Utc))
}
fn read(
    client: &reqwest::blocking::Client,
    identity: &Identity,
) -> Result<(Option<Poll>, Option<Prediction>), String> {
    let me = identity.user_id.as_str();
    let polls = helix(
        client,
        identity,
        reqwest::Method::GET,
        "polls",
        &[("broadcaster_id", me), ("first", "1")],
        None,
    )?;
    let poll = polls["data"].as_array().and_then(|a| a.first()).map(|p| {
        let started = time(&p["started_at"]);
        let duration = p["duration"].as_i64().unwrap_or(0);
        Poll {
            id: text(&p["id"], 64),
            title: text(&p["title"], 60),
            status: text(&p["status"], 16),
            ends: started.map(|s| s + chrono::Duration::seconds(duration)),
            choices: p["choices"]
                .as_array()
                .into_iter()
                .flatten()
                .take(5)
                .map(|c| Choice {
                    id: text(&c["id"], 64),
                    title: text(&c["title"], 25),
                    votes: c["votes"].as_u64().unwrap_or(0),
                    points: 0,
                    users: 0,
                })
                .collect(),
        }
    });
    let predictions = helix(
        client,
        identity,
        reqwest::Method::GET,
        "predictions",
        &[("broadcaster_id", me), ("first", "1")],
        None,
    )?;
    let prediction = predictions["data"]
        .as_array()
        .and_then(|a| a.first())
        .map(|p| {
            let created = time(&p["created_at"]);
            let window = p["prediction_window"].as_i64().unwrap_or(0);
            Prediction {
                id: text(&p["id"], 64),
                title: text(&p["title"], 45),
                status: text(&p["status"], 16),
                winner: p["winning_outcome_id"].as_str().map(str::to_owned),
                locks: created.map(|c| c + chrono::Duration::seconds(window)),
                outcomes: p["outcomes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(10)
                    .map(|o| Choice {
                        id: text(&o["id"], 64),
                        title: text(&o["title"], 25),
                        votes: 0,
                        points: o["channel_points"].as_u64().unwrap_or(0),
                        users: o["users"].as_u64().unwrap_or(0),
                    })
                    .collect(),
            }
        });
    Ok((poll, prediction))
}
fn run(
    client: &reqwest::blocking::Client,
    identity: &Identity,
    job: Job,
) -> Result<String, String> {
    use reqwest::Method as M;
    let me = identity.user_id.as_str();
    match job {
        Job::Refresh => Ok(String::new()),
        Job::CreatePoll {
            title,
            choices,
            seconds,
        } => {
            let body = json!({"broadcaster_id": me, "title": title, "duration": seconds,
                "choices": choices.iter().map(|c| json!({"title": c})).collect::<Vec<_>>()});
            helix(client, identity, M::POST, "polls", &[], Some(body))?;
            Ok("Poll started".into())
        }
        Job::EndPoll { id } => {
            helix(
                client,
                identity,
                M::PATCH,
                "polls",
                &[],
                Some(json!({"broadcaster_id": me, "id": id, "status": "TERMINATED"})),
            )?;
            Ok("Poll ended; results stay visible".into())
        }
        Job::CreatePrediction {
            title,
            outcomes,
            seconds,
        } => {
            let body = json!({"broadcaster_id": me, "title": title, "prediction_window": seconds,
                "outcomes": outcomes.iter().map(|o| json!({"title": o})).collect::<Vec<_>>()});
            helix(client, identity, M::POST, "predictions", &[], Some(body))?;
            Ok("Prediction started".into())
        }
        Job::LockPrediction { id } => {
            helix(
                client,
                identity,
                M::PATCH,
                "predictions",
                &[],
                Some(json!({"broadcaster_id": me, "id": id, "status": "LOCKED"})),
            )?;
            Ok("Prediction locked".into())
        }
        Job::ResolvePrediction { id, winner } => {
            helix(
                client,
                identity,
                M::PATCH,
                "predictions",
                &[],
                Some(
                    json!({"broadcaster_id": me, "id": id, "status": "RESOLVED", "winning_outcome_id": winner}),
                ),
            )?;
            Ok("Prediction resolved; points paid out".into())
        }
        Job::CancelPrediction { id } => {
            helix(
                client,
                identity,
                M::PATCH,
                "predictions",
                &[],
                Some(json!({"broadcaster_id": me, "id": id, "status": "CANCELED"})),
            )?;
            Ok("Prediction canceled; points refunded".into())
        }
    }
}
