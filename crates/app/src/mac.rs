//! The few things deck asks of macOS directly.
//!
//! gpui makes the window; this only reaches its native handle for what gpui
//! has no word for.

use std::ffi::c_void;

use gpui_kit::Window;
use objc2::ffi;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, NSObject, Sel};
use objc2::sel;
use objc2_app_kit::NSView;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Make this window one that never holds the keyboard.
///
/// For the bar. It is opened without focus, and that was meant to be the end
/// of it — but opening without focus only stops gpui *asking* for the
/// keyboard. The bar is a non-activating panel, and gpui's panels answer yes
/// to `canBecomeKeyWindow`; a panel like that can be made the key window
/// while its application stays in the background, and AppKit does exactly so
/// when it is the only window the application has. The reader's terminal
/// stayed in front, looked focused, and stopped receiving what they typed
/// (#7, and again after it was closed).
///
/// So the bar's window is marked, and the class it shares with the deck's
/// own window is taught to answer no for a marked window. The mark lives and
/// dies with the window. It still takes the mouse — the two buttons on it
/// work — and nothing can hand it the keyboard, now or on a later click.
///
/// The window keeps its class. Giving it a subclass of its own was the
/// obvious way and it aborted the program: gpui frees a window by its class.
pub fn never_key(window: &Window) {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: gpui hands out its own live content view, on the main thread,
    // which is the only thread a window is ever built on.
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    let Some(native) = view.window() else {
        return;
    };
    let object: &AnyObject = native.as_ref();
    teach(object.class());
    let mark = NSObject::new();
    // SAFETY: a live window, a key that is the address of a static, and an
    // object the window is asked to keep for as long as it lives.
    unsafe {
        ffi::objc_setAssociatedObject(
            std::ptr::from_ref(object).cast_mut().cast(),
            MARK.cast(),
            objc2::rc::Retained::as_ptr(&mark).cast_mut().cast(),
            ffi::OBJC_ASSOCIATION_RETAIN_NONATOMIC,
        );
    }
    // Already key, because it was shown before this could run: taken off the
    // screen and put back, which — now that it cannot be key — gives the
    // keyboard back to whichever window had it.
    if native.isKeyWindow() {
        native.orderOut(None);
        native.orderFrontRegardless();
    }
}

/// What a marked window is found by: the address of this, and nothing in it.
static MARKED: u8 = 0;
const MARK: *const c_void = (&raw const MARKED).cast();

/// Whether a window may hold the keyboard: any may, but one that is marked.
extern "C-unwind" fn unless_marked(this: &AnyObject, _: Sel) -> Bool {
    // SAFETY: asking a live object for something associated with it.
    let marked = unsafe { ffi::objc_getAssociatedObject(std::ptr::from_ref(this).cast(), MARK) };
    Bool::new(marked.is_null())
}

/// Have `class` ask [`unless_marked`], once for the life of the program.
fn teach(class: &AnyClass) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        for selector in [sel!(canBecomeKeyWindow), sel!(canBecomeMainWindow)] {
            let Some(method) = class.instance_method(selector) else {
                continue;
            };
            // SAFETY: the replacement takes the same nothing and returns the
            // same BOOL as the method it replaces.
            unsafe {
                let imp: Imp = std::mem::transmute::<
                    extern "C-unwind" fn(&AnyObject, Sel) -> Bool,
                    Imp,
                >(unless_marked);
                method.set_implementation(imp);
            }
        }
    });
}
