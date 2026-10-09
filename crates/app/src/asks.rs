//! The questions a group offers: a row of pills under the window.
//!
//! The agent writes up to five per group (`deck group --ask`), as hooks: one
//! line each, from what the claim stands on to what it puts at risk. Pressing
//! one asks it, straight away, the way *Ask now* does. With the secondary
//! modifier held — ⌘ on a Mac, Ctrl elsewhere — it goes into the comment box
//! instead, to be edited first.
//!
//! The row is drawn the same wherever it lives: in a window of its own just
//! below the deck on macOS, and inside the deck above its footer everywhere
//! else, or when there is no room below it. So it is a function of what to
//! show and what to do when pressed, and knows nothing about where it is.

use std::rc::Rc;

use deck_core::theme::Palette;
#[cfg(target_os = "macos")]
use gpui_kit::component::Root;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette::paint;

/// How tall a pill is.
pub const PILL: f32 = 30.;

/// The gap between pills, and between a pill and the arrows.
const GAP: f32 = 8.;

/// The widest a pill may be. A question is meant to fit on one line well
/// short of this; past it, it is cut short and the tooltip has the rest.
const WIDEST: f32 = 460.;

/// What the row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The group's questions, in the order the agent wrote them.
    pub asks: Vec<SharedString>,
    /// The first one in view: the arrows move this.
    pub at: usize,
    /// Which have been asked, by index.
    pub asked: Vec<bool>,
}

/// Pressed: which question, and whether to edit it first.
pub type Pick = Rc<dyn Fn(usize, bool, &mut Window, &mut App)>;

/// An arrow: one pill back, or one forward.
pub type Step = Rc<dyn Fn(isize, &mut Window, &mut App)>;

/// What the tooltip says, in this platform's words for the modifier.
fn how() -> &'static str {
    if cfg!(target_os = "macos") {
        "Click to ask · \u{2318}-click to edit first"
    } else {
        "Click to ask · Ctrl-click to edit first"
    }
}

/// About how wide a question's pill is: its words at the pill's size, its
/// padding and its border. A little generous, so a pill judged to fit does.
#[must_use]
pub fn width_of(ask: &str) -> f32 {
    let words = ask.chars().count() as f32 * 7.0;
    (words + 28.).min(WIDEST)
}

/// The pills from `at` that fit, whole, in `room`.
///
/// Never none: the first one in view is shown even when it is wider than the
/// room, cut short, because an empty row between two arrows says nothing.
#[must_use]
pub fn fitting(asks: &[SharedString], at: usize, room: f32) -> usize {
    let mut used = 0.;
    let mut count = 0;
    for ask in asks.iter().skip(at) {
        let wide = width_of(ask) + if count == 0 { 0. } else { GAP };
        if count > 0 && used + wide > room {
            break;
        }
        used += wide;
        count += 1;
    }
    count
}

/// The arrows' width, and the gap beside each.
const ARROWS: f32 = 2. * (36. + GAP);

/// The row, as an element, `wide` across. `corner` is the window's own
/// radius, which every pill and both arrows share, so they read as made of
/// the same thing.
///
/// Only the pills that fit whole are drawn. Left to overflow, the layout
/// centred a row wider than its box, and the first questions — the ones the
/// reader is meant to see first — were the ones pushed out of sight.
pub fn render(
    row: &Row,
    palette: &Palette,
    corner: f32,
    wide: f32,
    pick: &Pick,
    step: &Step,
) -> Div {
    let surface = |id: ElementId| {
        div()
            .id(id)
            .flex_none()
            .h(px(PILL))
            .flex()
            .items_center()
            .rounded(px(corner))
            .border_1()
            .border_color(paint(palette.edge))
            .bg(paint(palette.band))
            .text_color(paint(palette.fg))
            .cursor_pointer()
            .hover(|style| style.bg(paint(palette.wash)))
    };
    let arrow = |id: &'static str, glyph: &'static str, by: isize| {
        let step = step.clone();
        surface(ElementId::from(id))
            .w(px(36.))
            .justify_center()
            .text_size(px(15.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| step(by, window, cx))
            .child(glyph)
    };
    let at = row.at.min(row.asks.len().saturating_sub(1));
    let room = (wide - ARROWS).max(80.);
    let shown = fitting(&row.asks, at, room);
    let pills = row
        .asks
        .iter()
        .enumerate()
        .skip(at)
        .take(shown)
        .map(|(ix, ask)| {
            let asked = row.asked.get(ix).copied().unwrap_or(false);
            let pick = pick.clone();
            let tip = SharedString::from(format!("{ask}\n{}", how()));
            surface(ElementId::from(("ask", ix)))
                .max_w(px(room.min(WIDEST)))
                .px(px(13.))
                .text_size(px(13.))
                .when(asked, |this| {
                    this.border_color(paint(palette.accent))
                        .bg(paint(palette.band.mix(palette.accent, 0.16)))
                })
                .tooltip(move |_window, cx| {
                    cx.new(|_| gpui_kit::component::tooltip::Tooltip::new(tip.clone()))
                        .into()
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |event, window, cx| {
                    pick(ix, event.modifiers().secondary(), window, cx);
                })
                .child(div().min_w_0().truncate().child(ask.clone()))
        });
    div()
        .w(px(wide))
        .h_flex()
        .gap(px(GAP))
        .child(arrow("ask-back", "\u{2039}", -1))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_flex()
                .gap(px(GAP))
                .children(pills),
        )
        .child(arrow("ask-on", "\u{203a}", 1))
}

/// How far the questions hang below the deck.
#[cfg(target_os = "macos")]
pub const HANG: f32 = 10.;

/// The shelf's own height: a pill, and a hair either side so the hover
/// colour and the border are not clipped.
#[cfg(target_os = "macos")]
pub const SHELF: f32 = PILL + 4.;

/// Clear room under the pills, for a pill's tooltip. A tooltip is drawn
/// inside its window, and in a window only a pill high it covered the pill it
/// was explaining and lost its second line. Nothing is drawn here otherwise,
/// and AppKit passes a click on a window's clear pixels to what is under it.
#[cfg(target_os = "macos")]
pub const TIP: f32 = 56.;

/// The shelf window's whole height.
#[cfg(target_os = "macos")]
pub const TALL: f32 = SHELF + TIP;

/// The questions in a window of their own, hung just below the deck.
///
/// macOS only: there a window can be made a child of another and moved with
/// it, which is what keeps this under the deck as the deck is dragged. It
/// holds no state of its own: the deck tells it what to draw.
#[cfg(target_os = "macos")]
pub struct Shelf {
    /// What to draw, or nothing while the deck shows them inside itself.
    pub row: Option<Row>,
    pub palette: Palette,
    pub corner: f32,
    /// How wide to draw: the deck's own width, told by the deck. The shelf's
    /// viewport lags its native frame by a resize, so it is not asked.
    pub wide: f32,
    pub deck: WeakEntity<crate::view::DeckView>,
    /// The deck's window: a question is asked there, where the comment box
    /// and the keyboard are.
    pub deck_window: AnyWindowHandle,
}

#[cfg(target_os = "macos")]
impl Render for Shelf {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let Some(row) = self.row.clone() else {
            return div();
        };
        let pick: Pick = {
            let deck = self.deck.clone();
            let deck_window = self.deck_window;
            Rc::new(move |ix, edit, _window, cx| {
                let deck = deck.clone();
                let _ = deck_window.update(cx, |_, window, cx| {
                    let _ = deck.update(cx, |deck, cx| deck.ask_suggested(ix, edit, window, cx));
                });
            })
        };
        let step: Step = {
            let deck = self.deck.clone();
            Rc::new(move |by, _window, cx| {
                let _ = deck.update(cx, |deck, cx| deck.step_asks(by, cx));
            })
        };
        div().size_full().flex().flex_col().child(
            div()
                .h(px(SHELF))
                .flex_none()
                .flex()
                .items_center()
                .child(render(
                    &row,
                    &self.palette,
                    self.corner,
                    self.wide,
                    &pick,
                    &step,
                )),
        )
    }
}

/// Open the shelf for the deck in `deck_window`, and hand it to the deck.
///
/// Opened once, empty and off the screen; the deck hangs it and fills it as
/// groups with questions come and go.
#[cfg(target_os = "macos")]
pub fn open_shelf(deck_window: WindowHandle<Root>, cx: &mut App) {
    let Ok(Some((deck, deck_ns, palette))) = deck_window.update(cx, |root, window, cx| {
        let deck = root
            .view()
            .clone()
            .downcast::<crate::view::DeckView>()
            .ok()?;
        let palette = deck.read(cx).palette();
        Some((deck, crate::mac::ns_window(window)?, palette))
    }) else {
        return;
    };
    let options = WindowOptions {
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(-4000.), px(-4000.)),
            size: size(px(600.), px(TALL)),
        })),
        kind: WindowKind::PopUp,
        window_background: WindowBackgroundAppearance::Transparent,
        is_movable: false,
        is_resizable: false,
        focus: false,
        show: false,
        ..Default::default()
    };
    let weak = deck.downgrade();
    let any = deck_window.into();
    let Ok(shelf) = cx.open_window(options, |window, cx| {
        window.set_background_appearance(WindowBackgroundAppearance::Transparent);
        crate::mac::never_key(window);
        cx.new(|_| Shelf {
            row: None,
            palette,
            corner: 10.,
            wide: 600.,
            deck: weak,
            deck_window: any,
        })
    }) else {
        return;
    };
    let Ok(Some(shelf_ns)) = shelf.update(cx, |_, window, _| crate::mac::ns_window(window)) else {
        return;
    };
    // gpui makes even a window without a title bar a titled one, and AppKit
    // draws a titled window a light edge along its top: a hairline across the
    // gap under the deck. Borderless has none, and no shadow either.
    shelf_ns.setStyleMask(
        objc2_app_kit::NSWindowStyleMask::Borderless
            | objc2_app_kit::NSWindowStyleMask::NonactivatingPanel,
    );
    shelf_ns.setHasShadow(false);
    // The shelf belongs to this deck and goes with it: put away to the bar,
    // closed, or quit. A deck reopened from the bar opens a shelf of its own.
    cx.observe_release(&deck, move |_, cx| {
        let _ = shelf.update(cx, |_, window, _| window.remove_window());
    })
    .detach();
    deck.update(cx, |deck, cx| {
        deck.adopt_shelf(shelf, deck_ns, shelf_ns, cx)
    });
}

/// Where the first pill in view lands after one press of an arrow: never
/// before the first, never past the last.
#[must_use]
pub fn stepped(at: usize, by: isize, count: usize) -> usize {
    let last = count.saturating_sub(1);
    at.saturating_add_signed(by).min(last)
}

#[cfg(test)]
mod tests {
    // Spelled out: the gpui glob above exports a `test` attribute of its own.
    use core::prelude::v1::test;

    use super::*;

    #[test]
    fn the_arrows_stop_at_either_end() {
        assert_eq!(stepped(0, -1, 5), 0);
        assert_eq!(stepped(0, 1, 5), 1);
        assert_eq!(stepped(4, 1, 5), 4);
        assert_eq!(stepped(2, -1, 5), 1);
        assert_eq!(stepped(0, 1, 0), 0);
    }

    #[test]
    fn only_whole_pills_are_shown_and_never_none() {
        let asks: Vec<SharedString> = ["a short one?", "another short one?", "and a third?"]
            .into_iter()
            .map(SharedString::from)
            .collect();
        let all: f32 = asks.iter().map(|a| width_of(a)).sum::<f32>() + 2. * GAP;
        assert_eq!(fitting(&asks, 0, all), 3);
        assert_eq!(fitting(&asks, 0, all - 1.), 2);
        assert_eq!(fitting(&asks, 2, 10.), 1, "the first in view, cut short");
        assert_eq!(fitting(&asks, 1, all), 2);
    }
}
