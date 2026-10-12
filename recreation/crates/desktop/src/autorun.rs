//! "Start with Windows" (Chatterino's autorun): a value under the current
//! user's Run key, written with Windows' own `reg` tool (no admin rights).
const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Jawjack";

fn reg(args: &[&str]) -> Option<std::process::Output> {
    let mut command = std::process::Command::new("reg");
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command.output().ok()
}

/// Whether Jawjack starts with Windows (false elsewhere).
pub fn enabled() -> bool {
    cfg!(windows) && reg(&["query", KEY, "/v", VALUE]).is_some_and(|o| o.status.success())
}

pub fn set(on: bool) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("Start with Windows is only available on Windows.".into());
    }
    let ok = if on {
        let exe = std::env::current_exe().map_err(|_| "Couldn't find Jawjack's program file.")?;
        let command = format!("\"{}\"", exe.display());
        reg(&[
            "add", KEY, "/v", VALUE, "/t", "REG_SZ", "/d", &command, "/f",
        ])
    } else {
        reg(&["delete", KEY, "/v", VALUE, "/f"])
    }
    .is_some_and(|o| o.status.success());
    if ok {
        Ok(())
    } else {
        Err("Windows didn't accept the change.".into())
    }
}
