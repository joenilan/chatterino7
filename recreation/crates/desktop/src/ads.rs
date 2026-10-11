//! Ad schedule for the signed-in broadcaster (Helix Get Ad Schedule /
//! Snooze Next Ad; `channel:read:ads`, `channel:manage:ads`). Polled once a
//! minute while live; snoozing is an explicit click.
use crate::{auth::CLIENT_ID, live::Identity};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

const REFRESH: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schedule {
    pub next_ad_at: Option<DateTime<Utc>>,
    /// Seconds of the next ad.
    pub duration: u64,
    pub snoozes: u64,
    pub snooze_refresh_at: Option<DateTime<Utc>>,
}

/// Twitch documents RFC 3339 strings; older responses used Unix seconds.
fn time(v: &Value) -> Option<DateTime<Utc>> {
    if let Some(s) = v.as_str().filter(|s| !s.is_empty()) {
        if let Ok(t) = DateTime::parse_from_rfc3339(s) {
            return Some(t.with_timezone(&Utc));
        }
        return s
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .and_then(|n| DateTime::from_timestamp(n, 0));
    }
    v.as_i64()
        .filter(|n| *n > 0)
        .and_then(|n| DateTime::from_timestamp(n, 0))
}
pub fn parse(data: &Value) -> Schedule {
    let number = |v: &Value| {
        v.as_u64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            .unwrap_or(0)
    };
    Schedule {
        next_ad_at: time(&data["next_ad_at"]),
        duration: number(&data["duration"]),
        snoozes: number(&data["snooze_count"]),
        snooze_refresh_at: time(&data["snooze_refresh_at"]),
    }
}

enum Job {
    Fetch(Identity),
    Snooze(Identity),
}
pub struct Ads {
    pub schedule: Option<Schedule>,
    /// Feedback from the last snooze ("Snoozed" or the reason it failed).
    pub notice: Option<String>,
    /// The ad time already warned about, so the heads-up shows once per ad.
    pub warned: Option<DateTime<Utc>>,
    last: Option<Instant>,
    pending: bool,
    tx: mpsc::SyncSender<Job>,
    rx: mpsc::Receiver<(Option<Schedule>, Option<String>)>,
}
impl Ads {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Job>(2);
        let (results, rx) = mpsc::sync_channel(2);
        let _ = std::thread::Builder::new()
            .name("jawjack-ads".into())
            .spawn(move || {
                let Ok(client) = crate::live::client() else {
                    return;
                };
                while let Ok(job) = jobs.recv() {
                    let (identity, snooze) = match job {
                        Job::Fetch(i) => (i, false),
                        Job::Snooze(i) => (i, true),
                    };
                    let request = if snooze {
                        client.post("https://api.twitch.tv/helix/channels/ads/schedule/snooze")
                    } else {
                        client.get("https://api.twitch.tv/helix/channels/ads")
                    };
                    let result = request
                        .query(&[("broadcaster_id", identity.user_id.as_str())])
                        .header("Client-Id", CLIENT_ID)
                        .bearer_auth(&identity.access)
                        .send()
                        .ok()
                        .and_then(|r| crate::live::response_json(r).ok());
                    let reply = match result {
                        Some((200, value)) => (
                            Some(parse(&value["data"][0])),
                            snooze.then(|| "Next ad snoozed by 5 minutes.".to_owned()),
                        ),
                        Some((429, _)) if snooze => {
                            (None, Some("No snoozes left right now.".into()))
                        }
                        Some((_, value)) if snooze => (
                            None,
                            Some(format!(
                                "Couldn't snooze: {}",
                                value["message"]
                                    .as_str()
                                    .unwrap_or("Twitch refused")
                                    .chars()
                                    .take(120)
                                    .collect::<String>()
                            )),
                        ),
                        _ => (None, None),
                    };
                    if results.send(reply).is_err() {
                        break;
                    }
                }
            });
        Self {
            schedule: None,
            notice: None,
            warned: None,
            last: None,
            pending: false,
            tx,
            rx,
        }
    }
    pub fn snooze(&mut self, identity: &Identity) {
        let _ = self.tx.try_send(Job::Snooze(identity.clone()));
    }
    /// Poll while live; returns true when something changed.
    pub fn pump(&mut self, identity: Option<&Identity>, live: bool) -> bool {
        let mut changed = false;
        while let Ok((schedule, notice)) = self.rx.try_recv() {
            self.pending = false;
            if let Some(schedule) = schedule {
                changed |= self.schedule.as_ref() != Some(&schedule);
                self.schedule = Some(schedule);
            }
            if notice.is_some() {
                self.notice = notice;
                changed = true;
            }
        }
        if !live {
            if self.schedule.take().is_some() {
                changed = true;
            }
            self.last = None;
            return changed;
        }
        if let Some(identity) = identity.filter(|i| i.has("channel:read:ads"))
            && !self.pending
            && self.last.is_none_or(|t| t.elapsed() >= REFRESH)
            && self.tx.try_send(Job::Fetch(identity.clone())).is_ok()
        {
            self.pending = true;
            self.last = Some(Instant::now());
        }
        changed
    }
}

/// "12:34" until `at`, or None once it has passed.
pub fn countdown(at: DateTime<Utc>) -> Option<String> {
    let s = (at - Utc::now()).num_seconds();
    (s > 0).then(|| format!("{}:{:02}", s / 60, s % 60))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    #[test]
    fn parses_both_time_formats() {
        let a = super::parse(
            &json!({"next_ad_at":"2026-10-11T20:00:00Z","duration":90,"snooze_count":3}),
        );
        assert_eq!(a.duration, 90);
        assert_eq!(a.snoozes, 3);
        assert!(a.next_ad_at.is_some());
        let b =
            super::parse(&json!({"next_ad_at":"1760212800","duration":"60","snooze_count":"1"}));
        assert_eq!(b.next_ad_at.unwrap().timestamp(), 1760212800);
        assert_eq!(super::parse(&json!({"next_ad_at":""})).next_ad_at, None);
    }
}
