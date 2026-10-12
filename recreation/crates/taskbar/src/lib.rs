//! Flash a window's taskbar button until the window is focused, like Chatterino
//! does on mentions, and keep a window above others ("always on top"). The
//! desktop app forbids `unsafe`; this crate holds the two Win32 calls GPUI
//! doesn't offer so that rule stays intact everywhere else.

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
