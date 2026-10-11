//! Guided setup for connections that need something done outside Jawjack:
//! numbered steps, buttons that open the exact page, a field to paste into,
//! and a Test that proves it works before anything is saved.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Guide {
    YouTube,
    /// Sending on YouTube: an OAuth client and Sign in with Google.
    YouTubeChat,
    Obs,
    StreamerBot,
}

struct Step {
    title: &'static str,
    detail: &'static str,
    link: Option<(&'static str, &'static str)>,
}

fn steps(guide: Guide) -> Vec<Step> {
    match guide {
        Guide::YouTube => vec![
            Step {
                title: "Create a Google Cloud project",
                detail: "Free, no billing needed. Any name works, e.g. \"Jawjack\".",
                link: Some((
                    "Open Google Cloud",
                    "https://console.cloud.google.com/projectcreate",
                )),
            },
            Step {
                title: "Turn on the YouTube Data API v3",
                detail: "With your new project selected, press Enable.",
                link: Some((
                    "Open the YouTube API page",
                    "https://console.cloud.google.com/apis/library/youtube.googleapis.com",
                )),
            },
            Step {
                title: "Create an API key",
                detail: "Credentials → Create credentials → API key. Optional but wise: Edit key → API restrictions → YouTube Data API v3.",
                link: Some((
                    "Open Credentials",
                    "https://console.cloud.google.com/apis/credentials",
                )),
            },
            Step {
                title: "Paste the key below and press Test",
                detail: "Jawjack checks it with YouTube, then keeps it in Windows Credential Manager. The free quota covers about 2–3 hours of live chat a day.",
                link: None,
            },
        ],
        Guide::YouTubeChat => vec![
            Step {
                title: "Use the same Google Cloud project",
                detail: "The one from the YouTube key. Sending needs Google sign-in, which apps can only do with their own OAuth client.",
                link: None,
            },
            Step {
                title: "Set up the sign-in screen and add yourself as a test user",
                detail: "Google Auth Platform → Get started: app name \"Jawjack\", your email, Audience: External. Then Audience → Test users → add the Google account you stream with.",
                link: Some((
                    "Open Audience (test users)",
                    "https://console.cloud.google.com/auth/audience",
                )),
            },
            Step {
                title: "Create an OAuth client: type \"Desktop app\"",
                detail: "Clients → Create client → Application type: Desktop app → Create. Copy the Client ID and Client secret (or download the JSON).",
                link: Some((
                    "Create OAuth client",
                    "https://console.cloud.google.com/auth/clients/create",
                )),
            },
            Step {
                title: "Paste them below and sign in",
                detail: "Your browser opens Google's consent page; approve YouTube access. While the project is in Testing, Google signs you out after 7 days (just sign in again). Each message uses 50 quota units.",
                link: None,
            },
        ],
        Guide::Obs => vec![
            Step {
                title: "In OBS: Tools → WebSocket Server Settings",
                detail: "Tick \"Enable WebSocket server\". OBS 28 or newer has it built in.",
                link: Some((
                    "OBS remote-control guide",
                    "https://obsproject.com/kb/remote-control-guide",
                )),
            },
            Step {
                title: "Press \"Show Connect Info\"",
                detail: "It shows the server IP, port (usually 4455) and password. Leave that window open.",
                link: None,
            },
            Step {
                title: "OBS on another PC?",
                detail: "Allow OBS through Windows Firewall on that PC (Private networks). Jawjack can find it for you on the next screen.",
                link: None,
            },
            Step {
                title: "Connect",
                detail: "Next: enter the address (or press Find OBS on my network) and the password.",
                link: None,
            },
        ],
        Guide::StreamerBot => vec![
            Step {
                title: "In Streamer.bot: Servers/Clients → WebSocket Server",
                detail: "Tick Auto Start, then press Start Server. The default address is 127.0.0.1, port 8080.",
                link: Some((
                    "Streamer.bot WebSocket docs",
                    "https://docs.streamer.bot/api/websocket",
                )),
            },
            Step {
                title: "Authentication (optional)",
                detail: "If you turned on \"Enable Authentication\", copy the password from the same screen.",
                link: None,
            },
            Step {
                title: "Connect",
                detail: "Next: enter the address and the password if you set one. Pin the actions you use live afterwards.",
                link: None,
            },
        ],
    }
}

/// Check a YouTube key with one cheap request (1 quota unit).
fn test_youtube_key(key: &str) -> Result<(), String> {
    let client = crate::live::client()?;
    let response = client
        .get("https://www.googleapis.com/youtube/v3/videos")
        .query(&[("part", "id"), ("id", "jNQXAC9IVRw"), ("key", key)])
        .send()
        .map_err(|_| "Couldn't reach YouTube. Check your connection.".to_owned())?;
    let status = response.status().as_u16();
    let body = response.text().unwrap_or_default();
    match status {
        200 => Ok(()),
        _ if body.contains("API_KEY_INVALID") || body.contains("keyInvalid") => Err("YouTube says that key isn't valid. Copy it again from Credentials.".into()),
        _ if body.contains("SERVICE_DISABLED") || body.contains("accessNotConfigured") => {
            Err("The key works, but the YouTube Data API v3 isn't enabled for its project yet (step 2). It can take a minute after enabling.".into())
        }
        _ if body.contains("quota") => Err("This key's daily quota is used up. It resets at midnight Pacific time.".into()),
        _ if body.contains("API_KEY_SERVICE_BLOCKED") || body.contains("blocked") => {
            Err("The key is restricted away from YouTube. In Credentials → Edit key, allow YouTube Data API v3.".into())
        }
        code => Err(format!("YouTube answered {code}. Check the key and try again.")),
    }
}

impl Workbench {
    pub(super) fn open_guide(&mut self, guide: Guide, window: &mut Window, cx: &mut Context<Self>) {
        let key_input = (guide == Guide::YouTube).then(|| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Paste your API key")
                    .masked(true)
            })
        });
        let client_inputs = (guide == Guide::YouTubeChat).then(|| {
            (
                cx.new(|cx| {
                    InputState::new(window, cx).placeholder(
                        "Client ID (…apps.googleusercontent.com) or the downloaded JSON",
                    )
                }),
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Client secret")
                        .masked(true)
                }),
            )
        });
        self.guide_status = None;
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(280., 460.);
        let (title, intro) = match guide {
            Guide::YouTube => (
                "Set up YouTube chat",
                "Jawjack reads YouTube live chat through YouTube's official API with your own free key. About 3 minutes.",
            ),
            Guide::YouTubeChat => (
                "Chat on YouTube",
                "To send messages, sign in with Google using your own OAuth client, a one-time set-up of about 5 minutes.",
            ),
            Guide::Obs => (
                "Connect OBS",
                "See if you're live, switch scenes, check your mic and stream health from Studio.",
            ),
            Guide::StreamerBot => (
                "Connect Streamer.bot",
                "Run your Streamer.bot actions from Studio and see YouTube, Kick and donation events in Activity.",
            ),
        };
        window.open_dialog(cx, move |dialog, _, cx| {
            let status = owner.upgrade().and_then(|o| o.read(cx).guide_status.clone());
            let steps = steps(guide);
            let count = steps.len();
            let list = steps.into_iter().enumerate().map(|(index, step)| {
                div()
                    .h_flex()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .size(px(22.))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(rgb(0x272237))
                            .text_color(rgb(theme::ACCENT))
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child((index + 1).to_string()),
                    )
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(step.title))
                            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(step.detail))
                            .when_some(step.link, |el, (label, url)| {
                                el.child(
                                    div().h_flex().child(
                                        Button::new(("guide-link", index))
                                            .xsmall()
                                            .icon(IconName::ExternalLink)
                                            .label(label)
                                            .on_click(move |_, _, cx| cx.open_url(url)),
                                    ),
                                )
                            }),
                    )
            });
            let body = div().v_flex().gap_3().child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child(intro)).children(list);
            let body = match &key_input {
                Some(input) => body.child(div().pl(px(34.)).child(Input::new(input).small().mask_toggle())),
                None => body,
            };
            let body = match &client_inputs {
                Some((id, secret)) => body.child(
                    div()
                        .pl(px(34.))
                        .v_flex()
                        .gap_1()
                        .child(Input::new(id).small())
                        .child(Input::new(secret).small().mask_toggle()),
                ),
                None => body,
            };
            let body = body.when_some(status, |el, (ok, text)| {
                el.child(
                    div()
                        .p_2()
                        .rounded(px(6.))
                        .bg(rgb(if ok { 0x192D27 } else { 0x3A1D20 }))
                        .text_size(px(12.))
                        .text_color(rgb(if ok { 0x91D7BA } else { 0xF2B8B8 }))
                        .child(text),
                )
            });
            let (next_owner, test_owner, test_input) = (owner.clone(), owner.clone(), key_input.clone());
            let (sign_owner, sign_inputs) = (owner.clone(), client_inputs.clone());
            dialog
                .w(px(width))
                .title(title)
                .on_ok(|_, _, _| false)
                .child(body)
                .footer(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().text_size(px(10.)).text_color(rgb(theme::MUTED)).child(format!("{count} steps")))
                        .child(Button::new("guide-close").small().ghost().label("Close").on_click(|_, w, cx| w.close_dialog(cx)))
                        .when(guide == Guide::YouTube, |el| {
                            el.child(Button::new("guide-test").small().primary().label("Test & save").on_click(move |_, window, cx| {
                                let Some(input) = &test_input else { return };
                                let key = input.read(cx).value().trim().to_owned();
                                if key.is_empty() {
                                    let _ = test_owner.update(cx, |this, cx| {
                                        this.guide_status = Some((false, "Paste the key from step 3 first.".into()));
                                        cx.notify();
                                    });
                                    window.refresh();
                                    return;
                                }
                                let owner = test_owner.clone();
                                let _ = owner.update(cx, |this, cx| {
                                    this.guide_status = Some((true, "Checking with YouTube…".into()));
                                    cx.notify();
                                });
                                let task = cx.background_spawn({
                                    let key = key.clone();
                                    async move { test_youtube_key(&key) }
                                });
                                window
                                    .spawn(cx, async move |cx| {
                                        let result = task.await;
                                        let _ = cx.update(|window, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                match result {
                                                    Ok(()) => {
                                                        this.save_youtube_key(key, cx);
                                                        this.guide_status = Some((
                                                            true,
                                                            "It works. Key saved. Paste a YouTube live link into Add channel (Ctrl+K) to open its chat.".into(),
                                                        ));
                                                    }
                                                    Err(e) => this.guide_status = Some((false, e)),
                                                }
                                                cx.notify();
                                            });
                                            window.refresh();
                                        });
                                    })
                                    .detach();
                            }))
                        })
                        .when(guide == Guide::YouTubeChat, |el| {
                            el.child(Button::new("guide-google").small().primary().label("Sign in with Google").on_click(move |_, window, cx| {
                                let Some((id, secret)) = &sign_inputs else { return };
                                let (id, secret) = (id.read(cx).value().to_string(), secret.read(cx).value().to_string());
                                let _ = sign_owner.update(cx, |this, cx| this.google_sign_in(&id, &secret, window, cx));
                                window.refresh();
                            }))
                        })
                        .when(guide == Guide::Obs || guide == Guide::StreamerBot, |el| {
                            el.child(Button::new("guide-next").small().primary().label("Next: connect").on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                let _ = next_owner.update(cx, |this, cx| match guide {
                                    Guide::Obs => this.open_obs_settings(window, cx),
                                    _ => this.open_bot_settings(window, cx),
                                });
                            }))
                        }),
                )
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "network"]
    fn fake_key_is_rejected() {
        let r = super::test_youtube_key("AIzaFakeKeyForJawjackTest123");
        assert!(r.is_err(), "{r:?}");
    }
}
