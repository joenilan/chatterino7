//! Profiles: the installed app uses the default profile; `--profile dev` (or
//! `JAWJACK_PROFILE=dev`) gives a development copy its own settings folder
//! (`%LOCALAPPDATA%\Jawjack-Dev`) and its own Windows Credential Manager
//! entries (`https://jawjack.zombie.digital/dev/…`), so it can run beside the
//! installed app without sharing a sign-in. Two copies sharing one Twitch
//! refresh token sign each other out.
use std::{path::PathBuf, sync::OnceLock};

static PROFILE: OnceLock<Option<String>> = OnceLock::new();

/// Read `--profile NAME` from the arguments (removing it) or `JAWJACK_PROFILE`.
/// Call once, first thing in `main`. Names are 1–32 letters, digits or `-`;
/// "default" means no profile.
pub fn init(args: &mut Vec<String>) -> Result<(), String> {
    let mut name = std::env::var("JAWJACK_PROFILE").ok();
    if let Some(at) = args.iter().position(|a| a == "--profile") {
        if at + 1 >= args.len() {
            return Err("Usage: --profile NAME".into());
        }
        name = Some(args.remove(at + 1));
        args.remove(at);
    }
    let name = name.filter(|n| !n.is_empty() && n != "default");
    if let Some(name) = &name
        && (name.len() > 32 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
    {
        return Err("A profile name is 1–32 letters, digits or hyphens.".into());
    }
    let name = name.map(|n| n.to_ascii_lowercase());
    let _ = PROFILE.set(name);
    Ok(())
}

/// The profile name, or None for the default (installed) profile.
pub fn name() -> Option<&'static str> {
    PROFILE.get().and_then(|p| p.as_deref())
}

/// "Dev" for the dev profile: shown in the window title and title bar.
pub fn label() -> Option<String> {
    name().map(|n| {
        let mut chars = n.chars();
        chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    })
}

/// Jawjack's settings folder: `%LOCALAPPDATA%\ChatWorkbench` by default,
/// `%LOCALAPPDATA%\Jawjack-<Profile>` for another profile.
pub fn data_root() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(base).join(match label() {
        Some(label) => format!("Jawjack-{label}"),
        None => "ChatWorkbench".into(),
    }))
}

/// A credential vault name for this profile: the default profile keeps the
/// original names; another profile inserts its name after the host.
pub fn vault(base: &'static str) -> &'static str {
    const HOST: &str = "https://jawjack.zombie.digital/";
    match (name(), base.strip_prefix(HOST)) {
        (Some(name), Some(rest)) => {
            static CACHE: std::sync::Mutex<Vec<(&'static str, &'static str)>> =
                std::sync::Mutex::new(Vec::new());
            let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((_, named)) = cache.iter().find(|(b, _)| *b == base) {
                return named;
            }
            let named: &'static str = Box::leak(format!("{HOST}{name}/{rest}").into_boxed_str());
            cache.push((base, named));
            named
        }
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn profile_arguments() {
        let mut args = vec!["--profile".to_owned(), "Dev".to_owned(), "--x".to_owned()];
        let at = args.iter().position(|a| a == "--profile").unwrap();
        let name = args.remove(at + 1);
        args.remove(at);
        assert_eq!(name, "Dev");
        assert_eq!(args, ["--x"]);
    }
}
