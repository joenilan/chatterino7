//! Sign in with Google for YouTube chat sending: the installed-app OAuth flow
//! (browser consent, loopback redirect to 127.0.0.1, PKCE), scope
//! `youtube.force-ssl`, with your own OAuth client from Google Cloud. Tokens
//! live in Windows Credential Manager; this module only does the HTTP parts.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

pub const SCOPE: &str = "https://www.googleapis.com/auth/youtube.force-ssl";
const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthClient {
    pub id: String,
    pub secret: String,
}
#[derive(Clone, Debug)]
pub struct Tokens {
    pub refresh: String,
    pub access: String,
    pub expires: Instant,
}

/// Jawjack's own OAuth client, baked in at build time when the developer has
/// one (see build.rs); then "Connect YouTube" is a single click for everyone.
pub fn built_in() -> Option<OAuthClient> {
    Some(OAuthClient {
        id: option_env!("JAWJACK_GOOGLE_CLIENT_ID")?.to_owned(),
        secret: option_env!("JAWJACK_GOOGLE_CLIENT_SECRET")?.to_owned(),
    })
    .filter(|c| !c.id.is_empty())
}

fn random(bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|_| "No secure randomness available".to_owned())?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

/// Accepts the client JSON downloaded from Google Cloud, or "id" and "secret" pasted separately.
pub fn parse_client(id_or_json: &str, secret: &str) -> Option<OAuthClient> {
    let text = id_or_json.trim();
    if text.starts_with('{') {
        let v: Value = serde_json::from_str(text).ok()?;
        let c = if v["installed"].is_object() {
            &v["installed"]
        } else {
            &v["web"]
        };
        return Some(OAuthClient {
            id: c["client_id"].as_str()?.to_owned(),
            secret: c["client_secret"].as_str()?.to_owned(),
        });
    }
    (text.ends_with(".apps.googleusercontent.com") && !secret.trim().is_empty()).then(|| {
        OAuthClient {
            id: text.to_owned(),
            secret: secret.trim().to_owned(),
        }
    })
}

/// The whole browser sign-in; blocks up to 5 minutes waiting for the redirect.
/// `open` is called with the consent URL (to open the browser).
pub fn sign_in(client: &OAuthClient, open: impl FnOnce(String)) -> Result<Tokens, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|_| "Couldn't open a local port for the sign-in")?;
    let port = listener
        .local_addr()
        .map_err(|_| "Local port unavailable")?
        .port();
    let redirect = format!("http://127.0.0.1:{port}");
    let verifier = random(48)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random(16)?;
    let url = reqwest::Url::parse_with_params(
        AUTH,
        &[
            ("client_id", client.id.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", state.as_str()),
        ],
    )
    .map_err(|_| "Bad sign-in address")?;
    open(url.to_string());
    listener
        .set_nonblocking(true)
        .map_err(|_| "Local port error")?;
    let deadline = Instant::now() + Duration::from_secs(300);
    let code = loop {
        if Instant::now() > deadline {
            return Err("The Google sign-in timed out. Try again.".into());
        }
        let Ok((mut stream, _)) = listener.accept() else {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut request = [0u8; 8192];
        let n = stream.read(&mut request).unwrap_or(0);
        let line = String::from_utf8_lossy(&request[..n])
            .lines()
            .next()
            .unwrap_or("")
            .to_owned();
        let target = line.split_whitespace().nth(1).unwrap_or("");
        let query = reqwest::Url::parse(&format!("http://localhost{target}")).ok();
        let param = |name: &str| {
            query.as_ref().and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == name)
                    .map(|(_, v)| v.into_owned())
            })
        };
        let (title, ok) = match (param("code"), param("error")) {
            (Some(_), _) if param("state").as_deref() == Some(state.as_str()) => (
                "Signed in. You can close this tab and go back to Jawjack.",
                true,
            ),
            (_, Some(_)) => ("Sign-in was cancelled. You can close this tab.", false),
            _ => ("", false),
        };
        if title.is_empty() {
            // A favicon or stray request: answer and keep waiting.
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            continue;
        }
        let page = format!(
            "<!doctype html><meta charset=utf-8><title>Jawjack</title><body style=\"font:16px system-ui;background:#0a0b0d;color:#eceef1;display:grid;place-items:center;height:90vh\"><p>{title}</p>"
        );
        let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len()).as_bytes());
        if !ok {
            return Err("Google sign-in was cancelled.".into());
        }
        break param("code").unwrap_or_default();
    };
    let http = crate::live::client()?;
    let value = post_form(
        &http,
        &[
            ("code", code.as_str()),
            ("client_id", client.id.as_str()),
            ("client_secret", client.secret.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier.as_str()),
        ],
    )?;
    let refresh = value["refresh_token"]
        .as_str()
        .ok_or("Google didn't return a refresh token. Try again.")?
        .to_owned();
    Ok(Tokens {
        refresh,
        access: value["access_token"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        expires: Instant::now()
            + Duration::from_secs(
                value["expires_in"]
                    .as_u64()
                    .unwrap_or(3000)
                    .saturating_sub(60),
            ),
    })
}

fn post_form(http: &reqwest::blocking::Client, form: &[(&str, &str)]) -> Result<Value, String> {
    let response = http
        .post(TOKEN)
        .form(form)
        .send()
        .map_err(|_| "Couldn't reach Google".to_owned())?;
    let status = response.status().as_u16();
    let value: Value = response
        .text()
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if status == 200 {
        Ok(value)
    } else {
        Err(match value["error"].as_str() {
            Some("invalid_grant") => {
                "Google sign-in expired (testing-mode sign-ins last 7 days). Sign in again.".into()
            }
            Some("invalid_client") => {
                "Google doesn't recognise that OAuth client. Check the client ID and secret.".into()
            }
            Some(other) => format!("Google refused: {other}"),
            None => format!("Google error {status}"),
        })
    }
}

/// A fresh access token for `tokens` (refreshing if needed).
pub fn access(client: &OAuthClient, tokens: &mut Tokens) -> Result<String, String> {
    if Instant::now() < tokens.expires && !tokens.access.is_empty() {
        return Ok(tokens.access.clone());
    }
    let http = crate::live::client()?;
    let value = post_form(
        &http,
        &[
            ("client_id", client.id.as_str()),
            ("client_secret", client.secret.as_str()),
            ("refresh_token", tokens.refresh.as_str()),
            ("grant_type", "refresh_token"),
        ],
    )?;
    tokens.access = value["access_token"]
        .as_str()
        .ok_or("Google returned no access token")?
        .to_owned();
    tokens.expires = Instant::now()
        + Duration::from_secs(
            value["expires_in"]
                .as_u64()
                .unwrap_or(3000)
                .saturating_sub(60),
        );
    Ok(tokens.access.clone())
}

/// Post a chat message (liveChatMessages.insert, 50 quota units).
pub fn send(access: &str, live_chat: &str, text: &str) -> Result<(), String> {
    let http = crate::live::client()?;
    let body = json!({"snippet": {"liveChatId": live_chat, "type": "textMessageEvent", "textMessageDetails": {"messageText": text}}});
    let response = http
        .post("https://www.googleapis.com/youtube/v3/liveChat/messages")
        .query(&[("part", "snippet")])
        .bearer_auth(access)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .map_err(|_| "Couldn't reach YouTube".to_owned())?;
    let status = response.status().as_u16();
    if status == 200 {
        return Ok(());
    }
    let text = response.text().unwrap_or_default();
    Err(if text.contains("quota") {
        let reset = crate::youtube::next_quota_reset(chrono::Utc::now());
        format!(
            "YouTube's daily API limit is used up (back at {}). Draft kept.",
            reset.with_timezone(&chrono::Local).format("%-I:%M %p")
        )
    } else if text.contains("forbidden") || status == 403 {
        "YouTube didn't allow that message (slow mode, members-only or you're timed out). Draft kept.".into()
    } else {
        format!("YouTube refused the message ({status}). Draft kept.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_parsing() {
        let json = r#"{"installed":{"client_id":"123-abc.apps.googleusercontent.com","client_secret":"s3cret"}}"#;
        assert_eq!(
            parse_client(json, "").unwrap().id,
            "123-abc.apps.googleusercontent.com"
        );
        assert!(parse_client("123-abc.apps.googleusercontent.com", "s").is_some());
        assert!(parse_client("not-a-client", "s").is_none());
        assert_ne!(random(16).unwrap(), random(16).unwrap());
    }
}
