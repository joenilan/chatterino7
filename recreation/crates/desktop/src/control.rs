//! Explicit per-run local app control. No server or endpoint on normal startup.
use crate::Workbench;
use gpui_kit::*;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_BYTES: u64 = 16 * 1024;
fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn read_json(path: &Path) -> Result<Value, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Control frame exceeds 16 KiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn publish(path: &Path, value: &Value) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(value.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&temporary, path).map_err(|e| e.to_string())
}
fn session_path(name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name.len() > 48
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("Session name must be 1–48 ASCII letters, digits or hyphens".into());
    }
    // Use the current user's private application-data hierarchy, never a shared
    // repository, network path or arbitrary caller-selected filesystem location.
    let base = std::env::var_os("LOCALAPPDATA")
        .ok_or("Local control currently requires Windows LOCALAPPDATA")?;
    Ok(PathBuf::from(base)
        .join("ChatWorkbench")
        .join("control")
        .join(name))
}
pub struct Call {
    pub request: Value,
    pub deadline: u128,
    pub reply: mpsc::SyncSender<Value>,
}
pub struct Server {
    pub calls: mpsc::Receiver<Call>,
}

pub fn start(name: &str) -> Result<Server, String> {
    let directory = session_path(name)?;
    fs::create_dir_all(directory.parent().ok_or("Invalid session path")?)
        .map_err(|e| e.to_string())?;
    fs::create_dir(&directory).map_err(|e| format!("Use a fresh session name: {e}"))?;
    let session = format!("{}-{}", std::process::id(), now());
    publish(
        &directory.join("session.json"),
        &json!({"session":session,"pid":std::process::id()}),
    )?;
    let (send, calls) = mpsc::sync_channel::<Call>(1);
    std::thread::spawn(move || {
        let mut last_id = String::new();
        loop {
            let request_path = directory.join("request.json");
            if request_path.exists() {
                let frame = read_json(&request_path);
                let _ = fs::remove_file(&request_path);
                let response = match frame {
                    Ok(frame) => {
                        let id = frame["id"].as_str().unwrap_or("").to_owned();
                        let deadline = frame["deadline"].as_u64().unwrap_or(0) as u128;
                        let result = if frame["session"] != session
                            || id.is_empty()
                            || id == last_id
                            || deadline < now()
                            || deadline > now() + 10_000
                        {
                            json!({"error":"Stale, duplicate or invalid control request"})
                        } else {
                            last_id = id.clone();
                            let (reply, receive) = mpsc::sync_channel(1);
                            match send.try_send(Call { request: frame["request"].clone(), deadline, reply }) {
                                Ok(()) => receive.recv_timeout(Duration::from_millis(deadline.saturating_sub(now()).min(5000) as u64))
                                    .unwrap_or_else(|_| json!({"error":"Timed out; inspect before retrying a mutation"})),
                                Err(mpsc::TrySendError::Full(_)) => json!({"error":"App busy; request was not queued"}),
                                Err(mpsc::TrySendError::Disconnected(_)) => break,
                            }
                        };
                        json!({"session":session,"id":id,"result":result})
                    }
                    Err(error) => json!({"error":error}),
                };
                if publish(&directory.join("response.json"), &response).is_err() {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(33));
        }
    });
    Ok(Server { calls })
}

/// One bounded call to an explicitly launched control session; no GUI is opened.
pub fn client(name: &str, request: &str) -> Result<Value, String> {
    if request.len() as u64 > MAX_BYTES / 2 {
        return Err("Request too large".into());
    }
    let request: Value = serde_json::from_str(request).map_err(|e| e.to_string())?;
    let directory = session_path(name)?;
    let session = read_json(&directory.join("session.json"))?["session"].clone();
    let _lock = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("caller.lock"))
        .map_err(|_| "Another call is active; do not overlap calls")?;
    let result = (|| {
        if directory.join("request.json").exists() || directory.join("response.json").exists() {
            return Err(
                "Previous call unresolved; start a fresh session rather than replaying it".into(),
            );
        }
        let id = format!("{}-{}", std::process::id(), now());
        publish(
            &directory.join("request.json"),
            &json!({"session":session,"id":id,"deadline":(now()+5000) as u64,"request":request}),
        )?;
        let started = std::time::Instant::now();
        loop {
            let path = directory.join("response.json");
            if path.exists() {
                let response = read_json(&path)?;
                if response["id"] != id || response["session"] != session {
                    return Err("Response identity mismatch".into());
                }
                fs::remove_file(path).map_err(|e| e.to_string())?;
                return Ok(response["result"].clone());
            }
            if started.elapsed() > Duration::from_secs(6) {
                return Err(
                    "Call timed out; outcome unknown. Do not repeat the mutation blindly.".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(33));
        }
    })();
    drop(_lock);
    let _ = fs::remove_file(directory.join("caller.lock"));
    result
}

fn number(request: &Value, key: &str, max: f64) -> Result<f32, String> {
    let n = request[key]
        .as_f64()
        .ok_or_else(|| format!("Missing {key}"))?;
    if !n.is_finite() || n < 0. || n > max {
        return Err(format!("Invalid {key}"));
    }
    Ok(n as f32)
}
pub fn dispatch(
    view: &Entity<Workbench>,
    request: &Value,
    window: &mut Window,
    cx: &mut App,
) -> Result<Value, String> {
    match request["method"].as_str().ok_or("Missing method")? {
        "inspect" => {
            let panes: Vec<_> = view.read(cx).visible_panes().iter().map(|pane| {
                let pane = pane.read(cx);
                let bounds = pane.viewport.borrow().map(|b| json!({"x":f32::from(b.left()),"y":f32::from(b.top()),"width":f32::from(b.size.width),"height":f32::from(b.size.height)}));
                json!({"channel":pane.name.to_string(),"messages":pane.timeline.borrow().messages().len(),"following":pane.scroller.read(cx).is_following_tail(),"viewport":bounds,"selection":format!("{:?}",pane.selection.borrow()),"draft_characters":pane.draft.read(cx).value().chars().count(),"emote_picker_open":pane.picker.open,"emote_picker_matches":pane.picker.choices.len(),"emote_picker_page":pane.picker.page,"emote_suggestions":pane.picker.suggestions.iter().map(|c|c.label.clone()).collect::<Vec<_>>(),"last_copy_result":pane.last_copy_result,"connected":pane.connected,"connection_status":pane.connection,"send_pending":pane.pending.is_some(),"entrance_effects":pane.entrances.iter().filter(|entry| entry.active(std::time::Instant::now())).count(),"entrance_started":pane.entrances.iter().filter(|entry|entry.started.get().is_some() && entry.active(std::time::Instant::now())).count()})
            }).collect();
            Ok(
                json!({"workspace":view.read(cx).inspection(cx),"visible_panes":panes,"offline":!view.read(cx).panes().iter().any(|p|p.read(cx).connected),"control":"session-only","media":view.read(cx).media_inspection(),"reduced_motion":cx.reduce_motion(),"window_active":window.is_window_active()}),
            )
        }
        "focus" => {
            let target = request["target"].as_str().ok_or("Missing focus target")?;
            let pane = request["pane"].as_u64().unwrap_or(0) as usize;
            view.update(cx, |view, cx| view.focus_target(target, pane, window, cx))?;
            Ok(json!({"focused":true}))
        }
        "key" => {
            let name = request["key"]
                .as_str()
                .filter(|s| s.len() <= 80)
                .ok_or("Missing key")?;
            let key = Keystroke::parse(name).map_err(|e| e.to_string())?;
            if matches!(key.key.as_str(), "enter" | "tab") {
                window.dispatch_event(
                    KeyDownEvent {
                        keystroke: key.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    }
                    .to_platform_input(),
                    cx,
                );
            } else {
                window.dispatch_keystroke(key.clone(), cx);
            }
            window.dispatch_event(KeyUpEvent { keystroke: key }.to_platform_input(), cx);
            Ok(json!({"dispatched":true}))
        }
        "text" => {
            let text = request["text"]
                .as_str()
                .filter(|s| s.len() <= 2048)
                .ok_or("Text must be at most 2048 bytes")?;
            if text.chars().any(char::is_control) {
                return Err(
                    "Use key calls for control keys; text must contain printable characters".into(),
                );
            }
            for character in text.chars() {
                let text = character.to_string();
                let mut key = Keystroke::parse(&text).map_err(|e| e.to_string())?;
                key.key_char = Some(text);
                window.dispatch_keystroke(key, cx);
            }
            Ok(json!({"dispatched_characters":text.chars().count()}))
        }
        "pointer" => {
            let position = point(
                px(number(request, "x", 16384.)?),
                px(number(request, "y", 16384.)?),
            );
            let button = match request["button"].as_str().unwrap_or("left") {
                "left" => MouseButton::Left,
                "right" => MouseButton::Right,
                "middle" => MouseButton::Middle,
                _ => return Err("Unsupported button".into()),
            };
            let modifiers = Modifiers {
                shift: request["shift"].as_bool().unwrap_or(false),
                ..Default::default()
            };
            match request["kind"].as_str().ok_or("Missing pointer kind")? {
                "move" => {
                    window.dispatch_event(
                        MouseMoveEvent {
                            position,
                            pressed_button: request["held"]
                                .as_bool()
                                .unwrap_or(false)
                                .then_some(button),
                            modifiers,
                        }
                        .to_platform_input(),
                        cx,
                    );
                }
                "down" => {
                    window.dispatch_event(
                        MouseDownEvent {
                            position,
                            button,
                            modifiers,
                            click_count: 1,
                            first_mouse: false,
                        }
                        .to_platform_input(),
                        cx,
                    );
                }
                "up" => {
                    window.dispatch_event(
                        MouseUpEvent {
                            position,
                            button,
                            modifiers,
                            click_count: 1,
                        }
                        .to_platform_input(),
                        cx,
                    );
                }
                _ => return Err("Use down/move/up, one event per call".into()),
            }
            Ok(json!({"dispatched":true}))
        }
        "scroll" => {
            let position = point(
                px(number(request, "x", 16384.)?),
                px(number(request, "y", 16384.)?),
            );
            let lines = request["lines"]
                .as_i64()
                .filter(|n| (-120..=120).contains(n))
                .ok_or("Invalid scroll lines")?;
            window.dispatch_event(
                ScrollWheelEvent {
                    position,
                    delta: ScrollDelta::Lines(point(0., lines as f32)),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            Ok(json!({"dispatched":true}))
        }
        "resize" => {
            let width = number(request, "width", 8192.)?;
            let height = number(request, "height", 8192.)?;
            if width < 1050. || height < 640. {
                return Err("Minimum size is 1050 by 640".into());
            }
            window.resize(size(px(width), px(height)));
            Ok(json!({"requested":true}))
        }
        _ => Err(
            "Unknown method; supported: inspect, focus, key, text, pointer, scroll, resize".into(),
        ),
    }
}
pub fn attach(server: Server, entity: &Entity<Workbench>, window: &mut Window, cx: &mut App) {
    let weak = entity.downgrade();
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(33))
                .await;
            let Some(entity) = weak.upgrade() else { break };
            let Ok(call) = server.calls.try_recv() else {
                continue;
            };
            let result = cx.update(|app| {
                app.update_window(handle, |_, window, app| {
                    if call.deadline < now() {
                        return json!({"error":"Request expired before execution"});
                    }
                    // Drive the current layout; event handling below is the real GPUI
                    // route, not a direct mutation of selection or draft state.
                    window.draw(app).clear(app);
                    let result = dispatch(&entity, &call.request, window, app);
                    window.refresh();
                    match result {
                        Ok(value) => value,
                        Err(error) => json!({"error":error}),
                    }
                })
            });
            let response = match result {
                Ok(value) => value,
                _ => json!({"error":"Window unavailable"}),
            };
            let _ = call.reply.send(response);
        }
    })
    .detach();
}
