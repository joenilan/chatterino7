//! Local, versioned workspace preferences. Credentials never belong here.
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
/// Jawjack's settings folder (Settings → General → Open data folder).
pub fn folder() -> Option<PathBuf> {
    path().ok().and_then(|p| p.parent().map(PathBuf::from))
}
fn path() -> Result<PathBuf, String> {
    Ok(crate::profile::data_root()
        .ok_or("No user settings directory")?
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
/// Daily backups: `backups/workspace-YYYY-MM-DD.json`, newest seven kept.
const BACKUPS_KEPT: usize = 7;
fn backup_folder() -> Result<PathBuf, String> {
    Ok(path()?
        .parent()
        .ok_or("Invalid settings path")?
        .join("backups"))
}
/// At startup, copy today's settings once (before anything is saved).
pub fn backup_daily() {
    let (Ok(path), Ok(folder)) = (path(), backup_folder()) else {
        return;
    };
    if !path.exists() || fs::create_dir_all(&folder).is_err() {
        return;
    }
    let today = folder.join(format!(
        "workspace-{}.json",
        chrono::Local::now().format("%Y-%m-%d")
    ));
    if !today.exists() {
        let _ = fs::copy(&path, &today);
    }
    for (_, old) in backups().into_iter().skip(BACKUPS_KEPT) {
        let _ = fs::remove_file(old);
    }
}
/// Backups, newest first: (date "YYYY-MM-DD", file).
pub fn backups() -> Vec<(String, PathBuf)> {
    let Ok(folder) = backup_folder() else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let date = name.strip_prefix("workspace-")?.strip_suffix(".json")?;
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
            Some((date.to_owned(), entry.path()))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found
}
/// Replace the settings with a backup (checked first). Jawjack must then close
/// without saving, so the restored file is what opens next time.
pub fn restore(backup: &std::path::Path) -> Result<(), String> {
    if fs::metadata(backup).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("That backup is too large.".into());
    }
    let value: Value = serde_json::from_slice(&fs::read(backup).map_err(|e| e.to_string())?)
        .map_err(|_| "That backup can't be read.".to_owned())?;
    if value["version"] != 1 {
        return Err("That backup is from an unsupported version.".into());
    }
    save(&value)
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
