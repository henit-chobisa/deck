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

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

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

/// The widest a pill may be: room for the longest question `deck group`
/// takes, so a pill is never cut short and never needs its words repeated.
const WIDEST: f32 = 680.;

/// What the row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The group's questions, in the order the agent wrote them.
    pub asks: Vec<SharedString>,
    /// Which have been asked, by index.
    pub asked: Vec<bool>,
}

/// Pressed: which question, and whether to edit it first.
pub type Pick = Rc<dyn Fn(usize, bool, &mut Window, &mut App)>;

/// What a pill opens to show, beside its question, while the pointer is on
/// it: how to press it. Said in the pill itself, so it is read where the
/// pressing happens, and only by somebody about to press.
#[must_use]
pub fn how() -> &'static str {
    if cfg!(target_os = "macos") {
        "click to ask \u{b7} \u{2318}-click to edit"
    } else {
        "click to ask \u{b7} ctrl-click to edit"
    }
}

/// The room between a question and its hint, in an open pill.
const HINT_GAP: f32 = 12.;

/// How long a pill takes to open, and to close again.
const OPEN: Duration = Duration::from_millis(260);
const CLOSE: Duration = Duration::from_millis(320);

/// How long the light takes to cross a pill, once, as it opens.
const SHIMMER: Duration = Duration::from_millis(900);

/// How lit each character of the hint is, `t` of the way through the shimmer:
/// a soft band of light, crossing left to right, gone by the end.
#[must_use]
fn shimmer(chars: usize, t: f32) -> Vec<f32> {
    const BAND: f32 = 0.22;
    let at = -BAND + (1. + 2. * BAND) * t.clamp(0., 1.);
    (0..chars)
        .map(|ix| {
            let place = if chars > 1 {
                ix as f32 / (chars - 1) as f32
            } else {
                0.
            };
            let near = (1. - (place - at).abs() / BAND).max(0.);
            near * near * (3. - 2. * near)
        })
        .collect()
}

/// How long an arrow's slide takes.
const GLIDE: Duration = Duration::from_millis(320);

/// One slide under way: from where, to where, since when. Offsets are gpui's,
/// zero at the start of the row and going negative as it moves on.
#[derive(Clone, Copy)]
struct Glide {
    from: f32,
    to: f32,
    since: Instant,
}

/// The row's place: every question is in it, and it slides.
///
/// A trackpad scrolls it directly; the arrows slide it one question along,
/// eased, so the eye can follow where the row went. Each place the row is
/// drawn keeps one of these — it is where the row is, not what it holds.
#[derive(Clone, Default)]
pub struct Carousel {
    handle: ScrollHandle,
    glide: Rc<Cell<Option<Glide>>>,
    /// The pill opening, or open, under the pointer; and the one closing
    /// behind it, so moving across the row hands one to the next smoothly.
    open: Rc<Cell<Option<Opening>>>,
    closing: Rc<Cell<Option<Opening>>>,
    /// How much wider a pill is, open: its hint, as last measured.
    hint: Rc<Cell<f32>>,
}

/// One pill opening or closing: which, since when, and how open it was then.
#[derive(Clone, Copy)]
struct Opening {
    ix: usize,
    since: Instant,
    from: f32,
    /// Whether the light crosses it: only when it opens from shut.
    lit: bool,
}

/// Fast out, gentle in: a thing arriving.
fn ease(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

impl Carousel {
    /// Back to the first question, at once: a new group's row starts there.
    pub fn reset(&self) {
        self.glide.set(None);
        self.open.set(None);
        self.closing.set(None);
        self.handle.set_offset(point(px(0.), px(0.)));
    }

    /// Slide one question back, or one on.
    fn slide(&self, by: isize, window: &Window, cx: &mut App) {
        self.step(by);
        self.drive(Duration::ZERO, window, cx);
    }

    /// Keep the window drawing, frame by frame, for `until` and while a slide
    /// is under way.
    ///
    /// Driven by a timer rather than by asking for the next frame from inside
    /// a render: that way each frame waits for the one before it to be drawn
    /// and then for another, and a slide came out at half the screen's rate
    /// with frames dropped — five pictures in a quarter of a second.
    fn drive(&self, until: Duration, window: &Window, cx: &mut App) {
        let handle = window.window_handle();
        let glide = self.glide.clone();
        let from = Instant::now();
        cx.spawn(async move |cx| {
            while glide.get().is_some() || from.elapsed() < until + Duration::from_millis(40) {
                if handle.update(cx, |_, window, _| window.refresh()).is_err() {
                    return;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(4))
                    .await;
            }
        })
        .detach();
    }

    /// How open pill `ix` is, from shut at 0 to showing its hint at 1.
    fn openness(&self, ix: usize) -> f32 {
        if let Some(open) = self.open.get().filter(|open| open.ix == ix) {
            let t = open.since.elapsed().as_secs_f32() / OPEN.as_secs_f32();
            return open.from + (1. - open.from) * ease(t);
        }
        if let Some(closing) = self.closing.get().filter(|closing| closing.ix == ix) {
            let t = closing.since.elapsed().as_secs_f32() / CLOSE.as_secs_f32();
            return closing.from * (1. - ease(t));
        }
        0.
    }

    /// How far the light has crossed pill `ix`, while it is crossing.
    fn light(&self, ix: usize) -> Option<f32> {
        let open = self.open.get().filter(|open| open.ix == ix && open.lit)?;
        let t = open.since.elapsed().as_secs_f32() / SHIMMER.as_secs_f32();
        (t < 1.).then_some(t)
    }

    /// How much the row is given over to one pill: the others fade by this.
    fn spotlight(&self) -> Option<(usize, f32)> {
        let lit = self.open.get().or(self.closing.get())?;
        Some((lit.ix, self.openness(lit.ix)))
    }

    /// The pointer came onto pill `ix`, or left it.
    fn peek(&self, ix: usize, on: bool, window: &Window, cx: &mut App) {
        let now = Instant::now();
        if on {
            if self.open.get().is_some_and(|open| open.ix == ix) {
                return;
            }
            if let Some(open) = self.open.get() {
                self.closing.set(Some(Opening {
                    from: self.openness(open.ix),
                    since: now,
                    ..open
                }));
            }
            let from = self.openness(ix);
            if self.closing.get().is_some_and(|closing| closing.ix == ix) {
                self.closing.set(None);
            }
            self.open.set(Some(Opening {
                ix,
                since: now,
                from,
                lit: from == 0.,
            }));
            self.keep_in_view(ix, from);
        } else if let Some(open) = self.open.get().filter(|open| open.ix == ix) {
            self.closing.set(Some(Opening {
                from: self.openness(ix),
                since: now,
                lit: false,
                ..open
            }));
            self.open.set(None);
        }
        self.drive(SHIMMER.max(OPEN).max(CLOSE), window, cx);
    }

    /// Slide the row along, if pill `ix` would open past its right edge.
    fn keep_in_view(&self, ix: usize, from: f32) {
        let Some(item) = self.handle.bounds_for_item(ix) else {
            return;
        };
        let view = self.handle.bounds();
        let right = f32::from(item.right() - view.origin.x) + self.hint.get() * (1. - from);
        let now = f32::from(self.handle.offset().x);
        let past = right - (-now + f32::from(view.size.width));
        if past > 0. {
            self.glide.set(Some(Glide {
                from: now,
                to: now - past,
                since: Instant::now(),
            }));
        }
    }

    /// Start sliding one question back, or one on.
    fn step(&self, by: isize) {
        let now = f32::from(self.handle.offset().x);
        let view = self.handle.bounds();
        let lefts: Vec<f32> = (0..)
            .map_while(|ix| self.handle.bounds_for_item(ix))
            // Laid out where they would be unscrolled: the row's own terms.
            .map(|item| f32::from(item.origin.x - view.origin.x))
            .collect();
        let max = f32::from(self.handle.max_offset().x);
        let from = self.glide.get().map_or(now, |glide| glide.to);
        let to = -next_left(&lefts, -from, max, by);
        if to != from {
            self.glide.set(Some(Glide {
                from: now,
                to,
                since: Instant::now(),
            }));
        }
    }

    /// Move the row to where its slide has got to, if one is under way.
    fn advance(&self) {
        let Some(glide) = self.glide.get() else {
            return;
        };
        let done = glide.since.elapsed().as_secs_f32() / GLIDE.as_secs_f32();
        let t = done.min(1.);
        let eased = 1. - (1. - t).powi(3);
        let x = glide.from + (glide.to - glide.from) * eased;
        self.handle.set_offset(point(px(x), px(0.)));
        if t >= 1. {
            self.glide.set(None);
        }
    }
}

/// Where the row's left edge goes after one press of an arrow, in the row's
/// own terms: `lefts` is each question's left edge, `at` how far along the row
/// is now, and `max` how far along it can go. One on is the first question
/// starting past where the row is; one back the last starting before it.
#[must_use]
pub fn next_left(lefts: &[f32], at: f32, max: f32, by: isize) -> f32 {
    let target = if by > 0 {
        lefts.iter().copied().find(|left| *left > at + 1.)
    } else {
        lefts.iter().copied().rev().find(|left| *left < at - 1.)
    };
    target
        .unwrap_or(if by > 0 { max } else { 0. })
        .clamp(0., max.max(0.))
}

/// The row, as an element, `wide` across. `corner` is the window's own
/// radius, which every pill and both arrows share, so they read as made of
/// the same thing.
pub fn render(
    row: &Row,
    palette: &Palette,
    corner: f32,
    wide: f32,
    carousel: &Carousel,
    pick: &Pick,
    window: &Window,
) -> Div {
    carousel.advance();
    let hint = how();
    let hint_size = px(12.);
    let hint_w = {
        let font = window.text_style().font();
        let run = TextRun {
            len: hint.len(),
            font,
            color: paint(palette.muted),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        f32::from(
            window
                .text_system()
                .shape_line(hint.into(), hint_size, &[run], None)
                .width,
        ) + HINT_GAP
            + 2.
    };
    carousel.hint.set(hint_w);
    let spotlight = carousel.spotlight();
    // The hint is quieter than the question it sits beside.
    let tertiary = palette.muted.mix(palette.band, 0.15);
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
        let carousel = carousel.clone();
        surface(ElementId::from(id))
            .w(px(36.))
            .justify_center()
            .text_size(px(15.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| carousel.slide(by, window, cx))
            .child(glyph)
    };
    let pills = row.asks.iter().enumerate().map(|(ix, ask)| {
        let asked = row.asked.get(ix).copied().unwrap_or(false);
        let pick = pick.clone();
        let open = carousel.openness(ix);
        let dim = match spotlight {
            Some((lit, by)) if lit != ix => 1. - 0.55 * by,
            _ => 1.,
        };
        // The light crosses the question and then the hint, as one line:
        // the question towards the accent, the hint up to the text colour.
        let words = ask.chars().count();
        let lit = carousel
            .light(ix)
            .map(|t| shimmer(words + hint.chars().count(), t))
            .unwrap_or_default();
        let glow = |at: usize| lit.get(at).copied().unwrap_or(0.);
        let runs =
            |text: &str, skip: usize, rest: deck_core::theme::Rgb, to: deck_core::theme::Rgb| {
                text.char_indices()
                    .enumerate()
                    .map(|(n, (at, ch))| {
                        (
                            at..at + ch.len_utf8(),
                            HighlightStyle {
                                color: Some(paint(rest.mix(to, 0.75 * glow(skip + n)))),
                                ..Default::default()
                            },
                        )
                    })
                    .collect::<Vec<_>>()
            };
        let question = runs(ask, 0, palette.fg, palette.accent);
        let said = runs(hint, words, tertiary, palette.fg);
        let carousel = carousel.clone();
        surface(ElementId::from(("ask", ix)))
            .max_w(px(WIDEST + hint_w))
            .px(px(13.))
            .text_size(px(13.))
            .opacity(dim)
            .when(asked, |this| {
                this.border_color(paint(palette.accent))
                    .bg(paint(palette.band.mix(palette.accent, 0.16)))
            })
            .on_hover(move |hovered, window, cx| carousel.peek(ix, *hovered, window, cx))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |event, window, cx| {
                pick(ix, event.modifiers().secondary(), window, cx);
            })
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(StyledText::new(ask.clone()).with_highlights(question)),
            )
            .child(
                // The padding inside, not on the part that opens: a shut pill
                // is exactly as wide as its question.
                div()
                    .flex_none()
                    .w(px(hint_w * open))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .whitespace_nowrap()
                            .pl(px(HINT_GAP))
                            .text_size(hint_size)
                            .child(StyledText::new(hint).with_highlights(said)),
                    ),
            )
    });
    div()
        .w(px(wide))
        .h_flex()
        .gap(px(GAP))
        .child(arrow("ask-back", "\u{2039}", -1))
        .child(
            div()
                .id("asks")
                .flex_1()
                .min_w_0()
                .h(px(PILL))
                .overflow_x_scroll()
                .track_scroll(&carousel.handle)
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

/// The questions in a window of their own, hung just below the deck.
///
/// macOS only: there a window can be made a child of another and moved with
/// it, which is what keeps this under the deck as the deck is dragged. It
/// holds no state of its own: the deck tells it what to draw.
#[cfg(target_os = "macos")]
pub struct Shelf {
    /// What to draw, or nothing while the deck shows them inside itself.
    pub row: Option<Row>,
    pub carousel: Carousel,
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
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
        div().size_full().flex().items_center().child(render(
            &row,
            &self.palette,
            self.corner,
            self.wide,
            &self.carousel,
            &pick,
            window,
        ))
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
            size: size(px(600.), px(SHELF)),
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
            carousel: Carousel::default(),
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

#[cfg(test)]
mod tests {
    // Spelled out: the gpui glob above exports a `test` attribute of its own.
    use core::prelude::v1::test;

    use super::*;

    #[test]
    fn the_light_crosses_once_and_leaves_nothing_lit() {
        let before = shimmer(20, 0.);
        let middle = shimmer(20, 0.5);
        let after = shimmer(20, 1.);
        assert!(
            before.iter().chain(&after).all(|lit| *lit == 0.),
            "dark either side"
        );
        assert!(middle[10] > 0.9, "lit where the light is");
        assert_eq!(middle[0], 0., "and only there");
        assert_eq!(middle[19], 0.);
    }

    #[test]
    fn an_arrow_moves_one_question_and_stops_at_either_end() {
        let lefts = [0., 300., 560., 900.];
        let max = 700.;
        assert_eq!(next_left(&lefts, 0., max, 1), 300.);
        assert_eq!(next_left(&lefts, 300., max, 1), 560.);
        assert_eq!(
            next_left(&lefts, 560., max, 1),
            700.,
            "no further than the end"
        );
        assert_eq!(next_left(&lefts, 700., max, 1), 700.);
        assert_eq!(next_left(&lefts, 700., max, -1), 560.);
        assert_eq!(next_left(&lefts, 150., max, -1), 0.);
        assert_eq!(next_left(&lefts, 0., max, -1), 0.);
        assert_eq!(
            next_left(&lefts, 0., 0., 1),
            0.,
            "a row that fits does not move"
        );
    }
}
