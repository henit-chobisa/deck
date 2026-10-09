//! The few things deck asks of macOS directly.
//!
//! gpui makes the window; this only reaches its native handle for what gpui
//! has no word for.

use std::ffi::c_void;

use gpui_kit::Window;
use objc2::ffi;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, NSObject, Sel};
use objc2::sel;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSRunningApplication, NSView, NSWorkspace,
};
use objc2_foundation::NSObjectProtocol as _;
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
    // gpui's own panel class, and taught on that class and no other. The
    // method is looked up through superclasses, so taught on a class that did
    // not define it this would rewrite AppKit's own answer for every window in
    // the program.
    //
    // But the window may be an instance of a subclass made from it — key-value
    // observing, for one, swaps in `NSKVONotifying_GPUIPanel` when anything in
    // this process observes the window. Matching the name alone would skip
    // such a window entirely and leave the bar free to take the keyboard,
    // which macOS 27 hands panels on a click. So the class is matched by
    // descent, not by name.
    let Some(panel) = AnyClass::get(c"GPUIPanel") else {
        return;
    };
    if !descends(object.class(), panel) {
        return;
    }
    teach(panel);
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

/// The native window behind a gpui one.
#[must_use]
pub fn ns_window(window: &Window) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // SAFETY: as in `never_key` — gpui's own live content view, on the main
    // thread.
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    view.window()
}

/// Where the pointer is, in `window`'s own points from its top left.
///
/// Asked of the system rather than taken from the window's last mouse event:
/// a pointer that leaves a window as small as the questions' shelf does not
/// always tell it so, and the window goes on believing it is still there.
// Points on a screen: far inside what an f32 holds.
#[allow(clippy::cast_possible_truncation)]
pub fn pointer(window: &Window) -> Option<gpui_kit::Point<gpui_kit::Pixels>> {
    let native = ns_window(window)?;
    let at = objc2_app_kit::NSEvent::mouseLocation();
    let content = native.contentRectForFrameRect(native.frame());
    Some(gpui_kit::point(
        gpui_kit::px((at.x - content.origin.x) as f32),
        gpui_kit::px((content.origin.y + content.size.height - at.y) as f32),
    ))
}

/// Hang `child` just below `parent`, across its whole width, and keep it there.
///
/// Made a child window, so AppKit moves it with the deck when the deck is
/// dragged, and orders it with it. Says whether it could: not over a deck in
/// full screen, where there is no below, and not when the space under the
/// deck runs off the bottom of the screen — the questions would be cut, and
/// they are shown inside the window instead.
pub fn hang_below(
    parent: &objc2_app_kit::NSWindow,
    child: &objc2_app_kit::NSWindow,
    height: f64,
    gap: f64,
) -> bool {
    use objc2_app_kit::{NSWindowOrderingMode, NSWindowStyleMask};
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let room = || -> Option<NSRect> {
        if !parent.isVisible() || parent.styleMask().contains(NSWindowStyleMask::FullScreen) {
            return None;
        }
        let frame = parent.frame();
        let below = frame.origin.y - gap - height;
        let screen = parent.screen()?.visibleFrame();
        (below >= screen.origin.y).then(|| {
            NSRect::new(
                NSPoint::new(frame.origin.x, below),
                NSSize::new(frame.size.width, height),
            )
        })
    };
    let Some(target) = room() else {
        if child.isVisible() {
            // Taken off the parent as well as the screen, so a later
            // `addChildWindow` puts it back cleanly.
            parent.removeChildWindow(child);
            child.orderOut(None);
        }
        return false;
    };
    // Only moved from here. Its size is gpui's to set (`Window::resize`): a
    // size set natively does not reliably reach what gpui draws into, and the
    // shelf went on drawing at the width it was opened with.
    if child.frame().origin != target.origin {
        child.setFrameOrigin(target.origin);
    }
    if !child.isVisible() {
        // SAFETY: both are live windows of this process, on the main thread.
        unsafe { parent.addChildWindow_ordered(child, NSWindowOrderingMode::Above) };
    }
    true
}

/// Take `child` off the screen, and off `parent`.
pub fn unhang(parent: &objc2_app_kit::NSWindow, child: &objc2_app_kit::NSWindow) {
    if child.isVisible() {
        parent.removeChildWindow(child);
        child.orderOut(None);
    }
}

thread_local! {
    /// The application the reader was in when they opened a deck.
    static BEFORE: std::cell::RefCell<Option<objc2::rc::Retained<NSRunningApplication>>> =
        const { std::cell::RefCell::new(None) };
}

/// Note which application is in front, before deck comes forward to be read.
pub fn remember_front() {
    let front = NSWorkspace::sharedWorkspace().frontmostApplication();
    let ours = NSRunningApplication::currentApplication();
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
    if before.isTerminated() {
        return;
    }
    // Offered, then asked for: since macOS 14 an application comes forward
    // only if the one in front lets it. The offer does not exist before 14,
    // and sending it there is an unrecognised selector — asked first.
    let app = NSApplication::sharedApplication(main);
    if app.respondsToSelector(sel!(yieldActivationToApplication:)) {
        app.yieldActivationToApplication(&before);
    }
    before.activateWithOptions(NSApplicationActivationOptions::empty());
}

/// Whether `class` is `ancestor` or was made from it.
fn descends(class: &AnyClass, ancestor: &AnyClass) -> bool {
    let mut at = Some(class);
    while let Some(class) = at {
        if std::ptr::eq(class, ancestor) {
            return true;
        }
        at = class.superclass();
    }
    false
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
    use super::{descends, mark, swap};
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

    /// A window whose class was made from the panel's is still the panel; an
    /// unrelated class is not. The first would have been skipped (#119).
    #[test]
    fn a_renamed_panel_is_still_the_panel() {
        let panel = ClassBuilder::new(c"DeckTestPanelBase", NSObject::class())
            .expect("a fresh class")
            .register();
        let renamed = ClassBuilder::new(c"NSKVONotifying_DeckTestPanelBase", panel)
            .expect("a fresh class")
            .register();
        let other = ClassBuilder::new(c"DeckTestOther", NSObject::class())
            .expect("a fresh class")
            .register();
        assert!(descends(panel, panel));
        assert!(descends(renamed, panel));
        assert!(!descends(other, panel));
    }

    /// What the fix is for: a marked window of a subclass refuses the keyboard
    /// once only the base class has been taught.
    #[test]
    fn a_marked_window_of_a_subclass_refuses_the_keyboard() {
        extern "C-unwind" fn yes(_: &AnyObject, _: Sel) -> Bool {
            Bool::YES
        }
        let mut builder =
            ClassBuilder::new(c"DeckTestBasePanel", NSObject::class()).expect("a fresh class");
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
        let base = builder.register();
        let renamed = ClassBuilder::new(c"NSKVONotifying_DeckTestBasePanel", base)
            .expect("a fresh class")
            .register();
        swap(base);

        // SAFETY: plain `new` on a class with no state of its own, and a
        // message it inherits an answer to.
        unsafe {
            let bar: objc2::rc::Retained<AnyObject> = msg_send![renamed, new];
            let deck: objc2::rc::Retained<AnyObject> = msg_send![renamed, new];
            mark(&bar);
            let bar_key: Bool = msg_send![&*bar, canBecomeKeyWindow];
            let bar_main: Bool = msg_send![&*bar, canBecomeMainWindow];
            let deck_key: Bool = msg_send![&*deck, canBecomeKeyWindow];
            assert!(!bar_key.as_bool() && !bar_main.as_bool());
            assert!(
                deck_key.as_bool(),
                "an unmarked window of the subclass still may"
            );
        }
    }
}
