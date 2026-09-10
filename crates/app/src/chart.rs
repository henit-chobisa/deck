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
//! the top, so it wraps, takes the deck's face, and can be clicked.
//!
//! The split is not incidental. A node is selectable, and a selected node is
//! what a remark about a diagram is pinned to; that needs a real element with a
//! real hit area, which a painted shape is not.

use deck_core::diagram::{Cluster, Diagram, Direction, Edge, Layout, Line, Role, Weight};
use deck_core::protocol::DiagramRef;
use deck_core::theme::Palette;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette::paint;
use crate::sheet::Slot;
use crate::view::DeckView;

/// How wide a box is.
const NODE_W: f32 = 168.;
/// How tall a box is.
const NODE_H: f32 = 58.;
/// The gutter between columns. Every arrow that has to travel sideways travels
/// down the middle of one of these, which is what keeps a route off the boxes.
///
/// Wide enough to hold a short edge label, because that is where a label goes.
const GAP_X: f32 = 56.;
/// The gutter between rows.
const GAP_Y: f32 = 42.;
/// The margin around the whole drawing. At least half a gutter, so an arrow
/// leaving the outermost column still has somewhere to turn.
const PAD: f32 = 30.;
/// How far a cluster's frame stands off the boxes it holds.
const CLUSTER_PAD: f32 = 15.;
/// Extra room at the top of a cluster, for its name.
const CLUSTER_HEAD: f32 = 19.;
/// How thick an arrow is.
const STROKE: f32 = 1.25;
/// How far back an arrowhead reaches from its point, and how wide it is there.
const HEAD: (f32, f32) = (9., 6.5);

/// Where one node ended up, in pixels.
pub struct Spot {
    /// Index into [`Diagram::nodes`].
    pub node_ix: usize,
    /// The box it fills.
    pub at: Bounds<Pixels>,
}

/// One arrow, as the corners it turns.
///
/// Always four points: out of the source, into a gutter, along it, and into the
/// target. Two of them coincide when the nodes line up, which draws as a
/// straight arrow without any special case for it.
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

/// Turn a placed grid into a drawing.
///
/// Pure, and the reason this is a free function rather than a method: it is
/// the whole of the geometry, and it can be checked without a window.
#[must_use]
pub fn plan(diagram: &Diagram, layout: &Layout) -> Plan {
    let spots: Vec<Spot> = layout
        .placed
        .iter()
        .map(|placed| Spot {
            node_ix: placed.node_ix,
            at: Bounds {
                origin: point(
                    px(PAD + placed.col as f32 * (NODE_W + GAP_X)),
                    px(PAD + placed.row as f32 * (NODE_H + GAP_Y)),
                ),
                size: size(px(NODE_W), px(NODE_H)),
            },
        })
        .collect();

    // A box is findable by node index rather than by position in `spots`,
    // because an edge names nodes and clusters name nodes.
    let box_of = |node_ix: usize| spots.iter().find(|s| s.node_ix == node_ix).map(|s| s.at);
    let index = |id: &str| diagram.nodes.iter().position(|node| node.id == id);

    let routes: Vec<Route> = diagram
        .edges
        .iter()
        .enumerate()
        .filter_map(|(edge_ix, edge)| {
            let (from, to) = (index(&edge.from)?, index(&edge.to)?);
            let back = layout.back_edges.contains(&edge_ix);
            Some(Route {
                edge_ix,
                points: route(box_of(from)?, box_of(to)?, diagram.flow, back),
                back,
            })
        })
        .collect();

    let frames: Vec<Frame> = diagram
        .clusters
        .iter()
        .enumerate()
        .filter_map(|(cluster_ix, cluster)| {
            Some(Frame {
                cluster_ix,
                at: around(cluster, &index, &box_of)?,
            })
        })
        .collect();

    // The drawing is as big as everything in it, plus a margin. Taken from what
    // was actually placed rather than from the grid's dimensions, because a
    // cluster's frame stands outside the boxes it holds.
    let mut width: f32 = 0.;
    let mut height: f32 = 0.;
    let mut stretch = |at: Bounds<Pixels>| {
        width = width.max(f32::from(at.origin.x + at.size.width));
        height = height.max(f32::from(at.origin.y + at.size.height));
    };
    for spot in &spots {
        stretch(spot.at);
    }
    for frame in &frames {
        stretch(frame.at);
    }

    Plan {
        size: size(px(width + PAD), px(height + PAD)),
        spots,
        routes,
        frames,
    }
}

/// The frame around a cluster's members, or `None` if it names none that exist.
fn around(
    cluster: &Cluster,
    index: &impl Fn(&str) -> Option<usize>,
    box_of: &impl Fn(usize) -> Option<Bounds<Pixels>>,
) -> Option<Bounds<Pixels>> {
    let boxes: Vec<Bounds<Pixels>> = cluster
        .nodes
        .iter()
        .filter_map(|id| index(id).and_then(&box_of))
        .collect();
    let first = *boxes.first()?;

    let mut left = f32::from(first.origin.x);
    let mut top = f32::from(first.origin.y);
    let mut right = left;
    let mut bottom = top;
    for at in boxes {
        left = left.min(f32::from(at.origin.x));
        top = top.min(f32::from(at.origin.y));
        right = right.max(f32::from(at.origin.x + at.size.width));
        bottom = bottom.max(f32::from(at.origin.y + at.size.height));
    }

    Some(Bounds {
        origin: point(px(left - CLUSTER_PAD), px(top - CLUSTER_PAD - CLUSTER_HEAD)),
        size: size(
            px(right - left + CLUSTER_PAD * 2.),
            px(bottom - top + CLUSTER_PAD * 2. + CLUSTER_HEAD),
        ),
    })
}

/// The corners one arrow turns, from `from`'s edge to `to`'s.
///
/// Every route leaves a box, reaches a gutter, travels along it, and arrives.
/// Since the gutters are empty by construction — the grid puts nothing in them
/// — a route drawn this way cannot cross a box, and no two-arrow case needs a
/// special rule.
///
/// A back edge is the exception, and it is why the gutter matters. It runs
/// against the flow, so it cannot use the gutter *between* the ranks it spans;
/// it steps sideways into the neighbouring one and travels there instead, which
/// is what makes a return arrow read as returning rather than as a mistake.
fn route(
    from: Bounds<Pixels>,
    to: Bounds<Pixels>,
    flow: Direction,
    back: bool,
) -> Vec<Point<Pixels>> {
    let side = |at: Bounds<Pixels>| {
        (
            f32::from(at.origin.x),
            f32::from(at.origin.y),
            f32::from(at.origin.x + at.size.width),
            f32::from(at.origin.y + at.size.height),
        )
    };
    let (fl, ft, fr, fb) = side(from);
    let (tl, tt, tr, tb) = side(to);
    let (fx, fy) = ((fl + fr) / 2., (ft + fb) / 2.);
    let (tx, ty) = ((tl + tr) / 2., (tt + tb) / 2.);

    let corners = match (flow, back) {
        // With the flow: out of the trailing edge, into the gutter that runs
        // right up against the target, along it, and in.
        //
        // Against the target rather than halfway between the two, which is
        // what this did first. Halfway is a gutter only when the two nodes are
        // one rank apart; over a longer span it is the middle of whatever rank
        // it lands in, and the sideways run went straight through the nodes
        // sitting there — arriving at one of them edge-on, so the arrow looked
        // like it joined a node it had nothing to do with. The gutter beside
        // the target is a gutter however far the edge has come, and for
        // neighbouring ranks it is the same line as before.
        (Direction::Down, false) => {
            let lane = tt - GAP_Y / 2.;
            [(fx, fb), (fx, lane), (tx, lane), (tx, tt)]
        }
        (Direction::Right, false) => {
            let lane = tl - GAP_X / 2.;
            [(fr, fy), (lane, fy), (lane, ty), (tl, ty)]
        }
        // Against it: sideways into the next gutter over, back along that, and
        // in through the same side it left by.
        (Direction::Down, true) => {
            let lane = fr + GAP_X / 2.;
            [(fr, fy), (lane, fy), (lane, ty), (tr, ty)]
        }
        (Direction::Right, true) => {
            let lane = fb + GAP_Y / 2.;
            [(fx, fb), (fx, lane), (tx, lane), (tx, tb)]
        }
    };

    corners
        .into_iter()
        .map(|(x, y)| point(px(x), px(y)))
        .collect()
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
    /// The node the reader has picked, if any.
    ///
    /// A diagram has no lines, so a remark about one is pinned to a node
    /// instead — and to nothing at all when none is picked, which makes it a
    /// remark about the whole picture.
    pub selected: Option<usize>,
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
}

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

/// What a flow is doing to one node or one edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lit {
    /// Nothing is playing. The picture reads as it always does.
    Resting,
    /// On the path, and this far reached: zero as the front arrives, one once
    /// it has passed. Everything between is the current going through.
    ///
    /// The colour is what the flow or the step asked for, and `None` means
    /// the deck's own accent — which is the right answer for a picture with
    /// one path through it.
    On(f32, Option<deck_core::theme::Rgb>),
    /// Not on the path, while a path is being shown.
    Aside,
}

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
            diagram: spec.diagram.clone(),
            plan,
            scroll: ScrollHandle::new(),
            selected: None,
            nudge: point(px(0.), px(0.)),
            playing: None,
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

    /// The pane, label and all.
    pub fn render(&self, slot: &Slot, cx: &App) -> impl IntoElement {
        let palette = slot.palette;

        div()
            .v_flex()
            .flex_grow(slot.share)
            .flex_shrink(1.)
            .flex_basis(px(0.))
            .h_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(paint(palette.wash))
            .child(crate::pane::chrome(
                self.label.clone(),
                self.note.clone(),
                None,
                palette,
                cx,
            ))
            // The flows float over the pane's own top-right rather than in a
            // box of their own. A wrapper here was a block div — the default
            // this file has been caught by before — and the drawing inside it
            // had no height to fill, so the picture went and the buttons went
            // with it.
            .relative()
            .child(self.render_drawing(slot.ix, palette, slot.view))
            .children(self.render_flows(slot.ix, palette, slot.view, slot.pace))
    }

    /// The buttons that play this diagram's flows.
    ///
    /// Nothing at all when there are none, which is most diagrams.
    fn render_flows(
        &self,
        pane_ix: usize,
        palette: &Palette,
        view: &WeakEntity<DeckView>,
        pace: crate::view::Pace,
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
                // The speed, before the paths. It applies to all of them, and
                // one button that cycles three words is smaller than three
                // buttons of which two are always wrong.
                .child({
                    let view = view.clone();
                    div()
                        .id(("pace", pane_ix))
                        .px(px(8.))
                        .py(px(3.))
                        .rounded(px(11.))
                        .border_1()
                        .border_color(paint(palette.edge))
                        .bg(paint(palette.bg))
                        .text_size(px(10.5))
                        .text_color(paint(palette.muted))
                        .cursor_pointer()
                        .hover(|this| this.border_color(paint(palette.accent)))
                        .child(pace.label())
                        .on_click(move |_, _window, cx| {
                            let _ = view.update(cx, |deck, cx| deck.change_pace(pane_ix, cx));
                        })
                })
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
    ) -> impl IntoElement {
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
                                    .w(self.plan.size.width)
                                    .h(self.plan.size.height)
                                    .child(self.paint(palette))
                                    .children(self.cluster_names(palette))
                                    .children(self.edge_labels(palette))
                                    .children(self.node_labels(pane_ix, palette, view)),
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

/// How a box is filled and outlined.
#[derive(Clone, Copy)]
struct Skin {
    fill: Hsla,
    edge: Hsla,
    width: f32,
}

/// Everything the canvas has to paint, as plain values it can own.
///
/// The paint callback outlives this frame's borrow of the chart, so it cannot
/// look anything up while painting. It is handed the finished drawing instead.
struct Ink {
    frames: Vec<Bounds<Pixels>>,
    frame_skin: Skin,
    arrows: Vec<(Vec<Point<Pixels>>, bool, Hsla)>,
    boxes: Vec<(Bounds<Pixels>, Role, Skin)>,
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

                for frame in &ink.frames {
                    shape(window, offset(*frame, at), Role::Step, ink.frame_skin);
                }
                for (points, dashed, colour) in &ink.arrows {
                    let points: Vec<Point<Pixels>> = points.iter().map(|p| *p + at).collect();
                    for pair in points.windows(2) {
                        stroke(window, pair[0], pair[1], *colour, *dashed);
                    }
                    if let [.., before, last] = points.as_slice() {
                        arrowhead(window, *before, *last, *colour);
                    }
                }
                for (at_box, role, skin) in &ink.boxes {
                    shape(window, offset(body(*at_box, *role), at), *role, *skin);
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
                    route.points.clone(),
                    edge.line == Line::Dashed,
                    // A returning arrow is drawn in the accent, because it is
                    // the one thing in a state machine a reader has to notice
                    // — and because otherwise it reads as an arrow pointing
                    // the wrong way by mistake.
                    paint({
                        let quiet = palette.fg.mix(palette.wash, 0.45);
                        match self.travelled(&edge.from, &edge.to) {
                            // Travelling is the thing being shown, so the
                            // arrows carry it as much as the boxes do — and
                            // they carry it first, since an arrow is the part
                            // of the path that is only ever motion.
                            Lit::On(much, tint) => quiet
                                .mix(palette.bg, ASIDE)
                                .mix(tint.unwrap_or(palette.accent), much),
                            Lit::Aside => quiet.mix(palette.bg, ASIDE),
                            Lit::Resting => {
                                if route.back {
                                    palette.accent
                                } else {
                                    quiet
                                }
                            }
                        }
                    }),
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
                Some((spot.at, node.role, skin(node.weight, picked, lit, palette)))
            })
            .collect();

        Ink {
            frames: self.plan.frames.iter().map(|frame| frame.at).collect(),
            // A cluster is a ground, not an object: it sits under the boxes and
            // is told apart from the pane by a shade, not by a border with any
            // weight to it.
            frame_skin: Skin {
                fill: paint(palette.wash.mix(palette.ground, 0.22)),
                edge: paint(palette.edge.mix(palette.wash, 0.35)),
                width: 1.,
            },
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

                let at = body(spot.at, node.role);

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
                        .gap(px(1.))
                        // The shape decides how much room the words have: a
                        // diamond's corners are empty, a pill's ends are round,
                        // and a label that ignores that spills over the edge.
                        .px(px(inset(node.role)))
                        // Below the lid, and above the foot.
                        .when(node.role == Role::Store, |this| this.pt(px(12.)).pb(px(6.)))
                        .text_align(TextAlign::Center)
                        .text_size(px(12.5))
                        // Leading spelled out rather than left to the face.
                        //
                        // A box is a fixed height and what goes in it has to be
                        // known to fit. With the default leading a label and a
                        // note came to more than the box, and the note was cut
                        // through by the bottom border — worst on a store,
                        // which gives up its head to the lid. A 15 and a 13
                        // leave room inside 50 even when the label wraps.
                        .line_height(px(15.))
                        .text_color(paint(words))
                        .when(node.weight == Weight::Accent, |this| this.font_semibold())
                        .child(node.label.clone())
                        .children(node.note.as_ref().map(|note| {
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
                                .truncate()
                                .text_center()
                                .text_size(px(10.5))
                                .line_height(px(13.))
                                .text_color(paint(palette.muted))
                                .child(note.clone())
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
    fn edge_labels(&self, palette: &Palette) -> Vec<AnyElement> {
        self.plan
            .routes
            .iter()
            .filter_map(|route| {
                let edge: &Edge = self.diagram.edges.get(route.edge_ix)?;
                let label = edge.label.as_ref()?;
                // The elbow: the middle of the run along the gutter, which is
                // the one stretch of an arrow with nothing else near it.
                let (a, b) = (route.points.get(1)?, route.points.get(2)?);
                let x = (f32::from(a.x) + f32::from(b.x)) / 2.;
                let y = (f32::from(a.y) + f32::from(b.y)) / 2.;

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

                let chip = div()
                    .px(px(4.))
                    .py(px(1.))
                    .rounded(px(3.))
                    // Kept to the gutter it sits in, give or take a few
                    // pixels. A label is meant to be a word or two — `writes`,
                    // `on submit` — and one that is not wraps rather than
                    // lying across the two nodes the arrow joins.
                    .max_w(px(GAP_X + 16.))
                    .text_center()
                    // On the pane's own ground, so the line passes behind the
                    // words rather than through them.
                    .bg(paint(palette.wash))
                    .text_size(px(10.))
                    .text_color(paint(palette.muted))
                    .child(label.clone());

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
                        .top(px(y - 4. - HANG))
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
                        .left(px(x + 7.))
                        .top(px(y - 8.))
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
                        .left(frame.at.origin.x + px(13.))
                        .top(frame.at.origin.y + px(5.))
                        .text_size(px(10.))
                        .text_color(paint(palette.muted))
                        .child(cluster.label.clone())
                        .into_any_element(),
                )
            })
            .collect()
    }
}

impl Chart {
    /// What the playing flow, if any, is doing to the node called `id`.
    #[must_use]
    pub fn lit(&self, id: &str) -> Lit {
        let Some(playing) = self.playing else {
            return Lit::Resting;
        };
        let Some(flow) = self.diagram.flows.get(playing.flow) else {
            return Lit::Resting;
        };
        let walk = flow.walk(&self.diagram);
        // A node the path visits twice belongs to the earlier visit, which is
        // what a reader watching it travel would expect: it lights when it is
        // first reached and stays lit.
        let Some(at) = walk.iter().position(|step| step.node() == id) else {
            return Lit::Aside;
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
        Lit::Aside
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
fn skin(weight: Weight, picked: bool, lit: Lit, palette: &Palette) -> Skin {
    let (fill, edge, width) = match (picked, weight) {
        (true, _) => (palette.focus.mix(palette.accent, 0.14), palette.accent, 1.5),
        (false, Weight::Accent) => (palette.focus, palette.accent, 1.5),
        (false, Weight::Normal) => (palette.bg, palette.edge, 1.),
        (false, Weight::Muted) => (palette.wash, palette.edge.mix(palette.wash, 0.5), 1.),
    };

    // A flow does not recolour the picture, it changes what stands out of it.
    // The nodes on the path keep their own skin and everything else recedes
    // toward the page — so a reader who has learned what a shape means does
    // not have to learn it again while a flow is playing.
    let (fill, edge, width) = match lit {
        Lit::Resting => (fill, edge, width),
        Lit::Aside => (
            fill.mix(palette.bg, ASIDE),
            edge.mix(palette.bg, ASIDE),
            width,
        ),
        // Mixed rather than switched. A box on the path is drawn between where
        // it was and where the current will leave it, so the light arrives
        // through it instead of landing on it.
        Lit::On(much, tint) => {
            let lit = tint.unwrap_or(palette.accent);
            let from = fill.mix(palette.bg, ASIDE);
            let was = edge.mix(palette.bg, ASIDE);
            (
                from.mix(fill.mix(lit, 0.16), much),
                was.mix(lit, much),
                width + (1.5_f32 - width).max(0.) * much,
            )
        }
    };

    Skin {
        fill: paint(fill),
        edge: paint(edge),
        width,
    }
}

/// How far a node that is not on the playing flow recedes toward the page.
///
/// Far enough to fall behind, near enough to still be read. A flow that hid
/// the rest of the diagram would answer the question by deleting the context
/// that makes it a question.
const ASIDE: f32 = 0.68;

/// How far a label has to stay inside a shape of this role.
fn inset(role: Role) -> f32 {
    match role {
        Role::Step | Role::Store => 10.,
        Role::Terminal => 18.,
        Role::Actor => 22.,
        Role::Decision => 30.,
    }
}

/// How much bigger than its cell a shape of this role is drawn.
///
/// Only the diamond, and it is not decoration. A rhombus is at its full width
/// on exactly one line and narrows to nothing at the points, so a label sized
/// to the cell hangs out over the slanted sides — which is what a branch node
/// looked like before this existed. It borrows from the gutter, and takes less
/// than half of it, so the arrows that travel down the middle of one still
/// have their lane.
fn swell(role: Role) -> (f32, f32) {
    match role {
        Role::Decision => (13., 7.),
        _ => (0., 0.),
    }
}

/// The box a role's shape is actually drawn in.
fn body(at: Bounds<Pixels>, role: Role) -> Bounds<Pixels> {
    let (x, y) = swell(role);
    Bounds {
        origin: point(at.origin.x - px(x), at.origin.y - px(y)),
        size: size(at.size.width + px(x * 2.), at.size.height + px(y * 2.)),
    }
}

/// `at`, moved to where the drawing actually landed on screen.
fn offset(at: Bounds<Pixels>, by: Point<Pixels>) -> Bounds<Pixels> {
    Bounds {
        origin: at.origin + by,
        size: at.size,
    }
}

/// Paint one node's body.
///
/// Role decides the outline, and only the outline. A diamond means a branch
/// everywhere diagrams are drawn, and throwing that away to keep the drawing
/// code simple would throw away information the reader already knows how to
/// read.
fn shape(window: &mut Window, at: Bounds<Pixels>, role: Role, skin: Skin) {
    let mut round = |radius: f32| {
        window.paint_quad(quad(
            at,
            gpui_kit::Corners::all(px(radius)),
            skin.fill,
            gpui_kit::Edges::all(px(skin.width)),
            skin.edge,
            BorderStyle::Solid,
        ));
    };

    match role {
        Role::Step => round(6.),
        Role::Store => cylinder(window, at, skin),
        Role::Terminal => round(f32::from(at.size.height) / 2.),
        // A path is filled, not stroked, so the border is a second, larger
        // shape painted underneath: the outline in the edge colour, then the
        // same outline inset by the border's own width in the fill.
        Role::Decision => {
            window.paint_path(diamond(at, 0.), skin.edge);
            window.paint_path(diamond(at, skin.width), skin.fill);
        }
        Role::Actor => {
            window.paint_path(ellipse(at, 0.), skin.edge);
            window.paint_path(ellipse(at, skin.width), skin.fill);
        }
    }
}

/// A diamond filling `at`, pulled in by `inset` on every side.
fn diamond(at: Bounds<Pixels>, inset: f32) -> Path<Pixels> {
    let (l, t) = (
        f32::from(at.origin.x) + inset,
        f32::from(at.origin.y) + inset,
    );
    let r = f32::from(at.origin.x + at.size.width) - inset;
    let b = f32::from(at.origin.y + at.size.height) - inset;
    let (cx, cy) = ((l + r) / 2., (t + b) / 2.);

    let mut path = Path::new(point(px(cx), px(t)));
    path.line_to(point(px(r), px(cy)));
    path.line_to(point(px(cx), px(b)));
    path.line_to(point(px(l), px(cy)));
    path
}

/// An ellipse filling `at`, pulled in by `inset` on every side.
///
/// Four quadratic curves with their controls at the corners. Not the exact
/// curve — the exact one needs cubics — but at this size the difference is
/// under a pixel, and what the shape has to say is "not a rectangle".
fn ellipse(at: Bounds<Pixels>, inset: f32) -> Path<Pixels> {
    let (l, t) = (
        f32::from(at.origin.x) + inset,
        f32::from(at.origin.y) + inset,
    );
    let r = f32::from(at.origin.x + at.size.width) - inset;
    let b = f32::from(at.origin.y + at.size.height) - inset;
    let (cx, cy) = ((l + r) / 2., (t + b) / 2.);
    let at = |x: f32, y: f32| point(px(x), px(y));

    let mut path = Path::new(at(l, cy));
    path.curve_to(at(cx, t), at(l, t));
    path.curve_to(at(r, cy), at(r, t));
    path.curve_to(at(cx, b), at(r, b));
    path.curve_to(at(l, cy), at(l, b));
    path
}

/// A store: a cylinder, drawn as a silhouette with a lid on it.
///
/// It was a rounded box with a rule across its head, and the rule read as a
/// stray bar rather than as a lid — the first thing anyone asked about it was
/// what it was. A rule is not a convention; a cylinder is, and a shape nobody
/// recognises is carrying no information at all.
///
/// The reason it was a rule is worth keeping: a path is filled, not stroked, so
/// a curved *line* has to be a closed sliver, and a sliver a pixel wide
/// tessellates into something visibly heavier and rougher than a hairline.
/// Filled curves are fine — the diamond and the actor are two of them. So the
/// cylinder is built out of fills only: the whole outline in the border colour,
/// the same shape inset by the border's width in the fill colour, and the lid
/// is a third pass of the cap that leaves a ring of the border showing.
fn cylinder(window: &mut Window, at: Bounds<Pixels>, skin: Skin) {
    /// How tall the caps are. Enough to read as an ellipse at this width
    /// without eating the room the label needs.
    const CAP: f32 = 13.;

    let (l, t) = (f32::from(at.origin.x), f32::from(at.origin.y));
    let w = f32::from(at.size.width);
    let h = f32::from(at.size.height);

    // The three pieces, each pulled in by `inset` on every side. The body is
    // the straight part between the two caps; the caps overhang it, which is
    // what makes the join look curved rather than cut.
    let silhouette = |window: &mut Window, inset: f32, colour: Hsla| {
        let cap = Bounds {
            origin: point(px(l + inset), px(t + inset)),
            size: size(px(w - inset * 2.), px(CAP)),
        };
        let foot = Bounds {
            origin: point(px(l + inset), px(t + h - CAP - inset)),
            size: size(px(w - inset * 2.), px(CAP)),
        };
        let body = Bounds {
            origin: point(px(l + inset), px(t + CAP / 2.)),
            size: size(px(w - inset * 2.), px(h - CAP)),
        };
        window.paint_path(ellipse(foot, 0.), colour);
        window.paint_quad(fill(body, colour));
        window.paint_path(ellipse(cap, 0.), colour);
    };

    silhouette(window, 0., skin.edge);
    silhouette(window, skin.width, skin.fill);

    // The lid: the cap outlined again, so the top reads as an opening rather
    // than as the flat end of a box.
    let cap = Bounds {
        origin: point(px(l + skin.width), px(t + skin.width)),
        size: size(px(w - skin.width * 2.), px(CAP)),
    };
    window.paint_path(ellipse(cap, 0.), skin.edge);
    window.paint_path(ellipse(cap, skin.width), skin.fill);
}

/// One straight run of an arrow.
///
/// Every run is horizontal or vertical by construction, so a segment is a thin
/// rectangle and needs no path.
fn stroke(window: &mut Window, from: Point<Pixels>, to: Point<Pixels>, colour: Hsla, dashed: bool) {
    let (x0, y0) = (f32::from(from.x), f32::from(from.y));
    let (x1, y1) = (f32::from(to.x), f32::from(to.y));
    let flat = (y1 - y0).abs() < (x1 - x0).abs();

    let span = |a: f32, b: f32| (a.min(b), (b - a).abs());
    let (start, length) = if flat { span(x0, x1) } else { span(y0, y1) };
    if length < 0.5 {
        return;
    }

    let mut bar = |at: f32, run: f32| {
        let bounds = if flat {
            Bounds {
                origin: point(px(at), px(y0 - STROKE / 2.)),
                size: size(px(run), px(STROKE)),
            }
        } else {
            Bounds {
                origin: point(px(x0 - STROKE / 2.), px(at)),
                size: size(px(STROKE), px(run)),
            }
        };
        window.paint_quad(fill(bounds, colour));
    };

    if !dashed {
        bar(start, length);
        return;
    }

    // Dashes drawn one at a time: the border style that would do this for free
    // belongs to a quad's outline, and this is a quad's body.
    const DASH: f32 = 5.;
    const GAP: f32 = 4.;
    let mut walked = 0.;
    while walked < length {
        bar(start + walked, DASH.min(length - walked));
        walked += DASH + GAP;
    }
}

/// The head of an arrow, pointing the way the last run was going.
fn arrowhead(window: &mut Window, from: Point<Pixels>, to: Point<Pixels>, colour: Hsla) {
    let (dx, dy) = (
        f32::from(to.x) - f32::from(from.x),
        f32::from(to.y) - f32::from(from.y),
    );
    let length = dx.hypot(dy);
    if length < 0.5 {
        return;
    }

    let (ux, uy) = (dx / length, dy / length);
    // The perpendicular, for the two corners at the back of the head.
    let (px_, py) = (-uy, ux);
    let (tip_x, tip_y) = (f32::from(to.x), f32::from(to.y));
    let (base_x, base_y) = (tip_x - ux * HEAD.0, tip_y - uy * HEAD.0);
    let wing = HEAD.1 / 2.;

    let mut path = Path::new(point(px(tip_x), px(tip_y)));
    path.line_to(point(px(base_x + px_ * wing), px(base_y + py * wing)));
    path.line_to(point(px(base_x - px_ * wing), px(base_y - py * wing)));
    window.paint_path(path, colour);
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this module glob-imports GPUI, which
    // exports a `test` attribute of its own and would otherwise shadow the
    // standard one.
    use core::prelude::v1::test;

    use deck_core::diagram::Node;

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

    fn drawn(
        flow: Direction,
        ids: &[&str],
        edges: &[(&str, &str)],
        clusters: &[(&str, &[&str])],
    ) -> Plan {
        let diagram = Diagram {
            title: None,
            flow,
            nodes: ids.iter().copied().map(node).collect(),
            edges: edges.iter().map(|(f, t)| edge(f, t)).collect(),
            clusters: clusters
                .iter()
                .map(|(label, nodes)| Cluster {
                    label: (*label).to_string(),
                    nodes: nodes.iter().map(|id| (*id).to_string()).collect(),
                })
                .collect(),
            flows: Vec::new(),
        };
        plan(&diagram, &diagram.layout())
    }

    /// The edges of a box, as `(left, top, right, bottom)`.
    fn sides(at: Bounds<Pixels>) -> (f32, f32, f32, f32) {
        (
            f32::from(at.origin.x),
            f32::from(at.origin.y),
            f32::from(at.origin.x + at.size.width),
            f32::from(at.origin.y + at.size.height),
        )
    }

    #[test]
    fn a_chain_is_drawn_down_one_column() {
        let plan = drawn(
            Direction::Down,
            &["a", "b", "c"],
            &[("a", "b"), ("b", "c")],
            &[],
        );

        let xs: Vec<f32> = plan.spots.iter().map(|s| sides(s.at).0).collect();
        assert_eq!(xs, vec![PAD, PAD, PAD], "one rank each, so one column");

        let ys: Vec<f32> = plan.spots.iter().map(|s| sides(s.at).1).collect();
        assert!(ys[0] < ys[1] && ys[1] < ys[2], "later things sit lower");
    }

    #[test]
    fn an_arrow_travels_down_the_gutter_and_never_over_a_box() {
        // Two nodes at the same rank and a third under both, so the arrow into
        // it has to move sideways. Where it does that is the whole question:
        // in the gap between the rows it is free, and anywhere else it would
        // cross something.
        let plan = drawn(
            Direction::Down,
            &["a", "b", "c"],
            &[("a", "c"), ("b", "c")],
            &[],
        );

        let a = sides(plan.spots[0].at);
        let c = sides(plan.spots[2].at);
        let route = &plan.routes[0];

        let lane = f32::from(route.points[1].y);
        assert!(
            lane > a.3 && lane < c.1,
            "the sideways run sits between the two rows, not on either of them"
        );
        assert!(
            !route.back,
            "an edge that runs with the flow is not a back edge"
        );
    }

    #[test]
    fn a_returning_arrow_steps_into_the_next_gutter_over() {
        // A cycle: one of these two edges cannot be ranked, and the one set
        // aside has to get back up the drawing without running through the
        // boxes it is passing.
        let plan = drawn(Direction::Down, &["a", "b"], &[("a", "b"), ("b", "a")], &[]);

        let back = plan
            .routes
            .iter()
            .find(|route| route.back)
            .expect("a cycle leaves one edge running against the flow");
        let from = sides(plan.spots[1].at);

        let lane = f32::from(back.points[1].x);
        assert!(
            lane > from.2 && lane < from.2 + GAP_X,
            "it climbs the gutter beside the column rather than over it"
        );
    }

    #[test]
    fn a_cluster_frames_its_members_with_room_for_its_name() {
        let plan = drawn(
            Direction::Down,
            &["a", "b"],
            &[("a", "b")],
            &[("core", &["a", "b"])],
        );

        let frame = sides(plan.frames[0].at);
        let a = sides(plan.spots[0].at);
        let b = sides(plan.spots[1].at);

        assert!(frame.0 < a.0 && frame.2 > a.2, "it holds the first box");
        assert!(frame.3 > b.3, "and the last one");
        assert!(
            a.1 - frame.1 > CLUSTER_PAD,
            "with more room above the boxes than beside them, for the name"
        );
    }

    #[test]
    fn the_drawing_is_big_enough_for_what_is_in_it() {
        let plan = drawn(
            Direction::Right,
            &["a", "b"],
            &[("a", "b")],
            &[("core", &["a", "b"])],
        );

        let frame = sides(plan.frames[0].at);
        assert!(f32::from(plan.size.width) > frame.2);
        assert!(f32::from(plan.size.height) > frame.3);
    }
}
