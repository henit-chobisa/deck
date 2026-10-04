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

/// A window of its own for a page's web view, standing over the pane.
///
/// On a Mac the web view is a child of the deck's window and is drawn over
/// what deck paints. Here a child window is drawn *under* it: gpui presents
/// through DirectComposition, whose surface covers the window's children, so
/// a page built as a child was there, laid out and running, and invisible.
///
/// So the page gets a window the deck owns. An owned window always stands
/// above its owner, goes away when the owner is minimised and is destroyed
/// with it — which is everything a child would have done, above the surface
/// rather than beneath it. It never takes the keyboard from the deck and has
/// no place in the taskbar. What it cannot do is move with the deck by
/// itself: it is put where the pane's hole is each time the deck draws, and
/// the deck draws when it moves (see `open_window`).
pub struct PageHost {
    host: isize,
    deck: isize,
    /// Where it was last put, in screen pixels, so it is only moved when
    /// the hole or the deck has.
    at: std::cell::Cell<Option<(i32, i32, i32, i32)>>,
}

impl PageHost {
    /// A host owned by `window`, not yet shown.
    pub fn new(window: &Window) -> Option<Self> {
        use windows::Win32::Graphics::Gdi::{GetStockObject, HBRUSH, NULL_BRUSH};
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, RegisterClassW, WNDCLASSW, WS_CLIPCHILDREN, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_POPUP,
        };
        use windows::core::w;

        let deck = hwnd(window)?;
        // SAFETY: plain Win32 window creation on the thread that runs gpui's
        // message loop, which is the one that will deliver its messages.
        let host = unsafe {
            let instance = GetModuleHandleW(None).ok()?;
            let class = w!("DeckPage");
            let wanted = WNDCLASSW {
                lpfnWndProc: Some(host_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                // Nothing of its own to paint: the web view fills it.
                hbrBackground: HBRUSH(GetStockObject(NULL_BRUSH).0),
                ..Default::default()
            };
            // Registering twice fails harmlessly; the class is already there.
            RegisterClassW(&wanted);
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class,
                None,
                WS_POPUP | WS_CLIPCHILDREN,
                0,
                0,
                0,
                0,
                // For a pop-up this is its owner, not its parent.
                Some(deck),
                None,
                Some(instance.into()),
                None,
            )
            .ok()?
        };
        Some(Self {
            host: host.0 as isize,
            deck: deck.0 as isize,
            at: std::cell::Cell::new(None),
        })
    }

    /// Stand over this rectangle of the deck, given in the deck's own pixels.
    ///
    /// Says the size it took, for the web view inside to fill.
    pub fn place(&self, left: f32, top: f32, wide: f32, tall: f32) -> (i32, i32) {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::Graphics::Gdi::ClientToScreen;
        use windows::Win32::UI::WindowsAndMessaging::{SWP_NOZORDER, SetWindowPos};

        let mut corner = POINT {
            x: whole(left),
            y: whole(top),
        };
        let (wide, tall) = (whole(wide), whole(tall));
        // SAFETY: two windows this thread owns, and a local written during
        // the call only.
        unsafe {
            let _ = ClientToScreen(HWND(self.deck as *mut _), &raw mut corner);
            let now = (corner.x, corner.y, wide, tall);
            if self.at.replace(Some(now)) != Some(now) {
                let _ = SetWindowPos(
                    HWND(self.host as *mut _),
                    None,
                    corner.x,
                    corner.y,
                    wide,
                    tall,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
        }
        (wide, tall)
    }

    /// Put it on screen or take it off, without ever taking the keyboard.
    pub fn show(&self, on: bool) {
        use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow};
        // SAFETY: a window this thread made and still owns.
        unsafe {
            let _ = ShowWindow(
                HWND(self.host as *mut _),
                if on { SW_SHOWNOACTIVATE } else { SW_HIDE },
            );
        }
    }
}

impl Drop for PageHost {
    fn drop(&mut self) {
        // SAFETY: a window this thread made. Already gone when the deck that
        // owned it was closed first, and then this fails and nothing follows.
        unsafe {
            let _ =
                windows::Win32::UI::WindowsAndMessaging::DestroyWindow(HWND(self.host as *mut _));
        }
    }
}

impl HasWindowHandle for PageHost {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let handle = std::num::NonZeroIsize::new(self.host)
            .ok_or(raw_window_handle::HandleError::Unavailable)?;
        let raw = RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(handle));
        // SAFETY: the window lives as long as `self`, which the borrow is tied to.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

/// A length in whole pixels, as Windows counts them.
// A window's rectangle: a few thousand pixels at most, far inside an `i32`.
#[allow(clippy::cast_possible_truncation)]
fn whole(length: f32) -> i32 {
    length.round() as i32
}

/// The host handles one thing itself: it never keeps the keyboard.
///
/// It is made not to activate, and a click inside the web view activates it
/// all the same — the browser takes the focus for itself. Left there, the
/// deck's own keys were dead after any click on a page until the reader
/// clicked the deck again. So a click is told not to activate, and when it
/// has anyway, the deck is put back in front. The click still lands: the
/// mouse goes to whatever is under it, focused or not.
unsafe extern "system" fn host_proc(
    hwnd: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, GW_OWNER, GetWindow, MA_NOACTIVATE, SetForegroundWindow, WA_INACTIVE,
        WM_ACTIVATE, WM_MOUSEACTIVATE,
    };
    // SAFETY: a message the system just delivered to a window of this class,
    // answered or forwarded, and the window's own owner asked for by handle.
    unsafe {
        match message {
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            // The low word says whether this is the window being made active.
            WM_ACTIVATE if (wparam.0 & 0xffff) != WA_INACTIVE as usize => {
                if let Ok(deck) = GetWindow(hwnd, GW_OWNER) {
                    let _ = SetForegroundWindow(deck);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
