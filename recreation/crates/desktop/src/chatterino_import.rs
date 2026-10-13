//! Read-only import of an existing Chatterino installation: channel layout,
//! highlights, nicknames, ignored phrases, commands and display preferences.
//! Chatterino's files are never written. Formats follow Chatterino's
//! WindowManager/WindowDescriptors, Settings.hpp, HighlightPhrase,
//! IgnorePhrase, Nickname and CommandController serialization.
use crate::dock::Dock;
use crate::highlight_rules::{COLORS, Rule};
use crate::ignores::Phrase;
use serde_json::Value;
use std::{io::Read, path::PathBuf};

/// Chatterino's own files are small; refuse anything unexpectedly large.
const MAX_FILE: u64 = 8 * 1024 * 1024;
/// Bound work on hostile or corrupt layouts.
const MAX_CHANNELS: usize = 400;
const MAX_DEPTH: usize = 16;

pub struct Plan {
    pub source: PathBuf,
    /// Workspaces to create, each with unique channel names.
    pub workspaces: Vec<(String, Dock)>,
    pub highlights: Vec<Rule>,
    /// Twitch login and local display alias.
    pub nicknames: Vec<(String, String)>,
    /// Chatterino's regex nicknames, as Jawjack nickname rules.
    pub nickname_rules: Vec<crate::nickname_rules::Rule>,
    pub ignored_phrases: Vec<Phrase>,
    pub commands: Vec<(String, String)>,
    /// Jawjack timestamp mode: 0 off, 1 hours:minutes, 2 with seconds.
    pub timestamps: Option<u8>,
    pub font_size: Option<f32>,
    pub history_limit: Option<usize>,
    /// Human-readable reasons for everything not carried over.
    pub skipped: Vec<String>,
}
impl Plan {
    pub fn channel_count(&self) -> usize {
        self.workspaces
            .iter()
            .map(|(_, d)| {
                let mut names = Vec::new();
                d.names(&mut names);
                names.len()
            })
            .sum()
    }
}

/// Chatterino's settings directory for the current Windows user, if present.
pub fn locate() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?)
        .join("Chatterino2")
        .join("Settings");
    dir.join("window-layout.json").is_file().then_some(dir)
}

fn read_json(path: &std::path::Path) -> Result<Option<Value>, String> {
    let Ok(file) = std::fs::File::open(path) else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| format!("Could not read {}", display(path)))?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(format!("{} is unexpectedly large", display(path)));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| format!("{} is not valid JSON", display(path)))
}
fn display(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn read(dir: PathBuf) -> Result<Plan, String> {
    let layout = read_json(&dir.join("window-layout.json"))?
        .ok_or("Chatterino's window-layout.json was not found")?;
    let settings = read_json(&dir.join("settings.json"))?.unwrap_or(Value::Null);
    let commands = read_json(&dir.join("commands.json"))?.unwrap_or(Value::Null);
    let mut plan = Plan {
        source: dir,
        workspaces: Vec::new(),
        highlights: Vec::new(),
        nicknames: Vec::new(),
        nickname_rules: Vec::new(),
        ignored_phrases: Vec::new(),
        commands: Vec::new(),
        timestamps: None,
        font_size: None,
        history_limit: None,
        skipped: Vec::new(),
    };
    read_layout(&layout, &mut plan)?;
    read_highlights(&settings, &mut plan);
    read_nicknames(&settings, &mut plan);
    read_ignores(&settings, &mut plan);
    read_commands(&commands, &mut plan);
    read_preferences(&settings, &mut plan);
    Ok(plan)
}

/// Single-channel tabs (the common Chatterino setup) become one tabbed
/// workspace; tabs with several splits keep their geometry as their own workspace.
fn read_layout(layout: &Value, plan: &mut Plan) -> Result<(), String> {
    let windows = layout["windows"]
        .as_array()
        .ok_or("Chatterino's window layout has no windows")?;
    // The main window first, then popups, matching Chatterino's own load order.
    let mut ordered: Vec<&Value> = windows.iter().filter(|w| w["type"] == "main").collect();
    ordered.extend(windows.iter().filter(|w| w["type"] != "main"));
    let mut deck: Vec<String> = Vec::new();
    let mut active: Option<String> = None;
    let mut duplicates: Vec<String> = Vec::new();
    let mut skipped_splits = std::collections::BTreeMap::<String, usize>::new();
    let mut total = 0usize;
    for window in ordered {
        for tab in window["tabs"].as_array().into_iter().flatten() {
            let mut names = Vec::new();
            let Some(dock) = node(
                &tab["splits2"],
                0,
                &mut names,
                &mut skipped_splits,
                &mut total,
            ) else {
                continue;
            };
            if let Dock::Leaf(channel) = &dock {
                if !deck.contains(channel) {
                    deck.push(channel.clone());
                } else if !duplicates.contains(channel) {
                    duplicates.push(channel.clone());
                }
                if tab["selected"].as_bool() == Some(true) && active.is_none() {
                    active = Some(channel.clone());
                }
            } else {
                let title = tab["title"]
                    .as_str()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(|t| t.chars().take(40).collect())
                    .unwrap_or_else(|| {
                        let joined = names
                            .iter()
                            .take(3)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(" + ");
                        format!("Chatterino · {joined}").chars().take(40).collect()
                    });
                plan.workspaces.push((title, dock));
            }
        }
    }
    if !deck.is_empty() {
        let dock = if deck.len() == 1 {
            Dock::Leaf(deck.remove(0))
        } else {
            let active = active
                .filter(|a| deck.contains(a))
                .unwrap_or_else(|| deck[0].clone());
            Dock::Deck {
                channels: deck,
                active,
            }
        };
        plan.workspaces.insert(0, ("Chatterino".into(), dock));
    }
    if !duplicates.is_empty() {
        let list = duplicates
            .iter()
            .map(|c| format!("#{c}"))
            .collect::<Vec<_>>()
            .join(", ");
        plan.skipped
            .push(format!("Duplicate tabs merged into one: {list}"));
    }
    for (kind, count) in skipped_splits {
        plan.skipped.push(format!(
            "{count} {kind} split(s): Jawjack has no equivalent view yet"
        ));
    }
    if total > MAX_CHANNELS {
        plan.skipped.push(format!(
            "Channels beyond the first {MAX_CHANNELS} were not read"
        ));
    }
    if plan.workspaces.is_empty() {
        return Err("No Twitch channels were found in Chatterino's layout".into());
    }
    Ok(())
}
/// Convert a Chatterino split node. Channel names are unique within a workspace.
fn node(
    value: &Value,
    depth: usize,
    names: &mut Vec<String>,
    skipped: &mut std::collections::BTreeMap<String, usize>,
    total: &mut usize,
) -> Option<Dock> {
    if depth > MAX_DEPTH {
        return None;
    }
    match value["type"].as_str()? {
        "split" => {
            let data = &value["data"];
            let kind = data["type"].as_str().unwrap_or("unknown");
            if kind != "twitch" {
                *skipped.entry(kind.to_owned()).or_default() += 1;
                return None;
            }
            let raw = data["name"].as_str().or(data["channel"].as_str())?;
            let login = chat_core::twitch_login(raw.trim().trim_start_matches('#'))?;
            *total += 1;
            if *total > MAX_CHANNELS || names.contains(&login) {
                return None;
            }
            names.push(login.clone());
            Some(Dock::Leaf(login))
        }
        kind @ ("horizontal" | "vertical") => {
            let vertical = kind == "vertical";
            let mut children = Vec::new();
            let mut weights = Vec::new();
            for item in value["items"].as_array().into_iter().flatten() {
                if let Some(child) = node(item, depth + 1, names, skipped, total) {
                    let flex = item[if vertical { "flexv" } else { "flexh" }]
                        .as_f64()
                        .filter(|f| f.is_finite() && *f > 0.)
                        .unwrap_or(1.);
                    weights.push(flex as f32);
                    children.push(child);
                }
            }
            match children.len() {
                0 => None,
                1 => children.pop(),
                _ => Some(Dock::Split {
                    vertical,
                    weights,
                    children,
                }),
            }
        }
        _ => None,
    }
}

/// Nearest Jawjack accent for a Chatterino "#AARRGGBB" / "#RRGGBB" color.
fn nearest_color(value: &Value) -> usize {
    let Some(hex) = value.as_str().and_then(|s| s.strip_prefix('#')) else {
        return 0;
    };
    let rgb = match hex.len() {
        8 => u32::from_str_radix(&hex[2..], 16).ok(),
        6 => u32::from_str_radix(hex, 16).ok(),
        _ => None,
    };
    let Some(rgb) = rgb else { return 0 };
    let split = |c: u32| [(c >> 16) & 255, (c >> 8) & 255, c & 255].map(|v| v as f32);
    let target = split(rgb);
    COLORS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let d = |c: u32| {
                let c = split(c);
                (0..3).map(|i| (c[i] - target[i]).powi(2)).sum::<f32>()
            };
            d(a.1).total_cmp(&d(b.1))
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}
fn read_highlights(settings: &Value, plan: &mut Plan) {
    for (path, author) in [("highlights", false), ("users", true)] {
        for item in settings["highlighting"][path]
            .as_array()
            .into_iter()
            .flatten()
        {
            let pattern = item["pattern"].as_str().unwrap_or("");
            let mut rule = Rule::default();
            rule.pattern = pattern.into();
            rule.regex = item["regex"].as_bool().unwrap_or(false);
            rule.author = author;
            rule.case_sensitive = item["case"].as_bool().unwrap_or(false);
            // Chatterino surrounds plain phrases with word boundaries.
            rule.whole_word = true;
            rule.color = nearest_color(&item["color"]);
            match rule.validate() {
                Ok(rule) => plan.highlights.push(rule),
                Err(reason) => plan.skipped.push(format!(
                    "Highlight \u{201c}{}\u{201d}: {reason}",
                    short(pattern)
                )),
            }
        }
    }
    let badges = settings["highlighting"]["badges"]
        .as_array()
        .map_or(0, Vec::len);
    if badges > 0 {
        plan.skipped
            .push(format!("{badges} badge highlight(s): not supported yet"));
    }
    let blacklist = settings["highlighting"]["blacklist"]
        .as_array()
        .map_or(0, Vec::len);
    if blacklist > 0 {
        plan.skipped.push(format!(
            "{blacklist} highlight-blacklisted user(s): not supported yet"
        ));
    }
}
fn read_nicknames(settings: &Value, plan: &mut Plan) {
    for item in settings["nicknames"].as_array().into_iter().flatten() {
        let name = item["name"].as_str().unwrap_or("");
        if item["isRegex"].as_bool() == Some(true) {
            match crate::nickname_rules::validate(crate::nickname_rules::Rule {
                pattern: name.into(),
                replace: item["replace"].as_str().unwrap_or("").into(),
                regex: true,
                case_sensitive: item["isCaseSensitive"].as_bool().unwrap_or(false),
            }) {
                Ok(rule) => plan.nickname_rules.push(rule),
                Err(problem) => plan.skipped.push(format!(
                    "Nickname rule \u{201c}{}\u{201d}: {problem}",
                    short(name)
                )),
            }
            continue;
        }
        let alias = item["replace"].as_str().unwrap_or("");
        match (
            chat_core::twitch_login(name.trim()),
            crate::chatter_appearance::validate_alias(alias),
        ) {
            (Some(login), Ok(alias)) if !alias.is_empty() => plan.nicknames.push((login, alias)),
            _ => plan.skipped.push(format!(
                "Nickname for \u{201c}{}\u{201d}: not a Twitch login or alias",
                short(name)
            )),
        }
    }
}
fn read_ignores(settings: &Value, plan: &mut Plan) {
    for item in settings["ignore"]["phrases"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let pattern = item["pattern"].as_str().unwrap_or("");
        let replace = (item["isBlock"].as_bool() != Some(true))
            .then(|| item["replaceWith"].as_str().unwrap_or("").to_owned());
        match Phrase::new(
            pattern,
            item["regex"].as_bool().unwrap_or(false),
            // Chatterino's ignore phrases default to case-sensitive.
            item["caseSensitive"].as_bool().unwrap_or(true),
            replace,
        ) {
            Ok(phrase) => plan.ignored_phrases.push(phrase),
            Err(reason) => plan.skipped.push(format!(
                "Ignored phrase \u{201c}{}\u{201d}: {reason}",
                short(pattern)
            )),
        }
    }
    if settings["ignore"]["enableTwitchBlockedUsers"].as_bool() != Some(false) {
        plan.skipped.push(
            "Blocked users live on your Twitch account, not in Chatterino's files; add chatters under Manage ignores".into(),
        );
    }
}
/// Map Chatterino placeholders to Jawjack's {1}–{9}, {args} and {channel}.
fn convert_template(func: &str) -> Result<String, String> {
    if func.trim_start().starts_with('/') {
        return Err("runs a Twitch command; Jawjack custom commands only prepare chat text".into());
    }
    let mut out = String::new();
    let mut rest = func;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start].replace('}', "}}"));
        let after = &rest[start + 1..];
        if let Some(stripped) = after.strip_prefix('{') {
            out.push_str("{{");
            rest = stripped;
            continue;
        }
        let end = after.find('}').ok_or("has an unclosed placeholder")?;
        let token = &after[..end];
        let mapped = match token {
            "channel" | "channel.name" => "{channel}".to_owned(),
            "1+" => "{args}".to_owned(),
            t if t.len() == 1 && t.as_bytes()[0].is_ascii_digit() && t != "0" => format!("{{{t}}}"),
            other => return Err(format!("uses {{{other}}}, which Jawjack does not support")),
        };
        out.push_str(&mapped);
        rest = &after[end + 1..];
    }
    out.push_str(&rest.replace('}', "}}"));
    Ok(out)
}
fn read_commands(commands: &Value, plan: &mut Plan) {
    for item in commands["commands"].as_array().into_iter().flatten() {
        let name = item["name"].as_str().unwrap_or("");
        let func = item["func"].as_str().unwrap_or("");
        let result = convert_template(func).and_then(|template| {
            crate::custom_commands::validate(name, &template).map(|n| (n, template))
        });
        match result {
            Ok(command) => plan.commands.push(command),
            Err(reason) => plan
                .skipped
                .push(format!("Command \u{201c}{}\u{201d}: {reason}", short(name))),
        }
    }
}
/// Display preferences, using Chatterino's defaults where a setting was never changed.
fn read_preferences(settings: &Value, plan: &mut Plan) {
    let messages = &settings["appearance"]["messages"];
    let show = messages["showTimestamps"].as_bool().unwrap_or(true);
    let format = messages["timestampFormat"].as_str().unwrap_or("h:mm");
    plan.timestamps = Some(match (show, format.contains('s')) {
        (false, _) => 0,
        (true, false) => 1,
        (true, true) => 2,
    });
    // Chatterino sizes are points; only an explicit choice is carried over.
    if let Some(points) = settings["appearance"]["currentFontSize"].as_f64() {
        plan.font_size = Some(((points * 96. / 72.).round() as f32).clamp(12., 24.));
    }
    if let Some(limit) = settings["misc"]["scrollback"]["splitLimit"].as_u64() {
        plan.history_limit = Some((limit as usize).clamp(500, 10_000));
    }
}
fn short(text: &str) -> String {
    let mut s: String = text.chars().take(32).collect();
    if text.chars().count() > 32 {
        s.push('…');
    }
    s
}

pub struct Apply;
impl gpui_kit::EventEmitter<Apply> for Preview {}
/// The confirmation step: shows exactly what will be created and what was skipped.
pub struct Preview {
    pub plan: Option<Plan>,
    error: Option<String>,
    show_all_skipped: bool,
}
impl Preview {
    pub fn load() -> Self {
        let (plan, error) = match locate()
            .ok_or_else(|| "No Chatterino settings were found for this Windows user.".to_string())
            .and_then(read)
        {
            Ok(plan) => (Some(plan), None),
            Err(e) => (None, Some(e)),
        };
        Self {
            plan,
            error,
            show_all_skipped: false,
        }
    }
}
impl gpui_kit::Render for Preview {
    fn render(
        &mut self,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use crate::theme;
        use gpui_kit::component::{Disableable, Sizable, StyledExt, button::Button};
        use gpui_kit::prelude::FluentBuilder;
        use gpui_kit::*;
        let height = (f32::from(window.viewport_size().height) - 150.).clamp(120., 560.);
        let muted = |text: String| {
            div()
                .text_size(px(11.))
                .text_color(rgb(theme::muted()))
                .child(text)
        };
        let mut body = div()
            .id("chatterino-import")
            .v_flex()
            .gap_2()
            .max_h(px(height))
            .overflow_y_scroll();
        let Some(plan) = &self.plan else {
            return body.child(
                div()
                    .text_color(rgb(crate::theme::tone(0xE9C99C)))
                    .child(self.error.clone().unwrap_or_default()),
            );
        };
        body = body.child(muted(format!(
            "Reads {} without changing it. Your current workspaces are kept; imported items are added.",
            plan.source.display()
        )));
        let mut lines = Vec::new();
        for (name, dock) in &plan.workspaces {
            let mut names = Vec::new();
            dock.names(&mut names);
            lines.push(format!(
                "New workspace \u{201c}{name}\u{201d} · {} channel{}",
                names.len(),
                if names.len() == 1 { "" } else { "s" }
            ));
        }
        let count =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        lines.push(count(
            plan.highlights.len(),
            "highlight rule",
            "highlight rules",
        ));
        lines.push(count(plan.nicknames.len(), "nickname", "nicknames"));
        if !plan.nickname_rules.is_empty() {
            lines.push(count(
                plan.nickname_rules.len(),
                "nickname rule",
                "nickname rules",
            ));
        }
        lines.push(count(
            plan.ignored_phrases.len(),
            "ignored phrase",
            "ignored phrases",
        ));
        lines.push(count(
            plan.commands.len(),
            "custom command",
            "custom commands",
        ));
        if let Some(mode) = plan.timestamps {
            lines.push(format!(
                "Timestamps: {}",
                ["off", "on (hours:minutes)", "on (with seconds)"][mode as usize]
            ));
        }
        if let Some(size) = plan.font_size {
            lines.push(format!("Chat text size: {size} px"));
        }
        if let Some(limit) = plan.history_limit {
            lines.push(format!("Messages kept per channel: {limit}"));
        }
        body = body.children(
            lines
                .into_iter()
                .map(|l| div().text_size(px(13.)).child(format!("• {l}"))),
        );
        if plan.channel_count() > crate::live::MAX_CHANNELS {
            body = body.child(muted(format!(
                "Twitch allows Jawjack about {} live channels at once. The workspace you are viewing connects first; others connect when there is room.",
                crate::live::MAX_CHANNELS
            )));
        }
        if !plan.skipped.is_empty() {
            let shown = if self.show_all_skipped {
                plan.skipped.len()
            } else {
                plan.skipped.len().min(6)
            };
            body = body
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("Not imported ({})", plan.skipped.len())),
                )
                .children(
                    plan.skipped
                        .iter()
                        .take(shown)
                        .map(|s| muted(format!("· {s}"))),
                )
                .when(plan.skipped.len() > shown, |el| {
                    el.child(
                        Button::new("import-more")
                            .xsmall()
                            .label(format!("Show {} more", plan.skipped.len() - shown))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_all_skipped = true;
                                cx.notify();
                            })),
                    )
                });
        }
        body.child(
            div().h_flex().gap_1().pt_1().child(
                Button::new("import-apply")
                    .small()
                    .label("Import")
                    .disabled(self.plan.is_none())
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(Apply))),
            ),
        )
    }
}

/// A resolved nickname: Twitch user ID, login, display name and local alias.
pub type Nickname = (String, String, String, String);
/// Jawjack keys nicknames by immutable user ID; look logins up in batches of 100.
pub fn resolve_nicknames(
    identity: crate::live::Identity,
    nicknames: Vec<(String, String)>,
) -> std::sync::mpsc::Receiver<Vec<Nickname>> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
        else {
            let _ = tx.send(Vec::new());
            return;
        };
        let mut resolved = Vec::new();
        for batch in nicknames.chunks(100) {
            let query: Vec<(&str, &str)> =
                batch.iter().map(|(l, _)| ("login", l.as_str())).collect();
            let Ok(response) = client
                .get("https://api.twitch.tv/helix/users")
                .query(&query)
                .header("Client-Id", crate::auth::CLIENT_ID)
                .bearer_auth(&identity.access)
                .send()
            else {
                continue;
            };
            let mut bytes = Vec::new();
            if response.take(1024 * 1024).read_to_end(&mut bytes).is_err() {
                continue;
            }
            let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            for user in value["data"].as_array().into_iter().flatten() {
                let (Some(id), Some(login)) = (user["id"].as_str(), user["login"].as_str()) else {
                    continue;
                };
                if let Some((_, alias)) = batch.iter().find(|(l, _)| l.eq_ignore_ascii_case(login))
                {
                    let display = user["display_name"].as_str().unwrap_or(login).trim();
                    resolved.push((
                        id.into(),
                        login.to_ascii_lowercase(),
                        display.into(),
                        alias.clone(),
                    ));
                }
            }
        }
        let _ = tx.send(resolved);
    });
    rx
}
