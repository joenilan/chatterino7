//! Flash a window's taskbar button until the window is focused, like Chatterino
//! does on mentions. The desktop app forbids `unsafe`; this crate holds the
//! single FFI call so that rule stays intact everywhere else.

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
