//! A diagram's ink: every shape, arrow and frame as a list of plain
//! primitives, in the order they are painted.
//!
//! [`crate::geometry`] says where things go; this says what is painted there.
//! It used to be the window's canvas code, which made a node's outline, an
//! arrowhead and a cylinder's lid things only the window could draw. Here they
//! are filled rectangles and filled paths in plain colours, so the window and
//! the exports to come (#170) paint the same picture from the same list.
//!
//! Only fills. A path is filled, not stroked, everywhere deck draws one: a
//! border is the outline in the edge colour with the same outline inset by the
//! border's width painted over it in the fill. That is how the window always
//! drew them, and it is what lets an SVG or a PDF reproduce them exactly.

use crate::diagram::{Role, Weight};
use crate::geometry::{Pt, Rect};
use crate::theme::{Palette, Rgb};

/// How thick an arrow is.
pub const STROKE: f32 = 1.25;
/// How far back an arrowhead reaches from its point, and how wide it is there.
pub const HEAD: (f32, f32) = (9., 6.5);

/// How far a node that is not on the playing flow recedes toward the page.
///
/// Far enough to fall behind, near enough to still be read. A flow that hid
/// the rest of the diagram would answer the question by deleting the context
/// that makes it a question — and at 0.68 the boxes off the path had gone so
/// far into the page that their edges were guesswork.
pub const ASIDE: f32 = 0.46;

/// A node's label: its size and its line height.
///
/// The line height is spelled out rather than left to the face. A box is a
/// fixed height and what goes in it has to be known to fit: with the default
/// leading a label and a note came to more than the box, and the note was cut
/// through by the bottom border. A 15 and a 13 leave room inside 50 even when
/// the label wraps.
pub const LABEL: (f32, f32) = (12., 15.);
/// A node's note, the one line under its label: size and line height.
pub const NOTE: (f32, f32) = (10.5, 13.);
/// The space between a label and its note.
pub const LABEL_GAP: f32 = 1.;
/// How far a store's words stay below its lid and above its foot.
pub const STORE_PAD: (f32, f32) = (12., 6.);
/// How big an edge label is.
pub const EDGE_LABEL: f32 = 10.;
/// How big a cluster's name is.
pub const CLUSTER_NAME: f32 = 10.;
/// Where a cluster's name sits, from its frame's top-left corner.
pub const CLUSTER_NAME_AT: (f32, f32) = (13., 5.);
/// The padding inside an edge label's chip, across and down.
pub const CHIP_PAD: (f32, f32) = (4., 1.);
/// How round an edge label's chip is.
pub const CHIP_RADIUS: f32 = 3.;
/// How far above a horizontal run its label's chip ends.
pub const CHIP_ABOVE: f32 = 4.;
/// Where a vertical run's label chip starts, from the middle of the run.
pub const CHIP_BESIDE: (f32, f32) = (7., -8.);

/// The widest an edge label's chip may be: the gutter it sits in, give or
/// take a few pixels. A label is meant to be a word or two, and one that is
/// not wraps rather than lying across the two nodes the arrow joins.
#[must_use]
pub fn chip_widest() -> f32 {
    crate::geometry::GAP_X + 16.
}

/// The line height text gets when nothing sets one: the golden ratio of its
/// size, rounded to a whole pixel, as GPUI's default text style has it.
///
/// The literal is GPUI's own `phi()`, digit for digit, rather than the
/// standard library's golden ratio: what matters is the window's number.
#[must_use]
#[allow(clippy::approx_constant)]
pub fn default_line(size: f32) -> f32 {
    (size * 1.618_034).round()
}

/// One thing to paint.
#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    /// A rectangle, perhaps with rounded corners and a border.
    Quad {
        /// Where it goes.
        at: Rect,
        /// The radius of its corners. Zero is square.
        radius: f32,
        /// What it is filled with.
        fill: Rgb,
        /// Its border, as a width and a colour, if it has one.
        border: Option<(f32, Rgb)>,
    },
    /// A closed outline, filled.
    Path {
        /// Where the outline starts.
        start: Pt,
        /// How it goes on from there.
        cmds: Vec<Cmd>,
        /// What it is filled with.
        fill: Rgb,
    },
}

/// One step along a path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cmd {
    /// A straight line to here.
    Line(Pt),
    /// A quadratic curve to `to`, pulled toward `ctrl`.
    Curve {
        /// Where the curve ends.
        to: Pt,
        /// The point it is pulled toward.
        ctrl: Pt,
    },
}

/// How a shape is coloured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Skin {
    /// Its body.
    pub fill: Rgb,
    /// Its outline.
    pub edge: Rgb,
    /// How thick the outline is.
    pub width: f32,
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
    On(f32, Option<Rgb>),
    /// Not on the path, while a path is being shown, and this far stepped
    /// back: zero is the picture at rest, one is all the way back.
    ///
    /// A level rather than a switch so that the agent's point can ease the
    /// rest of the picture back instead of cutting it. A flow always passes
    /// one, and `Aside(1.)` is exactly what plain `Aside` used to draw.
    Aside(f32),
    /// The block the agent is pointing at, part way up. The first number is
    /// how much the block is lit (zero is as far back as the rest of the
    /// picture, one is as the path would show it) and the second is how far
    /// back the rest of the picture is.
    ///
    /// Not `On`, because `On` starts from stepped-back and a block that has
    /// just been pointed at starts from where it was resting.
    Held(f32, f32),
}

/// How a node is coloured: by how much it matters, whether the reader has
/// picked it, and what a flow is doing to it.
#[must_use]
pub fn skin(weight: Weight, picked: bool, lit: Lit, palette: &Palette) -> Skin {
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
        Lit::Aside(back) => (
            fill.mix(palette.bg, ASIDE * back),
            edge.mix(palette.bg, ASIDE * back),
            width,
        ),
        // Between the block stepped back by `back` and the block fully lit.
        Lit::Held(much, back) => {
            let lit = palette.accent;
            let from = (
                fill.mix(palette.bg, ASIDE * back),
                edge.mix(palette.bg, ASIDE * back),
            );
            let to = (fill.mix(lit, 0.16), lit);
            (
                from.0.mix(to.0, much),
                from.1.mix(to.1, much),
                width + (1.5_f32 - width).max(0.) * much,
            )
        }
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

    Skin { fill, edge, width }
}

/// How a cluster's frame is coloured.
///
/// A cluster is a ground, not an object: it sits under the boxes and is told
/// apart from the pane by a shade, not by a border with any weight to it.
#[must_use]
pub fn frame_skin(palette: &Palette) -> Skin {
    Skin {
        fill: palette.wash.mix(palette.ground, 0.22),
        edge: palette.edge.mix(palette.wash, 0.35),
        width: 1.,
    }
}

/// The colour of one arrow.
///
/// A returning arrow is drawn in the accent, because it is the one thing in a
/// state machine a reader has to notice — and because otherwise it reads as an
/// arrow pointing the wrong way by mistake.
#[must_use]
pub fn arrow_colour(lit: Lit, back: bool, palette: &Palette) -> Rgb {
    let quiet = palette.fg.mix(palette.wash, 0.45);
    match lit {
        // Travelling is the thing being shown, so the arrows carry it as much
        // as the boxes do — and they carry it first, since an arrow is the part
        // of the path that is only ever motion.
        Lit::On(much, tint) => quiet
            .mix(palette.bg, ASIDE)
            .mix(tint.unwrap_or(palette.accent), much),
        Lit::Aside(back) => quiet.mix(palette.bg, ASIDE * back),
        Lit::Held(much, back) => quiet
            .mix(palette.bg, ASIDE * back)
            .mix(palette.accent, much),
        Lit::Resting => {
            if back {
                palette.accent
            } else {
                quiet
            }
        }
    }
}

/// How far a label has to stay inside a shape of this role.
#[must_use]
pub fn inset(role: Role) -> f32 {
    match role {
        Role::Step | Role::Store => 10.,
        Role::Terminal => 18.,
        Role::Actor => 22.,
        Role::Decision => 16.,
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
        Role::Decision => (8., 4.),
        _ => (0., 0.),
    }
}

/// The box a role's shape is actually drawn in.
#[must_use]
pub fn body(at: Rect, role: Role) -> Rect {
    let (x, y) = swell(role);
    Rect {
        x: at.x - x,
        y: at.y - y,
        w: at.w + x * 2.,
        h: at.h + y * 2.,
    }
}

/// One node's body, drawn in `at` (already its [`body`]).
///
/// Role decides the outline, and only the outline. A diamond means a branch
/// everywhere diagrams are drawn, and throwing that away to keep the drawing
/// code simple would throw away information the reader already knows how to
/// read.
pub fn shape(out: &mut Vec<Prim>, at: Rect, role: Role, skin: Skin) {
    let round = |radius: f32| Prim::Quad {
        at,
        radius,
        fill: skin.fill,
        border: Some((skin.width, skin.edge)),
    };
    match role {
        Role::Step => out.push(round(6.)),
        Role::Store => cylinder(out, at, skin),
        Role::Terminal => out.push(round(at.h / 2.)),
        // A path is filled, not stroked, so the border is a second, larger
        // shape painted underneath: the outline in the edge colour, then the
        // same outline inset by the border's own width in the fill.
        Role::Decision => {
            out.push(hexagon(at, 0., skin.edge));
            out.push(hexagon(at, skin.width, skin.fill));
        }
        Role::Actor => {
            out.push(ellipse(at, 0., skin.edge));
            out.push(ellipse(at, skin.width, skin.fill));
        }
    }
}

/// A hexagon filling `at`, pulled in by `inset` on every side.
///
/// A diamond was the obvious shape for a decision and the wrong one. Its widest
/// point is a single line through the middle and everything above and below
/// that is corner, so a question of more than three words either spilled over
/// the edges or had to be shrunk until it could not be read — and a question is
/// what a decision node holds.
///
/// A hexagon keeps what the diamond was saying: points to the left and right,
/// where the two answers leave. But its middle is a flat band the full height
/// of the box, so the words have somewhere to be.
#[must_use]
pub fn hexagon(at: Rect, inset: f32, fill: Rgb) -> Prim {
    let (l, t) = (at.x + inset, at.y + inset);
    let r = at.right() - inset;
    let b = at.bottom() - inset;
    let cy = (t + b) / 2.;

    // How far in the points come. Half the height gives sides at 45 degrees,
    // capped at a third of the width so a wide box does not become a bar with
    // two barely visible nicks in the ends.
    let cut = ((b - t) / 2.).min((r - l) / 3.);

    let pt = |x, y| Pt { x, y };
    Prim::Path {
        start: pt(l + cut, t),
        cmds: vec![
            Cmd::Line(pt(r - cut, t)),
            Cmd::Line(pt(r, cy)),
            Cmd::Line(pt(r - cut, b)),
            Cmd::Line(pt(l + cut, b)),
            Cmd::Line(pt(l, cy)),
        ],
        fill,
    }
}

/// An ellipse filling `at`, pulled in by `inset` on every side.
///
/// Four quadratic curves with their controls at the corners. Not the exact
/// curve — the exact one needs cubics — but at this size the difference is
/// under a pixel, and what the shape has to say is "not a rectangle". An
/// export draws these same four curves rather than a true ellipse, so it looks
/// exactly like the window.
#[must_use]
pub fn ellipse(at: Rect, inset: f32, fill: Rgb) -> Prim {
    let (l, t) = (at.x + inset, at.y + inset);
    let r = at.right() - inset;
    let b = at.bottom() - inset;
    let (cx, cy) = ((l + r) / 2., (t + b) / 2.);
    let pt = |x, y| Pt { x, y };

    Prim::Path {
        start: pt(l, cy),
        cmds: vec![
            Cmd::Curve {
                to: pt(cx, t),
                ctrl: pt(l, t),
            },
            Cmd::Curve {
                to: pt(r, cy),
                ctrl: pt(r, t),
            },
            Cmd::Curve {
                to: pt(cx, b),
                ctrl: pt(r, b),
            },
            Cmd::Curve {
                to: pt(l, cy),
                ctrl: pt(l, b),
            },
        ],
        fill,
    }
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
fn cylinder(out: &mut Vec<Prim>, at: Rect, skin: Skin) {
    /// How tall the caps are. Enough to read as an ellipse at this width
    /// without eating the room the label needs.
    const CAP: f32 = 13.;

    let (l, t, w, h) = (at.x, at.y, at.w, at.h);

    // The three pieces, each pulled in by `inset` on every side. The body is
    // the straight part between the two caps; the caps overhang it, which is
    // what makes the join look curved rather than cut.
    let mut silhouette = |inset: f32, colour: Rgb| {
        let cap = Rect {
            x: l + inset,
            y: t + inset,
            w: w - inset * 2.,
            h: CAP,
        };
        let foot = Rect {
            x: l + inset,
            y: t + h - CAP - inset,
            w: w - inset * 2.,
            h: CAP,
        };
        let body = Rect {
            x: l + inset,
            y: t + CAP / 2.,
            w: w - inset * 2.,
            h: h - CAP,
        };
        out.push(ellipse(foot, 0., colour));
        out.push(Prim::Quad {
            at: body,
            radius: 0.,
            fill: colour,
            border: None,
        });
        out.push(ellipse(cap, 0., colour));
    };

    silhouette(0., skin.edge);
    silhouette(skin.width, skin.fill);

    // The lid: the cap outlined again, so the top reads as an opening rather
    // than as the flat end of a box.
    let cap = Rect {
        x: l + skin.width,
        y: t + skin.width,
        w: w - skin.width * 2.,
        h: CAP,
    };
    out.push(ellipse(cap, 0., skin.edge));
    out.push(ellipse(cap, skin.width, skin.fill));
}

/// One straight run of an arrow.
///
/// Every run is horizontal or vertical by construction, so a segment is a thin
/// rectangle and needs no path.
pub fn stroke(out: &mut Vec<Prim>, from: Pt, to: Pt, colour: Rgb, dashed: bool) {
    let (x0, y0) = (from.x, from.y);
    let (x1, y1) = (to.x, to.y);
    let flat = (y1 - y0).abs() < (x1 - x0).abs();

    let span = |a: f32, b: f32| (a.min(b), (b - a).abs());
    let (start, length) = if flat { span(x0, x1) } else { span(y0, y1) };
    if length < 0.5 {
        return;
    }

    let mut bar = |at: f32, run: f32| {
        let bounds = if flat {
            Rect {
                x: at,
                y: y0 - STROKE / 2.,
                w: run,
                h: STROKE,
            }
        } else {
            Rect {
                x: x0 - STROKE / 2.,
                y: at,
                w: STROKE,
                h: run,
            }
        };
        out.push(Prim::Quad {
            at: bounds,
            radius: 0.,
            fill: colour,
            border: None,
        });
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
pub fn arrowhead(out: &mut Vec<Prim>, from: Pt, to: Pt, colour: Rgb) {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    if length < 0.5 {
        return;
    }

    let (ux, uy) = (dx / length, dy / length);
    // The perpendicular, for the two corners at the back of the head.
    let (px, py) = (-uy, ux);
    let (tip_x, tip_y) = (to.x, to.y);
    let (base_x, base_y) = (tip_x - ux * HEAD.0, tip_y - uy * HEAD.0);
    let wing = HEAD.1 / 2.;

    out.push(Prim::Path {
        start: Pt { x: tip_x, y: tip_y },
        cmds: vec![
            Cmd::Line(Pt {
                x: base_x + px * wing,
                y: base_y + py * wing,
            }),
            Cmd::Line(Pt {
                x: base_x - px * wing,
                y: base_y - py * wing,
            }),
        ],
        fill: colour,
    });
}

/// A whole arrow: each run, then the head on the last one.
pub fn arrow(out: &mut Vec<Prim>, points: &[Pt], colour: Rgb, dashed: bool) {
    for pair in points.windows(2) {
        stroke(out, pair[0], pair[1], colour, dashed);
    }
    if let [.., before, last] = points {
        arrowhead(out, *before, *last, colour);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Imported, Syntax, derive};

    #[test]
    fn a_stepped_back_level_runs_from_resting_to_the_old_aside() {
        // The point eases the picture back, so the ends have to be the looks
        // the drawing always had: none stepped back is resting, all the way
        // is what `Aside` used to be, and a block fully held is a block on.
        let palette = palette();
        let at = |lit| skin(Weight::Normal, false, lit, &palette);
        assert_eq!(at(Lit::Aside(0.)), at(Lit::Resting));
        assert_eq!(at(Lit::Held(0., 0.)), at(Lit::Resting));
        assert_eq!(at(Lit::Held(1., 1.)), at(Lit::On(1., None)));
        assert_ne!(at(Lit::Aside(1.)), at(Lit::Resting));
        assert_eq!(at(Lit::Aside(1.)).edge, palette.edge.mix(palette.bg, ASIDE));
    }

    fn palette() -> Palette {
        let hex = |s: &str| Rgb::from_hex(s).expect("a colour");
        derive(Imported {
            bg: hex("#282828"),
            fg: hex("#ebdbb2"),
            accent: hex("#fe8019"),
            comment: None,
            add: None,
            del: None,
            syntax: Syntax::default(),
        })
    }

    /// Every number a list of primitives holds, one line each.
    fn dump(prims: &[Prim]) -> String {
        let pt = |p: &Pt| format!("{},{}", p.x, p.y);
        prims
            .iter()
            .map(|prim| match prim {
                Prim::Quad {
                    at,
                    radius,
                    fill,
                    border,
                } => format!(
                    "quad {} {} {} {} r{} {}{}\n",
                    at.x,
                    at.y,
                    at.w,
                    at.h,
                    radius,
                    fill.to_hex(),
                    border.map_or_else(String::new, |(w, c)| format!(" border {w} {}", c.to_hex()))
                ),
                Prim::Path { start, cmds, fill } => {
                    let steps: Vec<String> = cmds
                        .iter()
                        .map(|cmd| match cmd {
                            Cmd::Line(to) => format!("L{}", pt(to)),
                            Cmd::Curve { to, ctrl } => format!("Q{}~{}", pt(ctrl), pt(to)),
                        })
                        .collect();
                    format!("path {} {} {}\n", fill.to_hex(), pt(start), steps.join(" "))
                }
            })
            .collect()
    }

    #[test]
    fn the_ink_is_what_it_was() {
        // What the window paints every diagram with, and every export will:
        // held still, so a change to a shape shows as exactly which numbers
        // moved. `UPDATE_GOLDEN=1` writes it again, on purpose.
        let palette = palette();
        let cell = Rect {
            x: 30.,
            y: 30.,
            w: 156.,
            h: 54.,
        };
        let mut now = String::new();
        for role in [
            Role::Step,
            Role::Decision,
            Role::Store,
            Role::Terminal,
            Role::Actor,
        ] {
            for (state, weight, picked, lit) in [
                ("normal", Weight::Normal, false, Lit::Resting),
                ("accent", Weight::Accent, false, Lit::Resting),
                ("muted", Weight::Muted, false, Lit::Resting),
                ("picked", Weight::Normal, true, Lit::Resting),
                ("aside", Weight::Normal, false, Lit::Aside(1.)),
                ("half lit", Weight::Normal, false, Lit::On(0.5, None)),
            ] {
                let mut prims = Vec::new();
                shape(
                    &mut prims,
                    body(cell, role),
                    role,
                    skin(weight, picked, lit, &palette),
                );
                now += &format!("# {role:?}, {state}\n{}", dump(&prims));
            }
        }
        let mut prims = Vec::new();
        shape(&mut prims, cell, Role::Step, frame_skin(&palette));
        now += &format!("# a cluster frame\n{}", dump(&prims));
        let corners = [
            Pt { x: 108., y: 84. },
            Pt { x: 108., y: 105. },
            Pt { x: 320., y: 105. },
            Pt { x: 320., y: 126. },
        ];
        for (name, dashed, lit, back) in [
            ("a solid arrow", false, Lit::Resting, false),
            ("a dashed arrow", true, Lit::Resting, false),
            ("a returning arrow", false, Lit::Resting, true),
            (
                "an arrow a flow is crossing",
                false,
                Lit::On(0.4, None),
                false,
            ),
        ] {
            let mut prims = Vec::new();
            arrow(
                &mut prims,
                &corners,
                arrow_colour(lit, back, &palette),
                dashed,
            );
            now += &format!("# {name}\n{}", dump(&prims));
        }

        let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/drawing.txt");
        if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
            std::fs::create_dir_all(at.parent().expect("a parent")).expect("writable");
            std::fs::write(&at, &now).expect("writable");
        }
        let was =
            std::fs::read_to_string(&at).expect("the golden file exists; run with UPDATE_GOLDEN=1");
        // Line endings are not ink: a checkout may have made them CRLF.
        let was = was.replace("\r\n", "\n");
        assert_eq!(
            now, was,
            "the ink moved; if that was meant, run with UPDATE_GOLDEN=1"
        );
    }

    #[test]
    fn a_short_run_draws_nothing_and_a_dash_never_overshoots() {
        let mut prims = Vec::new();
        stroke(
            &mut prims,
            Pt { x: 0., y: 0. },
            Pt { x: 0.3, y: 0. },
            Rgb::new(0, 0, 0),
            false,
        );
        assert!(
            prims.is_empty(),
            "a run shorter than half a point is not drawn"
        );

        stroke(
            &mut prims,
            Pt { x: 0., y: 0. },
            Pt { x: 12., y: 0. },
            Rgb::new(0, 0, 0),
            true,
        );
        let end = prims
            .iter()
            .map(|prim| match prim {
                Prim::Quad { at, .. } => at.right(),
                Prim::Path { .. } => 0.,
            })
            .fold(0., f32::max);
        assert!(end <= 12., "the last dash stops at the end of the run");
    }
}
