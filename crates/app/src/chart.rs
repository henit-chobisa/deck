//! One diagram pane: a picture of something that is not in any single file.
//!
//! [`deck_core::diagram`] places nodes on a grid of columns and rows and stops
//! there. It has to: a box's size depends on the face it is drawn in, and the
//! model must not know what a font is. So the grid arrives here as cells, and
//! this turns cells into pixels.
//!
//! # Why every box is the same size
//!
//! Nothing here measures a label and grows a box to fit it, which is what a
//! general diagram tool does — and is why two drawings of the same idea from
//! the same tool rarely look alike. A deck is read as a whole, so its pictures
//! have to resemble each other more than they resemble their contents. The
//! format keeps labels short for exactly this reason, and the grid is the thing
//! that makes an agent's drawing look like the deck rather than like the agent.
//!
//! # Why a canvas
//!
//! A diamond is not a rectangle and neither is an arrowhead, so the shapes and
//! the arrows are painted through [`canvas`]. Everything with words in it —
//! node labels, edge labels, cluster names — is an ordinary element laid over
//! the top, so it takes the deck's face and can be clicked. On a Mac, node
//! labels, notes and edge labels are broken by [`deck_render::text`] rather
//! than by the platform, so an export of the same diagram breaks them in the
//! same places. Cluster names are one line and are left to the window.
//!
//! The split is not incidental. A node is selectable, and a selected node is
//! what a remark about a diagram is pinned to; that needs a real element with a
//! real hit area, which a painted shape is not.

use deck_core::diagram::{Cluster, Diagram, Edge, Layout, Line, Role, Weight};
use deck_core::drawing;
use deck_core::geometry;
use deck_core::protocol::DiagramRef;
use deck_core::theme::Palette;
use deck_render::text as words;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette::paint;
use crate::sheet::Slot;
use crate::view::DeckView;

/// Whether deck breaks `text` itself, or leaves it to the window.
///
/// Left to the window:
///
/// - **Off a Mac.** The agreement test checks deck's breaks against CoreText,
///   and only on a Mac. DirectWrite and cosmic-text may shape the same words
///   a little wider, and a line drawn unwrapped would then hang out of its
///   box. Until a check runs there, they keep wrapping their own.
/// - **Text Plex has no glyph for**, a newline and a tab included. It is set
///   in a fallback face from the machine, which deck cannot measure. See
///   [`words::covers`].
fn ours(text: &str) -> bool {
    cfg!(target_os = "macos") && words::covers(text)
}

/// A width as layout really gives it: rounded to the device pixel grid, half
/// a pixel toward zero, as GPUI rounds every length before laying out.
///
/// Labels are broken against the room inside a box, and at a zoom that is
/// not a whole number that room is fractional. Unrounded, a note that fits
/// by a tenth of a pixel was cut by the box instead of with an ellipsis.
fn snap(width: f32, scale: f32) -> f32 {
    ((width * scale).abs() - 0.5).ceil().copysign(width) / scale
}

impl Chart {
    /// `text` as the lines deck breaks it into, one element each, so the
    /// window draws the breaks every export draws. See [`ours`].
    fn broken(&self, text: &str, size: f32, weight: words::Weight, room: f32) -> Vec<AnyElement> {
        if !ours(text) {
            return vec![text.to_string().into_any_element()];
        }
        self.remember(text, false, size, weight, room)
            .into_iter()
            .map(|line| div().whitespace_nowrap().child(line).into_any_element())
            .collect()
    }

    /// Drop every broken line unless they were broken at this zoom and scale.
    ///
    /// A pinch zooms by a new factor on every frame, and each one is a new
    /// size and a new room for every label, so kept forever the cache grew
    /// by a diagram's worth of lines a frame.
    fn forget_unless(&self, scale: f32) {
        let at = (self.zoom.to_bits(), scale.to_bits());
        let mut lines = self.lines.borrow_mut();
        if lines.at != at {
            lines.by.clear();
            lines.at = at;
        }
    }

    /// `text` broken to `room`, or cut to one line when `cut`, from
    /// [`Self::lines`] when it has been done before.
    fn remember(
        &self,
        text: &str,
        cut: bool,
        size: f32,
        weight: words::Weight,
        room: f32,
    ) -> Vec<String> {
        let mut lines = self.lines.borrow_mut();
        let by = lines
            .by
            .entry((cut, size.to_bits(), weight as u8, room.to_bits()))
            .or_default();
        if let Some(done) = by.get(text) {
            return done.clone();
        }
        let done = if cut {
            vec![words::truncate(text, size, weight, room)]
        } else {
            words::wrap(text, size, weight, room)
        };
        by.insert(text.to_string(), done.clone());
        done
    }
}

/// Where one node ended up, in pixels.
pub struct Spot {
    /// Index into [`Diagram::nodes`].
    pub node_ix: usize,
    /// The box it fills.
    pub at: Bounds<Pixels>,
}

/// One arrow, as the corners it turns. See [`geometry::Route`].
pub struct Route {
    /// Index into [`Diagram::edges`].
    pub edge_ix: usize,
    /// The corners, from the source's edge to the target's.
    pub points: Vec<Point<Pixels>>,
    /// Whether this edge runs against the flow. See [`Layout::back_edges`].
    pub back: bool,
}

/// Where one cluster's frame goes.
pub struct Frame {
    /// Index into [`Diagram::clusters`].
    pub cluster_ix: usize,
    /// The box it draws around its members.
    pub at: Bounds<Pixels>,
}

/// The whole drawing, in pixels.
pub struct Plan {
    /// How big the drawing is. Bigger than the pane, usually — it scrolls.
    pub size: gpui_kit::Size<Pixels>,
    /// Every node's box.
    pub spots: Vec<Spot>,
    /// Every arrow's corners.
    pub routes: Vec<Route>,
    /// Every cluster's frame.
    pub frames: Vec<Frame>,
}

/// Turn a placed grid into a drawing, in the window's pixels.
///
/// The geometry itself is [`geometry::plan`], which the exports will ask too,
/// so a picture in a PDF is the picture the reader saw. This only changes its
/// units at the window's edge.
#[must_use]
pub fn plan(diagram: &Diagram, layout: &Layout) -> Plan {
    let drawn = geometry::plan(diagram, layout);
    let bounds = |at: geometry::Rect| Bounds {
        origin: point(px(at.x), px(at.y)),
        size: size(px(at.w), px(at.h)),
    };
    Plan {
        size: size(px(drawn.width), px(drawn.height)),
        spots: drawn
            .spots
            .into_iter()
            .map(|spot| Spot {
                node_ix: spot.node_ix,
                at: bounds(spot.at),
            })
            .collect(),
        routes: drawn
            .routes
            .into_iter()
            .map(|route| Route {
                edge_ix: route.edge_ix,
                points: route
                    .points
                    .into_iter()
                    .map(|at| point(px(at.x), px(at.y)))
                    .collect(),
                back: route.back,
            })
            .collect(),
        frames: drawn
            .frames
            .into_iter()
            .map(|frame| Frame {
                cluster_ix: frame.cluster_ix,
                at: bounds(frame.at),
            })
            .collect(),
    }
}

/// One diagram pane.
pub struct Chart {
    diagram: Diagram,
    plan: Plan,
    scroll: ScrollHandle,
    label: SharedString,
    note: Option<SharedString>,
    /// The ref this pane shows, so a remark can name it.
    pub ref_id: SharedString,
    /// What the prose calls this picture, when the deck gave it a name.
    pub name: Option<SharedString>,
    /// The blocks a point in the narration is holding a light on.
    ///
    /// A picture's answer to a code pane's spotlight. The ids are the
    /// diagram's own, so the agent points at `[point checkout]` with the same
    /// word it wrote into the file.
    pointed: Vec<SharedString>,
    /// The light on the blocks being pointed at, rising when a point lands.
    ///
    /// The point on a code pane rises over 520ms, and this is the same fade at
    /// the same pace, so the agent's finger moves the same way on either kind
    /// of pane. See [`Self::point`].
    light: crate::pane::Fade,
    /// Every earlier point that is still going out, each with the light it is
    /// falling on. A hand-over: while they fall the new blocks rise, so a point
    /// moving from one block to the next crosses over. There can be several
    /// at once: pointing A, then B, then C inside one fall leaves A still
    /// going out when B starts to, and A must not be dropped to dark in a
    /// single frame because a newer point arrived.
    prior: Vec<(Vec<SharedString>, crate::pane::Fade)>,
    /// Whether the agent is talking about this pane. See [`Self::heed`].
    heeded: crate::pane::Fade,
    /// The node the reader has picked, if any.
    ///
    /// A diagram has no lines, so a remark about one is pinned to a node
    /// instead — and to nothing at all when none is picked, which makes it a
    /// remark about the whole picture.
    pub selected: Option<usize>,
    /// How much bigger or smaller than laid out the picture is drawn.
    ///
    /// The layout is not redone. A diagram's shape is its meaning — what is
    /// above what, what is beside what — and a zoom that reflowed it would be
    /// a different picture at every size. So the plan is worked out once and
    /// every measurement it holds is multiplied on the way to the screen.
    pub zoom: f32,
    /// How far the reader has carried the picture from where it was laid out.
    ///
    /// Not the scroll offset. The scroll container re-clamps its own offset to
    /// the overflow on every frame, so a picture that fitted could not be
    /// moved and one that did not could only go two of the four ways whatever
    /// was written into the handle. This is applied to the drawing itself, and
    /// nothing takes it back.
    pub nudge: Point<Pixels>,
    /// The flow being played, and how far along it is.
    ///
    /// `None` while nothing is playing, which is every diagram's resting
    /// state — a picture with a path through it is still a picture first.
    pub playing: Option<Playing>,
    /// Lines already broken, by how they were broken and then by the text.
    ///
    /// Shaping is not free, and the picture is redrawn on every frame of a
    /// drag, a zoom or a flow playing. The window's own text cache did this
    /// for it before deck broke its own lines. Only one zoom's worth is kept:
    /// see [`Self::forget_unless`].
    lines: std::cell::RefCell<Lines>,
}

/// Broken lines, keyed by how — whether cut to one line rather than wrapped,
/// the size, the weight and the room, sizes as bits so they hash — and then
/// by the text, so a lookup needs no owned key.
#[derive(Default)]
struct Lines {
    /// The zoom and scale they were broken at, as bits.
    at: (u32, u32),
    by: std::collections::HashMap<How, std::collections::HashMap<String, Vec<String>>>,
}

/// How a set of lines was broken: cut to one line or wrapped, then the size,
/// the weight and the room.
type How = (bool, u32, u8, u32);

/// A flow part-way through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playing {
    /// Which of the diagram's flows.
    pub flow: usize,
    /// How far along the path the front has travelled, in steps.
    ///
    /// A real number rather than a count, because the thing being shown is a
    /// current and not a slideshow. At 2.4 the third box is 40 percent lit and
    /// the arrow into it is 40 percent of the way across; nothing on the path
    /// ever changes state between one frame and the next.
    pub front: f32,
}

/// What a flow is doing to one node or one edge. See [`drawing::Lit`].
pub use deck_core::drawing::Lit;

impl Chart {
    /// Build a pane for `spec`.
    #[must_use]
    pub fn new(spec: &DiagramRef) -> Self {
        let layout = spec.diagram.layout();
        let plan = plan(&spec.diagram, &layout);

        Self {
            label: spec
                .diagram
                .title
                .clone()
                .unwrap_or_else(|| "diagram".to_string())
                .into(),
            note: spec.note.clone().map(SharedString::from),
            ref_id: spec.id.clone().into(),
            name: spec.name.clone().map(SharedString::from),
            pointed: Vec::new(),
            light: crate::pane::Fade::default(),
            prior: Vec::new(),
            heeded: crate::pane::Fade::default(),
            diagram: spec.diagram.clone(),
            plan,
            scroll: ScrollHandle::new(),
            selected: None,
            zoom: 1.,
            nudge: point(px(0.), px(0.)),
            playing: None,
            lines: std::cell::RefCell::default(),
        }
    }

    /// What a remark about this pane is pinned to.
    ///
    /// The picked node's label, or the diagram's own title. It goes into the
    /// comment's `quote`, which is how the far side knows what was meant
    /// without having the deck in front of it.
    #[must_use]
    pub fn quote(&self) -> String {
        self.selected
            .and_then(|ix| self.diagram.nodes.get(ix))
            .map_or_else(
                || self.label.to_string(),
                |node| match &node.note {
                    Some(note) => format!("{} — {note}", node.label),
                    None => node.label.clone(),
                },
            )
    }

    /// How wide the drawing is, so the page can tell whether it fits.
    #[must_use]
    pub fn drawn_width(&self) -> f32 {
        f32::from(self.plan.size.width)
    }

    /// The picture folded down to its spine: its name, and what it is of.
    pub fn render_folded(&self, slot: &Slot, cx: &App) -> AnyElement {
        let (title, under) =
            crate::pane::spine_words(self.name.as_ref(), &self.label, self.note.as_ref());
        let said = under.as_ref().map_or_else(
            || title.clone(),
            |under| format!("{title} — {under}").into(),
        );
        crate::pane::spine(slot, &title, under.as_ref(), said, cx)
    }

    /// The pane, label and all.
    /// Outline this pane, or stop: the agent is talking about it, or is not.
    pub fn heed(&mut self, on: bool) {
        self.heeded.set(on);
    }

    /// Whether the outline is still coming up or going out.
    #[must_use]
    pub fn fading(&self) -> bool {
        self.heeded.moving()
            || self.light.moving()
            || self.prior.iter().any(|(_, light)| light.moving())
    }

    /// The blocks a point is holding a light on.
    #[must_use]
    pub fn pointed(&self) -> &[SharedString] {
        &self.pointed
    }

    /// Point at these blocks, or at none.
    ///
    /// A point on code rises over 520ms and falls over 760, and the diagram's
    /// used to land in one frame: the block lit and every other block stepped
    /// back at once, while only the outline round the pane had a fade. That is
    /// the lapse this closes. The new blocks rise on their own light; the ones
    /// that were pointed at go out on theirs, kept here until they are dark so
    /// that moving from one block to the next crosses over rather than jumps;
    /// and the rest of the picture steps back by the same amount, so letting go
    /// of the point returns everything to rest by fading too.
    pub fn point(&mut self, ids: Vec<SharedString>) {
        if ids == self.pointed {
            return;
        }
        // Forget the ones that have finished going out; keep the rest falling.
        self.prior.retain(|(_, light)| light.moving());
        if !self.pointed.is_empty() {
            let mut light = self.light;
            light.set(false);
            self.prior.push((std::mem::take(&mut self.pointed), light));
        }
        self.light = crate::pane::Fade::default();
        if !ids.is_empty() {
            self.light.set(true);
        }
        self.pointed = ids;
    }

    /// How far the point has the rest of the picture stepped back: the higher
    /// of the rising light and the falling one, so a hand-over from one block
    /// to the next never lets the picture relax in the middle.
    fn pointed_back(&self) -> f32 {
        let rising = if self.pointed.is_empty() {
            0.
        } else {
            self.light.level()
        };
        let falling = self
            .prior
            .iter()
            .map(|(_, light)| light.level())
            .fold(0., f32::max);
        rising.max(falling)
    }

    /// How lit the block called `id` is by the point.
    fn pointed_much(&self, id: &str) -> f32 {
        let rising = if self.pointed.iter().any(|lit| lit == id) {
            self.light.level()
        } else {
            0.
        };
        // The strongest light among everything still falling on this block.
        let falling = self
            .prior
            .iter()
            .filter(|(ids, _)| ids.iter().any(|lit| lit == id))
            .map(|(_, light)| light.level())
            .fold(0., f32::max);
        rising.max(falling)
    }

    pub fn render(&self, slot: &Slot, cx: &App) -> impl IntoElement {
        let palette = slot.palette;
        let heeded = self.heeded.level();

        div()
            .v_flex()
            // Folding is a width, as it is for a file: the pane gives up its
            // share of the row a frame at a time and its spine takes the edge.
            .flex_grow(slot.share * (1. - slot.fold))
            // And fades as it goes, rather than being squeezed in full view.
            .opacity(1. - slot.fold)
            .flex_shrink(1.)
            .flex_basis(px(0.))
            .h_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(paint(palette.wash))
            .child(crate::pane::chrome(
                self.name.clone(),
                self.label.clone(),
                self.note.clone(),
                crate::pane::controls(slot, None),
                palette,
                cx,
            ))
            // The flows float over the pane's own top-right rather than in a
            // box of their own. A wrapper here was a block div — the default
            // this file has been caught by before — and the drawing inside it
            // had no height to fill, so the picture went and the buttons went
            // with it.
            .relative()
            .child(self.render_drawing(slot.ix, palette, slot.view, slot.scale))
            .children(self.render_flows(slot.ix, palette, slot.view))
            .children(self.render_pace(slot.ix, palette, slot.view, slot.pace))
            // The pane being talked about, outlined — as a code pane is. A
            // point lit the block inside the picture and left the pane itself
            // unmarked, so with two panes side by side there was nothing to
            // say which one the sentence meant (#49).
            // Not round a pane that has folded away: a name hovered in the
            // prose can ask for one, and there is nothing there to frame.
            .when(heeded > 0. && slot.fold < 1., |pane| {
                pane.child(crate::pane::outline(palette, heeded))
            })
    }

    /// The buttons that play this diagram's flows.
    ///
    /// Nothing at all when there are none, which is most diagrams.
    fn render_flows(
        &self,
        pane_ix: usize,
        palette: &Palette,
        view: &WeakEntity<DeckView>,
    ) -> Option<impl IntoElement> {
        let flows = self.flows();
        if flows.is_empty() {
            return None;
        }
        let playing = self.playing;
        Some(
            div()
                .absolute()
                // Below the pane's own label, which is 27px of it: this is
                // anchored to the whole pane, not to the drawing inside it.
                .top(px(35.))
                .right(px(10.))
                // A row, not a column. Stacked, each flow claimed a line of
                // the drawing down the side of it; side by side they take one
                // band across the top and read as what they are — a choice
                // between paths, not a list of them.
                .h_flex()
                .items_center()
                .gap(px(6.))
                .children(flows.into_iter().enumerate().map(|(ix, name)| {
                    let on = playing.is_some_and(|playing| playing.flow == ix);
                    // The pill wears the colour its path is lit in, so the
                    // button and what it does are the same thing twice.
                    let tint = self.flow_color(ix).unwrap_or(palette.accent);
                    let view = view.clone();
                    div()
                        .id(("flow", pane_ix * 64 + ix))
                        .h_flex()
                        .items_center()
                        .gap(px(6.))
                        .pl(px(9.))
                        .pr(px(7.))
                        .py(px(3.))
                        .rounded(px(11.))
                        .border_1()
                        .border_color(paint(if on { tint } else { palette.edge }))
                        .bg(paint(if on {
                            tint.mix(palette.bg, 0.86)
                        } else {
                            palette.bg
                        }))
                        .text_size(px(10.5))
                        .text_color(paint(if on { tint } else { palette.muted }))
                        .cursor_pointer()
                        .hover(move |this| this.border_color(paint(tint)))
                        .child(name)
                        // A triangle or a square, drawn rather than fetched,
                        // in a box of a fixed size. The two glyphs are not the
                        // same width, and swapping one for the other resized
                        // the pill under the pointer that had just pressed it
                        // — the one moment a control must not move.
                        .child(
                            div()
                                .flex_none()
                                .w(px(10.))
                                .flex()
                                .justify_center()
                                .text_size(px(8.))
                                .child(if on { "\u{25a0}" } else { "\u{25b6}" }),
                        )
                        .on_click(move |_, _window, cx| {
                            let _ = view.update(cx, |deck, cx| deck.play_flow(pane_ix, ix, cx));
                        })
                })),
        )
    }

    fn render_drawing(
        &self,
        pane_ix: usize,
        palette: &Palette,
        view: &WeakEntity<DeckView>,
        scale: f32,
    ) -> impl IntoElement {
        self.forget_unless(scale);
        let sheet = div()
            .id(("chart", pane_ix))
            .size_full()
            // A flex row, not the block a div is by default. Under block
            // layout the child fills the pane's width whatever its content
            // measures, so a drawing wider than the pane had nowhere to be
            // except centred inside a box too small for it — cut off at both
            // ends, and not reachable by scrolling either.
            .flex()
            .overflow_scroll()
            .track_scroll(&self.scroll)
            // Grab the picture and move it.
            //
            // A drawing bigger than its pane has to be reachable, and the
            // wheel is a poor way to travel in two directions at once —
            // especially on a pane that has just been made narrower on
            // purpose. Taking hold anywhere works, including on a node: a
            // press that does not move still selects, so nothing is lost by
            // letting the same press begin a drag.
            .cursor(CursorStyle::OpenHand)
            .on_mouse_down(MouseButton::Left, {
                let view = view.clone();
                move |event: &MouseDownEvent, _window, cx| {
                    let _ = view.update(cx, |deck, _| deck.start_pan(pane_ix, event.position));
                }
            })
            // Held modifier and wheel takes the picture in and out, which is
            // also what a trackpad pinch arrives as. Plain wheel is left
            // alone: it scrolls, which is what a wheel does everywhere else in
            // the window, and a diagram that zoomed on an unmodified wheel
            // would change size every time somebody scrolled past it.
            .on_scroll_wheel({
                let view = view.clone();
                move |event: &ScrollWheelEvent, _window, cx| {
                    let held = event.modifiers.control || event.modifiers.platform;
                    if !held {
                        return;
                    }
                    let by = f32::from(event.delta.pixel_delta(px(20.)).y);
                    let _ = view.update(cx, |deck, cx| deck.zoom_chart(pane_ix, by, cx));
                }
            })
            .child(
                // Carried by the reader's drag, before anything else about it
                // is decided.
                //
                // A flex row of its own, and at least as wide as the pane: a
                // block div here is only as wide as its content, which took
                // the centring away from the box inside it and left every
                // drawing hard against the left of its pane.
                div()
                    .relative()
                    .left(self.nudge.x)
                    .top(self.nudge.y)
                    .flex()
                    .flex_none()
                    .min_w(relative(1.))
                    .min_h(relative(1.))
                    .child(
                        // Centred when it fits, top-left when it does not.
                        //
                        // Both from one rule, and it has to be one rule because
                        // nothing here knows how big the pane is. This box is at least
                        // the size of the pane and at least the size of the drawing:
                        // when the drawing is the smaller of the two it sits in the
                        // middle of the pane, and when it is the larger the box is
                        // exactly the drawing and the centring has nothing left to do
                        // — which is what stops a wide picture being centred into a
                        // viewport it cannot be scrolled back out of.
                        div()
                            // Neither growing nor shrinking, so its width is its
                            // content's and the minimums are the only thing that can
                            // stretch it. Left to shrink it took the pane's width and
                            // centred a wider drawing inside that, which cut the left
                            // of the picture off with no way to scroll back to it.
                            .flex_none()
                            .min_w(relative(1.))
                            .min_h(relative(1.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .relative()
                                    .flex_none()
                                    .w(self.z(self.plan.size.width))
                                    .h(self.z(self.plan.size.height))
                                    .child(self.paint(palette))
                                    .children(self.cluster_names(palette))
                                    .children(self.edge_labels(palette, scale))
                                    .children(self.node_labels(pane_ix, palette, view, scale)),
                            ),
                    ),
            );

        div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(sheet)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(Scrollbar::width())
                    .child(
                        Scrollbar::vertical(&self.scroll)
                            .viewport_from_layout()
                            .max_fps(60),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(Scrollbar::width())
                    .child(
                        Scrollbar::horizontal(&self.scroll)
                            .viewport_from_layout()
                            .max_fps(60),
                    ),
            )
    }
}

/// Everything the canvas has to paint, as plain values it can own.
///
/// The paint callback outlives this frame's borrow of the chart, so it cannot
/// look anything up while painting. It is handed the finished drawing instead,
/// in the drawing's own coordinates; the shapes are built where the canvas
/// landed, by the same functions every export draws with.
struct Ink {
    frames: Vec<Bounds<Pixels>>,
    frame_skin: drawing::Skin,
    arrows: Vec<(Vec<Point<Pixels>>, bool, deck_core::theme::Rgb)>,
    boxes: Vec<(Bounds<Pixels>, Role, drawing::Skin)>,
}

impl Chart {
    /// The shapes and the arrows.
    fn paint(&self, palette: &Palette) -> impl IntoElement {
        let ink = self.ink(palette);

        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                // Everything in the plan is measured from the drawing's own
                // top-left; the canvas knows where that landed on screen.
                let at = bounds.origin;
                let mut prims = Vec::new();

                for frame in &ink.frames {
                    drawing::shape(
                        &mut prims,
                        rect(offset(*frame, at)),
                        Role::Step,
                        ink.frame_skin,
                    );
                }
                for (points, dashed, colour) in &ink.arrows {
                    let points: Vec<geometry::Pt> = points.iter().map(|p| pt(*p + at)).collect();
                    drawing::arrow(&mut prims, &points, *colour, *dashed);
                }
                for (at_box, role, skin) in &ink.boxes {
                    drawing::shape(
                        &mut prims,
                        rect(offset(body(*at_box, *role), at)),
                        *role,
                        *skin,
                    );
                }
                for prim in &prims {
                    paint_prim(window, prim);
                }
            },
        )
        .absolute()
        .size_full()
    }

    /// The drawing, as values the paint callback can own.
    fn ink(&self, palette: &Palette) -> Ink {
        let arrows = self
            .plan
            .routes
            .iter()
            .map(|route| {
                let edge = &self.diagram.edges[route.edge_ix];
                (
                    route.points.iter().map(|at| self.zp(*at)).collect(),
                    edge.line == Line::Dashed,
                    drawing::arrow_colour(
                        self.travelled(&edge.from, &edge.to),
                        route.back,
                        palette,
                    ),
                )
            })
            .collect();

        let boxes = self
            .plan
            .spots
            .iter()
            .filter_map(|spot| {
                let node = self.diagram.nodes.get(spot.node_ix)?;
                let picked = self.selected == Some(spot.node_ix);
                let lit = self.lit(&node.id);
                Some((
                    self.zb(spot.at),
                    node.role,
                    drawing::skin(node.weight, picked, lit, palette),
                ))
            })
            .collect();

        Ink {
            frames: self
                .plan
                .frames
                .iter()
                .map(|frame| self.zb(frame.at))
                .collect(),
            frame_skin: drawing::frame_skin(palette),
            arrows,
            boxes,
        }
    }

    /// The words inside the boxes, and the hit area that makes one selectable.
    fn node_labels(
        &self,
        pane_ix: usize,
        palette: &Palette,
        view: &WeakEntity<DeckView>,
        scale: f32,
    ) -> Vec<AnyElement> {
        self.plan
            .spots
            .iter()
            .filter_map(|spot| {
                let node_ix = spot.node_ix;
                let node = self.diagram.nodes.get(node_ix)?;
                let words = if node.weight == Weight::Muted {
                    palette.muted
                } else {
                    palette.fg
                };
                let view = view.clone();

                let at = self.zb(body(spot.at, node.role));
                // The room inside the shape. The inset is not zoomed: it is
                // padding, and the window does not scale padding.
                let room = snap(f32::from(at.size.width), scale)
                    - 2. * snap(drawing::inset(node.role), scale);
                let face = if node.weight == Weight::Accent {
                    words::Weight::Semibold
                } else {
                    words::Weight::Regular
                };

                Some(
                    div()
                        .id(("node", node_ix))
                        .absolute()
                        .left(at.origin.x)
                        .top(at.origin.y)
                        .w(at.size.width)
                        .h(at.size.height)
                        .v_flex()
                        .items_center()
                        .justify_center()
                        .gap(px(drawing::LABEL_GAP))
                        // The shape decides how much room the words have: a
                        // diamond's corners are empty, a pill's ends are round,
                        // and a label that ignores that spills over the edge.
                        .px(px(drawing::inset(node.role)))
                        // Below the lid, and above the foot.
                        .when(node.role == Role::Store, |this| {
                            this.pt(px(drawing::STORE_PAD.0))
                                .pb(px(drawing::STORE_PAD.1))
                        })
                        .text_align(TextAlign::Center)
                        .text_size(self.z(px(drawing::LABEL.0)))
                        // Leading spelled out rather than left to the face:
                        // see `drawing::LABEL`.
                        .line_height(self.z(px(drawing::LABEL.1)))
                        .text_color(paint(words))
                        .when(node.weight == Weight::Accent, |this| this.font_semibold())
                        // One element for the label, so the gap below is
                        // between the label and its note, never between lines.
                        .child(div().children(self.broken(
                            &node.label,
                            f32::from(self.z(px(drawing::LABEL.0))),
                            face,
                            room,
                        )))
                        .children(node.note.as_ref().map(|note| {
                            let size = f32::from(self.z(px(drawing::NOTE.0)));
                            div()
                                // One line, cut short if it will not fit.
                                //
                                // A note is documented as a second line and is
                                // meant to be read as one: left to wrap, a
                                // three-word note grew the words past the shape
                                // drawn behind them, and a label hanging out of
                                // its own box reads as a bug rather than as a
                                // long note. The label above may still wrap —
                                // it is the name, and cutting a name short
                                // loses the one thing the node is for.
                                .w_full()
                                .text_center()
                                .text_size(self.z(px(drawing::NOTE.0)))
                                .line_height(self.z(px(drawing::NOTE.1)))
                                .text_color(paint(palette.muted))
                                .map(|this| {
                                    if ours(note) {
                                        let cut = self.remember(note, true, size, face, room);
                                        this.whitespace_nowrap().overflow_hidden().children(cut)
                                    } else {
                                        this.truncate().child(note.clone())
                                    }
                                })
                        }))
                        .on_mouse_down(MouseButton::Left, {
                            move |_, _window, cx| {
                                let _ = view.update(cx, |deck, cx| {
                                    deck.pick_node(pane_ix, node_ix, cx);
                                });
                            }
                        })
                        .into_any_element(),
                )
            })
            .collect()
    }

    /// What each arrow is claiming, where it turns.
    fn edge_labels(&self, palette: &Palette, scale: f32) -> Vec<AnyElement> {
        self.plan
            .routes
            .iter()
            .filter_map(|route| {
                let edge: &Edge = self.diagram.edges.get(route.edge_ix)?;
                let label = edge.label.as_ref()?;
                // The elbow: the middle of the run along the gutter, which is
                // the one stretch of an arrow with nothing else near it.
                let (a, b) = (route.points.get(1)?, route.points.get(2)?);
                let x = f32::from(self.z(px((f32::from(a.x) + f32::from(b.x)) / 2.)));
                let y = f32::from(self.z(px((f32::from(a.y) + f32::from(b.y)) / 2.)));

                // Which way the arrow runs, taken from its two ends rather
                // than from the run through the gutter.
                //
                // That run is a point whenever the two nodes line up — a route
                // between neighbours in the same row turns no corners — and
                // asking a point which way it goes says "not horizontal", so
                // every straight arrow had its label parked on top of its own
                // arrowhead. The ends are never in the same place.
                let (from, to) = (route.points.first()?, route.points.last()?);
                let flat = (f32::from(from.y) - f32::from(to.y)).abs()
                    < (f32::from(from.x) - f32::from(to.x)).abs();

                // The widest the chip may be.
                let widest = self.z(px(drawing::chip_widest()));
                let lines = self.broken(
                    label,
                    f32::from(self.z(px(drawing::EDGE_LABEL))),
                    words::Weight::Regular,
                    snap(f32::from(widest), scale) - 2. * snap(drawing::CHIP_PAD.0, scale),
                );
                // A label that wraps fills the chip, as wrapped text fills
                // the width it was wrapped to; one line keeps to its words.
                let wraps = lines.len() > 1;
                let chip = div()
                    .px(px(drawing::CHIP_PAD.0))
                    .py(px(drawing::CHIP_PAD.1))
                    .rounded(px(drawing::CHIP_RADIUS))
                    // Kept to the gutter it sits in, give or take a few
                    // pixels. A label is meant to be a word or two — `writes`,
                    // `on submit` — and one that is not wraps rather than
                    // lying across the two nodes the arrow joins.
                    .max_w(widest)
                    .when(wraps, |this| this.w(widest))
                    .text_center()
                    // On the pane's own ground, so the line passes behind the
                    // words rather than through them.
                    .bg(paint(palette.wash))
                    .text_size(self.z(px(drawing::EDGE_LABEL)))
                    .text_color(paint(palette.muted))
                    .children(lines);

                // Above the line when the run is horizontal, beside it when
                // the run is vertical. Never *on* it.
                //
                // The chip is opaque — it has to be, or the arrow is drawn
                // through the words — and sitting on the line it covered the
                // last stretch of the arrow and the arrowhead with it. Every
                // labelled edge in the deck appeared to stop short of the node
                // it was pointing at.
                //
                // Nothing here can measure the words, so centring is done by a
                // box of known width that centres whatever it holds: a label
                // wider than the box spills evenly either side, which is the
                // same answer.
                Some(if flat {
                    /// How wide the box that does the centring is.
                    const ANCHOR: f32 = 140.;
                    /// And how tall. The chip hangs from its bottom edge, so a
                    /// label that wraps to two lines grows upward, away from
                    /// the line, rather than down across it.
                    const HANG: f32 = 44.;

                    div()
                        .absolute()
                        .left(px(x - ANCHOR / 2.))
                        .top(px(y - drawing::CHIP_ABOVE - HANG))
                        .w(px(ANCHOR))
                        .h(px(HANG))
                        .flex()
                        .items_end()
                        .justify_center()
                        .child(chip)
                        .into_any_element()
                } else {
                    div()
                        .absolute()
                        .left(px(x + drawing::CHIP_BESIDE.0))
                        .top(px(y + drawing::CHIP_BESIDE.1))
                        .child(chip)
                        .into_any_element()
                })
            })
            .collect()
    }

    /// The name on each cluster's frame.
    fn cluster_names(&self, palette: &Palette) -> Vec<AnyElement> {
        self.plan
            .frames
            .iter()
            .filter_map(|frame| {
                let cluster: &Cluster = self.diagram.clusters.get(frame.cluster_ix)?;
                Some(
                    div()
                        .absolute()
                        .left(self.z(frame.at.origin.x) + px(drawing::CLUSTER_NAME_AT.0))
                        .top(self.z(frame.at.origin.y) + px(drawing::CLUSTER_NAME_AT.1))
                        .text_size(self.z(px(drawing::CLUSTER_NAME)))
                        .text_color(paint(palette.muted))
                        .child(cluster.label.clone())
                        .into_any_element(),
                )
            })
            .collect()
    }
}

impl Chart {
    /// Whether this picture has a block by that id.
    ///
    /// How a point finds its pane: the same question a code pane answers with
    /// the range it is showing.
    #[must_use]
    pub fn has_block(&self, id: &str) -> bool {
        self.diagram.nodes.iter().any(|node| node.id == id)
    }

    /// What the playing flow, if any, is doing to the node called `id`.
    #[must_use]
    pub fn lit(&self, id: &str) -> Lit {
        let Some(playing) = self.playing else {
            // A point in the narration is the agent's finger: on this block for
            // as long as the words about it are being said, with the rest of
            // the picture stepped back.
            //
            // It yields to a flow, because a flow is the reader pressing a
            // button. Their path wins over the agent's finger for as long as it
            // runs, and the finger is still there when it finishes — the same
            // rule as a reader who scrolls a pane the agent was moving.
            //
            // It eases in and out rather than landing, for the same reason the
            // point on code does: a snap reads as a glitch, and a fade reads as
            // someone looking. At rest and at full the answers are the ones it
            // always gave.
            let back = self.pointed_back();
            let much = self.pointed_much(id);
            return if much >= 1. {
                Lit::On(1., None)
            } else if much > 0. {
                Lit::Held(much, back)
            } else if back > 0. {
                Lit::Aside(back)
            } else {
                Lit::Resting
            };
        };
        let Some(flow) = self.diagram.flows.get(playing.flow) else {
            return Lit::Resting;
        };
        let walk = flow.walk(&self.diagram);
        // A node the path visits twice belongs to the earlier visit, which is
        // what a reader watching it travel would expect: it lights when it is
        // first reached and stays lit.
        let Some(at) = walk.iter().position(|step| step.node() == id) else {
            return Lit::Aside(1.);
        };
        // The front is measured in steps, so how far this node has been
        // reached is simply how far past it the front has got — clamped, so a
        // box behind the front is fully lit and one ahead of it is not yet.
        Lit::On(
            (playing.front - at as f32).clamp(0., 1.),
            // The step's own colour beats the flow's, which beats the deck's.
            walk[at].color().or(flow.color),
        )
    }

    /// How far the flow has travelled along the edge from `from` to `to`.
    #[must_use]
    pub fn travelled(&self, from: &str, to: &str) -> Lit {
        let Some(playing) = self.playing else {
            return Lit::Resting;
        };
        let Some(flow) = self.diagram.flows.get(playing.flow) else {
            return Lit::Resting;
        };
        let walk = flow.walk(&self.diagram);
        // An edge is the space between two steps, so it fills as the front
        // crosses it and is full once the front has arrived at the far end.
        for at in 1..walk.len() {
            if walk[at - 1].node() == from && walk[at].node() == to {
                return Lit::On(
                    (playing.front - (at as f32 - 1.)).clamp(0., 1.),
                    // An arrow belongs to the step it arrives at, so it takes
                    // that step's colour.
                    walk[at].color().or(flow.color),
                );
            }
        }
        Lit::Aside(1.)
    }

    /// The flows this diagram offers, by name.
    #[must_use]
    pub fn flows(&self) -> Vec<SharedString> {
        self.diagram
            .flows
            .iter()
            .map(|flow| SharedString::from(flow.name.clone()))
            .collect()
    }

    /// How fast a flow travels: three words, the chosen one filled.
    ///
    /// Below the paths rather than beside them. It is a setting and they are
    /// actions, and a row that mixed the two put a word you press once beside
    /// words you press often. Bottom-right keeps it on the same edge, where a
    /// hand already knows to look, and out of the top of the picture where a
    /// flow usually begins.
    fn render_pace(
        &self,
        pane_ix: usize,
        palette: &Palette,
        view: &WeakEntity<DeckView>,
        pace: crate::view::Pace,
    ) -> Option<impl IntoElement> {
        let flows = !self.diagram.flows.is_empty();

        Some(
            div()
                .absolute()
                .bottom(px(10.))
                .right(px(10.))
                // Held to the pane, so a narrow one wraps the pace control
                // above the zoom rather than cutting off its left end.
                .left(px(10.))
                .h_flex()
                .flex_wrap()
                .justify_end()
                .items_center()
                .gap(px(6.))
                .child(self.render_zoom(pane_ix, palette, view))
                .when(flows, |this| {
                    this.child(
                        div()
                            // A press on a control is a press on the control,
                            // not the start of a drag on the picture under it.
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .h_flex()
                            .items_center()
                            .rounded(px(11.))
                            .border_1()
                            .border_color(paint(palette.edge))
                            .bg(paint(palette.bg))
                            .p(px(2.))
                            .rounded(px(11.))
                            .border_1()
                            .border_color(paint(palette.edge))
                            .bg(paint(palette.bg))
                            .children(crate::view::Pace::ALL.iter().map(|&which| {
                                let on = which == pace;
                                let view = view.clone();
                                div()
                                    .id(("pace", pane_ix * 8 + which as usize))
                                    .px(px(8.))
                                    .py(px(2.))
                                    .rounded(px(9.))
                                    .text_size(px(10.))
                                    .bg(paint(if on {
                                        palette.accent.mix(palette.bg, 0.82)
                                    } else {
                                        palette.bg
                                    }))
                                    .text_color(paint(if on {
                                        palette.accent
                                    } else {
                                        palette.muted
                                    }))
                                    .cursor_pointer()
                                    .hover(|this| this.text_color(paint(palette.fg)))
                                    .child(which.label())
                                    .on_click(move |_, _window, cx| {
                                        let _ = view.update(cx, |deck, cx| {
                                            deck.set_pace(pane_ix, which, cx)
                                        });
                                    })
                            })),
                    )
                }),
        )
    }

    /// Take the picture in and out, and back to the size it was drawn.
    ///
    /// Always there, small, in the corner: a held modifier and the wheel, or a
    /// pinch, also zoom, but nobody finds a gesture nothing on screen offers.
    /// The size between the two buttons puts the picture back when pressed.
    fn render_zoom(
        &self,
        pane_ix: usize,
        palette: &Palette,
        view: &WeakEntity<DeckView>,
    ) -> impl IntoElement {
        /// One press: a fifth bigger, or back by the same step.
        const STEP: f32 = 1.2;
        /// The wheel's worth of zoom that makes one step, in `zoom_chart`'s
        /// units (a pixel is 0.4% of a turn).
        const IN: f32 = (STEP - 1.) / 0.004;
        const OUT: f32 = (1. / STEP - 1.) / 0.004;
        let changed = (self.zoom - 1.).abs() >= 0.01;
        let button = |id: usize, label: &'static str, by: f32| {
            let view = view.clone();
            div()
                .id(("zoom-step", pane_ix * 4 + id))
                .w(px(20.))
                .h(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(9.))
                .text_size(px(12.))
                .text_color(paint(palette.muted))
                .cursor_pointer()
                .hover(|this| this.text_color(paint(palette.fg)).bg(paint(palette.wash)))
                .child(label)
                .on_click(move |_, _window, cx| {
                    let _ = view.update(cx, |deck, cx| deck.zoom_chart(pane_ix, by, cx));
                })
        };
        let reset = {
            let view = view.clone();
            div()
                .id(("zoom", pane_ix))
                .min_w(px(38.))
                .px(px(4.))
                .text_center()
                .text_size(px(10.))
                .text_color(paint(if changed { palette.fg } else { palette.muted }))
                .when(changed, |this| {
                    this.cursor_pointer()
                        .hover(|this| this.text_color(paint(palette.accent)))
                        .on_click(move |_, _window, cx| {
                            let _ = view.update(cx, |deck, cx| deck.reset_zoom(pane_ix, cx));
                        })
                })
                .child(format!("{:.0}%", self.zoom * 100.))
        };
        div()
            .id(("zoom-control", pane_ix))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .h_flex()
            .items_center()
            .p(px(2.))
            .rounded(px(11.))
            .border_1()
            .border_color(paint(palette.edge))
            .bg(paint(palette.bg))
            .child(button(0, "\u{2212}", OUT))
            .child(reset)
            .child(button(1, "+", IN))
    }

    /// A measurement, at the size the picture is being drawn.
    fn z(&self, value: Pixels) -> Pixels {
        value * self.zoom
    }

    /// A point, at the size the picture is being drawn.
    fn zp(&self, at: Point<Pixels>) -> Point<Pixels> {
        point(self.z(at.x), self.z(at.y))
    }

    /// A box, at the size the picture is being drawn.
    fn zb(&self, at: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds {
            origin: self.zp(at.origin),
            size: size(self.z(at.size.width), self.z(at.size.height)),
        }
    }

    /// Take the picture in or out, about its own middle.
    ///
    /// About the middle rather than about the pointer. Following the pointer
    /// is the nicer behaviour and it needs the pane's own rectangle, which is
    /// not something an element knows about itself here — and a drawing that
    /// starts centred and can be dragged anywhere loses little by growing from
    /// where it is.
    pub fn zoom_by(&mut self, factor: f32) {
        /// Small enough to see a whole picture, large enough to read a note on
        /// a machine that is not the one it was drawn on.
        const RANGE: (f32, f32) = (0.4, 3.0);
        self.zoom = (self.zoom * factor).clamp(RANGE.0, RANGE.1);
        // Back at its drawn size, exactly, after a button's step: steps that
        // began at a limit land a hair off 100%, which reads as 100% and would
        // not reset. Only a step: a pinch or the wheel arrives as many small
        // factors, and snapping those held the picture at 100% for good.
        if (factor - 1.).abs() >= 0.1 && (self.zoom - 1.).abs() < 0.02 {
            self.zoom = 1.;
        }
    }

    /// The colour the flow at `ix` is lit in, if it asked for one.
    #[must_use]
    pub fn flow_color(&self, ix: usize) -> Option<deck_core::theme::Rgb> {
        self.diagram.flows.get(ix).and_then(|flow| flow.color)
    }

    /// How many steps the flow at `ix` has.
    #[must_use]
    pub fn steps(&self, ix: usize) -> usize {
        self.diagram
            .flows
            .get(ix)
            .map_or(0, |flow| flow.walk(&self.diagram).len())
    }
}

/// How a node of this weight is filled and outlined.
///
/// Weight is emphasis, not meaning, so it only ever reaches for colours the
/// palette already has — the same ones the code panes use for the same job. A
/// picked node borrows the code pane's picked ground outright, because it is
/// the same act.
/// The box a role's shape is actually drawn in. See [`drawing::body`].
fn body(at: Bounds<Pixels>, role: Role) -> Bounds<Pixels> {
    let drawn = drawing::body(rect(at), role);
    Bounds {
        origin: point(px(drawn.x), px(drawn.y)),
        size: size(px(drawn.w), px(drawn.h)),
    }
}

/// `at`, moved to where the drawing actually landed on screen.
fn offset(at: Bounds<Pixels>, by: Point<Pixels>) -> Bounds<Pixels> {
    Bounds {
        origin: at.origin + by,
        size: at.size,
    }
}

/// A window rectangle in the drawing's plain numbers.
fn rect(at: Bounds<Pixels>) -> geometry::Rect {
    geometry::Rect {
        x: f32::from(at.origin.x),
        y: f32::from(at.origin.y),
        w: f32::from(at.size.width),
        h: f32::from(at.size.height),
    }
}

/// A window point in the drawing's plain numbers.
fn pt(at: Point<Pixels>) -> geometry::Pt {
    geometry::Pt {
        x: f32::from(at.x),
        y: f32::from(at.y),
    }
}

/// Paint one primitive of a drawing.
///
/// The whole of the window's part in drawing a diagram: every shape, arrow and
/// frame is decided in [`drawing`], and this only hands each one to the GPU.
fn paint_prim(window: &mut Window, prim: &drawing::Prim) {
    let at = |p: geometry::Pt| point(px(p.x), px(p.y));
    let bounds = |r: geometry::Rect| Bounds {
        origin: point(px(r.x), px(r.y)),
        size: size(px(r.w), px(r.h)),
    };
    match prim {
        drawing::Prim::Quad {
            at: r,
            radius,
            fill: colour,
            border: Some((width, edge)),
        } => window.paint_quad(quad(
            bounds(*r),
            gpui_kit::Corners::all(px(*radius)),
            paint(*colour),
            gpui_kit::Edges::all(px(*width)),
            paint(*edge),
            BorderStyle::Solid,
        )),
        drawing::Prim::Quad {
            at: r,
            fill: colour,
            border: None,
            ..
        } => window.paint_quad(fill(bounds(*r), paint(*colour))),
        drawing::Prim::Path {
            start,
            cmds,
            fill: colour,
        } => {
            let mut path = Path::new(at(*start));
            for cmd in cmds {
                match *cmd {
                    drawing::Cmd::Line(to) => path.line_to(at(to)),
                    drawing::Cmd::Curve { to, ctrl } => path.curve_to(at(to), at(ctrl)),
                }
            }
            window.paint_path(path, paint(*colour));
        }
    }
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this module glob-imports GPUI, which
    // exports a `test` attribute of its own and would otherwise shadow the
    // standard one.
    use core::prelude::v1::test;

    use deck_core::diagram::{Direction, Node};

    use super::*;

    fn node(id: &str) -> Node {
        Node {
            id: id.into(),
            label: id.into(),
            note: None,
            role: Role::Step,
            weight: Weight::Normal,
            lane: None,
        }
    }

    fn edge(from: &str, to: &str) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            label: None,
            line: Line::Solid,
        }
    }

    /// A chart over a bare diagram, for the things a chart does to a picture
    /// rather than the things the layout does to it.
    #[test]
    fn a_width_snaps_as_layout_rounds_it() {
        // 171.6 at 2x is 343.2 device pixels, which layout makes 343.
        assert!((snap(171.6, 2.) - 171.5).abs() < 1e-4);
        // Half a device pixel goes toward zero, not up.
        assert!((snap(10.25, 2.) - 10.).abs() < 1e-4);
        assert!((snap(10.5, 1.) - 10.).abs() < 1e-4);
        // A whole width is left alone.
        assert!((snap(156., 2.) - 156.).abs() < 1e-4);
    }

    fn charted(ids: &[&str], edges: &[(&str, &str)]) -> Chart {
        Chart::new(&deck_core::DiagramRef {
            id: "d1".to_string(),
            note: None,
            name: None,
            diagram: Diagram {
                title: None,
                flow: Direction::Down,
                nodes: ids.iter().copied().map(node).collect(),
                edges: edges.iter().map(|(f, t)| edge(f, t)).collect(),
                clusters: Vec::new(),
                flows: Vec::new(),
            },
        })
    }

    #[test]
    fn a_three_step_hand_over_never_drops_a_falling_light_to_dark() {
        let mut chart = charted(&["a", "b", "c", "d"], &[]);
        chart.point(vec!["a".into()]);
        // Let A be fully up, as if it had been pointed at a while.
        chart.light = chart.light.aged(std::time::Duration::from_millis(900));
        chart.point(vec!["b".into()]);
        // A is now falling. Point again before it has gone out.
        let before = chart.pointed_much("a");
        chart.point(vec!["c".into()]);
        let after = chart.pointed_much("a");
        assert!(before > 0.9, "{before}");
        assert!(after > 0.9, "A jumped to {after} when C arrived");
        assert!(chart.pointed_much("b") > 0. || chart.pointed_much("c") >= 0.);
        assert!(chart.fading());
        // Once the falls have run out, the list empties on the next point.
        for (_, light) in &mut chart.prior {
            *light = light.aged(std::time::Duration::from_millis(2000));
        }
        chart.point(vec!["d".into()]);
        assert_eq!(chart.prior.len(), 1, "only C, which is still falling");
    }

    #[test]
    fn a_point_lights_one_block_and_steps_the_rest_back() {
        // The picture's answer to a spotlight. Nothing is playing, so without a
        // point every block is resting; with one, that block is lit and the
        // others go aside — the same two states a flow puts them in, so the
        // drawing did not have to learn anything new.
        let mut chart = charted(&["fetch", "checkout", "persist"], &[("fetch", "checkout")]);
        assert!(chart.has_block("checkout"));
        assert!(!chart.has_block("nowhere"));
        assert_eq!(chart.lit("checkout"), Lit::Resting);

        chart.point(vec!["checkout".into()]);
        // It rises rather than lands: the instant after, the block is barely
        // lit and the rest has barely stepped back.
        assert!(matches!(chart.lit("checkout"), Lit::Held(much, _) if much < 0.1));
        assert!(matches!(chart.lit("fetch"), Lit::Aside(back) if back < 0.1));
        std::thread::sleep(std::time::Duration::from_millis(560));
        assert_eq!(chart.lit("checkout"), Lit::On(1., None));
        assert_eq!(chart.lit("fetch"), Lit::Aside(1.));

        // And a flow the reader pressed play on wins while it runs: their path
        // is theirs, and the finger is still there when it ends.
        chart.playing = Some(Playing { flow: 0, front: 0. });
        assert_ne!(
            chart.lit("checkout"),
            Lit::On(1., None),
            "the flow decides the light while it is playing"
        );
        chart.playing = None;
        assert_eq!(chart.lit("checkout"), Lit::On(1., None));
    }

    #[test]
    fn a_pinch_leaves_the_drawn_size_and_a_step_comes_back_to_it() {
        let mut chart = charted(&["a"], &[]);
        for _ in 0..5 {
            chart.zoom_by(1.004);
        }
        assert!(chart.zoom > 1.0, "small factors add up");
        let mut chart = charted(&["a"], &[]);
        chart.zoom = 0.4;
        for _ in 0..5 {
            chart.zoom_by(1.2);
        }
        assert!(
            (chart.zoom - 1.).abs() < f32::EPSILON,
            "five steps from 40% land on 100%"
        );
    }

    #[test]
    fn zooming_stays_within_reach() {
        // Small enough to see a whole picture, large enough to read a note on
        // a machine that is not the one it was drawn on — and never past
        // either, however long somebody spins the wheel.
        let mut chart = charted(&["a"], &[]);
        assert_eq!(chart.zoom, 1.);

        for _ in 0..200 {
            chart.zoom_by(1.1);
        }
        assert!(chart.zoom <= 3.0, "it stops growing");

        for _ in 0..400 {
            chart.zoom_by(0.9);
        }
        assert!(chart.zoom >= 0.4, "and stops shrinking");
    }

    #[test]
    fn a_measurement_is_multiplied_on_the_way_to_the_screen() {
        // The layout is not redone. A diagram's shape is its meaning, and a
        // zoom that reflowed it would be a different picture at every size.
        let mut chart = charted(&["a", "b"], &[("a", "b")]);
        let was = chart.plan.size.width;

        chart.zoom_by(2.);
        assert_eq!(chart.z(was), was * 2.);
        assert_eq!(chart.plan.size.width, was, "the plan itself is untouched");
    }

    #[test]
    fn the_window_draws_the_plan_every_export_draws() {
        // The geometry is core's, in points. The window only changes its
        // units, so a box, an arrow's corner and a frame land exactly where an
        // export puts them.
        let diagram = Diagram {
            title: None,
            flow: Direction::Down,
            nodes: ["a", "b", "c"].into_iter().map(node).collect(),
            edges: vec![edge("a", "c"), edge("b", "c"), edge("c", "a")],
            clusters: vec![Cluster {
                label: "core".to_string(),
                nodes: vec!["a".to_string(), "b".to_string()],
            }],
            flows: Vec::new(),
        };
        let layout = diagram.layout();
        let ours = plan(&diagram, &layout);
        let theirs = geometry::plan(&diagram, &layout);
        let rect = |at: Bounds<Pixels>| geometry::Rect {
            x: f32::from(at.origin.x),
            y: f32::from(at.origin.y),
            w: f32::from(at.size.width),
            h: f32::from(at.size.height),
        };

        assert_eq!(
            (ours.spots.len(), ours.routes.len(), ours.frames.len()),
            (theirs.spots.len(), theirs.routes.len(), theirs.frames.len())
        );
        assert_eq!(f32::from(ours.size.width), theirs.width);
        assert_eq!(f32::from(ours.size.height), theirs.height);
        for (ours, theirs) in ours.spots.iter().zip(&theirs.spots) {
            assert_eq!((ours.node_ix, rect(ours.at)), (theirs.node_ix, theirs.at));
        }
        for (ours, theirs) in ours.routes.iter().zip(&theirs.routes) {
            let points: Vec<geometry::Pt> = ours
                .points
                .iter()
                .map(|at| geometry::Pt {
                    x: f32::from(at.x),
                    y: f32::from(at.y),
                })
                .collect();
            assert_eq!(
                (ours.edge_ix, &points, ours.back),
                (theirs.edge_ix, &theirs.points, theirs.back)
            );
        }
        for (ours, theirs) in ours.frames.iter().zip(&theirs.frames) {
            assert_eq!(
                (ours.cluster_ix, rect(ours.at)),
                (theirs.cluster_ix, theirs.at)
            );
        }
    }
}
