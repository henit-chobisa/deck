//! What the window shows before the first group has landed.
//!
//! A sentence on an empty window says the deck is early; it does not say what
//! is about to happen, and a large grey nothing under it reads as broken rather
//! than as waiting. So the space shows the *shape* of what is coming, drawn in
//! the deck's own hairlines with nothing written in it yet.
//!
//! It arrives the way a deck does. One pane first — a file, a gutter, a range
//! lit inside it — and then, a beat later, it opens out and a diagram takes the
//! space beside it. Which is the two kinds of pane a group can hold, said
//! without a word of explanation.
//!
//! # Why it moves
//!
//! Everything here is a placeholder, and a still placeholder is
//! indistinguishable from a window that has stopped. The lit band breathes and
//! the diagram's boxes drift, on cycles long enough not to be a spinner and out
//! of phase with each other so the whole thing never pulses as one block. It
//! all stops the moment there is something to read, because by then the deck is
//! saying it in content instead.

use std::time::Duration;

use deck_core::theme::{Palette, Rgb};
use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette::paint;

/// How wide the file ghost is.
const SHEET_W: f32 = 430.;
/// How wide the diagram ghost opens to.
const BOARD_W: f32 = 300.;
/// The gap between them, once there are two.
const APART: f32 = 18.;
/// How tall the body of either ghost is.
const BODY_H: f32 = 180.;
/// One ghost row, matching a code pane's.
const ROW_H: f32 = 20.;
/// How many rows the file ghost has, and which of them are lit.
const ROWS: usize = 9;
const LIT: std::ops::Range<usize> = 3..6;
/// How wide each row's text runs, as a fraction of the column. Uneven on
/// purpose: a ladder of equal bars reads as a chart.
const RUNS: [f32; ROWS] = [0.58, 0.42, 0.74, 0.33, 0.66, 0.5, 0.7, 0.38, 0.55];

/// The width below which there is only room for one ghost.
///
/// Both of them plus the gap, plus enough page either side that the pair is
/// centred rather than wedged.
pub const ROOM: f32 = SHEET_W + BOARD_W + APART + 90.;

/// The waiting panel.
///
/// `spread` is whether the beat has passed and the diagram should be there;
/// `room` is whether the window is wide enough to hold both.
pub fn render(palette: &Palette, spread: bool, room: bool) -> impl IntoElement {
    let palette = *palette;

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(APART))
        // A window narrower than the pair would be pushed sideways by the
        // second ghost mid-slide. Nothing here is worth a scrollbar.
        .overflow_hidden()
        .child(sheet(&palette))
        // The diagram opens out rather than appearing.
        //
        // Only its width is animated, and that is the whole trick: the row is
        // centred, so a box growing on the right carries the file ghost left
        // in front of it. Nothing is positioned by hand and nothing has to be
        // told where the other one went.
        .when(spread && room, |this| {
            this.child(board(&palette).with_animation(
                "waiting-open",
                Animation::new(Duration::from_millis(620)).with_easing(ease_out_quint()),
                |this, open| this.w(px(BOARD_W * open)).opacity(open),
            ))
        })
}

/// A quiet bar: what a line of anything looks like before it is written.
fn bar(width: DefiniteLength, colour: Rgb) -> Div {
    div()
        .w(width)
        .h(px(5.))
        .flex_none()
        .rounded_full()
        .bg(paint(colour))
}

/// The header a pane wears, with nothing written on it yet.
fn header(palette: &Palette) -> Div {
    let quiet = palette.fg.mix(palette.wash, 0.72);
    div()
        .h_flex()
        .flex_none()
        .justify_between()
        .items_center()
        .pl(px(14.))
        .pr(px(14.))
        .pt(px(10.))
        .pb(px(9.))
        .child(bar(px(104.).into(), quiet))
        .child(bar(px(56.).into(), quiet))
}

/// The outline both ghosts share, so they read as two panes of one page.
fn ghost(palette: &Palette, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .v_flex()
        .overflow_hidden()
        .rounded(px(6.))
        .bg(paint(palette.wash))
        .border_1()
        .border_color(paint(palette.edge))
        .pb(px(11.))
}

/// The file ghost: rows, a gutter, and a range lit inside it.
fn sheet(palette: &Palette) -> Div {
    let palette = *palette;
    let quiet = palette.fg.mix(palette.wash, 0.72);

    let row = move |ix: usize| {
        let is_lit = LIT.contains(&ix);
        div()
            .h(px(ROW_H))
            .w_full()
            .h_flex()
            .items_center()
            .bg(paint(if is_lit { palette.focus } else { palette.wash }))
            .child(
                div()
                    .w(px(12.))
                    .flex_none()
                    .h_flex()
                    .justify_center()
                    .when(is_lit, |this| {
                        this.child(div().w(px(2.)).h(px(ROW_H)).bg(paint(palette.accent)))
                    }),
            )
            .child(
                div()
                    .w(px(44.))
                    .flex_none()
                    .pr(px(12.))
                    .h_flex()
                    .justify_end()
                    .child(bar(px(12.).into(), quiet)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pr(px(18.))
                    .child(bar(relative(RUNS[ix]), quiet)),
            )
    };
    let band = move |range: std::ops::Range<usize>| div().v_flex().children(range.map(row));

    ghost(&palette, SHEET_W)
        .child(header(&palette))
        .child(band(0..LIT.start))
        // The lit band is the one thing in the window that says *working*
        // rather than *stuck*.
        .child(
            band(LIT).with_animation(
                "waiting-lit",
                Animation::new(Duration::from_millis(1900))
                    .repeat()
                    .with_easing(pulsating_between(0.3, 1.0)),
                |this, breath| this.opacity(breath),
            ),
        )
        .child(band(LIT.end..ROWS))
}

/// Where a ghost node sits, and how big it is.
struct Block {
    left: f32,
    top: f32,
    width: f32,
    /// The one the group would be about.
    lit: bool,
}

/// Three boxes and the arrows between them: a diagram, before it is drawn.
///
/// Laid out absolutely rather than in a flex, because the boxes drift. A box
/// that moved by changing its margin would push its neighbours around, and the
/// whole point of the drift is that nothing else notices.
const BLOCKS: [Block; 3] = [
    Block {
        left: 16.,
        top: 73.,
        width: 96.,
        lit: false,
    },
    Block {
        left: 170.,
        top: 26.,
        width: 96.,
        lit: true,
    },
    Block {
        left: 170.,
        top: 120.,
        width: 96.,
        lit: false,
    },
];
/// How tall a ghost node is.
const BLOCK_H: f32 = 34.;
/// How far a box drifts from where it sits, each way.
const DRIFT: f32 = 3.5;

/// One box's share of the drift, `phase` of a turn behind the others.
///
/// A whole sine mapped onto 0..1, which is what makes it loopable: the value
/// at the end of a turn is the value at the start, so a repeating animation
/// comes round rather than snapping back. The phase is what stops three boxes
/// on the same cycle moving as one block — offset rather than given different
/// durations, so the drift stays even and only the timing is staggered.
fn drift(phase: f32) -> impl Fn(f32) -> f32 {
    move |turn| {
        let turn = (turn + phase).fract();
        (turn * std::f32::consts::TAU).sin().mul_add(0.5, 0.5)
    }
}

fn board(palette: &Palette) -> Div {
    let palette = *palette;
    let quiet = palette.fg.mix(palette.wash, 0.72);
    let wire = palette.fg.mix(palette.wash, 0.55);

    // The elbow out of the first box and into the other two. Hairlines, the
    // way a real route is.
    let rule = move |left: f32, top: f32, width: f32, height: f32| {
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(width))
            .h(px(height))
            .bg(paint(wire))
    };

    let boxes = BLOCKS.iter().enumerate().map(move |(ix, block)| {
        let (fill, edge) = if block.lit {
            (palette.focus, palette.accent)
        } else {
            (palette.bg, palette.edge)
        };
        // Each on its own cycle, so the three never drift as one block.
        #[allow(clippy::cast_precision_loss)]
        let phase = ix as f32 * 0.31;
        let top = block.top;

        div()
            .absolute()
            .left(px(block.left))
            .top(px(top))
            .w(px(block.width))
            .h(px(BLOCK_H))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.))
            .bg(paint(fill))
            .border_1()
            .border_color(paint(edge))
            .child(bar(px(44.).into(), quiet))
            .with_animation(
                ("waiting-block", ix),
                Animation::new(Duration::from_millis(2600))
                    .repeat()
                    .with_easing(drift(phase)),
                move |this, rise| this.top(px(top - DRIFT + rise * DRIFT * 2.)),
            )
            .into_any_element()
    });

    ghost(&palette, BOARD_W).child(header(&palette)).child(
        div()
            .relative()
            .w_full()
            .h(px(BODY_H))
            .child(rule(112., 89., 36., 1.))
            .child(rule(148., 43., 1., 95.))
            .child(rule(148., 43., 22., 1.))
            .child(rule(148., 137., 22., 1.))
            .children(boxes),
    )
}
