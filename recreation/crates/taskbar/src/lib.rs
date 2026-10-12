//! Flash a window's taskbar button until the window is focused, like Chatterino
//! does on mentions, and keep a window above others ("always on top"). The
//! desktop app forbids `unsafe`; this crate holds the Win32 calls GPUI doesn't
//! offer so that rule stays intact everywhere else (also: one window per
//! profile).

/// Flash the taskbar button of the window `hwnd` (a Win32 window handle) until
/// it comes to the foreground. Does nothing on other platforms or for 0.
pub fn flash(hwnd: isize) {
    #[cfg(windows)]
    if hwnd != 0 {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            FLASHW_TIMERNOFG, FLASHW_TRAY, FLASHWINFO, FlashWindowEx,
        };
        let info = FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd: hwnd as _,
            dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
            uCount: 0,
            dwTimeout: 0,
        };
        // SAFETY: `info` is a fully initialised FLASHWINFO on the stack, valid
        // for the call; FlashWindowEx only reads it, and an invalid window
        // handle makes the call fail harmlessly.
        unsafe {
            FlashWindowEx(&info);
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Keep the window `hwnd` above all normal windows, or stop doing so.
/// Returns false if Windows refused (or on other platforms).
pub fn set_topmost(hwnd: isize, on: bool) -> bool {
    #[cfg(windows)]
    if hwnd != 0 {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
        };
        // SAFETY: SetWindowPos takes plain values; with SWP_NOMOVE | SWP_NOSIZE
        // only the z-order changes, and an invalid handle makes it fail harmlessly.
        return unsafe {
            SetWindowPos(
                hwnd as _,
                if on { HWND_TOPMOST } else { HWND_NOTOPMOST },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            ) != 0
        };
    }
    #[cfg(not(windows))]
    let _ = (hwnd, on);
    false
}

/// Claim "the Jawjack window" for one profile (`key`, e.g. "default" or "dev").
/// Returns false when another Jawjack on the same profile already holds it; that
/// window (found by its title, `title`) is brought to the front. The claim lasts
/// until this process exits. Always true on other platforms.
pub fn claim_single_window(key: &str, title: &str) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
        use windows_sys::Win32::System::Threading::CreateMutexW;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow,
        };
        let wide = |s: &str| {
            s.encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        };
        let name = wide(&format!(r"Local\Jawjack-Window-{key}"));
        // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the
        // call; the handle is deliberately kept open for the process lifetime.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        // SAFETY: GetLastError reads the calling thread's last-error value.
        let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        if handle.is_null() || !exists {
            return true;
        }
        let title = wide(title);
        // SAFETY: `title` is NUL-terminated; a null class name matches any class.
        let window = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        if !window.is_null() {
            // SAFETY: plain calls on a window handle; failures are harmless.
            unsafe {
                ShowWindow(window, SW_RESTORE);
                SetForegroundWindow(window);
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        let _ = (key, title);
        true
    }
}
