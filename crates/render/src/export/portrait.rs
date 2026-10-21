//! Portrait pages: A4 upright, a cover, then each group stacked down a page
//! of its own, on one spacing scale.
//!
//! Every distance is a step of four points — 4, 8, 12, 16, 24, 32, 48 — and
//! every box sits on that grid, measured from the page's top-left corner. A4
//! is 595 × 842 points, neither a multiple of four, so the grid runs out two
//! points short at the right and the bottom: the column is 48 to 548, and the
//! footer sits on 800 to 824, eighteen points above the edge rather than the
//! sixteen a grid from the bottom would give. One scale everywhere, rather
//! than the mockups' values chosen one at a time, is what the margins and
//! padding were missing.
//!
//! The rules a group's pages keep:
//!
//! - **One group to a page, and at most one more.** Two groups never share a
//!   page. A group that does not fit on two first loses context — down to
//!   the lines that carry its evidence and one either side — and only then
//!   runs on, every page headed "Group n, continued (k of m)", and the
//!   export says so. Nothing it shows is ever dropped.
//! - **Panes in the order the prose reaches them**, as the document has them,
//!   each breaking between rows if it must, its header repeated.
//! - **The argument after its group**, before the foot.
//! - **The foot fills what is left** on a group's last page, tier by tier
//!   while each fits: the hand-off to the next group, what the code held, the
//!   deck's map with this group lit, and a field to write in.

use deck_core::document::{Author, Change, Document, Group, Line, Message, Pane, RowKind};
use deck_core::protocol::Kind as Remark;
use deck_core::theme::Rgb;

use super::layout::{
    Context, DIAGRAM_LEAST, Face, Item, Kind, Page, Rect, Tones, Visual, baseline, count, cut, day,
    fit, flow, level, line_items, measure, middle, number_width, numeral_slots, pane_name, repo,
    svg_size, tokens, up4, visual_rows,
};
use crate::text::{self, Weight};

/// The page, in points.
const WIDTH: f32 = 595.;
/// See [`WIDTH`].
const HEIGHT: f32 = 842.;
/// The side and top margins.
const MARGIN: f32 = 48.;
/// The text column's edges.
const LEFT: f32 = MARGIN;
/// See [`LEFT`].
const RIGHT: f32 = 548.;
/// The column's width.
const COLUMN: f32 = RIGHT - LEFT;
/// Where content stops: 32 above the footer's rule.
const BOTTOM: f32 = 768.;
/// The footer's rule, and its 24 points.
const FOOTER: f32 = 800.;
/// See [`FOOTER`].
const FOOTER_HIGH: f32 = 24.;
/// Where a group's continued pages start: under their heading.
const CONTINUED_TOP: f32 = MARGIN + 24. + 16.;

/// Padding inside the band.
const BAND_PAD: f32 = 16.;
/// A pane's inset across and down.
const PANE_X: f32 = 12.;
/// See [`PANE_X`].
const PANE_Y: f32 = 8.;
/// A pane's header row.
const PANE_HEAD: f32 = 20.;
/// Between panes, and from the prose to the first.
const BETWEEN: f32 = 16.;

/// The group's title.
const TITLE: (f32, f32) = (18., 24.);
/// The narration.
const PROSE: (f32, f32) = (10., 16.);
/// Code.
const CODE: (f32, f32) = (8., 12.);
/// Labels, the confidence, small print.
const SMALL: (f32, f32) = (8., 12.);
/// The footer's line.
const FOOT_TYPE: (f32, f32) = (7.5, 12.);

/// A margin numeral's disc, and the gap from its right edge to the column.
const NUMERAL_R: f32 = 5.;
/// See [`NUMERAL_R`].
const NUMERAL_GAP: f32 = 12.;

/// One row of a block: its height, and what is on it, placed from the row's
/// top.
#[derive(Debug, Clone, Default)]
struct Row {
    height: f32,
    items: Vec<Item>,
}

impl Row {
    fn new(height: f32) -> Self {
        Self {
            height,
            items: Vec::new(),
        }
    }

    fn with(height: f32, items: Vec<Item>) -> Self {
        Self { height, items }
    }
}

/// A run of rows that is laid out as one thing: the band, a pane, a remark.
/// It may break between rows; `head` is repeated at the top of each part.
#[derive(Debug, Clone, Default)]
struct Block {
    /// Space before it, unless it starts a page.
    gap: f32,
    /// Its ground, across the column, with `pad` inside it top and bottom.
    fill: Option<Rgb>,
    pad: f32,
    head: Vec<Row>,
    rows: Vec<Row>,
    /// Start on a page of its own, unless it starts one already: a diagram
    /// cut across pages, which reads best from the top of one.
    fresh: bool,
    /// What the person exporting should hear about it.
    warning: Option<String>,
}

/// `item` moved down by `dy`.
fn down(item: &Item, dy: f32) -> Item {
    let mut item = item.clone();
    match &mut item {
        Item::Fill { rect, .. }
        | Item::Chip { rect, .. }
        | Item::Frame { rect, .. }
        | Item::Svg { rect, .. }
        | Item::Dots { rect, .. }
        | Item::Url { rect, .. }
        | Item::Link { rect, .. } => rect.y += dy,
        Item::Rule { y, .. } => *y += dy,
        Item::Text { baseline, .. } => *baseline += dy,
        Item::Badge { cy, .. } | Item::Dot { cy, .. } => *cy += dy,
        Item::Ground { .. } => {}
    }
    item
}

/// A group's content, broken into pages: what is on each, and where the
/// last one's content ends.
fn paginate(blocks: &[Block]) -> (Vec<Vec<Item>>, f32) {
    let mut pages: Vec<Vec<Item>> = vec![Vec::new()];
    let mut top = MARGIN;
    let mut y = top;
    for block in blocks {
        let mut from = 0;
        if block.fresh && (y - top).abs() > f32::EPSILON {
            pages.push(Vec::new());
            top = CONTINUED_TOP;
            y = top;
        }
        loop {
            let start = if (y - top).abs() < f32::EPSILON {
                y
            } else {
                y + block.gap
            };
            let head: f32 = block.head.iter().map(|row| row.height).sum();
            let frame = 2. * block.pad + head;
            let mut used = frame;
            let mut took = 0;
            for row in &block.rows[from..] {
                if start + used + row.height > BOTTOM {
                    break;
                }
                used += row.height;
                took += 1;
            }
            let empty_page = (y - top).abs() < f32::EPSILON;
            if took == 0 && from < block.rows.len() {
                if empty_page {
                    // Taller than a page on its own: it goes anyway, as
                    // nothing that would fit on a page is ever this tall.
                    used += block.rows[from].height;
                    took = 1;
                } else {
                    pages.push(Vec::new());
                    top = CONTINUED_TOP;
                    y = top;
                    continue;
                }
            }
            let page = pages.last_mut().expect("a page");
            if let Some(fill) = block.fill {
                page.push(Item::Fill {
                    rect: Rect::new(LEFT, start, COLUMN, used),
                    colour: fill,
                });
            }
            let mut at = start + block.pad;
            for row in block.head.iter().chain(&block.rows[from..from + took]) {
                page.extend(row.items.iter().map(|item| down(item, at)));
                at += row.height;
            }
            y = start + used;
            from += took;
            if from >= block.rows.len() {
                break;
            }
            pages.push(Vec::new());
            top = CONTINUED_TOP;
            y = top;
        }
    }
    (pages, y)
}

/// `document` as portrait pages: the cover, then each group.
pub(super) fn pages(document: &Document) -> Vec<Page> {
    let tones = Tones::of(document);
    // Each group's pages, the evidence trimmed only where two pages are not
    // enough.
    let mut laid: Vec<(Vec<Vec<Item>>, f32)> = Vec::new();
    let mut said_of: Vec<Vec<String>> = Vec::new();
    for group in &document.groups {
        let try_with = |context| {
            let blocks = group_blocks(document, group, &tones, context);
            let warnings: Vec<String> = blocks.iter().filter_map(|b| b.warning.clone()).collect();
            (paginate(&blocks), warnings)
        };
        let (mut pages, mut warnings) = try_with(Context::Around(3));
        if pages.0.len() > 2 {
            (pages, warnings) = try_with(Context::Trimmed);
        }
        laid.push(pages);
        said_of.push(warnings);
    }
    let said = (!document.general.is_empty()).then(|| paginate(&said_blocks(document)));

    // Where each group starts, after however many pages the contents take.
    let covers = cover(document, &laid, &vec![0; laid.len()]).len();
    let mut first: Vec<usize> = Vec::new();
    let mut at = covers;
    for (pages, _) in &laid {
        first.push(at);
        at += pages.len();
    }
    let total = at + said.as_ref().map_or(0, |(pages, _)| pages.len());

    let mut out: Vec<Page> = cover(document, &laid, &first)
        .into_iter()
        .map(|items| Page {
            width: WIDTH,
            height: HEIGHT,
            scale: 1.,
            kind: Kind::Cover,
            items,
            warnings: Vec::new(),
        })
        .collect();
    for (ix, ((group, (pages, end)), blocked)) in
        document.groups.iter().zip(laid).zip(said_of).enumerate()
    {
        let of = u32::try_from(pages.len()).unwrap_or(u32::MAX);
        let last = pages.len() - 1;
        for (part, mut items) in pages.into_iter().enumerate() {
            let part32 = u32::try_from(part + 1).unwrap_or(u32::MAX);
            if part > 0 {
                items.extend(continued(document, group, part32, of));
            }
            if part == last {
                items.extend(foot(document, group, ix, end, &first));
            }
            // Past the one extra page a group may take: said once, on its
            // first page, so the person exporting knows to look.
            let mut warnings = Vec::new();
            if part == 0 {
                if of > 2 {
                    warnings.push(format!(
                        "group {} needs {of} pages even with its context trimmed; its pages \
                         after the first are headed \"continued\"",
                        group.n
                    ));
                }
                warnings.extend(blocked.iter().cloned());
            }
            out.push(Page {
                width: WIDTH,
                height: HEIGHT,
                scale: 1.,
                kind: Kind::Group {
                    n: group.n,
                    part: part32,
                    of,
                },
                items,
                warnings,
            });
        }
    }
    if let Some((pages, _)) = said {
        for items in pages {
            out.push(Page {
                width: WIDTH,
                height: HEIGHT,
                scale: 1.,
                kind: Kind::Said,
                items,
                warnings: Vec::new(),
            });
        }
    }
    for (ix, page) in out.iter_mut().enumerate() {
        let footer = footer(document, page.kind, ix + 1, total);
        page.items.insert(
            0,
            Item::Ground {
                colour: document.inks.page,
            },
        );
        page.items.extend(footer);
    }
    out
}

/// The heading of a group's second page and on.
fn continued(document: &Document, group: &Group, part: u32, of: u32) -> Vec<Item> {
    let (size, _) = PROSE;
    let words = format!("Group {}, continued ({part} of {of})", group.n);
    vec![Item::Text {
        x: LEFT,
        baseline: baseline(MARGIN, 24., Face::Medium, size),
        text: fit(&words, Face::Medium, size, COLUMN),
        face: Face::Medium,
        size,
        colour: document.inks.on_page.muted,
    }]
}

/// A plain line of text in a row `height` high at `x`.
fn text_at(x: f32, height: f32, text: &str, face: Face, size: f32, colour: Rgb) -> Item {
    Item::Text {
        x,
        baseline: baseline(0., height, face, size),
        text: text.to_string(),
        face,
        size,
        colour,
    }
}

/// `text` set right-aligned to end at `right`.
fn text_right(right: f32, height: f32, text: &str, face: Face, size: f32, colour: Rgb) -> Item {
    text_at(
        right - measure(text, face, size),
        height,
        text,
        face,
        size,
        colour,
    )
}

/// The band, the panes, and the argument, as blocks.
fn group_blocks(document: &Document, group: &Group, tones: &Tones, context: Context) -> Vec<Block> {
    let mut blocks = vec![band(document, group, tones)];
    for pane in &group.panes {
        let mut block = match pane {
            Pane::Code(code) => code_block(document, pane, code, context),
            Pane::Diagram(picture) => diagram_block(document, group, pane, picture),
            Pane::Page(_) => page_card(document, pane),
        };
        block.gap = BETWEEN;
        blocks.push(block);
    }
    blocks.extend(argument_blocks(document, group, &group.argument));
    blocks
}

/// The band: the group's number and title, its confidence, its narration.
fn band(document: &Document, group: &Group, tones: &Tones) -> Block {
    let inks = &document.inks;
    let inner = LEFT + BAND_PAD;
    let (size, line) = TITLE;
    let number = group.n.to_string();
    let number_width = measure(&number, Face::Semibold, size);
    let text_x = inner + number_width + 12.;
    let width = RIGHT - BAND_PAD - text_x;
    let mut rows = Vec::new();
    for (ix, title) in text::wrap(&group.title, size, Weight::Semibold, width)
        .iter()
        .enumerate()
    {
        let mut items = vec![text_at(
            text_x,
            line,
            title.trim_end(),
            Face::Semibold,
            size,
            inks.on_band.text,
        )];
        if ix == 0 {
            items.push(text_at(
                inner,
                line,
                &number,
                Face::Semibold,
                size,
                tones.number,
            ));
        }
        rows.push(Row::with(line, items));
    }
    if let Some(confidence) = &group.confidence {
        let (name, ink) = level(document, confidence.level);
        let (size, line) = SMALL;
        let name_width = measure(name, Face::Medium, size);
        let mut items = vec![text_at(text_x, line, name, Face::Medium, size, ink)];
        if let Some(why) = &confidence.why {
            let why = fit(
                &format!(" · {why}"),
                Face::Regular,
                size,
                width - name_width,
            );
            items.push(text_at(
                text_x + name_width,
                line,
                &why,
                Face::Regular,
                size,
                inks.on_band.muted,
            ));
        }
        rows.push(Row::with(line, items));
    }
    rows.push(Row::new(8.));
    let (size, line) = PROSE;
    for (ix, paragraph) in group.prose.iter().enumerate() {
        if ix > 0 {
            rows.push(Row::new(8.));
        }
        for placed in flow(&tokens(paragraph, document, tones), size, width) {
            rows.push(Row::with(
                line,
                line_items(&placed, text_x, 0., line, size, inks.band),
            ));
        }
    }
    Block {
        gap: 0.,
        fill: Some(inks.band),
        pad: BAND_PAD,
        head: Vec::new(),
        rows,
        ..Block::default()
    }
}

/// A pane's header: its name in the accent, then what it shows.
fn pane_head(document: &Document, pane: &Pane) -> Vec<Row> {
    let inks = &document.inks;
    let (name, what, note) = pane_name(pane);
    let (size, line) = SMALL;
    let x = LEFT + PANE_X;
    let mut items = Vec::new();
    let mut at = x;
    // The file and its range are kept whole; a long name gives way first.
    let room = RIGHT - PANE_X - x - measure(&what, Face::Mono, size) - 8.;
    if let Some(name) = name
        && room > measure("…", Face::Mono, size)
    {
        let name = fit(&name, Face::Mono, size, room);
        items.push(text_at(
            at,
            PANE_HEAD,
            &name,
            Face::Mono,
            size,
            inks.on_pane.accent,
        ));
        at += measure(&name, Face::Mono, size) + 8.;
    }
    items.push(text_at(
        at,
        PANE_HEAD,
        &fit(&what, Face::Mono, size, RIGHT - PANE_X - at),
        Face::Mono,
        size,
        inks.on_pane.muted,
    ));
    // The file and its range go to those lines on the repo's host, at the
    // packed commit, where there is an honest link to make.
    if let Pane::Code(code) = pane
        && let Some(url) = super::links::source(document.repo.as_ref(), code)
    {
        let shown = fit(&what, Face::Mono, size, RIGHT - PANE_X - at);
        let w = up4(measure(&shown, Face::Mono, size)).min(RIGHT - PANE_X - x);
        items.push(Item::Url {
            rect: Rect::new(x, 0., up4(at - x + w).min(COLUMN - 2. * PANE_X), PANE_HEAD),
            url,
        });
    }
    let mut rows = vec![Row::with(PANE_HEAD, items)];
    if let Some(note) = note {
        rows.push(Row::with(
            line,
            vec![text_at(
                x,
                line,
                &fit(&note, Face::Regular, size, COLUMN - 2. * PANE_X),
                Face::Regular,
                size,
                inks.on_pane.muted,
            )],
        ));
    }
    rows
}

/// The code a pane prints, row by row, with its numerals out in the margin.
fn code_block(
    document: &Document,
    pane: &Pane,
    code: &deck_core::document::Code,
    context: Context,
) -> Block {
    let inks = &document.inks;
    let head = pane_head(document, pane);
    if code.missing {
        let (size, line) = PROSE;
        return Block {
            gap: BETWEEN,
            fill: Some(inks.pane),
            pad: PANE_Y,
            head,
            rows: vec![Row::with(
                line,
                vec![text_at(
                    LEFT + PANE_X,
                    line,
                    "Not in this deck: these lines were not packed with it.",
                    Face::Regular,
                    size,
                    inks.on_pane.muted,
                )],
            )],
            ..Block::default()
        };
    }
    let rows = code_rows(
        document,
        &visual_rows(code, context),
        number_width(code),
        LEFT,
        COLUMN,
        true,
    );
    Block {
        gap: BETWEEN,
        fill: Some(inks.pane),
        pad: PANE_Y,
        head,
        rows,
        ..Block::default()
    }
}

/// The columns a row of code has, `width` wide with a number `digits` wide.
fn code_cols(width: f32, digits: usize) -> usize {
    let advance = text::mono_advance(CODE.0);
    let text = width - 2. * PANE_X - count(digits + 3) * advance;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cols = (text / advance).floor().max(8.) as usize;
    cols
}

/// Rows of code as rows of a block, across `left` to `left + width`. With
/// `margin`, numerals go out in the page's margin; a quote has none.
fn code_rows(
    document: &Document,
    visual: &[Visual],
    digits: usize,
    left: f32,
    width: f32,
    margin: bool,
) -> Vec<Row> {
    let inks = &document.inks;
    let (size, line) = CODE;
    let advance = text::mono_advance(size);
    let cols = code_cols(width, digits);
    let text_x = left + PANE_X + count(digits + 3) * advance;
    // Re-wrapped here, at this pane's width: the rows were chosen without
    // one.
    let mut out = Vec::new();
    for row in visual {
        match row {
            Visual::Gap { lines } => out.push(Row::with(
                line,
                vec![text_at(
                    text_x,
                    line,
                    &format!("⋯ {lines} {}", if *lines == 1 { "line" } else { "lines" }),
                    Face::Mono,
                    size,
                    inks.on_pane.muted,
                )],
            )),
            Visual::Preview {
                ground,
                indent,
                text,
                accent,
            } => {
                let mut items = Vec::new();
                if *ground != inks.pane {
                    items.push(Item::Fill {
                        rect: Rect::new(left, 0., width, line),
                        colour: *ground,
                    });
                }
                items.push(text_at(
                    text_x + count(*indent) * advance,
                    line,
                    &fit(text, Face::Mono, size, left + width - PANE_X - text_x),
                    Face::Mono,
                    size,
                    *accent,
                ));
                out.push(Row::with(line, items));
            }
            Visual::Code { .. } => {
                for piece in cut(row, cols) {
                    out.push(code_row(document, &piece, digits, left, width, margin));
                }
            }
        }
    }
    out
}

/// The mark a continuation row carries in the sign column.
fn continuation() -> &'static str {
    if text::mono_covers('↪') { "↪" } else { ">" }
}

/// One row of code.
fn code_row(
    document: &Document,
    row: &Visual,
    digits: usize,
    left: f32,
    width: f32,
    margin: bool,
) -> Row {
    let inks = &document.inks;
    let (size, line) = CODE;
    let advance = text::mono_advance(size);
    let Visual::Code {
        no,
        kind,
        ground,
        accent,
        lit,
        numerals,
        indent,
        runs,
    } = row
    else {
        return Row::new(line);
    };
    let mut items = Vec::new();
    if *ground != inks.pane {
        items.push(Item::Fill {
            rect: Rect::new(left, 0., width, line),
            colour: *ground,
        });
    }
    if *lit {
        items.push(Item::Fill {
            rect: Rect::new(left, 0., 4., line),
            colour: *accent,
        });
    }
    if margin {
        // Out in the margin, the first twelve points short of the column
        // and the rest outward from it, never past the page's edge.
        let slots = numeral_slots(numerals.len(), 4., LEFT - NUMERAL_GAP, NUMERAL_R);
        for (numeral, (cx, r)) in numerals.iter().zip(slots) {
            items.push(Item::Badge {
                cx,
                cy: line / 2.,
                r,
                numeral: *numeral,
                fill: document.inks.on_page.accent,
                ink: document.inks.page,
            });
        }
    }
    let number_x = left + PANE_X;
    let sign_x = number_x + count(digits + 1) * advance;
    let text_x = sign_x + count(2) * advance;
    match no {
        Some(no) => {
            let number = no.to_string();
            items.push(text_at(
                number_x + count(digits - number.len().min(digits)) * advance,
                line,
                &number,
                Face::Mono,
                size,
                if *lit { *accent } else { inks.on_pane.muted },
            ));
            let sign = match kind {
                RowKind::Going => Some(("−", inks.going)),
                RowKind::Arriving => Some(("+", inks.arriving)),
                _ => None,
            };
            if let Some((sign, ink)) = sign {
                items.push(text_at(sign_x, line, sign, Face::Mono, size, ink));
            }
        }
        None => items.push(text_at(
            sign_x,
            line,
            continuation(),
            Face::Mono,
            size,
            inks.on_pane.muted,
        )),
    }
    let mut x = text_x + count(*indent) * advance;
    for (text, ink, italic) in runs {
        let face = if *italic {
            Face::MonoItalic
        } else {
            Face::Mono
        };
        if !text.trim().is_empty() {
            let lead = text.chars().take_while(|ch| *ch == ' ').count();
            items.push(text_at(
                x + count(lead) * advance,
                line,
                text[lead..].trim_end(),
                face,
                size,
                *ink,
            ));
        }
        x += count(text.chars().count()) * advance;
    }
    Row::with(line, items)
}

/// The widest a diagram can be drawn: the column inside a pane.
const DIAGRAM_WIDE: f32 = COLUMN - 2. * PANE_X;

/// The tallest one page holds of a diagram: a page under a continued page's
/// heading, less the pane's header, its insets and a mark that it goes on.
const DIAGRAM_HIGH: f32 = BOTTOM - CONTINUED_TOP - PANE_HEAD - 2. * PANE_Y - 12.;

/// A diagram at the size deck draws it — a pixel being three quarters of a
/// point, as on any page — smaller only to fit, and never so small its
/// labels go under six points ([`DIAGRAM_LEAST`]). One that would need
/// smaller starts a page of its own and is cut across pages in bands of the
/// page's height, each saying it continues.
fn diagram_block(
    document: &Document,
    group: &Group,
    pane: &Pane,
    picture: &deck_core::document::Picture,
) -> Block {
    let inks = &document.inks;
    let (w, h) = svg_size(&picture.svg);
    let (w, h) = (w * 0.75, h * 0.75);
    let named = picture.name.clone().unwrap_or_else(|| picture.id.clone());
    let mut warning = None;
    let wide = (DIAGRAM_WIDE / w).min(1.);
    let mut scale = wide.min(DIAGRAM_HIGH / h);
    if scale < DIAGRAM_LEAST {
        scale = wide.max(DIAGRAM_LEAST);
    }
    if w * scale > DIAGRAM_WIDE + 0.5 {
        warning = Some(format!(
            "group {}: diagram {named} is wider than a page at a size its labels can be read; \
             it is cut at the page's edge",
            group.n
        ));
    }
    let drawn = h * scale;
    let mut rows = Vec::new();
    let fresh = drawn > DIAGRAM_HIGH;
    if fresh {
        // In bands of a page's height, from the top.
        let band = (DIAGRAM_HIGH / 4.).floor() * 4.;
        let per = band / (0.75 * scale);
        let natural = h / 0.75;
        let mut from = 0.;
        let mut bands = 0;
        while from < natural {
            let to = (from + per).min(natural);
            rows.push(Row::with(
                up4((to - from) * 0.75 * scale),
                vec![Item::Svg {
                    rect: Rect::new(
                        LEFT + PANE_X,
                        0.,
                        DIAGRAM_WIDE,
                        up4((to - from) * 0.75 * scale),
                    ),
                    svg: picture.svg.clone(),
                    scale: 0.75 * scale,
                    band: Some([from, to]),
                }],
            ));
            if to < natural {
                let (size, line) = SMALL;
                rows.push(Row::with(
                    line,
                    vec![text_at(
                        LEFT + PANE_X,
                        line,
                        "↓ the diagram continues on the next page",
                        Face::Regular,
                        size,
                        inks.on_pane.muted,
                    )],
                ));
            }
            from = to;
            bands += 1;
        }
        warning = Some(format!(
            "group {}: diagram {named} is too tall for a page at a size its labels can be \
             read; it is cut across {bands} pages",
            group.n
        ));
    } else {
        let high = up4(drawn);
        rows.push(Row::with(
            high,
            vec![Item::Svg {
                rect: Rect::new(LEFT + PANE_X, 0., DIAGRAM_WIDE, high),
                svg: picture.svg.clone(),
                scale: 0.75 * scale,
                band: None,
            }],
        ));
    }
    if !picture.marks.is_empty() {
        let (size, line) = SMALL;
        let mut items = Vec::new();
        let mut x = LEFT + PANE_X;
        for mark in &picture.marks {
            items.push(Item::Badge {
                cx: x + NUMERAL_R,
                cy: line / 2.,
                r: NUMERAL_R,
                numeral: mark.numeral,
                fill: inks.on_pane.accent,
                ink: inks.pane,
            });
            x += 2. * NUMERAL_R + 4.;
            items.push(text_at(
                x,
                line,
                &mark.block,
                Face::Mono,
                size,
                inks.on_pane.muted,
            ));
            x += measure(&mark.block, Face::Mono, size) + 12.;
        }
        rows.push(Row::new(8.));
        rows.push(Row::with(line, items));
    }
    Block {
        gap: BETWEEN,
        fill: Some(inks.pane),
        pad: PANE_Y,
        head: pane_head(document, pane),
        rows,
        fresh,
        warning,
    }
}

/// A page pane, which a PDF cannot run: a card that says where it can be.
fn page_card(document: &Document, pane: &Pane) -> Block {
    let inks = &document.inks;
    let (size, line) = PROSE;
    let (small, small_line) = SMALL;
    let x = LEFT + PANE_X;
    let card = Rect::new(x, 0., COLUMN - 2. * PANE_X, 40.);
    let (name, what, _) = pane_name(pane);
    let named = match name {
        Some(name) => format!("{name} · {what}"),
        None => what,
    };
    let items = vec![
        Item::Frame {
            rect: card,
            colour: inks.edge,
        },
        Item::Text {
            x: x + 12.,
            baseline: baseline(4., line, Face::Regular, size),
            text: "Interactive page: open the HTML export to use it".to_string(),
            face: Face::Regular,
            size,
            colour: inks.on_pane.text,
        },
        Item::Text {
            x: x + 12.,
            baseline: baseline(4. + line, small_line, Face::Regular, small),
            text: fit(&named, Face::Regular, small, card.w - 24.),
            face: Face::Regular,
            size: small,
            colour: inks.on_pane.muted,
        },
    ];
    Block {
        gap: BETWEEN,
        fill: Some(inks.pane),
        pad: PANE_Y,
        head: pane_head(document, pane),
        rows: vec![Row::with(40., items)],
        ..Block::default()
    }
}

/// What was said about a group: a heading, then each remark and answer, a
/// remark on a line carrying the line.
fn argument_blocks(document: &Document, group: &Group, said: &[Message]) -> Vec<Block> {
    if said.is_empty() {
        return Vec::new();
    }
    let inks = &document.inks;
    let (size, line) = SMALL;
    let mut blocks = vec![Block {
        gap: 24.,
        rows: vec![Row::with(
            line,
            vec![text_at(
                LEFT,
                line,
                &format!("THE ARGUMENT · {}", said.len()),
                Face::Medium,
                size,
                inks.on_page.muted,
            )],
        )],
        ..Block::default()
    }];
    for (ix, message) in said.iter().enumerate() {
        blocks.push(message_block(
            document,
            Some(group),
            message,
            ix,
            said.len(),
        ));
    }
    blocks
}

/// One remark or answer.
fn message_block(
    document: &Document,
    group: Option<&Group>,
    message: &Message,
    ix: usize,
    all: usize,
) -> Block {
    let inks = &document.inks;
    let (small, small_line) = SMALL;
    let (size, line) = PROSE;
    let x = LEFT + 16.;
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
    let mut rows = vec![Row::with(
        small_line,
        vec![
            Item::Dot {
                cx: LEFT + 4.,
                cy: small_line / 2.,
                r: 3.,
                colour: inks.on_page.accent,
                filled,
            },
            text_at(
                x,
                small_line,
                &fit(
                    &about,
                    Face::Regular,
                    small,
                    RIGHT - x - measure(&place, Face::Mono, small) - 12.,
                ),
                Face::Regular,
                small,
                inks.on_page.muted,
            ),
            text_right(
                RIGHT,
                small_line,
                &place,
                Face::Mono,
                small,
                inks.on_page.muted,
            ),
        ],
    )];
    for paragraph in message.text.split('\n') {
        for words in text::wrap(paragraph, size, Weight::Regular, RIGHT - x) {
            rows.push(Row::with(
                line,
                vec![text_at(
                    x,
                    line,
                    words.trim_end(),
                    Face::Regular,
                    size,
                    inks.on_page.text,
                )],
            ));
        }
    }
    let quoted = quote(group, message);
    if !quoted.is_empty() {
        rows.push(Row::new(4.));
        let digits = quoted
            .iter()
            .filter_map(|row| match row {
                Visual::Code { no: Some(no), .. } => Some(no.to_string().len()),
                _ => None,
            })
            .max()
            .unwrap_or(1);
        rows.extend(code_rows(document, &quoted, digits, x, RIGHT - x, false));
    } else if !message.quote.trim().is_empty() {
        let (size, line) = CODE;
        rows.push(Row::new(4.));
        rows.push(Row::with(
            line,
            vec![
                Item::Fill {
                    rect: Rect::new(x, 0., RIGHT - x, line),
                    colour: inks.pane,
                },
                text_at(
                    x + PANE_X,
                    line,
                    &fit(
                        message.quote.trim(),
                        Face::Mono,
                        size,
                        RIGHT - x - 2. * PANE_X,
                    ),
                    Face::Mono,
                    size,
                    inks.on_pane.text,
                ),
            ],
        ));
    }
    Block {
        gap: 12.,
        rows,
        ..Block::default()
    }
}

/// The rows of the lines a remark was about, as the group printed them: the
/// half of a change that is in the file.
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
        let mut lines: Vec<Line> = code
            .lines
            .iter()
            .filter(|line| {
                matches!(line, Line::Code(row) if range.contains(row.no) && row.kind != other)
            })
            .take(3)
            .cloned()
            .collect();
        // Context rows lose their fade: quoted, they are the subject.
        for line in &mut lines {
            if let Line::Code(row) = line
                && matches!(row.kind, RowKind::Context { .. })
            {
                row.kind = RowKind::Spot;
            }
        }
        let shown = deck_core::document::Code {
            lines,
            ..code.clone()
        };
        out.extend(visual_rows(&shown, Context::Around(0)));
    }
    out.retain(|row| !matches!(row, Visual::Gap { .. }));
    out
}

/// What was said about the deck as a whole, on its own pages at the end.
fn said_blocks(document: &Document) -> Vec<Block> {
    let inks = &document.inks;
    let (size, line) = TITLE;
    let mut blocks = vec![Block {
        gap: 0.,
        rows: vec![Row::with(
            line,
            vec![text_at(
                LEFT,
                line,
                "What was said about the deck",
                Face::Semibold,
                size,
                inks.on_page.text,
            )],
        )],
        ..Block::default()
    }];
    let all = document.general.len();
    for (ix, message) in document.general.iter().enumerate() {
        let mut block = message_block(document, None, message, ix, all);
        if ix == 0 {
            block.gap = 16.;
        }
        blocks.push(block);
    }
    blocks
}

/// The foot on a group's last page: whatever of the hand-off, the values,
/// the map and a field for notes fits under `end`, in that order.
fn foot(document: &Document, group: &Group, ix: usize, end: f32, first: &[usize]) -> Vec<Item> {
    let inks = &document.inks;
    let rule = up4(end) + 24.;
    let mut y = rule + 16.;
    if y + 16. > BOTTOM {
        return Vec::new();
    }
    let mut out = vec![Item::Rule {
        x: LEFT,
        y: rule,
        w: COLUMN,
        colour: inks.edge,
    }];
    let (small, small_line) = SMALL;
    let mut tiers: Vec<Vec<Row>> = Vec::new();

    // The hand-off.
    if let Some(next) = document.groups.get(ix + 1) {
        let (size, line) = PROSE;
        let page = format!("p.{}", first[ix + 1] + 1);
        let page_width = measure(&page, Face::Mono, small);
        let arrow = "Next →";
        let arrow_width = measure(arrow, Face::Medium, size) + 8.;
        let room = COLUMN - page_width - arrow_width - 8.;
        let title = fit(
            &format!("{} · {}", next.n, next.title),
            Face::Medium,
            size,
            room.min(COLUMN * 0.7),
        );
        let title_width = measure(&title, Face::Medium, size);
        let start = RIGHT - page_width - 8. - title_width - arrow_width;
        tiers.push(vec![Row::with(
            line,
            vec![
                text_at(start, line, arrow, Face::Medium, size, inks.on_page.accent),
                text_at(
                    start + arrow_width,
                    line,
                    &title,
                    Face::Medium,
                    size,
                    inks.on_page.text,
                ),
                text_right(RIGHT, line, &page, Face::Mono, small, inks.on_page.muted),
                Item::Link {
                    rect: Rect::new(LEFT, 0., COLUMN, line),
                    page: first[ix + 1],
                },
            ],
        )]);
    }
    // What the code held.
    if !group.ledger.is_empty() {
        let mut rows = vec![Row::with(
            small_line,
            vec![text_at(
                LEFT,
                small_line,
                "WHAT THE CODE HELD",
                Face::Medium,
                small,
                inks.on_page.muted,
            )],
        )];
        for held in &group.ledger {
            let at = format!("{}:{}", held.pane, held.line);
            rows.push(Row::with(
                small_line,
                vec![
                    text_at(
                        LEFT,
                        small_line,
                        &fit(&held.at, Face::Mono, small, COLUMN * 0.4),
                        Face::Mono,
                        small,
                        inks.on_page.text,
                    ),
                    text_at(
                        LEFT + COLUMN * 0.4,
                        small_line,
                        &fit(&held.is, Face::Mono, small, COLUMN * 0.4),
                        Face::Mono,
                        small,
                        inks.on_page.accent,
                    ),
                    text_right(
                        RIGHT,
                        small_line,
                        &at,
                        Face::Mono,
                        small,
                        inks.on_page.muted,
                    ),
                ],
            ));
        }
        tiers.push(rows);
    }
    // Where this sits.
    {
        let mut rows = vec![Row::with(
            small_line,
            vec![text_at(
                LEFT,
                small_line,
                "WHERE THIS SITS",
                Face::Medium,
                small,
                inks.on_page.muted,
            )],
        )];
        for other in &document.groups {
            let here = other.n == group.n;
            let ink = if here {
                inks.on_page.text
            } else {
                inks.on_page.muted
            };
            rows.push(Row::with(
                small_line,
                vec![
                    text_at(
                        LEFT,
                        small_line,
                        &other.n.to_string(),
                        Face::Medium,
                        small,
                        if here { inks.on_page.accent } else { ink },
                    ),
                    text_at(
                        LEFT + 16.,
                        small_line,
                        &fit(&other.title, Face::Regular, small, COLUMN - 16.),
                        Face::Regular,
                        small,
                        ink,
                    ),
                ],
            ));
        }
        tiers.push(rows);
    }
    for tier in tiers {
        let high: f32 = tier.iter().map(|row| row.height).sum();
        if y + high > BOTTOM {
            break;
        }
        for row in &tier {
            out.extend(row.items.iter().map(|item| down(item, y)));
            y += row.height;
        }
        y += 16.;
    }
    // A field to write in, with whatever room is left.
    if y + small_line + 4. + 48. <= BOTTOM {
        out.push(down(
            &text_at(
                LEFT,
                small_line,
                &format!("YOUR NOTES · GROUP {}", group.n),
                Face::Medium,
                small,
                inks.on_page.muted,
            ),
            y,
        ));
        let top = y + small_line + 4.;
        out.push(Item::Dots {
            rect: Rect::new(LEFT, top, COLUMN, BOTTOM - top),
            colour: inks.edge,
            pitch: 12.,
        });
    }
    out
}

/// The footer every page carries, on one line that never wraps: who made
/// it, the repo and the day, where this page is.
fn footer(document: &Document, kind: Kind, page: usize, total: usize) -> Vec<Item> {
    let inks = &document.inks;
    let (size, _) = FOOT_TYPE;
    let ink = inks.on_page.muted;
    let brand = "Powered by Deck";
    let brand_x = LEFT + 12.;
    let left_end = brand_x + measure(brand, Face::Regular, size);
    let here = match kind {
        Kind::Cover => "contents".to_string(),
        Kind::Group { n, .. } | Kind::Argument { n, .. } => {
            let title = document
                .groups
                .iter()
                .find(|group| group.n == n)
                .map(|group| group.title.as_str())
                .unwrap_or_default();
            format!("group {n} · {title}")
        }
        Kind::Said => "what was said".to_string(),
    };
    let place = format!(" · {page}/{total}");
    let repo_words = repo(document).unwrap_or_default();
    let date_words = day(&document.created).unwrap_or_default();
    // The right side keeps its page; the title gives way first, then the
    // repo is cut in the middle, so both of its ends still say which it is.
    let room = COLUMN - (left_end - LEFT) - 32.;
    let place_width = measure(&place, Face::Regular, size);
    let date_part = if date_words.is_empty() {
        String::new()
    } else if repo_words.is_empty() {
        date_words.clone()
    } else {
        format!(" · {date_words}")
    };
    let date_width = measure(&date_part, Face::Regular, size);
    let repo_room = (room * 0.45 - date_width).max(24.);
    let repo_short = middle(&repo_words, Face::Regular, size, repo_room);
    let mid = format!("{repo_short}{date_part}");
    let mid_width = measure(&mid, Face::Regular, size);
    let right_room = (room - mid_width - place_width).max(24.);
    let right = format!("{}{place}", fit(&here, Face::Regular, size, right_room));
    let right_width = measure(&right, Face::Regular, size);
    let gap_left = left_end + 16.;
    let gap_right = RIGHT - right_width - 16.;
    let mid_x = gap_left + ((gap_right - gap_left - mid_width) / 2.).max(0.);
    let top = FOOTER;
    vec![
        Item::Rule {
            x: LEFT,
            y: top,
            w: COLUMN,
            colour: inks.edge,
        },
        Item::Fill {
            rect: Rect::new(LEFT, top + 8., 8., 8.),
            colour: inks.on_page.accent,
        },
        Item::Text {
            x: brand_x,
            baseline: baseline(top, FOOTER_HIGH, Face::Regular, size),
            text: brand.to_string(),
            face: Face::Regular,
            size,
            colour: ink,
        },
        Item::Url {
            rect: Rect::new(LEFT, top, up4(left_end - LEFT), FOOTER_HIGH),
            url: super::links::DECK.to_string(),
        },
        Item::Text {
            x: mid_x,
            baseline: baseline(top, FOOTER_HIGH, Face::Regular, size),
            text: mid,
            face: Face::Regular,
            size,
            colour: ink,
        },
        Item::Text {
            x: RIGHT - right_width,
            baseline: baseline(top, FOOTER_HIGH, Face::Regular, size),
            text: right,
            face: Face::Regular,
            size,
            colour: ink,
        },
    ]
}

/// The cover: the deck's title, a line on what follows, the contents, and a
/// bar of how many pages each group takes, centred down the page with no
/// header and nothing about who or when — when it all fits on one. A deck
/// with more groups than a page holds runs its contents on, from the top,
/// onto pages headed "Contents, continued".
fn cover(document: &Document, laid: &[(Vec<Vec<Item>>, f32)], first: &[usize]) -> Vec<Vec<Item>> {
    let inks = &document.inks;
    let mut rows: Vec<Row> = Vec::new();
    let (size, line) = (28., 32.);
    for title in text::wrap(&document.title, size, Weight::Semibold, COLUMN) {
        rows.push(Row::with(
            line,
            vec![text_at(
                LEFT,
                line,
                title.trim_end(),
                Face::Semibold,
                size,
                inks.on_page.text,
            )],
        ));
    }
    rows.push(Row::new(8.));
    let groups = document.groups.len();
    let remarks: usize = document
        .contents
        .iter()
        .map(|e| e.remarks + e.answers)
        .sum();
    let mut intro = format!(
        "{} {}, one to a page.",
        groups,
        if groups == 1 { "group" } else { "groups" }
    );
    if remarks > 0 {
        intro.push_str(" What was said about each follows it.");
    }
    let (size, line) = (12., 18.);
    let intro_lines = text::wrap(&intro, size, Weight::Regular, COLUMN);
    let intro_high = up4(count(intro_lines.len()) * line);
    let mut items = Vec::new();
    for (ix, words) in intro_lines.iter().enumerate() {
        items.push(Item::Text {
            x: LEFT,
            baseline: baseline(count(ix) * line, line, Face::Regular, size),
            text: words.trim_end().to_string(),
            face: Face::Regular,
            size,
            colour: inks.on_page.muted,
        });
    }
    rows.push(Row::with(intro_high, items));
    rows.push(Row::new(24.));

    let (small, _) = SMALL;
    for (ix, group) in document.groups.iter().enumerate() {
        rows.push(contents_row(document, group, first[ix] + 1, first[ix]));
    }
    rows.push(Row::with(
        0.,
        vec![Item::Rule {
            x: LEFT,
            y: 0.,
            w: COLUMN,
            colour: inks.edge,
        }],
    ));
    rows.push(Row::new(16.));

    // How many pages each group takes, the contents first. Past the point
    // where a page is narrower than one step of the scale, the bar would be
    // a smear, and the caption says it alone.
    let covers = first.first().copied().unwrap_or(1).max(1);
    let pages: Vec<usize> = std::iter::once(covers)
        .chain(laid.iter().map(|(pages, _)| pages.len()))
        .collect();
    let total: usize = pages.iter().sum();
    let unit = ((COLUMN + 4.) / count(total) / 4.).floor() * 4.;
    if unit >= 4. {
        let mut bar = Vec::new();
        let mut x = LEFT;
        for (ix, n) in pages.iter().enumerate() {
            let w = count(*n) * unit - 4.;
            bar.push(Item::Fill {
                rect: Rect::new(x, 0., w.max(4.), 8.),
                colour: if ix == 0 {
                    inks.edge
                } else {
                    inks.page.mix(inks.on_page.accent, 0.55)
                },
            });
            x += count(*n) * unit;
        }
        rows.push(Row::with(8., bar));
        rows.push(Row::new(4.));
    }
    let caption = format!(
        "pages: the contents, then {}",
        laid.iter()
            .map(|(pages, _)| pages.len().to_string())
            .collect::<Vec<_>>()
            .join(" · ")
    );
    rows.push(Row::with(
        12.,
        vec![text_at(
            LEFT,
            12.,
            &fit(&caption, Face::Regular, small, COLUMN),
            Face::Regular,
            small,
            inks.on_page.muted,
        )],
    ));

    let high: f32 = rows.iter().map(|row| row.height).sum();
    if high <= BOTTOM - MARGIN {
        let top = MARGIN + ((BOTTOM - MARGIN - high) / 2. / 4.).floor() * 4.;
        let mut out = Vec::new();
        let mut y = top;
        for row in rows {
            out.extend(row.items.iter().map(|item| down(item, y)));
            y += row.height;
        }
        return vec![out];
    }
    // From the top, page after page.
    let mut pages: Vec<Vec<Item>> = vec![Vec::new()];
    let mut y = MARGIN;
    for row in rows {
        if y + row.height > BOTTOM {
            let mut page = Vec::new();
            let (size, _) = PROSE;
            page.push(text_at(
                LEFT,
                24.,
                "Contents, continued",
                Face::Medium,
                size,
                inks.on_page.muted,
            ));
            pages.push(page.into_iter().map(|item| down(&item, MARGIN)).collect());
            y = CONTINUED_TOP;
        }
        let page = pages.last_mut().expect("a page");
        page.extend(row.items.iter().map(|item| down(item, y)));
        y += row.height;
    }
    pages
}

/// One group in the contents: its number and title, its confidence and
/// basis, its panes, what was said about it, and the page it starts on.
fn contents_row(document: &Document, group: &Group, page: usize, index: usize) -> Row {
    let inks = &document.inks;
    let line = 20.;
    let (size, _) = PROSE;
    let (small, _) = SMALL;
    let entry = document.contents.iter().find(|entry| entry.n == group.n);
    let mut items = vec![
        Item::Rule {
            x: LEFT,
            y: 0.,
            w: COLUMN,
            colour: inks.edge,
        },
        text_at(
            LEFT,
            line,
            &group.n.to_string(),
            Face::Semibold,
            size,
            inks.page.mix(inks.on_page.accent, 0.6),
        ),
        Item::Link {
            rect: Rect::new(LEFT, 0., COLUMN, line),
            page: index,
        },
    ];
    // From the right: the page, the count, the panes, the confidence; the
    // title has what is left.
    let mut right = RIGHT;
    let page_words = format!("p.{page}");
    items.push(text_right(
        right,
        line,
        &page_words,
        Face::Mono,
        small,
        inks.on_page.muted,
    ));
    right -= measure(&page_words, Face::Mono, small) + 12.;
    if let Some(entry) = entry {
        let said = entry.remarks + entry.answers;
        if said > 0 {
            let words = format!("{said} said");
            items.push(text_right(
                right,
                line,
                &words,
                Face::Regular,
                small,
                inks.on_page.muted,
            ));
            right -= measure(&words, Face::Regular, small) + 12.;
        }
        for pane in entry.panes.iter().rev().take(3) {
            let chip = fit(pane, Face::Mono, small, 64.);
            let w = measure(&chip, Face::Mono, small) + 8.;
            if right - w < LEFT + 200. {
                break;
            }
            items.push(Item::Chip {
                rect: Rect::new(right - w, 4., w, 12.),
                colour: inks.page.mix(inks.on_page.accent, 0.14),
            });
            items.push(text_at(
                right - w + 4.,
                line,
                &chip,
                Face::Mono,
                small,
                inks.on_page.accent,
            ));
            right -= w + 4.;
        }
    }
    let title_x = LEFT + 24.;
    if let Some(confidence) = &group.confidence {
        let (name, ink) = level(document, confidence.level);
        let words = match &confidence.why {
            Some(why) => format!("{name} · {why}"),
            None => name.to_string(),
        };
        let room = (right - 8. - title_x) * 0.45;
        let words = fit(&words, Face::Regular, small, room);
        let w = measure(&words, Face::Regular, small);
        items.push(text_at(
            right - 4. - w,
            line,
            &words,
            Face::Regular,
            small,
            ink,
        ));
        right -= w + 16.;
    }
    items.push(text_at(
        title_x,
        line,
        &fit(&group.title, Face::Semibold, size, right - title_x),
        Face::Semibold,
        size,
        inks.on_page.text,
    ));
    Row::with(line, items)
}
