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
/// obvious way, and the program aborted when such a window was freed.
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
    // gpui's own panel class and no other. The method is looked up through
    // superclasses, so on a class that did not define it this would rewrite
    // AppKit's own answer for every window in the program.
    if object.class().name() != c"GPUIPanel" {
        return;
    }
    teach(object.class());
    mark(object);
    // Not expected to be key yet — this runs straight after the window is
    // shown, before the system has had a turn — but if it is, it is taken off
    // the screen and put back, which now that it cannot be key gives the
    // keyboard back to whichever window had it.
    if native.isKeyWindow() {
        native.orderOut(None);
        native.orderFrontRegardless();
    }
}

thread_local! {
    /// The application the reader was in when they opened a deck.
    static BEFORE: std::cell::RefCell<Option<objc2::rc::Retained<objc2_app_kit::NSRunningApplication>>> =
        const { std::cell::RefCell::new(None) };
}

/// Note which application is in front, before deck comes forward to be read.
pub fn remember_front() {
    let front = objc2_app_kit::NSWorkspace::sharedWorkspace().frontmostApplication();
    let ours = objc2_app_kit::NSRunningApplication::currentApplication();
    // Not deck itself: opening a second deck from the first would otherwise
    // forget where the reader really came from.
    if let Some(front) = front
        && front.processIdentifier() != ours.processIdentifier()
    {
        BEFORE.set(Some(front));
    }
}

/// Hand the keyboard back to the application that had it before deck.
///
/// For the moment the deck's window goes and only the bar is left. Deck was
/// brought forward to be read; left in front with a bar that takes no keys,
/// it would hold the keyboard and do nothing with it, and the reader would
/// have to click their terminal to type again.
pub fn step_back() {
    let Some(main) = objc2::MainThreadMarker::new() else {
        return;
    };
    let Some(before) = BEFORE.take() else {
        return;
    };
    // Offered, then asked for: since macOS 14 an application comes forward
    // only if the one in front lets it.
    objc2_app_kit::NSApplication::sharedApplication(main).yieldActivationToApplication(&before);
    before.activateWithOptions(objc2_app_kit::NSApplicationActivationOptions::empty());
}

/// Mark an object as one that must never hold the keyboard.
fn mark(object: &AnyObject) {
    let mark = NSObject::new();
    // SAFETY: a live object, a key that is the address of a static, and an
    // object it is asked to keep for as long as it lives.
    unsafe {
        ffi::objc_setAssociatedObject(
            std::ptr::from_ref(object).cast_mut().cast(),
            MARK.cast(),
            objc2::rc::Retained::as_ptr(&mark).cast_mut().cast(),
            ffi::OBJC_ASSOCIATION_RETAIN_NONATOMIC,
        );
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
    ONCE.call_once(|| swap(class));
}

/// Replace `class`'s two answers with [`unless_marked`].
fn swap(class: &AnyClass) {
    for selector in [sel!(canBecomeKeyWindow), sel!(canBecomeMainWindow)] {
        let Some(method) = class.instance_method(selector) else {
            continue;
        };
        // SAFETY: the replacement takes the same nothing and returns the
        // same BOOL as the method it replaces.
        unsafe {
            let imp: Imp = std::mem::transmute::<extern "C-unwind" fn(&AnyObject, Sel) -> Bool, Imp>(
                unless_marked,
            );
            method.set_implementation(imp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{mark, swap};
    use objc2::runtime::{AnyObject, Bool, ClassBuilder, NSObject, Sel};
    use objc2::{ClassType, msg_send, sel};

    /// A marked window says no, and every other window of the same class
    /// goes on saying yes — which is the deck's own window.
    #[test]
    fn only_a_marked_window_refuses_the_keyboard() {
        extern "C-unwind" fn yes(_: &AnyObject, _: Sel) -> Bool {
            Bool::YES
        }
        let mut builder =
            ClassBuilder::new(c"DeckTestPanel", NSObject::class()).expect("a fresh class");
        // SAFETY: both take nothing and return a BOOL, as `yes` does.
        unsafe {
            builder.add_method(
                sel!(canBecomeKeyWindow),
                yes as extern "C-unwind" fn(_, _) -> _,
            );
            builder.add_method(
                sel!(canBecomeMainWindow),
                yes as extern "C-unwind" fn(_, _) -> _,
            );
        }
        let class = builder.register();
        swap(class);

        // SAFETY: plain `new` on a class with no state of its own, and two
        // messages it was just given answers to.
        unsafe {
            let bar: objc2::rc::Retained<AnyObject> = msg_send![class, new];
            let deck: objc2::rc::Retained<AnyObject> = msg_send![class, new];
            mark(&bar);
            let bar_key: Bool = msg_send![&*bar, canBecomeKeyWindow];
            let bar_main: Bool = msg_send![&*bar, canBecomeMainWindow];
            let deck_key: Bool = msg_send![&*deck, canBecomeKeyWindow];
            assert!(!bar_key.as_bool() && !bar_main.as_bool());
            assert!(deck_key.as_bool());
        }
    }
}
