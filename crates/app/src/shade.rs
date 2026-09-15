//! Lights off: everything except the deck, turned down.
//!
//! A window cannot reach into another application and dim it, and nothing on
//! macOS will darken the desktop on request. What can be done is to put a
//! sheet of near-black across every display and leave the deck sitting on top
//! of it — which looks the same, and is the whole trick.
//!
//! # Why it stays below without being told to
//!
//! Nothing here sets a window level. The deck opens as [`WindowKind::PopUp`]
//! and the shade as [`WindowKind::Normal`], so the deck is above it, and macOS
//! keeps every window of the frontmost application above every window of the
//! others. Both facts together give the behaviour for free: while deck is the
//! application you are in, the shade covers everything else; the moment you
//! switch to something else, that something else comes out in front of both.
//!
//! Which is right. Zen is for while you are reading, and the way you stop
//! reading is by looking somewhere else.

use std::time::Duration;

use deck_core::config::Zen;
use gpui_kit::*;

/// Show or hide the menu bar and the Dock.
///
/// The one thing a shade cannot do by being a window. AppKit will not position
/// a window over the menu bar however it is asked — ask for the full display
/// and the window comes back moved down by exactly the bar's height, with a
/// seam along the top where it still shines through. The only way past that is
/// to ask the application to hide the bar, which is a message to the shared
/// application and not something GPUI exposes.
///
/// The Dock goes with it because AppKit requires it: auto-hiding the menu bar
/// without saying anything about the Dock raises. Which is the right answer
/// anyway — a Dock left lit under a dimmed screen is the same seam again, at
/// the other edge.
#[cfg(target_os = "macos")]
fn system_chrome(shown: bool) {
    use objc2_app_kit::{NSApplication, NSApplicationPresentationOptions};

    // SAFETY: every path into this is a keystroke or a window callback, both
    // of which are the main thread, which is what the marker asserts.
    let marker = unsafe { objc2::MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(marker);

    app.setPresentationOptions(if shown {
        NSApplicationPresentationOptions::Default
    } else {
        NSApplicationPresentationOptions::AutoHideMenuBar
            | NSApplicationPresentationOptions::AutoHideDock
    });
}

/// Elsewhere there is no menu bar of ours to hide.
#[cfg(not(target_os = "macos"))]
fn system_chrome(_shown: bool) {}

/// How long the lights take to go down.
const DOWN: Duration = Duration::from_millis(240);

/// And to come back up. Quicker: going dark is something a reader settles into,
/// and coming back is something they are waiting on.
const UP: Duration = Duration::from_millis(160);

/// The shade over one display.
struct Sheet {
    dim: f32,
    /// Whether the lights are on their way back up.
    ///
    /// A window cannot be faded and then removed in one act — removing it is
    /// instant and the fade would never be seen — so going away is a state the
    /// sheet is in for as long as the fade lasts, and the window is taken away
    /// at the end of it.
    going: bool,
}

impl Render for Sheet {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Black, not the palette's own ground. A shade is an absence of light
        // rather than a surface, and a themed one read as a second enormous
        // pane of the deck.
        //
        // Clicking it brings the lights up, and that is a safety valve as much
        // as a convenience. The platform offers no way to let a click fall
        // through a window, so a shade necessarily swallows one — and a click
        // it swallowed could take the keyboard with it, leaving `z` dead and a
        // dark screen with no obvious way out. Now the obvious thing works:
        // press the dark, and it goes.
        let (dim, going) = (self.dim, self.going);
        div()
            .id("shade")
            .size_full()
            .bg(gpui_kit::black())
            .on_click(|_, _, cx| {
                // Deferred: this hands back the window being drawn.
                cx.defer(lights_on);
            })
            // Faded, not switched. The screen going black between one frame
            // and the next reads as something having gone wrong; over a fifth
            // of a second it reads as the lights being turned down, which is
            // what it is. Coming back up is quicker than going down, the way
            // it is for anything a person is waiting on rather than settling
            // into.
            .with_animation(
                if going { "lights-up" } else { "lights-down" },
                Animation::new(if going { UP } else { DOWN }).with_easing(ease_in_out),
                move |this, much| this.opacity(if going { dim * (1. - much) } else { dim * much }),
            )
    }
}

/// Every shade currently up, one per display.
///
/// A global rather than something the deck holds, because the windows outlive
/// any one render and belong to the screen rather than to the view. Empty means
/// zen is off, which is the only state worth asking about.
#[derive(Default)]
pub struct Shades(Vec<WindowHandle<Sheet>>);

impl Global for Shades {}

/// What the reader asked for, from the config file.
///
/// A global because it is read once at startup and never changes, and because
/// the alternative was carrying a reading preference through `Session` — which
/// is the model, and has no business knowing how bright somebody likes their
/// screen.
#[derive(Default)]
pub struct Asked(pub Zen);

impl Global for Asked {}

/// Remember what the config said, before any window opens.
pub fn remember(zen: Zen, cx: &mut App) {
    cx.set_global(Asked(zen));
}

/// Whether the lights are currently off.
#[must_use]
pub fn on(cx: &App) -> bool {
    cx.try_global::<Shades>()
        .is_some_and(|shades| !shades.0.is_empty())
}

/// Whether every window still open is a shade.
///
/// The question behind it is "has the deck gone and left the dark behind?",
/// which is the one failure this module must not have: shades cover every
/// display and nothing is drawn on them, so a shade outliving its deck is a
/// black screen with nothing to press and no window to close.
#[must_use]
pub fn only_shades_left(cx: &App) -> bool {
    let up = cx.try_global::<Shades>().map_or(0, |shades| shades.0.len());
    up > 0 && cx.windows().len() == up
}

/// Turn the lights off, or back on.
///
/// Returns whether they ended up off, so a caller can say so.
pub fn toggle(deck: AnyWindowHandle, cx: &mut App) -> bool {
    if on(cx) {
        lights_on(cx);
        return false;
    }
    let zen = cx
        .try_global::<Asked>()
        .map_or_else(Zen::default, |asked| asked.0.clone());
    lights_off(&zen, cx);

    if !on(cx) {
        return false;
    }
    watch(deck, cx);
    true
}

/// Bring the lights up when the reader stops looking at the deck.
///
/// Zen is a posture rather than a setting: it is for while you are reading, and
/// the way somebody stops reading is by looking at something else. Leaving the
/// shade up when they have gone elsewhere turns it from a help into a thing
/// covering their screen — and since a shade swallows clicks, one they would
/// then have to work out how to get rid of.
///
/// Asked rather than waited for, because the platform offers no hook for it:
/// there is `is_window_active` and nothing to subscribe to. The cost is a
/// question every fifth of a second, and only for as long as the lights are
/// off.
fn watch(deck: AnyWindowHandle, cx: &mut App) {
    const EVERY: Duration = Duration::from_millis(200);

    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(EVERY).await;

            let carry_on = cx.update(|cx| {
                if !on(cx) {
                    return false;
                }
                // A window that has gone entirely is not one to keep watching,
                // and neither is one nobody is looking at.
                match deck.update(cx, |_, window, _| window.is_window_active()) {
                    Ok(true) => true,
                    Ok(false) | Err(_) => {
                        lights_on(cx);
                        false
                    }
                }
            });

            if !carry_on {
                return;
            }
        }
    })
    .detach();
}

/// Put a shade over every display.
///
/// Every display rather than the one the deck is on: a second monitor left at
/// full brightness beside a dimmed one is worse than not dimming at all,
/// because the bright one is now the only lit thing that is *not* the deck.
fn lights_off(zen: &Zen, cx: &mut App) {
    let dim = zen.opacity();
    let mut up = Vec::new();

    // The bar goes first, and that order is the whole of it.
    //
    // A window is clamped to the screen it can see *at the moment it opens*.
    // Hidden afterwards, the menu bar's strip of screen appears underneath a
    // shade that has already been placed below it — so the top of the screen
    // comes back undimmed and the shade hangs off the bottom by exactly the
    // height of the bar. Taken away first, the whole display is placeable and
    // the shade covers all of it.
    system_chrome(false);

    for display in cx.displays() {
        let options = WindowOptions {
            // The visible frame, not the whole display.
            //
            // Asking for the full bounds — menu bar included — came back
            // placed somewhere else entirely: the platform will not put a
            // window over the menu bar by ordinary means, and what it does
            // instead is move it. Asking for exactly the frame it would allow
            // lands exactly, every time. The cost is the menu bar staying lit,
            // which is system chrome rather than something competing for the
            // reader's attention.
            window_bounds: Some(WindowBounds::Windowed(display.bounds())),
            display_id: Some(display.id()),
            // No titlebar and no client decorations.
            //
            // Asking for a frame gets a frame: a rounded panel in the window's
            // own ground, with its own border and its own shadow. On a sheet
            // covering the whole screen that panel is a hairline box drawn
            // around the edge of everything and a seam where its corners are —
            // which is exactly what a shade must not have, because a shade is
            // supposed to be the absence of a window rather than a very large
            // one. The pill learned this first; see `open_pill_over`.
            titlebar: None,
            kind: WindowKind::Normal,
            // The deck keeps the keyboard. A shade that took focus would
            // swallow the very key that turns it off.
            focus: false,
            show: true,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            ..Default::default()
        };

        // The appearance is set here, in the window's own builder, and not
        // through the options.
        //
        // `window_background` in `WindowOptions` does not reach the platform:
        // GPUI's macOS window keeps it as a field it never reads at creation,
        // so the window opens opaque whatever was asked for — which is why
        // this looked like a flat black sheet rather than a blur. Asking again
        // from the handle afterwards works, but not before the window has
        // painted a frame or two, and each of those frames is opaque.
        let wanted = if zen.blur {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Transparent
        };
        match cx.open_window(options, |window, cx| {
            window.set_background_appearance(wanted);
            cx.new(|_| Sheet { dim, going: false })
        }) {
            Ok(handle) => up.push(handle),
            // One display refusing is not a reason to leave the others dark
            // with no way to say so — but it is worth saying out loud, because
            // a half-shaded screen looks like a bug rather than a failure.
            Err(err) => eprintln!("deck: could not dim a display: {err}"),
        }
    }

    if up.is_empty() {
        // Nothing to dim with, so nothing should have been taken away.
        system_chrome(true);
        return;
    }
    cx.set_global(Shades(up));
}

/// Take every shade away.
///
/// Called on every path where the deck itself goes: closing, hiding,
/// submitting. A shade outliving its deck is a near-black screen with nothing
/// on it to press, which is the one failure this module must not have.
pub fn lights_on(cx: &mut App) {
    let Some(shades) = cx.try_global::<Shades>() else {
        return;
    };
    let up: Vec<_> = shades.0.clone();
    if up.is_empty() {
        return;
    }

    // Cleared first, so anything that asks is told the lights are on from this
    // moment rather than when the fade finishes. A shade that is on its way
    // out must not count as one that is up: `only_shades_left` would keep the
    // process alive for it, and pressing `z` during the fade would read as
    // turning zen off twice rather than off and on again.
    cx.set_global(Shades::default());

    // The bar comes back at once rather than after the fade. It is not part of
    // the shade, and watching it slide in behind a sheet that is still fading
    // would be two animations disagreeing about what is happening.
    system_chrome(true);

    for handle in &up {
        let _ = handle.update(cx, |sheet, _, cx| {
            sheet.going = true;
            cx.notify();
        });
    }

    // And taken away once it has been seen to go.
    cx.spawn(async move |cx| {
        cx.background_executor().timer(UP).await;
        let _ = cx.update(|cx| {
            for handle in up {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        });
    })
    .detach();
}
