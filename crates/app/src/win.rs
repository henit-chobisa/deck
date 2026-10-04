//! The few things deck asks of Windows directly.
//!
//! gpui makes the window; this only reaches its native handle for what gpui
//! has no word for.

use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
};

/// The deck window's native handle.
pub fn hwnd(window: &Window) -> Option<HWND> {
    // The trait's, by name: gpui's `Window` has a `window_handle` of its own
    // that answers with gpui's handle rather than the platform's.
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
        _ => None,
    }
}

/// Keep the deck above other windows, as it is on a Mac.
///
/// It used to be a gpui `PopUp` here too, which on Windows is always on top —
/// and also has no frame at all, so it could be neither moved nor resized
/// (#50, #69). It is an ordinary window now, with an ordinary frame, and this
/// gives back the one thing the pop-up had that it should keep.
pub fn keep_on_top(window: &Window) {
    if let Some(hwnd) = hwnd(window) {
        // SAFETY: a live window's own handle, asked only to change its place
        // in the stacking order.
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}
