//! Lights off: everything but the deck, turned down.
//!
//! # Why this is not a gpui window
//!
//! It was once, and every hard part of it was the cost of that. gpui makes a
//! *titled* window — a titleless one is still titled underneath — and a titled
//! window on macOS has rounded corners, a hairline edge, and a frame AppKit
//! clamps below the menu bar. So the first version drew sixty pixels past every
//! edge of the screen to push the corners out of sight, and hid the menu bar
//! and the Dock outright because it could not cover them.
//!
//! Hiding somebody's menu bar to dim their screen is not dimming. It is the one
//! thing every good dimmer on this platform declines to do, and it is what made
//! the first version feel like a mode you were trapped in rather than a light
//! going down.
//!
//! A borderless `NSPanel` has none of those problems, and AppKit will not make
//! one on gpui's behalf. So this asks for it directly.
//!
//! # What makes it stop feeling like a window
//!
//! Five properties, and not one of them is about colour:
//!
//! - **borderless** — no corners, no edge, no clamp. It takes the whole
//!   `frame` of a screen, menu bar included, and nothing has to be hidden.
//! - **above the menu bar, below the deck** — the bar sits at level 24 and a
//!   gpui `PopUp` window at 101, so a sheet in between dims the bar and leaves
//!   the deck lit, without either being moved or touched.
//! - **`ignoresMouseEvents`** — clicks fall through to whatever is underneath.
//!   The first version could not do this and had to offer *press the dark to
//!   escape*, which is the behaviour of a modal dialog.
//! - **non-activating, `ignoresCycle`, `stationary`** — never key, never main,
//!   out of the switcher, out of Spaces animations. This is why a tiling window
//!   manager leaves it alone: there is no window there that it can see.
//! - **no shadow** — a shadow is a window announcing its edges.

#[cfg(target_os = "macos")]
mod mac {
    use std::cell::{Cell, RefCell};

    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSBackingStoreType, NSColor, NSPanel, NSScreen, NSVisualEffectBlendingMode,
        NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
        NSWindowCollectionBehavior, NSWindowStyleMask,
    };

    /// Where the sheets sit in the stack.
    ///
    /// The menu bar is 24 and the Dock is 20, so anything above 24 covers both
    /// without asking the application to hide either. The deck is a gpui
    /// `PopUp`, which is 101, so it stays lit above both of these.
    const BLUR: isize = 25;
    const DIM: isize = 26;

    thread_local! {
        /// Every sheet, made once and kept.
        ///
        /// They are faded rather than closed, which is what lets the lights
        /// come up *after* the deck has gone: a panel ordered out at the same
        /// moment its window is removed never gets to show the fade. An
        /// invisible one is click-through, out of the switcher and inert, and
        /// it goes when the process does.
        ///
        /// Thread-local rather than a gpui global: these are AppKit objects
        /// that may only be touched on the main thread, which is the same
        /// thread every path into this module already runs on.
        static SHEETS: RefCell<Vec<Retained<NSPanel>>> = const { RefCell::new(Vec::new()) };

        /// Whether they are currently showing.
        static LIT: Cell<bool> = const { Cell::new(false) };
    }

    /// Whether the lights are currently off.
    pub fn on() -> bool {
        LIT.get()
    }

    /// Put every sheet at `alpha`, now.
    pub fn set_alpha(alpha: f64) {
        SHEETS.with_borrow(|sheets| {
            for panel in sheets {
                panel.setAlphaValue(alpha);
            }
        });
    }

    /// What every sheet is showing at the moment.
    pub fn alpha() -> f64 {
        SHEETS.with_borrow(|sheets| sheets.first().map_or(0., |panel| panel.alphaValue()))
    }

    /// One sheet over one screen, configured the way a dimmer has to be.
    fn sheet(mtm: MainThreadMarker, screen: &NSScreen, level: isize) -> Retained<NSPanel> {
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            screen.frame(),
            // Borderless *and* non-activating. The first keeps AppKit from
            // giving it a frame to clamp; the second keeps it from ever
            // taking focus from whatever the reader is actually in.
            NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            false,
        );

        // Born at nothing: every sheet arrives by fading up, including the
        // first, and a panel ordered in at full strength is the snap to black
        // this is here to avoid.
        panel.setAlphaValue(0.);

        // The one that crashes if it is left alone. A window built with
        // `initWithContentRect:` defaults to releasing itself when closed, and
        // this one is also held by a `Retained` — so closing it releases once
        // and dropping the handle releases again, over the same object.
        unsafe { panel.setReleasedWhenClosed(false) };

        panel.setLevel(level);
        panel.setOpaque(false);
        panel.setHasShadow(false);
        panel.setIgnoresMouseEvents(true);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        panel
    }

    /// Put the lights down.
    ///
    /// Two sheets per screen rather than one, because the two halves of this
    /// are drawn by different things: the blur is the WindowServer compositing
    /// what is behind the panel, and the dim is a flat colour over the top of
    /// that. A single panel cannot be both — its background colour is painted
    /// *under* its content view, so a blur as content hides the dim entirely.
    pub fn lights_off(dim: f32, blur: bool) {
        if on() {
            return;
        }
        LIT.set(true);

        // Already made, so there is nothing to build. Coming back to a deck
        // should not cost a window.
        if SHEETS.with_borrow(|sheets| !sheets.is_empty()) {
            return;
        }

        // SAFETY: every path here is a keystroke or a window callback, and both
        // are the main thread, which is what the marker asserts.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        let mut up = Vec::new();

        for screen in NSScreen::screens(mtm) {
            if blur {
                let panel = sheet(mtm, &screen, BLUR);
                let effect = NSVisualEffectView::new(mtm);
                // Behind-window is the whole point: it blurs the desktop under
                // the sheet rather than the sheet's own contents, on the GPU,
                // by the same machinery a sidebar uses.
                effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
                effect.setMaterial(NSVisualEffectMaterial::FullScreenUI);
                effect.setState(NSVisualEffectState::Active);
                // Given the room outright. A view made with `new` has a zero
                // frame, and a blur over nothing is a panel that shows nothing.
                effect.setFrame(screen.frame());
                panel.setContentView(Some(&effect));
                panel.orderFrontRegardless();
                up.push(panel);
            }

            let panel = sheet(mtm, &screen, DIM);
            // No content view, so what shows is the window's own ground.
            let black =
                NSColor::colorWithRed_green_blue_alpha(0., 0., 0., f64::from(dim.clamp(0., 0.92)));
            panel.setBackgroundColor(Some(&black));
            panel.orderFrontRegardless();
            up.push(panel);
        }

        SHEETS.with_borrow_mut(|held| *held = up);
    }

    /// Bring them back.
    ///
    /// The sheets stay, at nothing. Ordering them out here would be the same
    /// bug as closing them was: `h` takes the deck's window away in the same
    /// breath, and a panel removed alongside it never shows the fade the reader
    /// was meant to see.
    pub fn lights_on() {
        LIT.set(false);
    }
}

#[cfg(not(target_os = "macos"))]
mod mac {
    //! Elsewhere there is nothing to dim yet.
    pub fn on() -> bool {
        false
    }
    pub fn lights_off(_dim: f32, _blur: bool) {}
    pub fn lights_on() {}
    pub fn set_alpha(_alpha: f64) {}
    pub fn alpha() -> f64 {
        0.
    }
}

use std::time::Duration;

use gpui_kit::App;

pub use mac::on;

/// One frame of the fade, at about sixty a second.
const STEP: Duration = Duration::from_millis(16);

/// How many of those the lights take to go down — a little under a third of a
/// second.
const DOWN: u32 = 18;

/// And to come back up. Quicker: going dark is something a reader settles into,
/// and coming back is something they are waiting on.
const UP: u32 = 11;

/// Turn the lights off, or back on. Returns whether they ended up off.
pub fn toggle(dim: f32, blur: bool, cx: &mut App) -> bool {
    if on() {
        mac::lights_on();
        fade(0., UP, cx);
        false
    } else {
        mac::lights_off(dim, blur);
        fade(f64::from(dim.clamp(0., 0.92)), DOWN, cx);
        on()
    }
}

/// Take the lights up, wherever they are.
pub fn lights_on(cx: &mut App) {
    if !on() {
        return;
    }
    mac::lights_on();
    fade(0., UP, cx);
}

/// Walk the sheets from where they are to `to`, a frame at a time.
///
/// Stepped here rather than handed to AppKit's animator, which is the obvious
/// way and does not work: the animator proxy wants a window it is allowed to
/// animate implicitly, and a borderless panel ordered in during the same run
/// loop turn is not one — it arrives at its new alpha immediately, which reads
/// as a pop.
///
/// On gpui's executor rather than a timer of the platform's, because it has to
/// outlive the window that started it. `h` takes the deck away in the same
/// breath as bringing the lights up, and a fade that stops there is the thing
/// being asked for going unseen. The executor belongs to the application, and
/// the bar keeps that alive.
fn fade(to: f64, frames: u32, cx: &mut App) {
    let from = mac::alpha();
    if (to - from).abs() < f64::EPSILON {
        return;
    }

    cx.spawn(async move |cx| {
        for frame in 1..=frames {
            cx.background_executor().timer(STEP).await;
            // Smoothstep, so it leaves and arrives slowly. A linear fade to
            // black reads as a shutter coming down.
            let much = f64::from(frame) / f64::from(frames);
            let eased = much * much * (3. - 2. * much);
            cx.update(|_| mac::set_alpha(from + (to - from) * eased));
        }
    })
    .detach();
}
