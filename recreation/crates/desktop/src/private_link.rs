//! "Open in a private window" for links (Chatterino's incognito links): finds
//! your default browser and starts it with its private-window switch.
//! Reads the registry through Windows' own `reg` tool (no unsafe code here).
use std::process::Command;

/// The private-window switch for a browser executable, if it has one we know.
fn private_switch(exe: &str) -> Option<&'static str> {
    let name = std::path::Path::new(exe)
        .file_name()?
        .to_string_lossy()
        .to_ascii_lowercase();
    Some(match name.as_str() {
        "chrome.exe" | "brave.exe" | "vivaldi.exe" | "chromium.exe" => "--incognito",
        "msedge.exe" => "--inprivate",
        "firefox.exe" | "librewolf.exe" | "waterfox.exe" => "-private-window",
        "opera.exe" => "--private",
        _ => return None,
    })
}

/// One value from `reg query` output.
fn reg_value(key: &str, value: Option<&str>) -> Option<String> {
    let mut command = Command::new("reg");
    command.arg("query").arg(key);
    match value {
        Some(value) => command.arg("/v").arg(value),
        None => command.arg("/ve"),
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text
        .lines()
        .find(|l| l.contains("REG_SZ") || l.contains("REG_EXPAND_SZ"))?;
    let (_, data) = line.split_once("REG_")?;
    let (_, data) = data.split_once(char::is_whitespace)?;
    Some(data.trim().to_owned())
}

/// The executable in a shell open command: `"C:\...\brave.exe" --single-argument %1`.
fn command_exe(command: &str) -> Option<String> {
    let command = command.trim();
    if let Some(rest) = command.strip_prefix('"') {
        return rest.split_once('"').map(|(exe, _)| exe.to_owned());
    }
    command.split_whitespace().next().map(str::to_owned)
}

/// The default browser's executable and private switch.
fn default_browser() -> Option<(String, &'static str)> {
    let prog_id = reg_value(
        r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
        Some("ProgId"),
    )?;
    if prog_id.is_empty() || prog_id.contains(['\\', '"']) {
        return None;
    }
    let command = reg_value(&format!(r"HKCR\{prog_id}\shell\open\command"), None)?;
    let exe = command_exe(&command)?;
    let switch = private_switch(&exe)?;
    Some((exe, switch))
}

/// Whether a private window can be offered (the default browser is known).
pub fn available() -> bool {
    use std::sync::OnceLock;
    static KNOWN: OnceLock<bool> = OnceLock::new();
    *KNOWN.get_or_init(|| default_browser().is_some())
}

/// Open an http(s) link in a private window of the default browser.
pub fn open(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "That isn't a web link.".to_owned())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only web links open in a private window.".into());
    }
    let (exe, switch) = default_browser()
        .ok_or("Your default browser doesn't have a private window Jawjack knows how to open.")?;
    Command::new(exe)
        .arg(switch)
        .arg(parsed.as_str())
        .spawn()
        .map(|_| ())
        .map_err(|_| "Couldn't start your browser.".into())
}

#[cfg(test)]
mod tests {
    use super::{command_exe, default_browser, private_switch};
    /// Machine-specific: run with --ignored to see what this PC's default is.
    #[test]
    #[ignore]
    fn finds_this_pcs_default_browser() {
        let found = default_browser();
        println!("{found:?}");
        assert!(found.is_some());
    }
    #[test]
    fn browsers_and_commands() {
        assert_eq!(
            command_exe(r#""C:\Brave\brave.exe" --single-argument %1"#).as_deref(),
            Some(r"C:\Brave\brave.exe")
        );
        assert_eq!(private_switch(r"C:\Brave\brave.exe"), Some("--incognito"));
        assert_eq!(private_switch(r"C:\Edge\msedge.exe"), Some("--inprivate"));
        assert_eq!(
            private_switch(r"C:\Mozilla\firefox.exe"),
            Some("-private-window")
        );
        assert_eq!(private_switch(r"C:\x\unknown.exe"), None);
    }
}
