//! A placed diagram, in points: where every box, arrow and frame goes.
//!
//! [`crate::diagram`] places nodes on a grid of columns and rows; this turns
//! the grid into a drawing. It used to live in the window, typed in the
//! window's own pixels, which left it the one part of a picture nothing else
//! could draw: an export would have had to work out the geometry again, and a
//! second answer to *where does this arrow go* is a picture that differs from
//! the one the reader saw. So it is here, in plain numbers, for the window and
//! for the exports to come (#170) to ask the same function.
//!
//! # Why every box is the same size
//!
//! Nothing here measures a label and grows a box to fit it, which is what a
//! general diagram tool does — and is why two drawings of the same idea from
//! the same tool rarely look alike. A deck is read as a whole, so its pictures
//! have to resemble each other more than they resemble their contents. The
//! format keeps labels short for exactly this reason, and the grid is the thing
//! that makes an agent's drawing look like the deck rather than like the agent.
//! It is also why this module needs no font: no size here depends on a word.

use crate::diagram::{Cluster, Diagram, Direction, Layout};

/// How wide a box is.
pub const NODE_W: f32 = 156.;
/// How tall a box is.
pub const NODE_H: f32 = 54.;
/// The gutter between columns. Every arrow that has to travel sideways travels
/// down the middle of one of these, which is what keeps a route off the boxes.
///
/// Wide enough to hold a short edge label, because that is where a label goes.
pub const GAP_X: f32 = 56.;
/// The gutter between two rows.
pub const GAP_Y: f32 = 42.;
/// The margin around the whole drawing. At least half a gutter, so an arrow
/// leaving the outermost column still has somewhere to turn.
pub const PAD: f32 = 30.;
/// How far a cluster's frame stands off the boxes inside it.
pub const CLUSTER_PAD: f32 = 15.;
/// The room a cluster's frame keeps above its boxes for its name.
pub const CLUSTER_HEAD: f32 = 19.;

/// A point in a drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    /// Across, from the drawing's left edge.
    pub x: f32,
    /// Down, from its top edge.
    pub y: f32,
}

/// A rectangle in a drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Its left edge.
    pub x: f32,
    /// Its top edge.
    pub y: f32,
    /// How wide it is.
    pub w: f32,
    /// How tall it is.
    pub h: f32,
}

impl Rect {
    /// Its right edge.
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// Its bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// Where one node ended up.
#[derive(Debug, Clone, PartialEq)]
pub struct Spot {
    /// Index into [`Diagram::nodes`].
    pub node_ix: usize,
    /// The box it fills.
    pub at: Rect,
}

/// One arrow, as the corners it turns.
///
/// Always four points: out of the source, into a gutter, along it, and into the
/// target. Two of them coincide when the nodes line up, which draws as a
/// straight arrow without any special case for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// Index into [`Diagram::edges`].
    pub edge_ix: usize,
    /// The corners, from the source's edge to the target's.
    pub points: Vec<Pt>,
    /// Whether this edge runs against the flow. See [`Layout::back_edges`].
    pub back: bool,
}

/// Where one cluster's frame goes.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Index into [`Diagram::clusters`].
    pub cluster_ix: usize,
    /// The box it draws around its members.
    pub at: Rect,
}

/// The whole drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// How wide the drawing is. Wider than the pane, usually — it scrolls.
    pub width: f32,
    /// How tall it is.
    pub height: f32,
    /// Every node's box.
    pub spots: Vec<Spot>,
    /// Every arrow's corners.
    pub routes: Vec<Route>,
    /// Every cluster's frame.
    pub frames: Vec<Frame>,
}

/// Turn a placed grid into a drawing.
#[must_use]
pub fn plan(diagram: &Diagram, layout: &Layout) -> Plan {
    let spots: Vec<Spot> = layout
        .placed
        .iter()
        .map(|placed| Spot {
            node_ix: placed.node_ix,
            at: Rect {
                x: PAD + placed.col as f32 * (NODE_W + GAP_X),
                y: PAD + placed.row as f32 * (NODE_H + GAP_Y),
                w: NODE_W,
                h: NODE_H,
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
    let mut stretch = |at: Rect| {
        width = width.max(at.right());
        height = height.max(at.bottom());
    };
    for spot in &spots {
        stretch(spot.at);
    }
    for frame in &frames {
        stretch(frame.at);
    }

    Plan {
        width: width + PAD,
        height: height + PAD,
        spots,
        routes,
        frames,
    }
}

/// The frame around a cluster's members, or `None` if it names none that exist.
fn around(
    cluster: &Cluster,
    index: &impl Fn(&str) -> Option<usize>,
    box_of: &impl Fn(usize) -> Option<Rect>,
) -> Option<Rect> {
    let boxes: Vec<Rect> = cluster
        .nodes
        .iter()
        .filter_map(|id| index(id).and_then(box_of))
        .collect();
    let first = *boxes.first()?;

    let mut left = first.x;
    let mut top = first.y;
    let mut right = left;
    let mut bottom = top;
    for at in boxes {
        left = left.min(at.x);
        top = top.min(at.y);
        right = right.max(at.right());
        bottom = bottom.max(at.bottom());
    }

    Some(Rect {
        x: left - CLUSTER_PAD,
        y: top - CLUSTER_PAD - CLUSTER_HEAD,
        w: right - left + CLUSTER_PAD * 2.,
        h: bottom - top + CLUSTER_PAD * 2. + CLUSTER_HEAD,
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
fn route(from: Rect, to: Rect, flow: Direction, back: bool) -> Vec<Pt> {
    let side = |at: Rect| (at.x, at.y, at.right(), at.bottom());
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

    corners.into_iter().map(|(x, y)| Pt { x, y }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::{Edge, Line, Node, Role, Weight};

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

    fn diagram(
        flow: Direction,
        ids: &[&str],
        edges: &[(&str, &str)],
        clusters: &[(&str, &[&str])],
    ) -> Diagram {
        Diagram {
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
        }
    }

    fn drawn(
        flow: Direction,
        ids: &[&str],
        edges: &[(&str, &str)],
        clusters: &[(&str, &[&str])],
    ) -> Plan {
        let diagram = diagram(flow, ids, edges, clusters);
        plan(&diagram, &diagram.layout())
    }

    /// The edges of a box, as `(left, top, right, bottom)`.
    fn sides(at: Rect) -> (f32, f32, f32, f32) {
        (at.x, at.y, at.right(), at.bottom())
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

        let lane = route.points[1].y;
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

        let lane = back.points[1].x;
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
        assert!(plan.width > frame.2);
        assert!(plan.height > frame.3);
    }

    /// Every number a plan holds, one line each, so a change to the geometry
    /// shows in a diff as exactly what moved.
    fn dump(name: &str, plan: &Plan) -> String {
        let rect = |at: Rect| format!("{} {} {} {}", at.x, at.y, at.w, at.h);
        let mut out = format!("# {name}\nsize {} {}\n", plan.width, plan.height);
        for spot in &plan.spots {
            out += &format!("node {} {}\n", spot.node_ix, rect(spot.at));
        }
        for route in &plan.routes {
            let points: Vec<String> = route
                .points
                .iter()
                .map(|at| format!("{},{}", at.x, at.y))
                .collect();
            out += &format!(
                "edge {}{} {}\n",
                route.edge_ix,
                if route.back { " back" } else { "" },
                points.join(" ")
            );
        }
        for frame in &plan.frames {
            out += &format!("frame {} {}\n", frame.cluster_ix, rect(frame.at));
        }
        out
    }

    #[test]
    fn the_geometry_is_what_it_was() {
        // The window and every export draw from this, so any change to it is
        // a change to every picture deck has ever shown. Held as a plain dump
        // of every number; `UPDATE_GOLDEN=1` writes it again on purpose.
        let fixtures = [
            (
                "a chain, down",
                diagram(
                    Direction::Down,
                    &["a", "b", "c"],
                    &[("a", "b"), ("b", "c")],
                    &[],
                ),
            ),
            (
                "a chain, right",
                diagram(
                    Direction::Right,
                    &["a", "b", "c"],
                    &[("a", "b"), ("b", "c")],
                    &[],
                ),
            ),
            (
                "a fan in",
                diagram(
                    Direction::Down,
                    &["a", "b", "c"],
                    &[("a", "c"), ("b", "c")],
                    &[],
                ),
            ),
            (
                "a cycle, down",
                diagram(Direction::Down, &["a", "b"], &[("a", "b"), ("b", "a")], &[]),
            ),
            (
                "a cycle, right",
                diagram(
                    Direction::Right,
                    &["a", "b", "c"],
                    &[("a", "b"), ("b", "c"), ("c", "a")],
                    &[],
                ),
            ),
            (
                "a cluster naming a node that is not there",
                diagram(
                    Direction::Down,
                    &["a", "b"],
                    &[("a", "b")],
                    &[("core", &["a", "ghost"]), ("empty", &["ghost"])],
                ),
            ),
            (
                "a back edge across columns",
                diagram(
                    Direction::Right,
                    &["a", "b", "c", "d"],
                    &[("a", "b"), ("a", "c"), ("c", "d"), ("d", "b"), ("b", "a")],
                    &[],
                ),
            ),
            (
                "clusters and a long edge",
                diagram(
                    Direction::Down,
                    &["click", "handler", "check", "api", "store"],
                    &[
                        ("click", "handler"),
                        ("handler", "check"),
                        ("check", "api"),
                        ("api", "store"),
                        ("click", "store"),
                    ],
                    &[
                        ("browser", &["click", "handler", "check"]),
                        ("server", &["api", "store"]),
                    ],
                ),
            ),
        ];
        let now: String = fixtures
            .iter()
            .map(|(name, diagram)| dump(name, &plan(diagram, &diagram.layout())))
            .collect::<Vec<_>>()
            .join("\n");
        let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/geometry.txt");
        if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
            std::fs::create_dir_all(at.parent().expect("a parent")).expect("writable");
            std::fs::write(&at, &now).expect("writable");
        }
        let was =
            std::fs::read_to_string(&at).expect("the golden file exists; run with UPDATE_GOLDEN=1");
        // Line endings are not geometry: a checkout may have made them CRLF.
        let was = was.replace("\r\n", "\n");
        assert_eq!(
            now, was,
            "the geometry moved; if that was meant, run with UPDATE_GOLDEN=1"
        );
    }
}
