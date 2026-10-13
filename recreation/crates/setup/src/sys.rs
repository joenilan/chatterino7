//! Small Win32 helpers: registry, shell shortcuts and known folders, process
//! discovery, graceful window close, the one-setup lock and task dialogs.

use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, GetLastError,
        HANDLE, HWND, LPARAM, WAIT_OBJECT_0,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
        LibraryLoader::GetModuleHandleW,
        Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
            RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteTreeW,
            RegGetValueW, RegSetValueExW,
        },
        Threading::{
            CreateMutexW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            QueryFullProcessImageNameW, WaitForSingleObject,
        },
    },
    UI::{
        Controls::{
            TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDF_ALLOW_DIALOG_CANCELLATION,
            TDF_VERIFICATION_FLAG_CHECKED, TaskDialogIndirect,
        },
        WindowsAndMessaging::{
            EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IDCANCEL, IsWindowVisible,
            MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW, PostMessageW, WM_CLOSE,
        },
    },
};

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

// ----------------------------------------------------------- one setup

/// A named mutex held while setup changes the installation; dropped on exit.
pub struct SetupLock(HANDLE);

impl Drop for SetupLock {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateMutexW and is closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// `None` when another setup already holds `name` (or it cannot be created).
pub fn setup_lock(name: &str) -> Option<SetupLock> {
    let name = wide(name);
    // SAFETY: NUL-terminated name; the handle is owned by the returned lock.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return None;
    }
    // SAFETY: read immediately after CreateMutexW on this thread.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // SAFETY: our handle to the existing mutex is closed once.
        unsafe { CloseHandle(handle) };
        return None;
    }
    Some(SetupLock(handle))
}

// ---------------------------------------------------------------- registry

pub fn reg_set_values(
    subkey: &str,
    strings: &[(&str, &str)],
    dwords: &[(&str, u32)],
) -> std::io::Result<()> {
    let subkey = wide(subkey);
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: NUL-terminated inputs; the opened key is closed below.
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    let mut result = Ok(());
    for (name, value) in strings {
        let name = wide(name);
        let data = wide(value);
        // SAFETY: `key` is open; `data` holds the given byte count.
        let status = unsafe {
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        if status != ERROR_SUCCESS {
            result = Err(std::io::Error::from_raw_os_error(status as i32));
        }
    }
    for (name, value) in dwords {
        let name = wide(name);
        // SAFETY: `key` is open; the value is four bytes.
        let status = unsafe {
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_DWORD,
                (value as *const u32).cast(),
                4,
            )
        };
        if status != ERROR_SUCCESS {
            result = Err(std::io::Error::from_raw_os_error(status as i32));
        }
    }
    // SAFETY: opened above.
    unsafe { RegCloseKey(key) };
    result
}

pub fn reg_get_string(subkey: &str, name: &str) -> Option<String> {
    let subkey = wide(subkey);
    let name = wide(name);
    let mut buffer = vec![0u16; 2048];
    let mut size = (buffer.len() * 2) as u32;
    // SAFETY: NUL-terminated inputs; the buffer holds `size` bytes.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let units = (size as usize / 2).min(buffer.len());
    let text = buffer[..units]
        .split(|unit| *unit == 0)
        .next()
        .unwrap_or_default();
    String::from_utf16(text).ok()
}

/// Delete `subkey` and everything below it. A missing key is already deleted.
pub fn reg_delete_tree(subkey: &str) -> std::io::Result<()> {
    let key = wide(subkey);
    // SAFETY: NUL-terminated input.
    let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
    if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    // SAFETY: NUL-terminated input; RegDeleteTreeW leaves the key itself.
    let status = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, key.as_ptr()) };
    if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

/// Delete an empty key; a key with children or values is left alone.
pub fn reg_delete_if_empty(subkey: &str) {
    let key = wide(subkey);
    // SAFETY: NUL-terminated input; RegDeleteKeyW fails on keys with subkeys.
    let _ = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, key.as_ptr()) };
}

// ------------------------------------------------------------ shell / COM

pub enum KnownFolder {
    Programs,
    Desktop,
}

pub fn known_folder(folder: KnownFolder) -> std::io::Result<PathBuf> {
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_Desktop, FOLDERID_Programs, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
    };
    let id = match folder {
        KnownFolder::Programs => &FOLDERID_Programs,
        KnownFolder::Desktop => &FOLDERID_Desktop,
    };
    // SAFETY: the returned buffer is freed with CoTaskMemFree.
    unsafe {
        let path =
            SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).map_err(std::io::Error::other)?;
        let result = PathBuf::from(OsString::from_wide(path.as_wide()));
        CoTaskMemFree(Some(path.0 as *const _));
        Ok(result)
    }
}

/// Create or replace a `.lnk` shortcut to `target` using its first icon.
pub fn create_shortcut(
    link: &Path,
    target: &Path,
    working: &Path,
    description: &str,
) -> std::io::Result<()> {
    use windows::{
        Win32::System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, IPersistFile,
        },
        Win32::UI::Shell::{IShellLinkW, ShellLink},
        core::{Interface, PCWSTR},
    };
    let target_w = wide_path(target);
    let working_w = wide_path(working);
    let description_w = wide(description);
    let link_w = wide_path(link);
    // SAFETY: COM is initialized for this call; all strings are NUL-terminated.
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let result = (|| -> windows::core::Result<()> {
            let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            shell.SetPath(PCWSTR(target_w.as_ptr()))?;
            shell.SetWorkingDirectory(PCWSTR(working_w.as_ptr()))?;
            shell.SetDescription(PCWSTR(description_w.as_ptr()))?;
            shell.SetIconLocation(PCWSTR(target_w.as_ptr()), 0)?;
            let file: IPersistFile = shell.cast()?;
            file.Save(PCWSTR(link_w.as_ptr()), true)?;
            Ok(())
        })();
        if initialized {
            CoUninitialize();
        }
        result.map_err(std::io::Error::other)
    }
}

// --------------------------------------------------------------- processes

pub struct RunningProcess {
    pub pid: u32,
    pub image: PathBuf,
}

pub fn process_image(pid: u32) -> Option<PathBuf> {
    // SAFETY: the handle is closed below.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buffer = vec![0u16; 32 * 1024];
    let mut length = buffer.len() as u32;
    // SAFETY: the buffer holds `length` UTF-16 units.
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) };
    unsafe { CloseHandle(handle) };
    (ok != 0 && length > 0).then(|| PathBuf::from(OsString::from_wide(&buffer[..length as usize])))
}

pub fn processes() -> Vec<RunningProcess> {
    let mut result = Vec::new();
    // SAFETY: the snapshot handle is closed below; the entry size is set.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() || snapshot as isize == -1 {
            return result;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        let own = std::process::id();
        while more {
            let pid = entry.th32ProcessID;
            if pid != 0
                && pid != own
                && let Some(image) = process_image(pid)
            {
                result.push(RunningProcess { pid, image });
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    result
}

/// Remove the `\?\` verbatim prefix `canonicalize` adds, for display,
/// shortcuts and registry values.
pub fn plain_path(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

/// True when `path` is `directory` itself or below it (case-insensitive).
pub fn path_within(path: &Path, directory: &Path) -> bool {
    let lower = |path: &Path| {
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase()
    };
    let path = lower(path);
    let directory = lower(directory);
    let directory = directory.trim_end_matches('\\');
    path == directory || path.starts_with(&format!("{directory}\\"))
}

/// Ask the visible top-level windows of `pids` to close (WM_CLOSE). The app
/// keeps its own unsaved-file and running-task prompts; nothing is killed.
pub fn request_close(pids: &[u32]) -> usize {
    struct State<'a> {
        pids: &'a [u32],
        posted: usize,
    }
    unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> i32 {
        // SAFETY: `data` is the State passed to EnumWindows below.
        let state = unsafe { &mut *(data as *mut State) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if state.pids.contains(&pid)
            && unsafe { IsWindowVisible(window) } != 0
            && unsafe { GetWindow(window, GW_OWNER) }.is_null()
            && unsafe { PostMessageW(window, WM_CLOSE, 0, 0) } != 0
        {
            state.posted += 1;
        }
        1
    }
    let mut state = State { pids, posted: 0 };
    // SAFETY: the callback only runs during this call.
    unsafe { EnumWindows(Some(visit), &mut state as *mut State as LPARAM) };
    state.posted
}

/// Wait until every process in `pids` has exited or `limit` elapses.
pub fn wait_for_exit(pids: &[u32], limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    for pid in pids {
        // SAFETY: the handle is closed below.
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, *pid) };
        if handle.is_null() {
            continue; // Already gone.
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let exited =
            unsafe { WaitForSingleObject(handle, remaining.as_millis() as u32) } == WAIT_OBJECT_0;
        unsafe { CloseHandle(handle) };
        if !exited {
            return false;
        }
    }
    true
}

// ----------------------------------------------------------------- dialogs

pub struct DialogResult {
    pub button: i32,
    pub verified: bool,
}

/// A task dialog with custom buttons and an optional checkbox. Quiet mode
/// never calls this.
pub fn task_dialog(
    main: &str,
    content: &str,
    buttons: &[(i32, &str)],
    checkbox: Option<(&str, bool)>,
) -> DialogResult {
    let title = wide("Jawjack Setup");
    let main_w = wide(main);
    let content_w = wide(content);
    let texts: Vec<Vec<u16>> = buttons.iter().map(|(_, text)| wide(text)).collect();
    let items: Vec<TASKDIALOG_BUTTON> = buttons
        .iter()
        .zip(&texts)
        .map(|((id, _), text)| TASKDIALOG_BUTTON {
            nButtonID: *id,
            pszButtonText: text.as_ptr(),
        })
        .collect();
    let checkbox_w = checkbox.map(|(text, _)| wide(text));
    // SAFETY: a zeroed TASKDIALOGCONFIG is valid; every pointer set below
    // outlives the modal call.
    unsafe {
        let mut config: TASKDIALOGCONFIG = std::mem::zeroed();
        config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
        config.hInstance = GetModuleHandleW(std::ptr::null());
        config.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION;
        config.pszWindowTitle = title.as_ptr();
        // No main icon: Jawjack has no icon resource yet.
        config.pszMainInstruction = main_w.as_ptr();
        config.pszContent = content_w.as_ptr();
        config.cButtons = items.len() as u32;
        config.pButtons = items.as_ptr();
        config.nDefaultButton = buttons.first().map_or(0, |(id, _)| *id);
        if let (Some((_, checked)), Some(text)) = (checkbox, checkbox_w.as_ref()) {
            config.pszVerificationText = text.as_ptr();
            if checked {
                config.dwFlags |= TDF_VERIFICATION_FLAG_CHECKED;
            }
        }
        let mut button = 0i32;
        let mut verified = 0;
        if TaskDialogIndirect(&config, &mut button, std::ptr::null_mut(), &mut verified) < 0 {
            // Without Common Controls 6 fall back to a plain message box.
            message(main, content, false);
            return DialogResult {
                button: IDCANCEL,
                verified: false,
            };
        }
        DialogResult {
            button,
            verified: verified != 0,
        }
    }
}

pub fn message(main: &str, content: &str, error: bool) {
    let title = wide("Jawjack Setup");
    let text = wide(&format!("{main}\n\n{content}"));
    // SAFETY: NUL-terminated strings.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK
                | if error {
                    MB_ICONERROR
                } else {
                    MB_ICONINFORMATION
                },
        )
    };
}
