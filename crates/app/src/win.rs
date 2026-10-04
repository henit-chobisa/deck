//! The few things deck asks of Windows directly.
//!
//! gpui makes the window; this only reaches its native handle for what gpui
//! has no word for.

use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
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

/// Make the frame Windows draws agree with the one deck paints.
///
/// As an ordinary window the deck gets Windows 11's own frame: a hairline
/// border and an 8-point rounded corner, with a shadow. Deck paints its own
/// corner on a transparent window, so the two disagreed — a grey line traced
/// a rectangle just outside the painted curve. The border goes, the corner is
/// asked to stay rounded, and deck paints its corner at the same 8 points on
/// Windows (see `view::WINDOW_CORNER`), so the shadow follows the deck.
///
/// Says whether Windows agreed to round the corner. Windows 10 has no such
/// setting and keeps its square frame, so deck paints a square corner there
/// (see [`rounds`]) rather than a curve inside a square.
pub fn fit_frame(window: &Window) {
    let Some(hwnd) = hwnd(window) else {
        return;
    };
    let none = DWMWA_COLOR_NONE;
    let round = DWMWCP_ROUND;
    // SAFETY: a live window's handle and pointers to two locals of the sizes
    // passed, read during the call only.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            (&raw const none).cast(),
            size_of(&none),
        );
        let rounded = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const round).cast(),
            size_of(&round),
        )
        .is_ok();
        ROUNDS.store(rounded, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The size of an attribute's value, as the window manager wants it told.
fn size_of<T>(value: &T) -> u32 {
    u32::try_from(size_of_val(value)).unwrap_or(0)
}

/// Whether this Windows rounds a window's corners. True until a window has
/// asked and been refused, which is Windows 10.
static ROUNDS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Whether the platform's frame around the deck is rounded.
pub fn rounds() -> bool {
    ROUNDS.load(std::sync::atomic::Ordering::Relaxed)
}
