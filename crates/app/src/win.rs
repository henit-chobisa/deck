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
/// rather than beneath it. It has no place in the taskbar, and the keys the
/// page has no use for go to the deck (see [`pass_key`]). What it cannot do is move with the deck by
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
        // Anything left under this handle belonged to a window that had it
        // before; it is not this one's.
        let stale = MADE.with_borrow_mut(|made| made.remove(&(host.0 as isize)));
        drop(stale);
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

        // Each edge rounded, and the size taken between them. Rounding the
        // size by itself leaves a pixel of seam on one side or the other.
        let mut corner = POINT {
            x: whole(left),
            y: whole(top),
        };
        let wide = (whole(left + wide) - corner.x).max(0);
        let tall = (whole(top + tall) - corner.y).max(0);
        // SAFETY: two windows this thread owns, and a local written during
        // the call only.
        unsafe {
            // Refused, the corner is still in the deck's own coordinates, and
            // the host would be put there on the screen. Better left alone.
            if !ClientToScreen(HWND(self.deck as *mut _), &raw mut corner).as_bool() {
                return (wide, tall);
            }
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
        // A view asked for and never made, or made and never collected, goes
        // with the window it was for — the view first.
        let asked = WAITING.with_borrow_mut(|waiting| waiting.remove(&self.host));
        let made = MADE.with_borrow_mut(|made| made.remove(&self.host));
        drop((asked, made));
        // SAFETY: a window this thread made. Already gone when the deck that
        // owned it was closed first, and then this fails and nothing follows.
        unsafe {
            let _ =
                windows::Win32::UI::WindowsAndMessaging::DestroyWindow(HWND(self.host as *mut _));
        }
    }
}

/// What makes a host's web view, given the host to make it in.
type Make = Box<dyn FnOnce(&Standing) -> Option<wry::WebView>>;

thread_local! {
    /// The views asked for and not yet made, by host.
    static WAITING: std::cell::RefCell<std::collections::HashMap<isize, Make>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// The views made and not yet collected, by host. `None` is one that
    /// could not be made, so nobody goes on waiting for it.
    static MADE: std::cell::RefCell<std::collections::HashMap<isize, Option<wry::WebView>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

impl PageHost {
    /// Ask for the web view to be made in this host, a moment from now.
    ///
    /// Not now, because now is the middle of the deck drawing itself. Making
    /// a WebView2 waits for the browser by running the message loop where it
    /// stands, and that loop hands gpui whatever work was queued for it —
    /// while gpui is already in the middle of some. It answers
    /// `RefCell already borrowed` and the process aborts. So the request is
    /// posted to the host's own window, and the view is made when the
    /// message loop gets to it with nothing of gpui's on the stack.
    pub fn make(&self, make: impl FnOnce(&Standing) -> Option<wry::WebView> + 'static) {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
        WAITING.with_borrow_mut(|waiting| waiting.insert(self.host, Box::new(make)));
        // SAFETY: a message to a window this thread made and still owns.
        unsafe {
            let _ = PostMessageW(
                Some(HWND(self.host as *mut _)),
                MAKE_VIEW,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }

    /// The view, once it has been made: `None` until then, and `Some(None)`
    /// if the window would not take one.
    pub fn made(&self) -> Option<Option<wry::WebView>> {
        MADE.with_borrow_mut(|made| made.remove(&self.host))
    }
}

/// A host's window, for the length of one call: what a web view is built in.
pub struct Standing(isize);

impl Standing {
    /// Which host this is, for [`pass_key`].
    #[must_use]
    pub fn host(&self) -> isize {
        self.0
    }
}

impl HasWindowHandle for Standing {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let handle = std::num::NonZeroIsize::new(self.0)
            .ok_or(raw_window_handle::HandleError::Unavailable)?;
        let raw = RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(handle));
        // SAFETY: only made inside the window's own procedure, while it is
        // handling a message, and not kept past it.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

/// A length in whole pixels, as Windows counts them.
// A window's rectangle: a few thousand pixels at most, far inside an `i32`.
#[allow(clippy::cast_possible_truncation)]
fn whole(length: f32) -> i32 {
    length.round() as i32
}

/// Posted to the host: make the web view that was asked for.
const MAKE_VIEW: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;

/// The host handles one thing itself: making its web view when asked (see
/// [`PageHost::make`]). Everything else is the web view's.
unsafe extern "system" fn host_proc(
    hwnd: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;
    if message == MAKE_VIEW {
        let host = hwnd.0 as isize;
        // Taken out before it is run, and put away after: making the view
        // runs the message loop, and nothing may be held across that.
        let asked = WAITING.with_borrow_mut(|waiting| waiting.remove(&host));
        if let Some(make) = asked {
            let view = make(&Standing(host));
            // Making it ran the message loop, and the pane may have closed
            // meanwhile and taken this window with it. A view kept for it
            // then would be collected by whichever window is next given the
            // same handle.
            // SAFETY: asking whether a handle still names a window.
            if !unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd)) }.as_bool() {
                drop(view);
                return LRESULT(0);
            }
            let old = MADE.with_borrow_mut(|made| made.insert(host, view));
            drop(old);
        }
        return LRESULT(0);
    }
    // SAFETY: forwarding a message the system just delivered.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

/// Hand a key pressed inside a page to the deck that owns it.
///
/// A click on a page gives the page's window the keyboard — the browser takes
/// it, whatever the window was made not to do — and the deck's keys would be
/// dead until the reader clicked the deck again. Taking the keyboard back on
/// the click costs the click: the browser drops a press it loses focus in the
/// middle of. So the page keeps the keyboard and gives up the keys: its
/// document reports each one it has no use for, the deck comes back to the
/// front, and the key is posted to it as if it had been pressed there.
pub fn pass_key(host: isize, key: u32) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, MAPVK_VK_TO_VSC, MapVirtualKeyW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GW_OWNER, GetForegroundWindow, GetWindow, PostMessageW, SetForegroundWindow, WM_KEYDOWN,
        WM_KEYUP,
    };
    // SAFETY: the host's owner asked for by handle, brought forward, and two
    // messages posted to it; all refusals are ignored.
    unsafe {
        // Only a key a person is holding down, on the page in front of them.
        // The page is markup the agent wrote, and its script can say `key:`
        // whenever it likes: taken at its word, a page could press `s` and
        // submit its own review, or `q`, from the background. A key that is
        // not physically down, or a page that is not the window in front, is
        // nobody's key press.
        if GetForegroundWindow() != HWND(host as *mut _) {
            return;
        }
        let Ok(vk) = i32::try_from(key) else {
            return;
        };
        if GetAsyncKeyState(vk).cast_unsigned() & 0x8000 == 0 {
            return;
        }
        let Ok(deck) = GetWindow(HWND(host as *mut _), GW_OWNER) else {
            return;
        };
        let _ = SetForegroundWindow(deck);
        // As the keyboard would have said it: a repeat count of one, and the
        // key's scan code, which is how gpui tells keys apart.
        let scan = (MapVirtualKeyW(key, MAPVK_VK_TO_VSC) & 0xff) as isize;
        let down = 1 | (scan << 16);
        let up = down | (0b11 << 30);
        let _ = PostMessageW(Some(deck), WM_KEYDOWN, WPARAM(key as usize), LPARAM(down));
        let _ = PostMessageW(Some(deck), WM_KEYUP, WPARAM(key as usize), LPARAM(up));
    }
}
