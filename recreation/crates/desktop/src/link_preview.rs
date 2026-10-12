//! Link previews on hover (Chatterino's link tooltips), fetched directly from
//! this PC only when a link is hovered: the page's title, description and site,
//! where a shortened/redirecting link really goes, and warnings for lookalike
//! (punycode) domains and plain-http links. No third-party resolver.
use std::{
    collections::HashMap,
    io::Read,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Settings → Chat → "Link previews" (on by default).
pub static ENABLED: AtomicBool = AtomicBool::new(true);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preview {
    pub title: String,
    pub description: String,
    pub site: String,
    /// Where the link ends up after redirects, when that's a different site.
    pub redirected_to: Option<String>,
    pub warnings: Vec<String>,
    /// The page's preview image (og:image), as an absolute HTTPS URL.
    pub image: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Loading,
    Ready(Preview),
    Failed(String),
}

const CACHE: usize = 256;
const BODY: u64 = 512 * 1024;

fn cache() -> &'static Mutex<HashMap<String, State>> {
    static CACHE_MAP: OnceLock<Mutex<HashMap<String, State>>> = OnceLock::new();
    CACHE_MAP.get_or_init(Default::default)
}

/// The preview for `url`, starting a fetch the first time it's asked for.
pub fn get(url: &str) -> State {
    let Ok(mut map) = cache().lock() else {
        return State::Failed("Preview unavailable".into());
    };
    if let Some(state) = map.get(url) {
        return state.clone();
    }
    if map.len() >= CACHE {
        map.clear();
    }
    map.insert(url.to_owned(), State::Loading);
    let url = url.to_owned();
    let _ = std::thread::Builder::new()
        .name("jawjack-link-preview".into())
        .spawn(move || {
            let state = match fetch(&url) {
                Ok(preview) => State::Ready(preview),
                Err(error) => State::Failed(error),
            };
            if let Ok(mut map) = cache().lock() {
                map.insert(url, state);
            }
        });
    State::Loading
}

fn host(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.trim_start_matches("www.").to_ascii_lowercase())
        })
        .unwrap_or_default()
}

/// Warnings that don't need the network.
fn warnings(url: &str) -> Vec<String> {
    let mut out = Vec::new();
    let host = host(url);
    if host.split('.').any(|label| label.starts_with("xn--")) {
        out.push(format!(
            "Lookalike domain: \"{host}\" uses non-Latin characters (punycode)."
        ));
    }
    if url.starts_with("http://") {
        out.push("Not encrypted (http).".into());
    }
    out
}

/// The signed-in Twitch session, for rich Twitch link cards (Helix needs one).
static TWITCH: Mutex<Option<crate::live::Identity>> = Mutex::new(None);
pub fn set_identity(identity: Option<crate::live::Identity>) {
    if let Ok(mut slot) = TWITCH.lock() {
        *slot = identity;
    }
}

/// What a twitch.tv link points at.
#[derive(Debug, PartialEq)]
enum TwitchLink {
    Channel(String),
    Clip(String),
    Video(String),
}
fn twitch_link(url: &str) -> Option<TwitchLink> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed
        .host_str()?
        .trim_start_matches("www.")
        .trim_start_matches("m.");
    let parts: Vec<&str> = parsed.path_segments()?.filter(|p| !p.is_empty()).collect();
    let id = |s: &str| {
        (!s.is_empty()
            && s.len() <= 100
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
        .then(|| s.to_owned())
    };
    match (host, parts.as_slice()) {
        ("clips.twitch.tv", [slug]) => id(slug).map(TwitchLink::Clip),
        ("twitch.tv", [_, "clip", slug]) => id(slug).map(TwitchLink::Clip),
        ("twitch.tv", ["videos", video]) => id(video)
            .filter(|v| v.bytes().all(|b| b.is_ascii_digit()))
            .map(TwitchLink::Video),
        ("twitch.tv", [login]) => chat_core::twitch_login(login).map(TwitchLink::Channel),
        _ => None,
    }
}
fn helix(
    client: &reqwest::blocking::Client,
    identity: &crate::live::Identity,
    path: &str,
    query: &[(&str, &str)],
) -> Option<serde_json::Value> {
    let response = client
        .get(format!("https://api.twitch.tv/helix/{path}"))
        .query(query)
        .header("Client-Id", crate::auth::CLIENT_ID)
        .bearer_auth(&identity.access)
        .send()
        .ok()?;
    let (status, value) = crate::live::response_json(response).ok()?;
    (status == 200).then_some(value)
}
fn count(n: u64) -> String {
    crate::follows_live::viewers(n)
}
/// A Twitch channel, clip or VOD card from Helix, like Chatterino's.
fn twitch_card(url: &str, link: TwitchLink) -> Option<Preview> {
    let identity = TWITCH.lock().ok()?.clone()?;
    let client = crate::live::client().ok()?;
    let mut preview = Preview {
        site: "Twitch".into(),
        warnings: warnings(url),
        ..Default::default()
    };
    match link {
        TwitchLink::Clip(slug) => {
            let v = helix(&client, &identity, "clips", &[("id", &slug)])?;
            let c = &v["data"][0];
            preview.site = "Twitch clip".into();
            preview.title = clip(c["title"].as_str()?, 160);
            preview.description = format!(
                "{} · clipped by {} · {} views · {}s",
                c["broadcaster_name"].as_str().unwrap_or(""),
                c["creator_name"].as_str().unwrap_or(""),
                count(c["view_count"].as_u64().unwrap_or(0)),
                c["duration"].as_f64().unwrap_or(0.).round()
            );
            preview.image = c["thumbnail_url"].as_str().map(str::to_owned);
        }
        TwitchLink::Video(video) => {
            let v = helix(&client, &identity, "videos", &[("id", &video)])?;
            let d = &v["data"][0];
            preview.site = "Twitch video".into();
            preview.title = clip(d["title"].as_str()?, 160);
            preview.description = format!(
                "{} · {} · {} views",
                d["user_name"].as_str().unwrap_or(""),
                d["duration"].as_str().unwrap_or(""),
                count(d["view_count"].as_u64().unwrap_or(0))
            );
            preview.image = d["thumbnail_url"]
                .as_str()
                .map(|t| t.replace("%{width}", "320").replace("%{height}", "180"))
                .filter(|t| !t.is_empty());
        }
        TwitchLink::Channel(login) => {
            let users = helix(&client, &identity, "users", &[("login", &login)])?;
            let user = &users["data"][0];
            let name = user["display_name"].as_str()?.to_owned();
            let stream = helix(&client, &identity, "streams", &[("user_login", &login)])
                .map(|v| v["data"][0].clone())
                .filter(|s| s.is_object());
            preview.site = "Twitch channel".into();
            preview.title = name;
            match &stream {
                Some(s) => {
                    preview.description = format!(
                        "Live: {} · {} · {} viewers",
                        s["title"].as_str().unwrap_or(""),
                        s["game_name"].as_str().unwrap_or(""),
                        count(s["viewer_count"].as_u64().unwrap_or(0))
                    );
                    preview.image = s["thumbnail_url"]
                        .as_str()
                        .map(|t| t.replace("{width}", "320").replace("{height}", "180"));
                }
                None => {
                    let bio = user["description"].as_str().unwrap_or("");
                    preview.description = if bio.is_empty() {
                        "Offline".into()
                    } else {
                        format!("Offline · {bio}")
                    };
                    preview.image = user["profile_image_url"].as_str().map(str::to_owned);
                }
            }
            preview.description = clip(&preview.description, 300);
        }
    }
    Some(preview)
}

fn fetch(url: &str) -> Result<Preview, String> {
    // Twitch links get a real card (needs the signed-in session); else the page.
    if let Some(link) = twitch_link(url)
        && let Some(card) = twitch_card(url, link)
    {
        return Ok(card);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Jawjack link preview")
        .build()
        .map_err(|_| "Preview unavailable".to_owned())?;
    let response = client
        .get(url)
        .header("Accept", "text/html,application/xhtml+xml")
        .send()
        .map_err(|_| "Couldn't reach the site".to_owned())?;
    let final_url = response.url().to_string();
    let mut preview = Preview {
        site: host(&final_url),
        warnings: warnings(url),
        ..Default::default()
    };
    if host(&final_url) != host(url) {
        preview.redirected_to = Some(final_url.clone());
        for warning in warnings(&final_url) {
            if !preview.warnings.contains(&warning) {
                preview.warnings.push(warning);
            }
        }
    }
    let status = response.status();
    if !status.is_success() {
        return Err(format!("The site answered {}", status.as_u16()));
    }
    let html = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|t| t.contains("html"));
    if !html {
        let kind = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("file")
            .split(';')
            .next()
            .unwrap_or("file")
            .to_owned();
        preview.title = format!("A {kind} file");
        return Ok(preview);
    }
    let mut body = Vec::new();
    response
        .take(BODY)
        .read_to_end(&mut body)
        .map_err(|_| "The page didn't finish loading".to_owned())?;
    let page = String::from_utf8_lossy(&body);
    preview.title = meta(&page, "og:title")
        .or_else(|| meta(&page, "twitter:title"))
        .or_else(|| title(&page))
        .unwrap_or_default();
    preview.description = meta(&page, "og:description")
        .or_else(|| meta(&page, "description"))
        .unwrap_or_default();
    preview.image = meta(&page, "og:image")
        .or_else(|| meta(&page, "twitter:image"))
        .and_then(|src| reqwest::Url::parse(&final_url).ok()?.join(src.trim()).ok())
        .filter(|u| u.scheme() == "https")
        .map(String::from);
    if let Some(site) = meta(&page, "og:site_name") {
        preview.site = site;
    }
    preview.title = clip(&preview.title, 160);
    preview.description = clip(&preview.description, 300);
    Ok(preview)
}

fn clip(text: &str, max: usize) -> String {
    let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > max {
        format!("{}…", text.chars().take(max).collect::<String>().trim_end())
    } else {
        text
    }
}

/// `<meta property|name="key" content="…">` in either attribute order.
fn meta(page: &str, key: &str) -> Option<String> {
    static TAG: OnceLock<regex::Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| regex::Regex::new(r"(?is)<meta\s[^>]*>").expect("meta regex"));
    let attr = |tag: &str, name: &str| -> Option<String> {
        let pattern = format!(r#"(?is)\b{name}\s*=\s*("([^"]*)"|'([^']*)')"#);
        let re = regex::Regex::new(&pattern).ok()?;
        let caps = re.captures(tag)?;
        caps.get(2)
            .or_else(|| caps.get(3))
            .map(|m| m.as_str().to_owned())
    };
    tag.find_iter(page).find_map(|m| {
        let t = m.as_str();
        let matches = attr(t, "property")
            .or_else(|| attr(t, "name"))
            .is_some_and(|k| k.eq_ignore_ascii_case(key));
        if matches {
            attr(t, "content")
                .map(|c| decode(&c))
                .filter(|c| !c.trim().is_empty())
        } else {
            None
        }
    })
}

fn title(page: &str) -> Option<String> {
    static TITLE: OnceLock<regex::Regex> = OnceLock::new();
    let re = TITLE
        .get_or_init(|| regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("title regex"));
    re.captures(page)
        .and_then(|c| c.get(1))
        .map(|m| decode(m.as_str()))
        .filter(|t| !t.trim().is_empty())
}

fn decode(text: &str) -> String {
    let mut out = text
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ");
    out = out.replace("&amp;", "&");
    out
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_pages_and_warns() {
        let page = r#"<html><head><title>Fallback &amp; title</title>
            <meta content="A &quot;great&quot; video" property="og:description">
            <meta property='og:title' content='The real title'>
            <meta property="og:site_name" content="YouTube"></head></html>"#;
        assert_eq!(meta(page, "og:title").as_deref(), Some("The real title"));
        assert_eq!(
            meta(page, "og:description").as_deref(),
            Some("A \"great\" video")
        );
        assert_eq!(title(page).as_deref(), Some("Fallback & title"));
        assert!(warnings("https://xn--pple-43d.com/x")[0].contains("Lookalike"));
        assert!(warnings("http://example.com")[0].contains("http"));
        assert!(warnings("https://example.com").is_empty());
        assert_eq!(host("https://www.YouTube.com/watch"), "youtube.com");
        assert_eq!(
            twitch_link("https://twitch.tv/mittensgg"),
            Some(TwitchLink::Channel("mittensgg".into()))
        );
        assert_eq!(
            twitch_link("https://clips.twitch.tv/FunnyClip-abc_1"),
            Some(TwitchLink::Clip("FunnyClip-abc_1".into()))
        );
        assert_eq!(
            twitch_link("https://www.twitch.tv/x/clip/Slug"),
            Some(TwitchLink::Clip("Slug".into()))
        );
        assert_eq!(
            twitch_link("https://www.twitch.tv/videos/123"),
            Some(TwitchLink::Video("123".into()))
        );
        assert_eq!(
            twitch_link("https://twitch.tv/directory/category/art"),
            None
        );
    }
}
