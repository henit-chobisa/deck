//! A diagram as an SVG, drawn the way the window draws it.
//!
//! Nothing here decides how a diagram looks. Where the boxes go is
//! [`deck_core::geometry`], what is painted there is [`deck_core::drawing`],
//! and where a label breaks is [`crate::text`]: the same three the window
//! paints from. This only writes them down, so a diagram in a PDF or an HTML
//! export is the diagram the reader saw, at rest — nothing picked and no flow
//! playing.
//!
//! The words are real text, not outlines, so a document can be searched and
//! copied from. Each line is placed by its own left edge, worked out from its
//! shaped width, rather than centred by the viewer: a viewer centres a line
//! without the trailing space the window centres it with, and would set every
//! wrapped line half a space off.

use std::fmt::Write as _;

use deck_core::diagram::{Diagram, Line, Role, Weight};
use deck_core::drawing::{self, Cmd, Lit, Prim};
use deck_core::geometry::{self, Pt, Rect};
use deck_core::theme::{Palette, Rgb};

use crate::text::{self, PLEX};

/// Whether the SVG carries its own copy of the face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fonts {
    /// Names IBM Plex Sans and leaves finding it to whatever shows the SVG:
    /// an HTML page that loads the face once for every diagram on it.
    Named,
    /// Embeds the three cuts, so the SVG stands on its own anywhere.
    Embedded,
}

/// `diagram` as an SVG document, at rest, in `palette`.
#[must_use]
pub fn diagram(diagram: &Diagram, palette: &Palette, fonts: Fonts) -> String {
    let plan = geometry::plan(diagram, &diagram.layout());
    // The picture's bounds: the plan, grown to take in any cluster frame
    // that reaches past it. A cluster in the first row stands its name above
    // the margin, and the window, which does not clip its canvas, shows the
    // frame whole whenever the pane has room (#174). An export has no pane
    // around it, so it makes the room itself.
    let (mut left, mut top, mut right, mut bottom) = (0f32, 0f32, plan.width, plan.height);
    for frame in &plan.frames {
        left = left.min(frame.at.x);
        top = top.min(frame.at.y);
        right = right.max(frame.at.right());
        bottom = bottom.max(frame.at.bottom());
    }
    // And the margin a frame keeps everywhere else, on any side it passed.
    let margin = geometry::CLUSTER_PAD;
    if left < 0. {
        left -= margin;
    }
    if top < 0. {
        top -= margin;
    }
    if right > plan.width {
        right += margin;
    }
    if bottom > plan.height {
        bottom += margin;
    }
    let (w, h) = (right - left, bottom - top);
    let title = diagram.title.as_deref().unwrap_or("diagram");
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="{} {} {} {}" role="img" aria-label="{}" font-family="'IBM Plex Sans', sans-serif" style="font-kerning:normal">"#,
        num(w),
        num(h),
        num(left),
        num(top),
        num(w),
        num(h),
        escape(title),
    );
    out.push('\n');
    let _ = writeln!(out, "<title>{}</title>", escape(title));
    if fonts == Fonts::Embedded {
        out.push_str("<style>");
        for ((bytes, _), weight) in PLEX.iter().zip([400, 500, 600]) {
            let _ = write!(
                out,
                "@font-face{{font-family:'IBM Plex Sans';font-weight:{weight};src:url(data:font/ttf;base64,{})}}",
                base64(bytes)
            );
        }
        out.push_str("</style>\n");
    }
    // The pane's own ground, which the edge labels are cut out of.
    let _ = writeln!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
        num(left),
        num(top),
        num(w),
        num(h),
        hex(palette.wash)
    );

    // The ink, in the order the window paints it: frames, arrows, boxes.
    let mut prims = Vec::new();
    for frame in &plan.frames {
        drawing::shape(
            &mut prims,
            frame.at,
            Role::Step,
            drawing::frame_skin(palette),
        );
    }
    for route in &plan.routes {
        let edge = &diagram.edges[route.edge_ix];
        let colour = drawing::arrow_colour(Lit::Resting, route.back, palette);
        drawing::arrow(&mut prims, &route.points, colour, edge.line == Line::Dashed);
    }
    for prim in &prims {
        prim_to(&mut out, prim);
    }
    // Each box in a group that says how far down it reaches, from the
    // picture's top: what a page that shows the picture in bands of its
    // height cuts between, and leaves out when a box is not in its band.
    for spot in &plan.spots {
        let node = &diagram.nodes[spot.node_ix];
        let skin = drawing::skin(node.weight, false, Lit::Resting, palette);
        let mut shape = Vec::new();
        let body = drawing::body(spot.at, node.role);
        drawing::shape(&mut shape, body, node.role, skin);
        let (reach, end) = (spot.at.y.min(body.y), spot.at.bottom().max(body.bottom()));
        let _ = writeln!(
            out,
            r#"<g data-top="{}" data-bottom="{}">"#,
            num(reach - top),
            num(end - top)
        );
        for prim in &shape {
            prim_to(&mut out, prim);
        }
        out.push_str("</g>\n");
    }

    // The words, in the window's order too: cluster names, edge labels, then
    // the labels inside the boxes.
    for frame in &plan.frames {
        let Some(cluster) = diagram.clusters.get(frame.cluster_ix) else {
            continue;
        };
        let size = drawing::CLUSTER_NAME;
        let top = frame.at.y + drawing::CLUSTER_NAME_AT.1;
        let x = frame.at.x + drawing::CLUSTER_NAME_AT.0;
        line_to(
            &mut out,
            &cluster.label,
            x,
            top,
            size,
            drawing::default_line(size),
            text::Weight::Regular,
            palette.muted,
        );
    }
    for route in &plan.routes {
        let edge = &diagram.edges[route.edge_ix];
        if let Some(label) = &edge.label {
            edge_label(&mut out, label, &route.points, palette);
        }
    }
    for spot in &plan.spots {
        let node = &diagram.nodes[spot.node_ix];
        node_label(&mut out, node, drawing::body(spot.at, node.role), palette);
    }
    out.push_str("</svg>\n");
    out
}

/// A node's label and note, centred in the box the way the window's column
/// centres them.
fn node_label(out: &mut String, node: &deck_core::diagram::Node, at: Rect, palette: &Palette) {
    let weight = if node.weight == Weight::Accent {
        text::Weight::Semibold
    } else {
        text::Weight::Regular
    };
    let ink = if node.weight == Weight::Muted {
        palette.muted
    } else {
        palette.fg
    };
    let inset = drawing::inset(node.role);
    let room = at.w - 2. * inset;
    let (label_size, label_line) = drawing::LABEL;
    let (note_size, note_line) = drawing::NOTE;
    let lines = text::wrap(&node.label, label_size, weight, room);

    #[allow(clippy::cast_precision_loss)]
    let mut height = lines.len() as f32 * label_line;
    if node.note.is_some() {
        height += drawing::LABEL_GAP + note_line;
    }
    let (pad_top, pad_bottom) = if node.role == Role::Store {
        drawing::STORE_PAD
    } else {
        (0., 0.)
    };
    let inner = at.h - pad_top - pad_bottom;
    // Where the words come to more than the box, the window's column gives
    // way at the note and nowhere else: the note clips what overflows it, so
    // it may shrink, and the label may not. A store, which gives up its head
    // to the lid, is where this happens.
    let mut note_height = note_line;
    if node.note.is_some() && height > inner {
        let give = (height - inner).min(note_line);
        note_height -= give;
        height -= give;
    }
    let mut top = at.y + pad_top + (inner - height) / 2.;
    let middle = at.x + at.w / 2.;

    for line in &lines {
        let width = text::shaped_width(line, label_size, weight);
        line_to(
            out,
            line,
            middle - width / 2.,
            top,
            label_size,
            label_line,
            weight,
            ink,
        );
        top += label_line;
    }
    if let Some(note) = &node.note {
        top += drawing::LABEL_GAP;
        let cut = text::truncate(note, note_size, weight, room);
        let width = text::shaped_width(&cut, note_size, weight);
        let left = at.x + inset + (room - width) / 2.;
        if note_height < note_line {
            let _ = write!(
                out,
                r#"<svg x="{}" y="{}" width="{}" height="{}" overflow="hidden"><g transform="translate({} {})">"#,
                num(at.x + inset),
                num(top),
                num(room),
                num(note_height.max(0.)),
                num(-(at.x + inset)),
                num(-top)
            );
            out.push('\n');
            line_to(
                out,
                &cut,
                left,
                top,
                note_size,
                note_line,
                weight,
                palette.muted,
            );
            out.push_str("</g></svg>\n");
        } else {
            line_to(
                out,
                &cut,
                left,
                top,
                note_size,
                note_line,
                weight,
                palette.muted,
            );
        }
    }
}

/// What an arrow claims, on a chip of the pane's ground, beside the line or
/// above it and never on it. See `chart.rs` in the window for why.
fn edge_label(out: &mut String, label: &str, points: &[Pt], palette: &Palette) {
    let (Some(a), Some(b), Some(from), Some(to)) =
        (points.get(1), points.get(2), points.first(), points.last())
    else {
        return;
    };
    let x = (a.x + b.x) / 2.;
    let y = (a.y + b.y) / 2.;
    let flat = (from.y - to.y).abs() < (from.x - to.x).abs();

    let size = drawing::EDGE_LABEL;
    let leading = drawing::default_line(size);
    let (pad_x, pad_y) = drawing::CHIP_PAD;
    let widest = drawing::chip_widest();
    let lines = text::wrap(label, size, text::Weight::Regular, widest - 2. * pad_x);
    let widths: Vec<f32> = lines
        .iter()
        .map(|line| text::shaped_width(line, size, text::Weight::Regular))
        .collect();
    let content = if lines.len() > 1 {
        widest - 2. * pad_x
    } else {
        widths
            .iter()
            .copied()
            .fold(0., f32::max)
            .min(widest - 2. * pad_x)
    };
    let w = content + 2. * pad_x;
    #[allow(clippy::cast_precision_loss)]
    let h = lines.len() as f32 * leading + 2. * pad_y;
    let (left, top) = if flat {
        (x - w / 2., y - drawing::CHIP_ABOVE - h)
    } else {
        (x + drawing::CHIP_BESIDE.0, y + drawing::CHIP_BESIDE.1)
    };

    let _ = writeln!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}"/>"#,
        num(left),
        num(top),
        num(w),
        num(h),
        num(drawing::CHIP_RADIUS),
        hex(palette.wash)
    );
    let mut line_top = top + pad_y;
    for (line, width) in lines.iter().zip(widths) {
        let line_left = left + pad_x + (content - width) / 2.;
        line_to(
            out,
            line,
            line_left,
            line_top,
            size,
            leading,
            text::Weight::Regular,
            palette.muted,
        );
        line_top += leading;
    }
}

/// One line of words, its box's top-left at `left`, `top`, its baseline
/// where the window puts it: centred in the line height by the face's own
/// ascent and descent.
#[allow(clippy::too_many_arguments)]
fn line_to(
    out: &mut String,
    words: &str,
    left: f32,
    top: f32,
    size: f32,
    leading: f32,
    weight: text::Weight,
    ink: Rgb,
) {
    let (ascent, descent) = text::ascent_descent(size, weight);
    let baseline = top + (leading - ascent - descent) / 2. + ascent;
    let bold = match weight {
        text::Weight::Regular => 400,
        text::Weight::Medium => 500,
        text::Weight::Semibold => 600,
    };
    let _ = writeln!(
        out,
        r#"<text x="{}" y="{}" font-size="{}" font-weight="{bold}" fill="{}" xml:space="preserve">{}</text>"#,
        num(left),
        num(baseline),
        num(size),
        hex(ink),
        escape(words)
    );
}

/// One primitive. A border is painted inside the box, as the window paints a
/// quad's, by stroking a rectangle pulled in by half the border's width.
fn prim_to(out: &mut String, prim: &Prim) {
    match prim {
        Prim::Quad {
            at,
            radius,
            fill,
            border,
        } => {
            let _ = write!(
                out,
                r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}""#,
                num(at.x),
                num(at.y),
                num(at.w),
                num(at.h),
                num(*radius),
                hex(*fill)
            );
            if let Some((width, colour)) = border {
                let half = width / 2.;
                // The border as its own rectangle, pulled in by half its
                // width, so its stroke lies wholly inside the box.
                let _ = write!(
                    out,
                    r#"/>
<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="none" stroke="{}" stroke-width="{}""#,
                    num(at.x + half),
                    num(at.y + half),
                    num(at.w - width),
                    num(at.h - width),
                    num((radius - half).max(0.)),
                    hex(*colour),
                    num(*width)
                );
            }
            out.push_str("/>\n");
        }
        Prim::Path { start, cmds, fill } => {
            let mut d = format!("M{} {}", num(start.x), num(start.y));
            for cmd in cmds {
                match cmd {
                    Cmd::Line(to) => {
                        let _ = write!(d, "L{} {}", num(to.x), num(to.y));
                    }
                    Cmd::Curve { to, ctrl } => {
                        let _ = write!(
                            d,
                            "Q{} {} {} {}",
                            num(ctrl.x),
                            num(ctrl.y),
                            num(to.x),
                            num(to.y)
                        );
                    }
                }
            }
            let _ = writeln!(out, r#"<path d="{d}Z" fill="{}"/>"#, hex(*fill));
        }
    }
}

/// A number as short as it can be written and still read back the same to
/// a hundredth of a pixel.
fn num(value: f32) -> String {
    let rounded = (value * 100.).round() / 100.;
    let text = format!("{rounded:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

fn hex(colour: Rgb) -> String {
    colour.to_hex()
}

fn escape(words: &str) -> String {
    let mut out = String::with_capacity(words.len());
    for c in words.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Standard base64, for a face embedded as a data URL.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// deck's own dark palette, as the window paints with it by default.
    fn palette() -> Palette {
        let hex = |s: &str| Rgb::from_hex(s).expect("a colour");
        Palette {
            bg: hex("#282828"),
            fg: hex("#ebdbb2"),
            band: hex("#191b1b"),
            wash: hex("#1f1f1f"),
            focus: hex("#332b21"),
            accent: hex("#fe8019"),
            on_accent: hex("#282828"),
            add: hex("#b8bb26"),
            del: hex("#fb4934"),
            comment: hex("#a89984"),
            muted: hex("#a89984"),
            punctuation: hex("#a89984"),
            edge: hex("#3a3735"),
            ground: hex("#0d0e0e"),
            gone: Rgb::new(0, 0, 0),
            fresh: Rgb::new(0, 0, 0),
            gone_deep: Rgb::new(0, 0, 0),
            fresh_deep: Rgb::new(0, 0, 0),
        }
    }

    /// Every fixture diagram, as the SVG it was last checked to be.
    ///
    /// Checked by eye against the window: each was drawn in Chrome at 2x and
    /// laid over a capture of the same diagram in deck, and they agree to
    /// within the half pixel the window snaps a baseline to. Run with
    /// `UPDATE_GOLDEN=1` when a change is meant to move one, and look again.
    #[test]
    fn fixture_diagrams_draw_as_they_did() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        for name in ["shapes", "wrap", "across"] {
            let json = std::fs::read_to_string(root.join(format!("fixtures/diagrams/{name}.json")))
                .expect("the fixture exists");
            let picture: Diagram = serde_json::from_str(&json).expect("a diagram");
            let now = diagram(&picture, &palette(), Fonts::Named);
            let at = root.join(format!("golden/{name}.svg"));
            if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
                std::fs::write(&at, &now).expect("writable");
            }
            let was = std::fs::read_to_string(&at)
                .expect("the golden file exists; run with UPDATE_GOLDEN=1")
                .replace("\r\n", "\n");
            assert_eq!(
                now, was,
                "{name}.svg moved; if that was meant, run with UPDATE_GOLDEN=1"
            );
        }
    }

    #[test]
    fn an_embedded_face_is_all_three_cuts() {
        let json = r#"{"nodes":[{"id":"a","label":"a"}]}"#;
        let picture: Diagram = serde_json::from_str(json).expect("a diagram");
        let svg = diagram(&picture, &palette(), Fonts::Embedded);
        assert_eq!(svg.matches("@font-face").count(), 3);
        assert!(!diagram(&picture, &palette(), Fonts::Named).contains("@font-face"));
    }

    #[test]
    fn a_cluster_in_the_first_row_is_drawn_whole() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let json = std::fs::read_to_string(root.join("fixtures/diagrams/across.json"))
            .expect("the fixture exists");
        let picture: Diagram = serde_json::from_str(&json).expect("a diagram");
        let plan = geometry::plan(&picture, &picture.layout());
        let top = plan.frames.iter().map(|f| f.at.y).fold(0f32, f32::min);
        assert!(top < 0., "the fixture's frame should reach above the plan");
        let svg = diagram(&picture, &palette(), Fonts::Named);
        // The frame's top, less the margin a frame keeps on every side.
        let edge = num(top - geometry::CLUSTER_PAD);
        assert!(svg.contains(&format!(r#"viewBox="0 {edge} "#)), "{svg}");
    }

    #[test]
    fn base64_is_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn numbers_are_short() {
        assert_eq!(num(156.), "156");
        assert_eq!(num(12.5), "12.5");
        assert_eq!(num(1. / 3.), "0.33");
        assert_eq!(num(-0.001), "0");
    }

    #[test]
    fn words_are_escaped() {
        assert_eq!(escape("a < b & \"c\""), "a &lt; b &amp; &quot;c&quot;");
    }
}
