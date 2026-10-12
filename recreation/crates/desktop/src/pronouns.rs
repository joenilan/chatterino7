//! Pronouns on user cards, from pronouns.alejo.io as Chatterino uses (Settings →
//! Chat → "Show pronouns", off by default). Looked up only when a card opens.
use std::{
    collections::HashMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub static ENABLED: AtomicBool = AtomicBool::new(false);
pub const SERVICE: &str = "pronouns.alejo.io";
const API: &str = "https://api.pronouns.alejo.io/v1";

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Loading,
    /// None: the chatter hasn't set any.
    Ready(Option<String>),
    Failed,
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// pronoun ID → (subject, object, singular)
type Catalog = HashMap<String, (String, String, bool)>;

fn users() -> &'static Mutex<HashMap<String, State>> {
    static USERS: OnceLock<Mutex<HashMap<String, State>>> = OnceLock::new();
    USERS.get_or_init(Default::default)
}

fn client() -> Option<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(concat!("Jawjack/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()
}

/// The pronoun list, loaded once (retried on the next lookup if it failed).
fn catalog(client: &reqwest::blocking::Client) -> Option<Catalog> {
    static CATALOG: Mutex<Option<Catalog>> = Mutex::new(None);
    if let Some(known) = CATALOG.lock().ok()?.clone() {
        return Some(known);
    }
    let value: serde_json::Value = client
        .get(format!("{API}/pronouns"))
        .send()
        .ok()?
        .text()
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())?;
    let parsed: Catalog = value
        .as_object()?
        .iter()
        .filter_map(|(id, p)| {
            let subject = p["subject"].as_str().filter(|s| !s.is_empty())?;
            let object = p["object"].as_str().filter(|s| !s.is_empty())?;
            Some((
                id.clone(),
                (
                    subject.to_owned(),
                    object.to_owned(),
                    p["singular"].as_bool().unwrap_or(false),
                ),
            ))
        })
        .collect();
    if parsed.is_empty() {
        return None;
    }
    *CATALOG.lock().ok()? = Some(parsed.clone());
    Some(parsed)
}

/// "she/her", "they/them", "he/they" or a singular one like "any", as Chatterino shows them.
pub fn label(catalog: &Catalog, main: &str, alt: Option<&str>) -> Option<String> {
    let (subject, object, singular) = catalog.get(main)?;
    Some(match alt.and_then(|a| catalog.get(a)) {
        Some((alt_subject, _, _)) => format!("{subject}/{alt_subject}"),
        None if *singular => subject.clone(),
        None => format!("{subject}/{object}"),
    })
}

fn fetch(login: &str) -> State {
    let Some(client) = client() else {
        return State::Failed;
    };
    let Some(catalog) = catalog(&client) else {
        return State::Failed;
    };
    let Ok(response) = client.get(format!("{API}/users/{login}")).send() else {
        return State::Failed;
    };
    if response.status().as_u16() == 404 {
        return State::Ready(None);
    }
    let Some(value) = response
        .text()
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        return State::Failed;
    };
    match value["pronoun_id"].as_str() {
        Some(main) => State::Ready(label(&catalog, main, value["alt_pronoun_id"].as_str())),
        None => State::Ready(None),
    }
}

/// A chatter's pronouns, starting a lookup the first time they're asked for.
pub fn get(login: &str) -> State {
    let Some(login) = chat_core::twitch_login(login) else {
        return State::Ready(None);
    };
    let Ok(mut map) = users().lock() else {
        return State::Failed;
    };
    if let Some(state) = map.get(&login) {
        return state.clone();
    }
    if map.len() >= 512 {
        map.clear();
    }
    map.insert(login.clone(), State::Loading);
    let _ = std::thread::Builder::new()
        .name("jawjack-pronouns".into())
        .spawn(move || {
            let state = fetch(&login);
            if let Ok(mut map) = users().lock() {
                map.insert(login, state);
            }
        });
    State::Loading
}

#[cfg(test)]
mod tests {
    #[test]
    fn labels() {
        let catalog = [
            ("hehim", ("He", "Him", false)),
            ("theythem", ("They", "Them", false)),
            ("any", ("Any", "Any", true)),
        ]
        .into_iter()
        .map(|(id, (s, o, one))| (id.to_owned(), (s.to_owned(), o.to_owned(), one)))
        .collect();
        assert_eq!(
            super::label(&catalog, "hehim", None).as_deref(),
            Some("He/Him")
        );
        assert_eq!(
            super::label(&catalog, "hehim", Some("theythem")).as_deref(),
            Some("He/They")
        );
        assert_eq!(super::label(&catalog, "any", None).as_deref(), Some("Any"));
        assert_eq!(super::label(&catalog, "nope", None), None);
    }
}
