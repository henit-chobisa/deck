//! Pictures of things that are not in any one file.
//!
//! A code ref can only point at code that exists. Plenty of what needs
//! explaining does not: how two crates depend on each other, what happens
//! between a header landing and a review being written, why one shape becomes
//! another. That is structure *between* files, and prose is a poor way to carry
//! it.
//!
//! So a group may point at a diagram the way it points at a file, and the
//! diagram gets a pane.
//!
//! # Why a format instead of a drawing
//!
//! An agent handed a blank canvas draws three rounded boxes and an arrow that
//! restates the sentence above it. The cure is not asking it to try harder, it
//! is giving it a vocabulary in which decoration is not expressible: nodes,
//! edges, and where things sit. Everything visual — colour, spacing, corners,
//! type — comes from the palette, so every diagram belongs to the same deck.
//!
//! # What it can draw
//!
//! One shape, bent by two knobs, covers the kinds that come up:
//!
//! - **Data flow** — nodes and edges, `flow: right`.
//! - **Flowchart** — a [`Role::Decision`] node, with a labelled edge each way out.
//! - **Layers** — `flow: down`, with a [`Cluster`] per layer.
//! - **Sequence** — `flow: down`, each participant given a [`Node::lane`], so
//!   time runs down and lanes stay put.
//! - **State machine** — a graph with cycles; see [`Layout::back_edges`].
//! - **Before and after** — two clusters, no edges between them.
//!
//! # What this module does and does not do
//!
//! It places nodes on a grid of columns and rows. It does not know what a pixel
//! is, how wide a label renders, or where an arrow's head goes — box sizes
//! depend on text metrics, which belong to whatever is drawing. The view takes
//! these cells and does the geometry.

use crate::theme::Rgb;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Which way the story runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Direction {
    /// Later things sit below earlier ones. The default: it suits layers,
    /// sequences and anything read top to bottom.
    #[default]
    Down,
    /// Later things sit to the right. Suits a pipeline.
    Right,
}

/// Read from the wire, forgivingly.
///
/// Anything this version does not know reads as the default rather than
/// failing the whole group. A later version may name a shape, a weight or a
/// line this one has never heard of, and a diagram that mostly makes sense is
/// worth more than a group that will not open.
impl From<String> for Direction {
    fn from(name: String) -> Self {
        match name.as_str() {
            "down" => Self::Down,
            "right" => Self::Right,
            _ => Self::Down,
        }
    }
}

/// How much a node wants to be looked at.
///
/// Emphasis, not meaning: the palette decides what each one looks like, so a
/// diagram cannot invent a colour that belongs to nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Weight {
    /// An ordinary node.
    #[default]
    Normal,
    /// The one the group is actually about.
    Accent,
    /// Present for context; not the point.
    Muted,
}

/// Read from the wire, forgivingly.
///
/// Anything this version does not know reads as the default rather than
/// failing the whole group. A later version may name a shape, a weight or a
/// line this one has never heard of, and a diagram that mostly makes sense is
/// worth more than a group that will not open.
impl From<String> for Weight {
    fn from(name: String) -> Self {
        match name.as_str() {
            "normal" => Self::Normal,
            "accent" => Self::Accent,
            "muted" => Self::Muted,
            _ => Self::Normal,
        }
    }
}

/// What kind of thing a node is.
///
/// Shape carries meaning in every diagram convention worth borrowing from — a
/// diamond is a branch, a cylinder is something stored. Losing that loses
/// information, so it is here.
///
/// What is *not* here is a free choice of shape. An agent naming a triangle
/// because the last three boxes were rectangles is decorating, and two decks
/// drawing the same idea differently is worse than either drawing alone. So a
/// node says what it *is* and the view owns the geometry, the same way
/// [`Weight`] says what matters and the palette owns the colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Role {
    /// Something that happens, or something that is. Drawn as a box.
    #[default]
    Step,
    /// A branch. Drawn as a diamond, and worth an edge label on each way out.
    Decision,
    /// Something written down and read back: a file, a queue, a table. Drawn as
    /// a cylinder.
    Store,
    /// Where the story starts or stops. Drawn as a pill.
    Terminal,
    /// A person, or a system outside the picture. Drawn as a circle.
    Actor,
}

/// Read from the wire, forgivingly.
///
/// Anything this version does not know reads as the default rather than
/// failing the whole group. A later version may name a shape, a weight or a
/// line this one has never heard of, and a diagram that mostly makes sense is
/// worth more than a group that will not open.
impl From<String> for Role {
    fn from(name: String) -> Self {
        match name.as_str() {
            "step" => Self::Step,
            "decision" => Self::Decision,
            "store" => Self::Store,
            "terminal" => Self::Terminal,
            "actor" => Self::Actor,
            _ => Self::Step,
        }
    }
}

/// What an edge is claiming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Line {
    /// A real dependency, call, or step.
    #[default]
    Solid,
    /// Conditional, optional, or "only sometimes".
    Dashed,
}

/// Read from the wire, forgivingly.
///
/// Anything this version does not know reads as the default rather than
/// failing the whole group. A later version may name a shape, a weight or a
/// line this one has never heard of, and a diagram that mostly makes sense is
/// worth more than a group that will not open.
impl From<String> for Line {
    fn from(name: String) -> Self {
        match name.as_str() {
            "solid" => Self::Solid,
            "dashed" => Self::Dashed,
            _ => Self::Solid,
        }
    }
}

/// One box.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Referred to by edges and clusters.
    pub id: String,
    /// The name in the box. Short — a type, a crate, a step.
    pub label: String,
    /// A second line, smaller. Where the "why" goes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// What kind of thing it is, which decides its shape.
    #[serde(default)]
    pub role: Role,
    /// How much to emphasise it.
    #[serde(default)]
    pub weight: Weight,
    /// Pin the node to a fixed track across the flow.
    ///
    /// Ordinarily nodes are packed across as they are declared. A lane holds
    /// one still — which is what makes a sequence diagram a sequence diagram,
    /// each participant keeping its own column while time runs down.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane: Option<u32>,
}

/// One arrow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// The node it leaves, by id.
    pub from: String,
    /// The node it enters, by id.
    pub to: String,
    /// What the arrow means. `anchor()`, `writes`, `on submit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// How to draw it.
    #[serde(default)]
    pub line: Line,
}

/// A box drawn around several nodes.
///
/// Containment, not layout: the nodes inside are still ranked by their edges.
/// A cluster says "these belong together" — one crate, one layer, one side of a
/// comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// The name on the surrounding box.
    pub label: String,
    /// Which nodes it holds, by id.
    pub nodes: Vec<String>,
}

/// A path through the picture, walked one node at a time.
///
/// A diagram of any size answers several questions at once, and a reader
/// looking for one of them has to find it among the rest. A flow is the agent
/// saying *this* is the part I mean: the nodes it touches, in the order they
/// happen, so the picture can be walked rather than only read.
///
/// The edge between two consecutive steps lights with the second of them. That
/// is what makes it read as travel rather than as boxes taking turns to blink,
/// and it is why the steps are nodes and not edges — an agent describing a path
/// names the places, and the arrows between them follow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flow {
    /// What this path is called. Shown on the button that plays it.
    pub name: String,
    /// The nodes it touches, in order. Ids that name no node are skipped.
    pub steps: Vec<Step>,
    /// The colour this path is lit in.
    ///
    /// Two flows over one picture are two answers, and telling them apart by
    /// which button is pressed means holding the last one in your head. Given
    /// a colour each, they can be compared. Left out, a flow uses the deck's
    /// accent, which is the right answer for a diagram with only one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgb>,
}

/// One stop on a path.
///
/// Written as a plain id, or as an object when that one node wants a colour of
/// its own — the step where it goes wrong, in a flow that is otherwise calm.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Step {
    /// Just the node.
    Node(String),
    /// The node, and how to light it.
    Lit {
        /// Which node.
        node: String,
        /// The colour for this step alone, over the flow's own.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Rgb>,
    },
}

impl Step {
    /// Which node this step is.
    #[must_use]
    pub fn node(&self) -> &str {
        match self {
            Self::Node(id) => id,
            Self::Lit { node, .. } => node,
        }
    }

    /// The colour asked for, if this step asked for one.
    #[must_use]
    pub fn color(&self) -> Option<Rgb> {
        match self {
            Self::Node(_) => None,
            Self::Lit { color, .. } => *color,
        }
    }
}

/// A picture of how some things relate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagram {
    /// A caption, shown where a code pane shows `file:range`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Which way the story runs.
    #[serde(default)]
    pub flow: Direction,
    /// The boxes, in the order they should be packed across the flow.
    pub nodes: Vec<Node>,
    /// The arrows.
    #[serde(default)]
    pub edges: Vec<Edge>,
    /// Boxes drawn around groups of nodes.
    #[serde(default)]
    pub clusters: Vec<Cluster>,
    /// Paths through the picture that a reader can play.
    ///
    /// Optional, and a diagram with none is the ordinary case — a picture that
    /// says one thing does not need a path through it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flows: Vec<Flow>,
}

impl Flow {
    /// The steps that name a node that exists, in order.
    ///
    /// An id that matches nothing is dropped rather than refused, the same way
    /// an edge naming a missing node is: a flow written against a diagram that
    /// has since lost a box should play the part it still can.
    #[must_use]
    pub fn walk<'a>(&'a self, diagram: &'a Diagram) -> Vec<&'a Step> {
        self.steps
            .iter()
            .filter(|step| diagram.nodes.iter().any(|node| node.id == step.node()))
            .collect()
    }
}

/// Where one node ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    /// Index into [`Diagram::nodes`].
    pub node_ix: usize,
    /// Column, from 0.
    pub col: u32,
    /// Row, from 0.
    pub row: u32,
}

/// Where everything ended up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// One entry per node, in declaration order.
    pub placed: Vec<Placed>,
    /// Columns used, at least one.
    pub cols: u32,
    /// Rows used, at least one.
    pub rows: u32,
    /// Indices into [`Diagram::edges`] for edges that run backwards against the
    /// flow.
    ///
    /// A cycle cannot be ranked, so one edge of it is set aside to make the
    /// graph acyclic and reported here. A state machine is mostly these, and a
    /// view should draw them differently — an arrow that visibly returns,
    /// rather than one that appears to point the wrong way by mistake.
    pub back_edges: Vec<usize>,
}

impl Diagram {
    /// Place every node on a grid.
    ///
    /// Rank comes from the edges: a node sits one step past the furthest thing
    /// pointing at it, so an arrow always runs with the flow. Nodes sharing a
    /// rank are packed across it in declaration order, except where a
    /// [`Node::lane`] pins one.
    #[must_use]
    pub fn layout(&self) -> Layout {
        let index = self.index();
        let back_edges = self.back_edges(&index);
        let rank = self.rank(&index, &back_edges);
        let track = self.pack(&rank);

        let placed: Vec<Placed> = (0..self.nodes.len())
            .map(|ix| {
                let (rank, track) = (rank[ix], track[ix]);
                let (col, row) = match self.flow {
                    Direction::Down => (track, rank),
                    Direction::Right => (rank, track),
                };
                Placed {
                    node_ix: ix,
                    col,
                    row,
                }
            })
            .collect();

        Layout {
            cols: placed.iter().map(|p| p.col).max().unwrap_or(0) + 1,
            rows: placed.iter().map(|p| p.row).max().unwrap_or(0) + 1,
            placed,
            back_edges,
        }
    }

    /// Node id to its position in [`Diagram::nodes`].
    fn index(&self) -> HashMap<&str, usize> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(ix, node)| (node.id.as_str(), ix))
            .collect()
    }

    /// The edges that have to be set aside for the graph to be rankable.
    ///
    /// A depth-first walk in declaration order: an edge reaching a node still
    /// on the current path closes a loop, and that edge is the one dropped.
    /// Which edge of a cycle gets chosen depends on declaration order, so an
    /// agent that writes a state machine's steps in order gets its return arrow
    /// picked, which is the one a reader expects to see bending back.
    fn back_edges(&self, index: &HashMap<&str, usize>) -> Vec<usize> {
        let mut out = Vec::new();
        let mut done = vec![false; self.nodes.len()];
        let mut on_path = vec![false; self.nodes.len()];

        // An explicit stack rather than recursion: a diagram is agent-written,
        // and a long chain must not be able to blow the real one.
        for start in 0..self.nodes.len() {
            if done[start] {
                continue;
            }
            let mut stack = vec![(start, 0usize)];
            on_path[start] = true;

            while let Some((node, seen)) = stack.pop() {
                let next = self
                    .edges
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| index.get(e.from.as_str()) == Some(&node))
                    .nth(seen);

                match next {
                    Some((edge_ix, edge)) => {
                        stack.push((node, seen + 1));
                        let Some(&to) = index.get(edge.to.as_str()) else {
                            continue; // an edge naming a node that does not exist
                        };
                        if on_path[to] {
                            out.push(edge_ix);
                        } else if !done[to] {
                            on_path[to] = true;
                            stack.push((to, 0));
                        }
                    }
                    None => {
                        on_path[node] = false;
                        done[node] = true;
                    }
                }
            }
        }

        out.sort_unstable();
        out
    }

    /// How far along the flow each node sits.
    ///
    /// One past the furthest thing that points at it, so no arrow ever has to
    /// run backwards except the ones already set aside.
    fn rank(&self, index: &HashMap<&str, usize>, back_edges: &[usize]) -> Vec<u32> {
        let skip: HashSet<usize> = back_edges.iter().copied().collect();
        let mut rank = vec![0u32; self.nodes.len()];

        // Relaxing once per node is enough for the longest path in a DAG, and
        // it terminates on any input — which matters more here than speed,
        // because the input is written by a model.
        for _ in 0..self.nodes.len() {
            let mut moved = false;
            for (edge_ix, edge) in self.edges.iter().enumerate() {
                if skip.contains(&edge_ix) {
                    continue;
                }
                let (Some(&from), Some(&to)) =
                    (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
                else {
                    continue;
                };
                if rank[to] < rank[from] + 1 {
                    rank[to] = rank[from] + 1;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }

        rank
    }

    /// Where each node sits across its rank.
    ///
    /// Nodes are packed in declaration order, so an agent controls the
    /// left-to-right reading by the order it writes them. A node with a lane
    /// takes that track and holds it; the others fill in around it.
    fn pack(&self, rank: &[u32]) -> Vec<u32> {
        let mut track = vec![0u32; self.nodes.len()];
        let mut taken: HashMap<u32, HashSet<u32>> = HashMap::new();

        for (ix, node) in self.nodes.iter().enumerate() {
            if let Some(lane) = node.lane {
                track[ix] = lane;
                taken.entry(rank[ix]).or_default().insert(lane);
            }
        }

        for (ix, node) in self.nodes.iter().enumerate() {
            if node.lane.is_some() {
                continue;
            }
            let used = taken.entry(rank[ix]).or_default();
            let mut lane = 0;
            while used.contains(&lane) {
                lane += 1;
            }
            used.insert(lane);
            track[ix] = lane;
        }

        track
    }
}

#[cfg(test)]
mod tests {
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

    fn diagram(flow: Direction, nodes: &[&str], edges: &[(&str, &str)]) -> Diagram {
        Diagram {
            title: None,
            flow,
            nodes: nodes.iter().map(|id| node(id)).collect(),
            edges: edges.iter().map(|(f, t)| edge(f, t)).collect(),
            clusters: Vec::new(),
            flows: Vec::new(),
        }
    }

    /// Where the node called `id` ended up.
    fn at(d: &Diagram, layout: &Layout, id: &str) -> (u32, u32) {
        let ix = d.nodes.iter().position(|n| n.id == id).unwrap();
        let p = layout.placed.iter().find(|p| p.node_ix == ix).unwrap();
        (p.col, p.row)
    }

    #[test]
    fn the_last_node_of_a_path_lights() {
        // The front runs to `steps`, not `steps - 1`. A node at index i is
        // full once the front reaches i + 1, so a front that stopped one short
        // left the last node of every path dark — the one the whole path was
        // walked to arrive at.
        let steps = 4;
        let front = steps as f32;
        let last = (front - (steps - 1) as f32).clamp(0., 1.);
        assert_eq!(last, 1., "the node the path was walked to reach");
    }

    #[test]
    fn a_step_can_be_a_bare_id_or_a_colour_of_its_own() {
        // The plain form has to keep working, because it is what almost every
        // flow is, and the object form is for the one step that goes wrong in
        // a path that is otherwise calm.
        let flow: Flow = serde_json::from_str(
            r##"{ "name": "p", "steps": ["a", { "node": "b", "color": "#c33" }] }"##,
        )
        .expect("both forms parse");

        assert_eq!(flow.steps[0].node(), "a");
        assert_eq!(flow.steps[0].color(), None);
        assert_eq!(flow.steps[1].node(), "b");
        assert_eq!(flow.steps[1].color(), Rgb::from_hex("#c33"));
        assert_eq!(flow.color, None, "and a flow need not have one either");
    }

    #[test]
    fn a_flow_walks_only_the_nodes_that_exist() {
        // A flow written against a diagram that has since lost a box should
        // play the part it still can, the same way an edge naming a missing
        // node is dropped rather than refused.
        let mut d = diagram(Direction::Down, &["a", "b"], &[("a", "b")]);
        d.flows = vec![Flow {
            name: "the path".to_string(),
            steps: vec![
                Step::Node("a".into()),
                Step::Node("gone".into()),
                Step::Node("b".into()),
            ],
            color: None,
        }];

        let walked: Vec<&str> = d.flows[0].walk(&d).iter().map(|s| s.node()).collect();
        assert_eq!(walked, vec!["a", "b"]);
    }

    #[test]
    fn a_diagram_with_no_flows_reads_and_writes_without_them() {
        // Every diagram written before flows existed has to keep working, and
        // one with none must not grow an empty array on the way out.
        let plain: Diagram =
            serde_json::from_str(r#"{ "nodes": [ { "id": "a", "label": "A" } ] }"#)
                .expect("a diagram with no flows is a diagram");
        assert!(plain.flows.is_empty());

        let back = serde_json::to_string(&plain).expect("it serialises");
        assert!(!back.contains("flows"), "and says nothing about them");
    }

    #[test]
    fn a_shape_this_version_does_not_know_is_drawn_as_a_step() {
        // A later protocol may name a shape this one has never heard of. A
        // diagram that mostly makes sense beats a group that will not open.
        let node: Node = serde_json::from_str(
            r#"{ "id": "a", "label": "a", "role": "hexagon", "weight": "loud" }"#,
        )
        .expect("an unknown role is not a parse failure");

        assert_eq!(node.role, Role::Step);
        assert_eq!(node.weight, Weight::Normal);
    }

    #[test]
    fn a_chain_runs_the_way_the_flow_points() {
        let d = diagram(
            Direction::Right,
            &["a", "b", "c"],
            &[("a", "b"), ("b", "c")],
        );
        let l = d.layout();

        assert_eq!(at(&d, &l, "a"), (0, 0));
        assert_eq!(at(&d, &l, "b"), (1, 0));
        assert_eq!(at(&d, &l, "c"), (2, 0));
        assert_eq!((l.cols, l.rows), (3, 1));
    }

    #[test]
    fn the_same_chain_turned_down_swaps_the_axes() {
        let d = diagram(Direction::Down, &["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let l = d.layout();

        assert_eq!(at(&d, &l, "c"), (0, 2));
        assert_eq!((l.cols, l.rows), (1, 3));
    }

    #[test]
    fn things_at_the_same_stage_sit_side_by_side_in_the_order_written() {
        // One source feeding three readers: an HLD fan-out.
        let d = diagram(
            Direction::Down,
            &["core", "ui", "cli", "web"],
            &[("core", "ui"), ("core", "cli"), ("core", "web")],
        );
        let l = d.layout();

        assert_eq!(at(&d, &l, "core"), (0, 0));
        assert_eq!(at(&d, &l, "ui"), (0, 1));
        assert_eq!(at(&d, &l, "cli"), (1, 1));
        assert_eq!(at(&d, &l, "web"), (2, 1));
    }

    #[test]
    fn a_node_sits_past_the_furthest_thing_pointing_at_it() {
        // `c` is reachable in one step and in two. The long way wins, so the
        // short edge is drawn spanning a rank rather than pointing backwards.
        let d = diagram(
            Direction::Right,
            &["a", "b", "c"],
            &[("a", "b"), ("b", "c"), ("a", "c")],
        );
        let l = d.layout();

        assert_eq!(at(&d, &l, "c").0, 2);
        assert!(l.back_edges.is_empty());
    }

    #[test]
    fn a_cycle_is_ranked_by_setting_one_edge_aside() {
        // A state machine: idle -> running -> done -> idle.
        let d = diagram(
            Direction::Right,
            &["idle", "running", "done"],
            &[("idle", "running"), ("running", "done"), ("done", "idle")],
        );
        let l = d.layout();

        assert_eq!(l.back_edges, vec![2], "the returning edge is the one bent");
        assert_eq!(at(&d, &l, "idle").0, 0);
        assert_eq!(at(&d, &l, "done").0, 2);
    }

    #[test]
    fn a_lane_holds_its_track_while_the_flow_runs_past_it() {
        // A sequence diagram: two participants, time running down. Every step
        // keeps its column even though the steps alternate.
        let mut d = diagram(
            Direction::Down,
            &["ask", "answer", "again"],
            &[("ask", "answer"), ("answer", "again")],
        );
        d.nodes[0].lane = Some(0);
        d.nodes[1].lane = Some(1);
        d.nodes[2].lane = Some(0);
        let l = d.layout();

        assert_eq!(at(&d, &l, "ask"), (0, 0));
        assert_eq!(at(&d, &l, "answer"), (1, 1));
        assert_eq!(at(&d, &l, "again"), (0, 2));
    }

    #[test]
    fn an_unlaned_node_fills_in_around_a_laned_one() {
        let mut d = diagram(Direction::Down, &["pinned", "loose"], &[]);
        d.nodes[0].lane = Some(0);
        let l = d.layout();

        assert_eq!(at(&d, &l, "pinned"), (0, 0));
        assert_eq!(at(&d, &l, "loose"), (1, 0), "must not land on the lane");
    }

    #[test]
    fn two_unconnected_halves_sit_beside_each_other() {
        // Before and after: no edge crosses, so both start at the same rank.
        let d = diagram(
            Direction::Down,
            &["was", "was_more", "now"],
            &[("was", "was_more")],
        );
        let l = d.layout();

        assert_eq!(at(&d, &l, "was"), (0, 0));
        assert_eq!(at(&d, &l, "now"), (1, 0));
        assert_eq!(at(&d, &l, "was_more"), (0, 1));
    }

    #[test]
    fn an_edge_naming_a_node_that_is_not_there_is_ignored() {
        let d = diagram(Direction::Right, &["a"], &[("a", "ghost"), ("ghost", "a")]);
        let l = d.layout();

        assert_eq!(at(&d, &l, "a"), (0, 0));
        assert_eq!((l.cols, l.rows), (1, 1));
    }

    #[test]
    fn a_node_pointing_at_itself_does_not_hang() {
        let d = diagram(Direction::Right, &["a", "b"], &[("a", "a"), ("a", "b")]);
        let l = d.layout();

        assert_eq!(l.back_edges, vec![0]);
        assert_eq!(at(&d, &l, "b").0, 1);
    }

    #[test]
    fn a_branch_puts_both_answers_at_the_same_stage() {
        // A flowchart: the decision and one labelled edge per way out. Both
        // outcomes are one step past it, so they sit side by side and the
        // reader compares them rather than reading one as following the other.
        let json = r#"{
            "flow": "down",
            "nodes": [
                { "id": "q",    "label": "range mapped?", "role": "decision" },
                { "id": "yes",  "label": "Source::Diff" },
                { "id": "no",   "label": "Source::Stale", "weight": "muted" },
                { "id": "file", "label": "snapshot", "role": "store" }
            ],
            "edges": [
                { "from": "file", "to": "q" },
                { "from": "q", "to": "yes", "label": "yes" },
                { "from": "q", "to": "no",  "label": "no", "line": "dashed" }
            ]
        }"#;

        let d: Diagram = serde_json::from_str(json).unwrap();
        assert_eq!(d.nodes[0].role, Role::Decision);
        assert_eq!(d.nodes[3].role, Role::Store);

        let l = d.layout();
        assert_eq!(at(&d, &l, "yes").1, at(&d, &l, "no").1);
        assert_ne!(at(&d, &l, "yes").0, at(&d, &l, "no").0);
    }

    #[test]
    fn a_role_changes_the_drawing_and_not_the_placement() {
        // Shape is the view's business. Ranking must not shift because a node
        // became a circle.
        let plain = diagram(Direction::Right, &["a", "b"], &[("a", "b")]);
        let mut shaped = diagram(Direction::Right, &["a", "b"], &[("a", "b")]);
        shaped.nodes[0].role = Role::Actor;
        shaped.nodes[1].role = Role::Terminal;

        assert_eq!(plain.layout(), shaped.layout());
    }

    #[test]
    fn a_diagram_with_no_nodes_still_has_a_size() {
        let d = diagram(Direction::Down, &[], &[]);
        let l = d.layout();

        assert!(l.placed.is_empty());
        assert_eq!((l.cols, l.rows), (1, 1));
    }

    #[test]
    fn the_wire_form_needs_only_nodes() {
        let json = r#"{
            "nodes": [
                { "id": "wire",  "label": "RefSpec", "note": "what lands on disk" },
                { "id": "model", "label": "Anchor",  "weight": "accent" }
            ],
            "edges": [{ "from": "wire", "to": "model", "label": "anchor()" }]
        }"#;

        let d: Diagram = serde_json::from_str(json).unwrap();
        assert_eq!(d.flow, Direction::Down);
        assert_eq!(d.nodes[1].weight, Weight::Accent);
        assert_eq!(
            d.nodes[0].role,
            Role::Step,
            "a node is a box unless it says"
        );
        assert_eq!(d.edges[0].line, Line::Solid);
        assert!(d.clusters.is_empty());

        let l = d.layout();
        assert_eq!(at(&d, &l, "model"), (0, 1));
    }
}
