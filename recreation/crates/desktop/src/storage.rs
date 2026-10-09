//! Local, versioned workspace preferences. Credentials never belong here.
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
fn path() -> Result<PathBuf, String> {
    let root = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("No user settings directory")?;
    Ok(PathBuf::from(root)
        .join("ChatWorkbench")
        .join("workspace.json"))
}
pub fn load() -> Result<Value, String> {
    let path = path()?;
    if !path.exists() {
        return Ok(json!({"version":1}));
    }
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("Workspace file is too large; original left untouched".into());
    }
    let value: Value = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Workspace could not be read; original left untouched: {e}"))?;
    if value["version"] != 1 {
        return Err("Unsupported workspace version; original left untouched".into());
    }
    Ok(value)
}
pub fn save(value: &Value) -> Result<(), String> {
    let path = path()?;
    let parent = path.parent().ok_or("Invalid settings path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Workspace exceeds local storage limit".into());
    }
    let temporary = parent.join(format!("workspace-{}.tmp", std::process::id()));
    fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    if path.exists() {
        fs::copy(&path, parent.join("workspace.previous.json")).map_err(|e| e.to_string())?;
    }
    fs::rename(temporary, path).map_err(|e| e.to_string())
}
