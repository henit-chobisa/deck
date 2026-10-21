//! Landscape pages: the window on paper.
//!
//! A4 on its side is 1123 × 794 at 96 pixels to the inch, and every measure
//! here is the window's own, in its pixels, from [`crate::metrics`]: the band
//! with the group's number and the line hanging from it, the panes under it,
//! the contents down the side, the strip along the bottom. A PDF draws the
//! page at three quarters of a point to the pixel, which is what A4 is.
//!
//! What was said about a group gets a page of its own after it — a thin band
//! naming the group, the messages in two columns across the width, a remark
//! on a line carrying the line — as the HTML export lays it out.
//!
//! The window scrolls and a page cannot. A pane with more rows than its room
//! first loses context, down to the lines that carry its evidence; what still
//! does not fit is counted in a last row and the export says so, pointing at
//! the portrait pages, which never cut anything.

use deck_core::document::{Author, Change, Document, Group, Line, Message, Pane, RowKind};
use deck_core::protocol::Kind as Remark;
use deck_core::theme::Rgb;

use super::layout::{
    Context, DIAGRAM_LEAST, Face, Item, Kind, Page, Placed, Rect, Token, Tones, Visual, baseline,
    count, cut, day, fit, flow, level, line_items, measure, middle, number_width, numeral_slots,
    pane_name, repo, svg_size, tokens, visual_rows, whole,
};
use crate::metrics::{
    BAND_BOTTOM, BAND_GAP, BAND_LEAST, BAND_SIDE, BAND_TOP, CONFIDENCE_SIZE, DECK_TITLE_SIZE,
    LINE_HEIGHT, MONO_SIZE, NUMBER_SIZE, NUMBER_SIZE_TENS, PROSE_LEADING, PROSE_SIZE,
    PROSE_SPACING, RAIL, RAIL_LEADING, RAIL_SIZE, STEM, STEM_GAP, STRIP_GAP, STRIP_ROW, STRIP_SIDE,
    STRIP_SIZE, TITLE_SIZE,
};
use crate::text::{self, Weight};

/// A4 on its side, in the window's pixels.
const WIDTH: f32 = 1123.;
/// See [`WIDTH`].
const HEIGHT: f32 = 794.;
/// Points to the pixel.
const SCALE: f32 = 0.75;
/// The main column, left of the rail.
const MAIN: f32 = WIDTH - RAIL;
/// The strip's height: its text and its padding.
const STRIP_HIGH: f32 = 30.;
/// Where the strip starts.
const STRIP_TOP: f32 = HEIGHT - STRIP_HIGH;
/// A code row, as a pane measures it.
fn row_high() -> f32 {
    (MONO_SIZE * LINE_HEIGHT).round()
}
/// A pane's header: its padding and its line.
const PANE_HEAD: f32 = 33.;
/// The gutter before code: numerals, the bar, the number.
const GUTTER: f32 = 78.;

/// `document` as landscape pages: each group, then its argument.
pub(super) fn pages(document: &Document) -> Vec<Page> {
    let tones = Tones::of(document);
    // Each group's stops, to know the page numbers before writing a strip.
    let mut stops: Vec<(Kind, Vec<Item>, Vec<String>)> = Vec::new();
    if document.groups.is_empty() {
        stops.push((Kind::Cover, empty(document), Vec::new()));
    }
    for group in &document.groups {
        let (items, mut warnings, leftover) = group_page(document, group, &tones);
        let more = if leftover.is_empty() {
            Vec::new()
        } else {
            narration_pages(document, group, leftover)
        };
        let of = u32::try_from(more.len() + 1).unwrap_or(u32::MAX);
        if !more.is_empty() {
            warnings.push(format!(
                "group {}: its narration is longer than a landscape band holds; it continues \
                 on the page after",
                group.n
            ));
        }
        stops.push((
            Kind::Group {
                n: group.n,
                part: 1,
                of,
            },
            items,
            warnings,
        ));
        for (ix, items) in more.into_iter().enumerate() {
            stops.push((
                Kind::Group {
                    n: group.n,
                    part: u32::try_from(ix + 2).unwrap_or(u32::MAX),
                    of,
                },
                items,
                Vec::new(),
            ));
        }
        if !group.argument.is_empty() {
            let pages = argument_pages(document, Some(group), &group.argument);
            let of = u32::try_from(pages.len()).unwrap_or(u32::MAX);
            for (ix, items) in pages.into_iter().enumerate() {
                stops.push((
                    Kind::Argument {
                        n: group.n,
                        part: u32::try_from(ix + 1).unwrap_or(u32::MAX),
                        of,
                    },
                    items,
                    Vec::new(),
                ));
            }
        }
    }
    if !document.general.is_empty() {
        for items in argument_pages(document, None, &document.general) {
            stops.push((Kind::Said, items, Vec::new()));
        }
    }
    let first: Vec<(u32, usize)> = stops
        .iter()
        .enumerate()
        .filter_map(|(ix, (kind, _, _))| match kind {
            Kind::Group { n, part: 1, .. } => Some((*n, ix)),
            _ => None,
        })
        .collect();
    let argued: Vec<(u32, usize)> = stops
        .iter()
        .enumerate()
        .filter_map(|(ix, (kind, _, _))| match kind {
            Kind::Argument { n, part: 1, .. } => Some((*n, ix)),
            _ => None,
        })
        .collect();
    let total = stops.len();
    stops
        .into_iter()
        .enumerate()
        .map(|(ix, (kind, mut items, warnings))| {
            items.extend(side(document, kind, &first, &argued));
            items.extend(strip(document, kind, ix + 1, total));
            Page {
                width: WIDTH,
                height: HEIGHT,
                scale: SCALE,
                kind,
                items,
                warnings,
            }
        })
        .collect()
}

/// The one page of a deck with no groups to show: its title, and that.
fn empty(document: &Document) -> Vec<Item> {
    let inks = &document.inks;
    vec![
        Item::Ground { colour: inks.page },
        text_at(
            BAND_SIDE + STEM + STEM_GAP,
            300.,
            40.,
            &fit(&document.title, Face::Semibold, 28., MAIN - 120.),
            Face::Semibold,
            28.,
            inks.on_page.text,
        ),
        text_at(
            BAND_SIDE + STEM + STEM_GAP,
            344.,
            24.,
            "0 groups",
            Face::Regular,
            16.,
            inks.on_page.muted,
        ),
    ]
}

/// A line of text in a row `height` high whose top is `top`.
fn text_at(x: f32, top: f32, height: f32, text: &str, face: Face, size: f32, colour: Rgb) -> Item {
    Item::Text {
        x,
        baseline: baseline(top, height, face, size),
        text: text.to_string(),
        face,
        size,
        colour,
    }
}

/// The group's page: the band over the panes.
/// A group's page, and the narration the band could not hold.
type GroupPage = (Vec<Item>, Vec<String>, Vec<Vec<Placed>>);

fn group_page(document: &Document, group: &Group, tones: &Tones) -> GroupPage {
    let inks = &document.inks;
    let mut items = vec![Item::Ground { colour: inks.page }];
    let (band_high, leftover) = band(document, group, tones, &mut items);
    let top = band_high + 1.;
    let mut warnings = Vec::new();
    items.push(Item::Fill {
        rect: Rect::new(0., top, MAIN, STRIP_TOP - top),
        colour: inks.pane,
    });
    // The panes side by side, code a little wider, as the window gives it.
    let weights: Vec<f32> = group
        .panes
        .iter()
        .map(|pane| {
            if matches!(pane, Pane::Code(_)) {
                1.3
            } else {
                1.
            }
        })
        .collect();
    let all: f32 = weights.iter().sum();
    let room = MAIN - count(group.panes.len().saturating_sub(1));
    let mut x = 0.;
    for (ix, (pane, weight)) in group.panes.iter().zip(&weights).enumerate() {
        let w = (room * weight / all).floor();
        let w = if ix + 1 == group.panes.len() {
            MAIN - x
        } else {
            w
        };
        let at = Rect::new(x, top, w, STRIP_TOP - top);
        match pane {
            Pane::Code(code) => {
                if let Some(cut_off) = code_pane(document, pane, code, at, &mut items) {
                    warnings.push(format!(
                        "group {}: {} of {} did not fit its landscape pane; the portrait pages show all of it",
                        group.n,
                        if cut_off == 1 {
                            "1 line".to_string()
                        } else {
                            format!("{cut_off} lines")
                        },
                        code.name.clone().unwrap_or_else(|| code.file.clone())
                    ));
                }
            }
            Pane::Diagram(picture) => {
                header(document, pane, at, &mut items);
                let inner = Rect::new(
                    at.x + 18.,
                    at.y + PANE_HEAD + 4.,
                    at.w - 36.,
                    at.h - PANE_HEAD - 14.,
                );
                let (w, h) = svg_size(&picture.svg);
                let fit_scale = (inner.w / w).min(inner.h / h).min(1.);
                // Never so small its labels cannot be read; past that it is
                // cut at the pane's edge, and the portrait pages show it all.
                let least = DIAGRAM_LEAST * 0.75 / SCALE;
                let scale = fit_scale.max(least);
                if fit_scale < least {
                    warnings.push(format!(
                        "group {}: diagram {} is too large for its landscape pane at a size its \
                         labels can be read; it is cut at the pane's edge, and the portrait \
                         pages show it whole",
                        group.n,
                        picture.name.clone().unwrap_or_else(|| picture.id.clone())
                    ));
                }
                items.push(Item::Svg {
                    rect: Rect::new(inner.x, inner.y, inner.w, (h * scale).min(inner.h)),
                    svg: picture.svg.clone(),
                    scale,
                    band: if h * scale > inner.h {
                        Some([0., inner.h / scale])
                    } else {
                        None
                    },
                });
                if !picture.marks.is_empty() {
                    let mut mx = at.x + 16.;
                    let my = inner.y + h * scale + 8.;
                    for mark in &picture.marks {
                        items.push(Item::Badge {
                            cx: mx + 7.5,
                            cy: my + 8.,
                            r: 7.5,
                            numeral: mark.numeral,
                            fill: inks.on_pane.accent,
                            ink: inks.pane,
                        });
                        mx += 19.;
                        items.push(text_at(
                            mx,
                            my,
                            16.,
                            &mark.block,
                            Face::Mono,
                            11.,
                            inks.on_pane.muted,
                        ));
                        mx += measure(&mark.block, Face::Mono, 11.) + 12.;
                    }
                }
            }
            Pane::Page(_) => {
                header(document, pane, at, &mut items);
                let lines = text::wrap(
                    "Interactive page: open the HTML export to use it",
                    13.,
                    Weight::Regular,
                    at.w - 64.,
                );
                let card_high = 20. * count(lines.len()) + 44.;
                let card = Rect::new(at.x + 16., at.y + PANE_HEAD + 8., at.w - 32., card_high);
                items.push(Item::Frame {
                    rect: card,
                    colour: inks.edge,
                });
                for (ix, line) in lines.iter().enumerate() {
                    items.push(text_at(
                        card.x + 16.,
                        card.y + 12. + 20. * count(ix),
                        20.,
                        line.trim_end(),
                        Face::Regular,
                        13.,
                        inks.on_pane.text,
                    ));
                }
                let (name, what, _) = pane_name(pane);
                let named = name.map_or(what.clone(), |name| format!("{name} · {what}"));
                items.push(text_at(
                    card.x + 16.,
                    card.y + 14. + 20. * count(lines.len()),
                    16.,
                    &fit(&named, Face::Regular, 11., card.w - 32.),
                    Face::Regular,
                    11.,
                    inks.on_pane.muted,
                ));
            }
        }
        x += w;
        if ix + 1 < group.panes.len() {
            items.push(Item::Fill {
                rect: Rect::new(x, top, 1., STRIP_TOP - top),
                colour: inks.edge,
            });
            x += 1.;
        }
    }
    (items, warnings, leftover)
}

/// The band, as the window draws it: the number, the deck's title over the
/// group's, the confidence, and the narration hung from the number's stem.
/// Returns its height: the window's least and most when the prose allows,
/// taller when it does not, since a page cannot scroll it.
fn band(
    document: &Document,
    group: &Group,
    tones: &Tones,
    items: &mut Vec<Item>,
) -> (f32, Vec<Vec<Placed>>) {
    let inks = &document.inks;
    let titles_x = BAND_SIDE + STEM + STEM_GAP;
    let prose_x = titles_x;
    let width = MAIN - BAND_SIDE - prose_x;
    let mut lines: Vec<Vec<Placed>> = group
        .prose
        .iter()
        .flat_map(|paragraph| {
            let mut lines = flow(&tokens(paragraph, document, tones), PROSE_SIZE, width);
            if let Some(last) = lines.last_mut() {
                // A marker for the space after a paragraph.
                last.push(Placed {
                    x: -1.,
                    width: 0.,
                    token: Token {
                        text: String::new(),
                        face: Face::Regular,
                        colour: inks.band,
                        ground: None,
                        numeral: None,
                    },
                });
            }
            lines
        })
        .collect();
    let head = 38.;
    let prose_top = BAND_TOP + head + BAND_GAP - 4.;
    // As much of the prose as the band holds at its tallest; the rest goes
    // on a page of its own, so the panes keep their room.
    let room = BAND_CAP - prose_top - BAND_BOTTOM;
    let mut used = 0.;
    let mut fits = 0;
    for line in &lines {
        let ends = line.last().is_some_and(|piece| piece.x < 0.);
        let step = PROSE_LEADING + if ends { PROSE_SPACING } else { 0. };
        if used + PROSE_LEADING > room {
            break;
        }
        used += step;
        fits += 1;
    }
    let leftover = lines.split_off(fits);
    let prose_high = used;
    let high = (prose_top + prose_high + BAND_BOTTOM)
        .clamp(BAND_LEAST, BAND_CAP)
        .round();
    items.push(Item::Fill {
        rect: Rect::new(0., 0., MAIN, high),
        colour: inks.band,
    });
    items.push(Item::Fill {
        rect: Rect::new(0., high, MAIN, 1.),
        colour: inks.edge,
    });
    // The number, and the deck's title over the group's.
    let size = if group.n >= 10 {
        NUMBER_SIZE_TENS
    } else {
        NUMBER_SIZE
    };
    let number = group.n.to_string();
    let number_width = measure(&number, Face::Regular, size);
    items.push(text_at(
        BAND_SIDE + (STEM - number_width) / 2.,
        BAND_TOP,
        36.,
        &number,
        Face::Regular,
        size,
        tones.number,
    ));
    items.push(text_at(
        titles_x,
        BAND_TOP,
        16.,
        &fit(
            &document.title,
            Face::Regular,
            DECK_TITLE_SIZE,
            MAIN - BAND_SIDE - titles_x,
        ),
        Face::Regular,
        DECK_TITLE_SIZE,
        inks.on_band.muted,
    ));
    let mut confidence_width = 0.;
    let mut confidence = Vec::new();
    if let Some(judged) = &group.confidence {
        let (name, ink) = level(document, judged.level);
        let mut words = name.to_string();
        if let Some(why) = &judged.why {
            words.push_str(" · ");
            words.push_str(why);
        }
        let words = fit(&words, Face::Regular, CONFIDENCE_SIZE, width * 0.45);
        confidence_width = measure(&words, Face::Regular, CONFIDENCE_SIZE) + 10.;
        confidence.push((words, ink));
    }
    let title = fit(
        &group.title,
        Face::Semibold,
        TITLE_SIZE,
        MAIN - BAND_SIDE - titles_x - confidence_width,
    );
    let title_width = measure(&title, Face::Semibold, TITLE_SIZE);
    items.push(text_at(
        titles_x,
        BAND_TOP + 18.,
        20.,
        &title,
        Face::Semibold,
        TITLE_SIZE,
        inks.on_band.text,
    ));
    for (words, ink) in confidence {
        items.push(text_at(
            titles_x + title_width + 10.,
            BAND_TOP + 18.,
            20.,
            &words,
            Face::Regular,
            CONFIDENCE_SIZE,
            ink,
        ));
    }
    // The prose, and the line it hangs from.
    items.push(Item::Fill {
        rect: Rect::new(
            BAND_SIDE + STEM / 2.,
            BAND_TOP + 40.,
            1.,
            prose_top + prose_high - BAND_TOP - 40.,
        ),
        colour: inks.edge.mix(inks.on_band.muted, 0.25),
    });
    let mut y = prose_top;
    for line in lines {
        let end_of_paragraph = line.last().is_some_and(|piece| piece.x < 0.);
        let pieces: Vec<_> = line.into_iter().filter(|piece| piece.x >= 0.).collect();
        items.extend(line_items(
            &pieces,
            prose_x,
            y,
            PROSE_LEADING,
            PROSE_SIZE,
            inks.band,
        ));
        y += PROSE_LEADING;
        if end_of_paragraph {
            y += PROSE_SPACING;
        }
    }
    (high, leftover)
}

/// The tallest the band grows on a page: under half of it, so the panes
/// under it always have their room.
const BAND_CAP: f32 = 357.;

/// Narration too long for the band, on pages of its own after the group's:
/// the band's ground and its measure, the prose from the top, headed as a
/// continuation.
fn narration_pages(document: &Document, group: &Group, lines: Vec<Vec<Placed>>) -> Vec<Vec<Item>> {
    let inks = &document.inks;
    let prose_x = BAND_SIDE + STEM + STEM_GAP;
    let top = BAND_TOP + 40.;
    let bottom = STRIP_TOP - BAND_BOTTOM;
    let mut pages: Vec<Vec<Item>> = Vec::new();
    let mut y = bottom;
    for line in lines {
        if y + PROSE_LEADING > bottom {
            pages.push(vec![
                Item::Ground { colour: inks.page },
                Item::Fill {
                    rect: Rect::new(0., 0., MAIN, STRIP_TOP),
                    colour: inks.band,
                },
            ]);
            y = top;
        }
        let page = pages.last_mut().expect("a page");
        let end_of_paragraph = line.last().is_some_and(|piece| piece.x < 0.);
        let pieces: Vec<_> = line.into_iter().filter(|piece| piece.x >= 0.).collect();
        page.extend(line_items(
            &pieces,
            prose_x,
            y,
            PROSE_LEADING,
            PROSE_SIZE,
            inks.band,
        ));
        y += PROSE_LEADING;
        if end_of_paragraph {
            y += PROSE_SPACING;
        }
    }
    let of = pages.len() + 1;
    for (ix, page) in pages.iter_mut().enumerate() {
        page.push(text_at(
            prose_x,
            BAND_TOP,
            20.,
            &fit(
                &format!(
                    "Group {}, continued ({} of {of}) · {}",
                    group.n,
                    ix + 2,
                    group.title
                ),
                Face::Medium,
                TITLE_SIZE,
                MAIN - BAND_SIDE - prose_x,
            ),
            Face::Medium,
            TITLE_SIZE,
            inks.on_band.muted,
        ));
    }
    pages
}

/// A pane's header: its name in the accent, then what it shows, and its
/// note under it.
fn header(document: &Document, pane: &Pane, at: Rect, items: &mut Vec<Item>) -> f32 {
    let inks = &document.inks;
    let (name, what, note) = pane_name(pane);
    let size = 11.5;
    let mut x = at.x + 16.;
    // The file and its range are kept whole; a long name gives way first.
    let room = at.w - 32. - measure(&what, Face::Mono, size) - 8.;
    if let Some(name) = name
        && room > measure("…", Face::Mono, size)
    {
        let name = fit(&name, Face::Mono, size, room);
        items.push(text_at(
            x,
            at.y + 12.,
            15.,
            &name,
            Face::Mono,
            size,
            inks.on_pane.accent,
        ));
        x += measure(&name, Face::Mono, size) + 8.;
    }
    let shown = fit(&what, Face::Mono, size, at.x + at.w - 16. - x);
    items.push(text_at(
        x,
        at.y + 12.,
        15.,
        &shown,
        Face::Mono,
        size,
        inks.on_pane.muted,
    ));
    if let Pane::Code(code) = pane
        && let Some(url) = super::links::source(document.repo.as_ref(), code)
    {
        items.push(Item::Url {
            rect: Rect::new(
                at.x + 16.,
                at.y + 8.,
                x + measure(&shown, Face::Mono, size) - at.x - 16.,
                23.,
            ),
            url,
        });
    }
    let mut high = PANE_HEAD;
    if let Some(note) = note {
        items.push(text_at(
            at.x + 16.,
            at.y + high - 2.,
            17.,
            &fit(&note, Face::Regular, 11., at.w - 32.),
            Face::Regular,
            11.,
            inks.on_pane.muted,
        ));
        high += 17.;
    }
    high
}

/// A code pane. Returns how many lines it had to leave out, if any.
fn code_pane(
    document: &Document,
    pane: &Pane,
    code: &deck_core::document::Code,
    at: Rect,
    items: &mut Vec<Item>,
) -> Option<usize> {
    let inks = &document.inks;
    let head = header(document, pane, at, items);
    let row = row_high();
    let top = at.y + head + 4.;
    let room = ((at.y + at.h - 10. - top) / row).floor().max(1.);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let room = room as usize;
    if code.missing {
        items.push(text_at(
            at.x + 16.,
            top,
            row,
            "Not in this deck: these lines were not packed with it.",
            Face::Regular,
            12.,
            inks.on_pane.muted,
        ));
        return None;
    }
    let advance = text::mono_advance(MONO_SIZE);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cols = ((at.w - GUTTER - 16.) / advance).floor().max(8.) as usize;
    let fitted = |context| -> Vec<Visual> {
        visual_rows(code, context)
            .iter()
            .flat_map(|row| cut(row, cols))
            .collect()
    };
    let mut rows = fitted(Context::Around(3));
    if rows.len() > room {
        rows = fitted(Context::Trimmed);
    }
    let mut left_out = None;
    if rows.len() > room {
        let lines = rows[room - 1..]
            .iter()
            .filter(|row| matches!(row, Visual::Code { no: Some(_), .. }))
            .count();
        rows.truncate(room - 1);
        left_out = Some(lines);
    }
    let digits = number_width(code);
    let mut y = top;
    for visual in &rows {
        code_row(document, visual, at, y, digits, items);
        y += row;
    }
    if let Some(lines) = left_out {
        items.push(text_at(
            at.x + GUTTER,
            y,
            row,
            &format!(
                "⋯ {lines} more {} · in the portrait export",
                if lines == 1 { "line" } else { "lines" }
            ),
            Face::Mono,
            11.,
            inks.on_pane.muted,
        ));
    }
    left_out
}

/// One row of a code pane at `y`.
fn code_row(
    document: &Document,
    visual: &Visual,
    at: Rect,
    y: f32,
    digits: usize,
    items: &mut Vec<Item>,
) {
    let inks = &document.inks;
    let row = row_high();
    let advance = text::mono_advance(MONO_SIZE);
    let text_x = at.x + GUTTER;
    match visual {
        Visual::Gap { lines } => items.push(text_at(
            text_x,
            y,
            row,
            &format!("⋯ {lines} {}", if *lines == 1 { "line" } else { "lines" }),
            Face::Mono,
            11.,
            inks.on_pane.muted,
        )),
        Visual::Preview {
            ground,
            indent,
            text,
            accent,
        } => {
            if *ground != inks.pane {
                items.push(Item::Fill {
                    rect: Rect::new(at.x, y, at.w, row),
                    colour: *ground,
                });
            }
            items.push(text_at(
                text_x + count(*indent) * advance,
                y,
                row,
                &fit(text, Face::Mono, 11., at.x + at.w - 16. - text_x),
                Face::Mono,
                11.,
                *accent,
            ));
        }
        Visual::Code {
            no,
            kind,
            ground,
            accent,
            lit,
            numerals,
            indent,
            runs,
        } => {
            if *ground != inks.pane {
                items.push(Item::Fill {
                    rect: Rect::new(at.x, y, at.w, row),
                    colour: *ground,
                });
            }
            let bar_x = at.x + 26. + 3.;
            match kind {
                RowKind::Going | RowKind::Arriving => {
                    let (sign, ink) = if *kind == RowKind::Going {
                        ("−", inks.going)
                    } else {
                        ("+", inks.arriving)
                    };
                    if no.is_some() {
                        items.push(text_at(at.x + 26., y, row, sign, Face::Mono, 12., ink));
                    }
                }
                RowKind::Spot => items.push(Item::Fill {
                    rect: Rect::new(bar_x, y, 3., row),
                    colour: if *lit { *accent } else { inks.on_pane.muted },
                }),
                RowKind::Context { .. } if *lit => items.push(Item::Fill {
                    rect: Rect::new(bar_x, y, 3., row),
                    colour: *accent,
                }),
                RowKind::Context { .. } => {}
            }
            // The first beside the code, the rest outward, all inside the
            // gutter however many start here.
            let slots = numeral_slots(numerals.len(), at.x + 2., at.x + 26., 7.5);
            for (numeral, (cx, r)) in numerals.iter().zip(slots) {
                items.push(Item::Badge {
                    cx,
                    cy: y + row / 2.,
                    r,
                    numeral: *numeral,
                    fill: *accent,
                    ink: *ground,
                });
            }
            match no {
                Some(no) => {
                    let number = no.to_string();
                    items.push(text_at(
                        at.x + 38. + count(digits.saturating_sub(number.len())) * advance,
                        y,
                        row,
                        &number,
                        Face::Mono,
                        MONO_SIZE,
                        if *lit { *accent } else { inks.on_pane.muted },
                    ));
                }
                None => items.push(text_at(
                    at.x + 66.,
                    y,
                    row,
                    if text::mono_covers('↪') { "↪" } else { ">" },
                    Face::Mono,
                    11.,
                    inks.on_pane.muted,
                )),
            }
            let mut x = text_x + count(*indent) * advance;
            for (text, ink, italic) in runs {
                if !text.trim().is_empty() {
                    let lead = text.chars().take_while(|ch| *ch == ' ').count();
                    items.push(text_at(
                        x + count(lead) * advance,
                        y,
                        row,
                        text[lead..].trim_end(),
                        if *italic {
                            Face::MonoItalic
                        } else {
                            Face::Mono
                        },
                        MONO_SIZE,
                        *ink,
                    ));
                }
                x += count(text.chars().count()) * advance;
            }
        }
    }
}

/// What was said, as pages of its own: a thin band, then the messages in two
/// columns, down the first and then the second, then a page more.
fn argument_pages(document: &Document, group: Option<&Group>, said: &[Message]) -> Vec<Vec<Item>> {
    let inks = &document.inks;
    let band_high = 60.;
    let top = band_high + 24.;
    let bottom = STRIP_TOP - 24.;
    let gap = 34.;
    let column = (MAIN - 60. - gap) / 2.;
    let lefts = [30., 30. + column + gap];
    let blocks: Vec<(f32, Vec<Item>)> = said
        .iter()
        .enumerate()
        .map(|(ix, message)| message_block(document, group, message, ix, said.len(), column))
        .collect();

    let mut pages: Vec<Vec<Item>> = Vec::new();
    let mut page: Vec<Item> = Vec::new();
    let mut col = 0;
    let mut y = top;
    let start = |page: &mut Vec<Item>| {
        page.push(Item::Ground { colour: inks.page });
        page.extend(thin_band(document, group, said.len(), band_high));
        page.push(Item::Fill {
            rect: Rect::new(lefts[1] - gap / 2., top, 1., bottom - top),
            colour: inks.edge,
        });
    };
    start(&mut page);
    // When everything fits on one page, the two columns are balanced, as
    // the HTML export's columns balance: the second starts at the message
    // that crosses half the height, rather than after the first is full.
    let total: f32 = blocks.iter().map(|(high, _)| high + 16.).sum();
    let balance = total <= 2. * (bottom - top);
    for (high, items) in blocks {
        let past_half = balance && col == 0 && y > top && (y - top) + high / 2. > total / 2.;
        if past_half {
            col = 1;
            y = top;
        } else if y + high > bottom && y > top {
            if col == 0 {
                col = 1;
            } else {
                pages.push(std::mem::take(&mut page));
                start(&mut page);
                col = 0;
            }
            y = top;
        }
        page.extend(items.into_iter().map(|item| shifted(&item, lefts[col], y)));
        y += high + 16.;
    }
    pages.push(page);
    pages
}

/// `item` moved across by `dx` and down by `dy`.
fn shifted(item: &Item, dx: f32, dy: f32) -> Item {
    let mut item = item.clone();
    match &mut item {
        Item::Fill { rect, .. }
        | Item::Chip { rect, .. }
        | Item::Frame { rect, .. }
        | Item::Svg { rect, .. }
        | Item::Dots { rect, .. }
        | Item::Url { rect, .. }
        | Item::Link { rect, .. } => {
            rect.x += dx;
            rect.y += dy;
        }
        Item::Rule { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
        Item::Text { x, baseline, .. } => {
            *x += dx;
            *baseline += dy;
        }
        Item::Badge { cx, cy, .. } | Item::Dot { cx, cy, .. } => {
            *cx += dx;
            *cy += dy;
        }
        Item::Ground { .. } => {}
    }
    item
}

/// The band over an argument: the group's number, its title, and how much
/// was said.
fn thin_band(document: &Document, group: Option<&Group>, said: usize, high: f32) -> Vec<Item> {
    let inks = &document.inks;
    let tones = Tones::of(document);
    let mut items = vec![
        Item::Fill {
            rect: Rect::new(0., 0., MAIN, high),
            colour: inks.band,
        },
        Item::Fill {
            rect: Rect::new(0., high, MAIN, 1.),
            colour: inks.edge,
        },
    ];
    let count_words = format!("{said} said");
    let count_width = measure(&count_words, Face::Mono, 11.);
    items.push(text_at(
        MAIN - BAND_SIDE - count_width,
        BAND_TOP,
        30.,
        &count_words,
        Face::Mono,
        11.,
        inks.on_band.muted,
    ));
    let mut x = BAND_SIDE;
    let (title, what) = match group {
        Some(group) => {
            items.push(text_at(
                x,
                BAND_TOP,
                30.,
                &group.n.to_string(),
                Face::Regular,
                30.,
                tones.number,
            ));
            x += STEM + STEM_GAP;
            (group.title.as_str(), " — the argument")
        }
        None => (document.title.as_str(), " — what was said about the deck"),
    };
    let what_width = measure(what, Face::Regular, TITLE_SIZE);
    let title = fit(
        title,
        Face::Semibold,
        TITLE_SIZE,
        MAIN - BAND_SIDE - count_width - 16. - x - what_width,
    );
    let title_width = measure(&title, Face::Semibold, TITLE_SIZE);
    items.push(text_at(
        x,
        BAND_TOP,
        30.,
        &title,
        Face::Semibold,
        TITLE_SIZE,
        inks.on_band.text,
    ));
    items.push(text_at(
        x + title_width,
        BAND_TOP,
        30.,
        what,
        Face::Regular,
        TITLE_SIZE,
        inks.on_band.muted,
    ));
    items
}

/// One message, `width` wide, placed from its own top-left: its height and
/// its items.
fn message_block(
    document: &Document,
    group: Option<&Group>,
    message: &Message,
    ix: usize,
    all: usize,
    width: f32,
) -> (f32, Vec<Item>) {
    let inks = &document.inks;
    let mut items = Vec::new();
    let (who, filled) = match message.author {
        Author::Reader => ("Reader", true),
        Author::Agent => ("Agent", false),
    };
    let mut about = who.to_string();
    if message.author == Author::Reader
        && let Some(kind) = message.kind
    {
        about.push_str(match kind {
            Remark::MustFix => " · must fix",
            Remark::Question => " · question",
            Remark::Nit => " · nit",
        });
    }
    match (&message.pane, message.range) {
        (Some(pane), Some(range)) if range.first == range.last => {
            about.push_str(&format!(" · {pane}, line {}", range.first));
        }
        (Some(pane), Some(range)) => {
            about.push_str(&format!(" · {pane}, lines {}–{}", range.first, range.last));
        }
        (Some(pane), None) => about.push_str(&format!(" · {pane}")),
        (None, _) => {}
    }
    let place = format!("{}/{all}", ix + 1);
    let x = 18.;
    items.push(Item::Dot {
        cx: 4.,
        cy: 8.,
        r: 4.,
        colour: inks.on_page.accent,
        filled,
    });
    let place_width = measure(&place, Face::Mono, 10.5);
    items.push(text_at(
        x,
        0.,
        16.,
        &fit(&about, Face::Regular, 10.5, width - x - place_width - 12.),
        Face::Regular,
        10.5,
        inks.on_page.muted,
    ));
    items.push(text_at(
        width - place_width,
        0.,
        16.,
        &place,
        Face::Mono,
        10.5,
        inks.on_page.muted,
    ));
    let mut y = 18.;
    for paragraph in message.text.split('\n') {
        for words in text::wrap(paragraph, 13., Weight::Regular, width - x) {
            items.push(text_at(
                x,
                y,
                20.,
                words.trim_end(),
                Face::Regular,
                13.,
                inks.on_page.text,
            ));
            y += 20.;
        }
    }
    // The line it was about, as the group printed it.
    let quoted = quote(group, message);
    if !quoted.is_empty() {
        y += 6.;
        let row = 19.;
        let advance = text::mono_advance(11.5);
        for visual in &quoted {
            let Visual::Code {
                no,
                ground,
                accent,
                runs,
                indent,
                ..
            } = visual
            else {
                continue;
            };
            items.push(Item::Fill {
                rect: Rect::new(x, y, width - x, row),
                colour: *ground,
            });
            items.push(Item::Fill {
                rect: Rect::new(x, y, 3., row),
                colour: *accent,
            });
            if let Some(no) = no {
                items.push(text_at(
                    x + 10.,
                    y,
                    row,
                    &no.to_string(),
                    Face::Mono,
                    11.5,
                    inks.on_pane.muted,
                ));
            }
            let mut cx = x + 44. + count(*indent) * advance;
            for (text, ink, italic) in runs {
                if !text.trim().is_empty() {
                    let lead = text.chars().take_while(|ch| *ch == ' ').count();
                    items.push(text_at(
                        cx + count(lead) * advance,
                        y,
                        row,
                        text[lead..].trim_end(),
                        if *italic {
                            Face::MonoItalic
                        } else {
                            Face::Mono
                        },
                        11.5,
                        *ink,
                    ));
                }
                cx += count(text.chars().count()) * advance;
            }
            y += row;
        }
    } else if !message.quote.trim().is_empty() {
        y += 6.;
        items.push(Item::Fill {
            rect: Rect::new(x, y, width - x, 19.),
            colour: inks.pane,
        });
        items.push(text_at(
            x + 10.,
            y,
            19.,
            &fit(message.quote.trim(), Face::Mono, 11.5, width - x - 20.),
            Face::Mono,
            11.5,
            inks.on_pane.text,
        ));
        y += 19.;
    }
    (y, items)
}

/// The rows a remark is about, cut to a column: the half of a change that is
/// in the file.
fn quote(group: Option<&Group>, message: &Message) -> Vec<Visual> {
    let (Some(group), Some(id), Some(range)) = (group, &message.ref_id, message.range) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for pane in &group.panes {
        let Pane::Code(code) = pane else { continue };
        if &code.id != id {
            continue;
        }
        let other = if code.change == Some(Change::Made) {
            RowKind::Going
        } else {
            RowKind::Arriving
        };
        for line in code.lines.iter().filter(
            |line| matches!(line, Line::Code(row) if range.contains(row.no) && row.kind != other),
        ) {
            if let Line::Code(row) = line {
                let mut row = row.clone();
                if matches!(row.kind, RowKind::Context { .. }) {
                    row.kind = RowKind::Spot;
                }
                for visual in whole(&row) {
                    out.extend(cut(&visual, 44));
                }
            }
            if out.len() >= 3 {
                break;
            }
        }
    }
    out.retain(|row| matches!(row, Visual::Code { .. }));
    out
}

/// The rail down the side: the contents, this stop's group lit, each linked
/// to its page, and its argument under it.
fn side(
    document: &Document,
    kind: Kind,
    first: &[(u32, usize)],
    argued: &[(u32, usize)],
) -> Vec<Item> {
    let inks = &document.inks;
    let here = match kind {
        Kind::Group { n, .. } | Kind::Argument { n, .. } => Some(n),
        Kind::Cover | Kind::Said => None,
    };
    let x = MAIN + 22.;
    let width = RAIL - 44.;
    let mut items = vec![
        Item::Fill {
            rect: Rect::new(MAIN, 0., RAIL, STRIP_TOP),
            colour: inks.band,
        },
        Item::Fill {
            rect: Rect::new(MAIN, 0., 1., STRIP_TOP),
            colour: inks.edge,
        },
        text_at(
            x,
            26.,
            14.,
            "CONTENTS",
            Face::Medium,
            11.,
            inks.on_band.muted,
        ),
    ];
    if let Some(n) = here
        && let Some(at) = document.groups.iter().position(|g| g.n == n)
    {
        let words = format!("{} OF {}", at + 1, document.groups.len());
        items.push(text_at(
            MAIN + RAIL - 22. - measure(&words, Face::Medium, 11.),
            26.,
            14.,
            &words,
            Face::Medium,
            11.,
            inks.on_band.muted,
        ));
    }
    let mut y = 26. + 14. + 14.;
    for group in &document.groups {
        let on = here == Some(group.n);
        let ink = if on {
            inks.on_band.text
        } else {
            inks.on_band.muted
        };
        let start = y;
        items.push(text_at(
            x,
            y,
            RAIL_LEADING,
            &group.n.to_string(),
            Face::Medium,
            RAIL_SIZE,
            if on { inks.on_band.accent } else { ink },
        ));
        let lines = text::wrap(&group.title, RAIL_SIZE, Weight::Regular, width - 24.);
        for line in lines.iter().take(2) {
            items.push(text_at(
                x + 24.,
                y,
                RAIL_LEADING,
                line.trim_end(),
                Face::Regular,
                RAIL_SIZE,
                ink,
            ));
            y += RAIL_LEADING;
        }
        if let Some(&(_, page)) = first.iter().find(|(n, _)| *n == group.n) {
            items.push(Item::Link {
                rect: Rect::new(x, start, width, y - start),
                page,
            });
        }
        if let Some(&(_, page)) = argued.iter().find(|(n, _)| *n == group.n) {
            let lit = matches!(kind, Kind::Argument { n, .. } if n == group.n);
            let words = format!("the argument · {}", group.argument.len());
            items.push(text_at(
                x + 24.,
                y,
                16.,
                &words,
                Face::Regular,
                11.,
                if lit {
                    inks.on_band.accent
                } else {
                    inks.on_band.muted
                },
            ));
            items.push(Item::Link {
                rect: Rect::new(x + 24., y, width - 24., 16.),
                page,
            });
            y += 16.;
        }
        y += 9.;
        if y > STRIP_TOP - 40. {
            break;
        }
    }
    items
}

/// The strip along the bottom: who made it, where this is, which code it is
/// about and when, and the page.
fn strip(document: &Document, kind: Kind, page: usize, total: usize) -> Vec<Item> {
    let inks = &document.inks;
    let ink = inks.on_band.muted;
    let mut items = vec![
        Item::Fill {
            rect: Rect::new(0., STRIP_TOP, WIDTH, STRIP_HIGH),
            colour: inks.band,
        },
        Item::Fill {
            rect: Rect::new(0., STRIP_TOP, WIDTH, 1.),
            colour: inks.edge,
        },
        Item::Fill {
            rect: Rect::new(STRIP_SIDE, STRIP_TOP + 10.5, 9., 9.),
            colour: inks.on_band.accent,
        },
    ];
    let row = |x: f32, words: &str| {
        text_at(
            x,
            STRIP_TOP + STRIP_ROW,
            STRIP_HIGH - 2. * STRIP_ROW,
            words,
            Face::Mono,
            STRIP_SIZE,
            ink,
        )
    };
    let mut x = STRIP_SIDE + 15.;
    let brand = "Powered by Deck";
    items.push(row(x, brand));
    x += measure(brand, Face::Mono, STRIP_SIZE);
    items.push(Item::Url {
        rect: Rect::new(STRIP_SIDE, STRIP_TOP, x - STRIP_SIDE, STRIP_HIGH),
        url: super::links::DECK.to_string(),
    });
    x += STRIP_GAP;
    let of = if super::layout::complete(document) {
        format!("/{}", document.groups.len())
    } else {
        String::new()
    };
    let place = match kind {
        Kind::Group { n, .. } => format!("group {n}{of}"),
        Kind::Argument { n, .. } => format!("group {n}{of} · the argument"),
        Kind::Said | Kind::Cover => "the deck".to_string(),
    };
    items.push(row(x, &place));
    x += measure(&place, Face::Mono, STRIP_SIZE) + STRIP_GAP;
    // The right side, from the right.
    // A long repo is cut in the middle, as the portrait footer cuts it, so
    // both of its ends still say which one it is.
    let right: Vec<String> = [
        repo(document).map(|repo| middle(&repo, Face::Mono, STRIP_SIZE, 260.)),
        day(&document.created),
    ]
    .into_iter()
    .flatten()
    .chain(std::iter::once(format!("page {page} of {total}")))
    .collect();
    let mut rx = WIDTH - STRIP_SIDE;
    for words in right.iter().rev() {
        let w = measure(words, Face::Mono, STRIP_SIZE);
        rx -= w;
        items.push(row(rx, words));
        rx -= STRIP_GAP;
    }
    let title = fit(&document.title, Face::Mono, STRIP_SIZE, rx - x);
    items.push(row(x, &title));
    items
}
