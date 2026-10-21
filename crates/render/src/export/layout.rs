//! A document as pages of placed boxes: the part of a paged export that
//! decides where everything goes, and nothing about how it is drawn.
//!
//! A PDF is written from what this hands back and from nothing else, so every
//! decision about a page — what fits, where a pane breaks, which tier of the
//! foot gets the room that is left — is made once, here, by a pure function a
//! test can read. It measures with deck's own metrics: Plex shaped as the
//! window shapes it ([`crate::text`]) for words, JetBrains Mono's single
//! advance for code, and a diagram at the size deck draws it.
//!
//! Two shapes of page, [`Format::Portrait`] and [`Format::Landscape`], in
//! [`super::portrait`] and [`super::landscape`]. What they share — how a
//! paragraph of marked words breaks, and how a pane's rows are chosen and
//! wrapped — is here.

use deck_core::document::{Code, Document, Line, Marked, Pane, Row, RowKind, Run};
use deck_core::protocol::Level;
use deck_core::theme::{LEGIBLE, Rgb, legible};
use serde::Serialize;

use crate::text::{self, Weight};

/// The shape of the pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// A4 upright, in points: a cover, then each group stacked down its own
    /// page or two, on one spacing scale.
    Portrait,
    /// A4 on its side, as the window: a group to a page, its argument on the
    /// page after it.
    Landscape,
}

/// A box, by its top-left corner and its size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// A box at `x`, `y`, `w` wide and `h` high.
    #[must_use]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
}

/// Which face a run of text is set in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Face {
    /// Plex 400: the narration.
    Regular,
    /// Plex 500: labels that need a little more.
    Medium,
    /// Plex 600: titles, and what the narration says with weight.
    Semibold,
    /// JetBrains Mono 400: code.
    Mono,
    /// JetBrains Mono 400 italic: code the highlighter leans, comments.
    MonoItalic,
}

impl Face {
    fn weight(self) -> Option<Weight> {
        match self {
            Self::Regular => Some(Weight::Regular),
            Self::Medium => Some(Weight::Medium),
            Self::Semibold => Some(Weight::Semibold),
            Self::Mono | Self::MonoItalic => None,
        }
    }
}

/// One thing drawn on a page.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "item", rename_all = "lowercase")]
pub enum Item {
    /// The whole page, painted: the first thing on every page.
    Ground {
        /// In what.
        colour: Rgb,
    },
    /// A filled box.
    Fill {
        /// Where.
        rect: Rect,
        /// In what.
        colour: Rgb,
    },
    /// The ground behind a chip of text: placed by the text it is behind,
    /// not on any grid.
    Chip {
        /// Where.
        rect: Rect,
        /// In what.
        colour: Rgb,
    },
    /// A box's edge, as a hairline.
    Frame {
        /// Where.
        rect: Rect,
        /// In what.
        colour: Rgb,
    },
    /// A hairline across, `w` long from `x` at height `y`.
    Rule {
        /// Where it starts.
        x: f32,
        /// How far down.
        y: f32,
        /// How long.
        w: f32,
        /// In what.
        colour: Rgb,
    },
    /// One run of text in one face: real text, which a reader can select
    /// and search.
    Text {
        /// Its left edge.
        x: f32,
        /// The line it sits on.
        baseline: f32,
        /// The words.
        text: String,
        /// The face.
        face: Face,
        /// Its size.
        size: f32,
        /// Its ink.
        colour: Rgb,
    },
    /// A point's numeral, in a disc.
    Badge {
        /// The disc's centre, across.
        cx: f32,
        /// And down.
        cy: f32,
        /// Its radius.
        r: f32,
        /// The numeral.
        numeral: u32,
        /// The disc.
        fill: Rgb,
        /// The numeral's ink.
        ink: Rgb,
    },
    /// Who said something: the reader's dot filled, the agent's a ring.
    Dot {
        /// Its centre, across.
        cx: f32,
        /// And down.
        cy: f32,
        /// Its radius.
        r: f32,
        /// Its ink.
        colour: Rgb,
        /// Filled, or a ring.
        filled: bool,
    },
    /// A diagram, drawn at `scale` of its own size, centred in `rect`.
    Svg {
        /// The room it is centred in, and clipped to.
        rect: Rect,
        /// The drawing, as deck writes it.
        svg: String,
        /// Units of the page to one of the drawing's.
        scale: f32,
        /// The band of the drawing this shows, top and bottom in its own
        /// units, when it is too tall for one page and is cut across pages;
        /// `None` for all of it.
        #[serde(skip_serializing_if = "Option::is_none")]
        band: Option<[f32; 2]>,
    },
    /// A field of dots to write in.
    Dots {
        /// The field.
        rect: Rect,
        /// The dots' ink.
        colour: Rgb,
        /// How far apart.
        pitch: f32,
    },
    /// A part of the page that opens `url` when it is pressed: deck's
    /// site, or a pane's lines on the repo's host.
    Url {
        /// What can be pressed.
        rect: Rect,
        /// Where it goes.
        url: String,
    },
    /// A part of the page that goes to another page when it is pressed.
    Link {
        /// What can be pressed.
        rect: Rect,
        /// The page it goes to, from zero.
        page: usize,
    },
}

/// What a page is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "page", rename_all = "lowercase")]
pub enum Kind {
    /// The cover and contents.
    Cover,
    /// A group, or part of one.
    Group {
        /// Its number.
        n: u32,
        /// Which of its pages, from one.
        part: u32,
        /// Of how many.
        of: u32,
    },
    /// What was said about a group, on pages of its own.
    Argument {
        /// The group's number.
        n: u32,
        /// Which of its pages, from one.
        part: u32,
        /// Of how many.
        of: u32,
    },
    /// What was said about the deck as a whole.
    Said,
}

/// One page, laid out.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Page {
    /// How wide, in the units its items are placed in.
    pub width: f32,
    /// How high.
    pub height: f32,
    /// Points per unit: 1 for portrait, which is laid out in points, and
    /// 0.75 for landscape, which is laid out in the window's pixels.
    pub scale: f32,
    /// What it is.
    pub kind: Kind,
    /// What is on it, in the order it is drawn.
    pub items: Vec<Item>,
    /// What the person exporting should be told about this page: a group
    /// that ran past its two pages, a pane that could not show every line.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// `document` as pages of `format`.
///
/// The document is shown as it is: the groups it holds, and what was said
/// about them if it holds that. Choosing groups and leaving the comments out
/// is done to the document first ([`super::shown`]).
#[must_use]
pub fn layout(document: &Document, format: Format) -> Vec<Page> {
    match format {
        Format::Portrait => super::portrait::pages(document),
        Format::Landscape => super::landscape::pages(document),
    }
}

/// Up to the next multiple of four: the portrait scale's step.
pub(super) fn up4(v: f32) -> f32 {
    (v / 4.).ceil() * 4.
}

/// `n` as the length it is.
#[allow(clippy::cast_precision_loss)]
pub(super) fn count(n: usize) -> f32 {
    n as f32
}

/// How wide `text` is in `face` at `size`, as it will be drawn: words in
/// the face that will draw them, a character Plex lacks in the machine's
/// face [`super::fallback`] chooses for it, and code by its columns.
pub(super) fn measure(text: &str, face: Face, size: f32) -> f32 {
    match face.weight() {
        None => count(columns(text)) * text::mono_advance(size),
        Some(weight) if super::fallback::covered(face, text) => {
            text::shaped_width(text, size, weight)
        }
        Some(weight) => {
            let mut missing = std::collections::BTreeSet::new();
            super::fallback::runs(face, text, &mut missing)
                .into_iter()
                .map(|(source, range)| match source {
                    super::fallback::Source::Own => text::shaped_width(&text[range], size, weight),
                    super::fallback::Source::Machine(id) => {
                        super::fallback::machine_width(id, &text[range], size)
                    }
                })
                .sum()
        }
    }
}

/// How many columns `text` takes as code: one a character, two for a wide
/// one — CJK, most emoji — and none for a mark that sits on the one before.
pub(super) fn columns(text: &str) -> usize {
    text.chars().map(column).sum()
}

/// See [`columns`].
pub(super) fn column(ch: char) -> usize {
    unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1)
}

/// `text` broken into lines `width` wide in `face`: the window's own breaks
/// where Plex sets every character, and otherwise word by word, measured in
/// the faces that will draw them, a run with no spaces — Japanese — broken
/// where it fills the line.
pub(super) fn wrap(text: &str, face: Face, size: f32, width: f32) -> Vec<String> {
    if let Some(weight) = face.weight()
        && super::fallback::covered(face, text)
    {
        return text::wrap(text, size, weight, width);
    }
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        let tokens: Vec<Token> = paragraph
            .split_inclusive(' ')
            .map(|word| Token {
                text: word.to_string(),
                face,
                colour: Rgb::new(0, 0, 0),
                ground: None,
                numeral: None,
            })
            .collect();
        let lines = flow(&tokens, size, width);
        if lines.is_empty() {
            out.push(String::new());
        }
        for line in lines {
            out.push(line.into_iter().map(|piece| piece.token.text).collect());
        }
    }
    out
}

/// Where a line of `face` at `size` sits in a row `height` high whose top is
/// `top`: centred on the face's own ascent and descent, as the window centres
/// a line in its line height.
pub(super) fn baseline(top: f32, height: f32, face: Face, size: f32) -> f32 {
    let (ascent, descent) = match face.weight() {
        Some(weight) => text::ascent_descent(size, weight),
        None => text::mono_ascent_descent(size),
    };
    top + (height - (ascent + descent)) / 2. + ascent
}

/// `line` cut short with an ellipsis to fit `width` in `face`.
pub(super) fn fit(line: &str, face: Face, size: f32, width: f32) -> String {
    if let Some(weight) = face.weight()
        && super::fallback::covered(face, line)
    {
        return text::truncate(line, size, weight, width);
    }
    if measure(line, face, size) <= width {
        return line.to_string();
    }
    // Character by character, in the faces that will draw them, until the
    // ellipsis would no longer fit after them.
    let room = width - measure("…", face, size);
    let mut cut = String::new();
    for ch in line.chars() {
        let mut next = cut.clone();
        next.push(ch);
        if measure(&next, face, size) > room {
            break;
        }
        cut = next;
    }
    let mut cut = cut.trim_end().to_string();
    cut.push('…');
    cut
}

/// `text` shortened in the middle to fit `width`: for a repo, whose two ends
/// say which one it is.
pub(super) fn middle(text: &str, face: Face, size: f32, width: f32) -> String {
    if measure(text, face, size) <= width {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut keep = chars.len();
    while keep > 2 {
        keep -= 1;
        let head = keep.div_ceil(2);
        let tail = keep - head;
        let short: String = chars[..head]
            .iter()
            .chain(std::iter::once(&'…'))
            .chain(&chars[chars.len() - tail..])
            .collect();
        if measure(&short, face, size) <= width {
            return short;
        }
    }
    "…".to_string()
}

/// The inks a page is set in, beyond the document's own: what the window
/// mixes for itself.
#[derive(Debug, Clone, Copy)]
pub(super) struct Tones {
    /// The narration, a step off the band's ink.
    pub prose: Rgb,
    /// The group's number, faint in the accent.
    pub number: Rgb,
    /// A code chip's ink on the pane's ground.
    pub chip: Rgb,
    /// A pane's name in the narration, and its ground.
    pub name: Rgb,
    /// See `name`.
    pub name_ground: Rgb,
}

impl Tones {
    pub(super) fn of(document: &Document) -> Self {
        let inks = &document.inks;
        let name_ground = inks.on_band.accent.mix(inks.band, 0.88);
        Self {
            prose: legible(inks.on_band.text.mix(inks.band, 0.22), inks.band, LEGIBLE),
            number: inks.band.mix(inks.on_band.accent, 0.45),
            chip: legible(inks.on_band.text, inks.pane, LEGIBLE),
            name: legible(inks.on_band.accent, name_ground, LEGIBLE),
            name_ground,
        }
    }
}

/// A confidence level's name and its ink.
pub(super) fn level(document: &Document, level: Level) -> (&'static str, Rgb) {
    let levels = &document.inks.levels;
    match level {
        Level::High => ("High", levels.high),
        Level::Medium => ("Medium", levels.medium),
        Level::Low => ("Low", levels.low),
    }
}

/// One word, chip or numeral of a paragraph, ready to place.
#[derive(Debug, Clone)]
pub(super) struct Token {
    /// The words, with the space that followed them.
    pub text: String,
    /// The face.
    pub face: Face,
    /// The ink.
    pub colour: Rgb,
    /// A ground behind it: a chip.
    pub ground: Option<Rgb>,
    /// A point's numeral, drawn as a disc in place of the words.
    pub numeral: Option<u32>,
}

/// The narration's runs as tokens: marks become faces and inks, a point a
/// numeral.
pub(super) fn tokens(paragraph: &[Run], document: &Document, tones: &Tones) -> Vec<Token> {
    let inks = &document.inks;
    let mut out = Vec::new();
    for run in paragraph {
        match run {
            Run::Point { numeral } => out.push(Token {
                text: String::new(),
                face: Face::Regular,
                colour: inks.on_band.accent,
                ground: None,
                numeral: Some(*numeral),
            }),
            Run::Text { text, mark, .. } => {
                let (face, colour, ground) = match mark {
                    Some(Marked::Strong) => (Face::Semibold, inks.on_band.text, None),
                    Some(Marked::Emphasis) => (Face::Regular, inks.on_band.accent, None),
                    Some(Marked::Code) => (Face::Mono, tones.chip, Some(inks.pane)),
                    Some(Marked::Pane) => (Face::Regular, tones.name, Some(tones.name_ground)),
                    None => (Face::Regular, tones.prose, None),
                };
                if ground.is_some() {
                    // A chip is one word, its trailing space outside it.
                    let word = text.trim_end();
                    out.push(Token {
                        text: word.to_string(),
                        face,
                        colour,
                        ground,
                        numeral: None,
                    });
                    if word.len() < text.len() {
                        out.push(Token {
                            text: text[word.len()..].to_string(),
                            face: Face::Regular,
                            colour: tones.prose,
                            ground: None,
                            numeral: None,
                        });
                    }
                } else {
                    for word in text.split_inclusive(' ') {
                        out.push(Token {
                            text: word.to_string(),
                            face,
                            colour,
                            ground: None,
                            numeral: None,
                        });
                    }
                }
            }
        }
    }
    out
}

/// A token placed on its line, `x` from the line's left edge.
#[derive(Debug, Clone)]
pub(super) struct Placed {
    pub x: f32,
    pub width: f32,
    pub token: Token,
}

/// How wide a token is at `size`, a chip's padding included.
fn token_width(token: &Token, size: f32) -> f32 {
    if token.numeral.is_some() {
        return size * 1.5;
    }
    let pad = if token.ground.is_some() {
        size * 0.8
    } else {
        0.
    };
    // Code chips are set a step smaller, as the window sets them.
    let face_size = if token.face == Face::Mono {
        size * 0.92
    } else {
        size
    };
    measure(&token.text, token.face, face_size) + pad
}

/// `tokens` broken into lines `width` wide at `size`: each word goes on the
/// line it fits, and a word wider than a whole line is cut by characters.
/// What breaks a line is a word that does not fit, measured without the
/// space after it, as the window measures.
pub(super) fn flow(tokens: &[Token], size: f32, width: f32) -> Vec<Vec<Placed>> {
    let mut lines: Vec<Vec<Placed>> = vec![Vec::new()];
    let mut x = 0.;
    let mut queue: std::collections::VecDeque<Token> = tokens.iter().cloned().collect();
    while let Some(token) = queue.pop_front() {
        let full = token_width(&token, size);
        let trimmed = Token {
            text: token.text.trim_end().to_string(),
            ..token.clone()
        };
        let bare = token_width(&trimmed, size);
        let line_empty = lines.last().is_none_or(Vec::is_empty);
        if x + bare > width && !line_empty {
            lines.push(Vec::new());
            x = 0.;
            if token.text.trim().is_empty() && token.numeral.is_none() {
                continue;
            }
        }
        if bare > width && token.numeral.is_none() && token.text.chars().count() > 1 {
            // Too wide for any line: as much as fits, and the rest next.
            let mut head = String::new();
            for ch in token.text.chars() {
                let mut next = head.clone();
                next.push(ch);
                if !head.is_empty()
                    && token_width(
                        &Token {
                            text: next.clone(),
                            ..token.clone()
                        },
                        size,
                    ) > width - x
                {
                    break;
                }
                head = next;
            }
            let rest = token.text[head.len()..].to_string();
            let head = Token {
                text: head,
                ..token.clone()
            };
            let w = token_width(&head, size);
            lines.last_mut().expect("a line").push(Placed {
                x,
                width: w,
                token: head,
            });
            x += w;
            if !rest.is_empty() {
                queue.push_front(Token {
                    text: rest,
                    ..token
                });
            }
            continue;
        }
        lines.last_mut().expect("a line").push(Placed {
            x,
            width: full,
            token,
        });
        x += full;
    }
    lines.retain(|line| !line.is_empty());
    lines
}

/// One line of `placed` tokens as items, its left edge at `left` and its
/// row's top at `top`, `height` high: runs of one face and ink joined into
/// one piece of text, so it selects as the sentence it is.
pub(super) fn line_items(
    placed: &[Placed],
    left: f32,
    top: f32,
    height: f32,
    size: f32,
    ink_on_badge: Rgb,
) -> Vec<Item> {
    let mut out = Vec::new();
    let mut run: Option<(f32, String, Face, Rgb)> = None;
    let flush = |run: &mut Option<(f32, String, Face, Rgb)>, out: &mut Vec<Item>| {
        if let Some((x, text, face, colour)) = run.take() {
            let text = text.trim_end().to_string();
            if !text.is_empty() {
                out.push(Item::Text {
                    x,
                    baseline: baseline(top, height, face, size),
                    text,
                    face,
                    size,
                    colour,
                });
            }
        }
    };
    for piece in placed {
        let token = &piece.token;
        if let Some(numeral) = token.numeral {
            flush(&mut run, &mut out);
            let r = size * 0.6;
            out.push(Item::Badge {
                cx: left + piece.x + piece.width / 2.,
                cy: top + height / 2.,
                r,
                numeral,
                fill: token.colour,
                ink: ink_on_badge,
            });
            continue;
        }
        if let Some(ground) = token.ground {
            flush(&mut run, &mut out);
            let pad = size * 0.4;
            let chip_size = if token.face == Face::Mono {
                size * 0.92
            } else {
                size
            };
            out.push(Item::Chip {
                rect: Rect::new(
                    left + piece.x,
                    top + (height - size * 1.5) / 2.,
                    piece.width,
                    size * 1.5,
                ),
                colour: ground,
            });
            out.push(Item::Text {
                x: left + piece.x + pad,
                baseline: baseline(top, height, token.face, chip_size),
                text: token.text.clone(),
                face: token.face,
                size: chip_size,
                colour: token.colour,
            });
            continue;
        }
        match &mut run {
            Some((_, text, face, colour)) if *face == token.face && *colour == token.colour => {
                text.push_str(&token.text);
            }
            _ => {
                flush(&mut run, &mut out);
                run = Some((left + piece.x, token.text.clone(), token.face, token.colour));
            }
        }
    }
    flush(&mut run, &mut out);
    out
}

/// A row of a code pane as a page prints it: a line of the file, or a piece
/// of one too long for the pane, or what stands in for lines left out.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Visual {
    /// Code: the start of a line (`no` set) or its continuation.
    Code {
        /// The line's number, on its first row only.
        no: Option<u32>,
        /// What it is to the pane.
        kind: RowKind,
        /// Its ground.
        ground: Rgb,
        /// Its accent, legible on that ground.
        accent: Rgb,
        /// Whether a point lights it.
        lit: bool,
        /// The numerals that start here, on its first row only.
        numerals: Vec<u32>,
        /// Characters of indent before the runs: a continuation's hang.
        indent: usize,
        /// The runs: text, ink, italic.
        runs: Vec<(String, Rgb, bool)>,
    },
    /// Lines left out.
    Gap {
        /// How many.
        lines: u32,
    },
    /// A value with keys, under its line.
    Preview {
        /// Its ground.
        ground: Rgb,
        /// Characters in, where the code starts.
        indent: usize,
        /// `↳ name ▸ preview`.
        text: String,
        /// Its ink.
        accent: Rgb,
    },
}

/// How a pane's rows are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Context {
    /// Up to this many lines either side of the range, and every line a
    /// point lights.
    Around(u32),
    /// The range whole, as the model has it, and around it only the lines
    /// a point lights or starts on and the one line either side: what a
    /// group gives up when two pages are not enough. Nothing in the range
    /// goes, so no evidence does.
    Trimmed,
}

/// Every box of a drawing deck wrote, as how far down it starts and ends
/// from the drawing's top: the `data-top` and `data-bottom` the SVG writer
/// puts on each.
pub(super) fn boxes(svg: &str) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    for line in svg.lines() {
        let attr = |name: &str| {
            let key = format!("{name}=\"");
            let at = line.find(&key)? + key.len();
            line[at..at + line[at..].find('"')?].parse::<f32>().ok()
        };
        if line.starts_with("<g ")
            && let (Some(top), Some(bottom)) = (attr("data-top"), attr("data-bottom"))
        {
            out.push((top, bottom));
        }
    }
    out
}

/// Where a band that starts at `from` and could reach `limit` should end:
/// `limit` itself when no box crosses it, and otherwise just above the box
/// it would cut, so long as that leaves the band something. Whether it had
/// to cut through a box anyway: one taller than the band.
pub(super) fn snap(boxes: &[(f32, f32)], from: f32, limit: f32) -> (f32, bool) {
    let crosses = |at: f32| boxes.iter().any(|&(top, bottom)| top < at && at < bottom);
    if !crosses(limit) {
        return (limit, false);
    }
    // The highest point at or below `limit` in a gap: just above a box's
    // top, halfway into the gap over it where there is one.
    let mut best: Option<f32> = None;
    for &(top, _) in boxes {
        if top <= from || top > limit {
            continue;
        }
        let above = boxes
            .iter()
            .filter(|&&(_, bottom)| bottom <= top)
            .map(|&(_, bottom)| bottom)
            .fold(from, f32::max);
        let at = (above + top) / 2.;
        if at > from && !crosses(at) && best.is_none_or(|best| at > best) {
            best = Some(at);
        }
    }
    best.map_or((limit, true), |at| (at, false))
}

/// `svg` with only what lies in the band from `from` to `to` (in its own
/// units, from its top): the boxes that reach into the band and the words
/// whose line sits in it. The rest is clipped out of sight anyway, and left
/// in, its words would be in the page's text where nobody can see them.
pub(super) fn banded(svg: &str, from: f32, to: f32) -> String {
    let top = svg
        .find("viewBox=\"")
        .and_then(|at| svg[at + 9..].split_whitespace().nth(1))
        .and_then(|t| t.parse::<f32>().ok())
        .unwrap_or(0.);
    let mut out = String::with_capacity(svg.len());
    let mut skipping = false;
    for line in svg.lines() {
        let attr = |name: &str| {
            let key = format!(" {name}=\"");
            let at = line.find(&key)? + key.len();
            line[at..at + line[at..].find('"')?].parse::<f32>().ok()
        };
        if skipping {
            if line == "</g>" {
                skipping = false;
            }
            continue;
        }
        if line.starts_with("<g ")
            && let (Some(t), Some(b)) = (attr("data-top"), attr("data-bottom"))
            && (b <= from || t >= to)
        {
            skipping = true;
            continue;
        }
        if line.starts_with("<text ")
            && let Some(y) = attr("y")
            && !(from..=to).contains(&(y - top))
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// How many columns a tab takes, as the window's panes count it.
const TAB: usize = 4;

/// The smallest a diagram may be drawn, as a share of its own size: where a
/// box's label, set at [`deck_core::drawing::LABEL`], is six points on the
/// page. Smaller and it is not read, so a taller diagram is cut across
/// pages instead and a wider one is cut at the edge.
pub(super) const DIAGRAM_LEAST: f32 = 6. / (deck_core::drawing::LABEL.0 * 0.75);

/// Where `n` numerals go in a gutter from `left` to `right`: each disc's
/// centre across, how far below its row's middle it sits, and its radius.
/// The first is beside the code and the rest go outward from it; a disc is
/// `most` across when there is room and never under `least`, so its numeral
/// stays readable, and what one row of the gutter cannot hold goes on a
/// row under it rather than out of the gutter or smaller.
pub(super) fn numeral_slots(
    n: usize,
    left: f32,
    right: f32,
    most: f32,
    least: f32,
) -> Vec<(f32, f32, f32)> {
    if n == 0 {
        return Vec::new();
    }
    let r = most.min(((right - left) / count(n) - 1.) / 2.).max(least);
    let step = 2. * r + 1.;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let per_row = (((right - left) + 1.) / step).floor().max(1.) as usize;
    (0..n)
        .map(|ix| {
            let (row, col) = (ix / per_row, ix % per_row);
            (right - r - count(col) * step, count(row) * step, r)
        })
        .collect()
}

/// How far a context line's ink is taken toward its ground.
const CONTEXT_FADE: f32 = 0.35;

/// The rows of `code` to print, whole: [`cut`] fits them to a pane.
pub(super) fn visual_rows(code: &Code, context: Context) -> Vec<Visual> {
    // Which of the model's lines are printed at all.
    let lines: Vec<&Line> = code.lines.iter().collect();
    let keep: Vec<bool> = match context {
        Context::Around(shown) => lines
            .iter()
            .map(|line| match line {
                Line::Code(row) => match row.kind {
                    RowKind::Context { distance } => {
                        distance <= shown || !row.numerals.is_empty() || !row.lit_by.is_empty()
                    }
                    _ => true,
                },
                Line::Gap { .. } => true,
            })
            .collect(),
        Context::Trimmed => lines
            .iter()
            .map(|line| match line {
                Line::Code(row) => match row.kind {
                    RowKind::Context { distance } => {
                        distance <= 1 || !row.numerals.is_empty() || !row.lit_by.is_empty()
                    }
                    _ => true,
                },
                Line::Gap { .. } => true,
            })
            .collect(),
    };

    let mut out = Vec::new();
    let mut folded = 0u32;
    let mut started = false;
    for (line, keep) in lines.iter().zip(keep) {
        match line {
            Line::Gap { lines } if keep => folded += lines,
            Line::Code(row) if keep => {
                // Leading context that was not kept is not a gap: nothing
                // was cut from inside what the pane shows.
                if folded > 0 && started {
                    out.push(Visual::Gap { lines: folded });
                }
                folded = 0;
                started = true;
                out.extend(whole(row));
            }
            Line::Code(_) => folded += 1,
            Line::Gap { lines } => folded += lines,
        }
    }
    out
}

/// One row of the model as a page prints it: its runs with its inline
/// values in them, faded if it is context, and a preview row for each value
/// with keys.
pub(super) fn whole(row: &Row) -> Vec<Visual> {
    let context = matches!(row.kind, RowKind::Context { .. });
    let text: String = row.spans.iter().map(|span| span.text.as_str()).collect();
    // Every character with its ink, a value's characters spliced in after
    // its name as the window places it.
    let mut chars: Vec<(char, Rgb, bool)> = Vec::new();
    let mut inlays: Vec<(usize, String)> = row
        .values
        .iter()
        .filter(|value| value.keys.is_empty())
        .map(|value| (crate::after_name(&text, &value.at), value.is.clone()))
        .collect();
    inlays.sort_by_key(|(at, _)| *at);
    let mut inlays = inlays.into_iter().peekable();
    let mut at = 0;
    let mut put_values = |at: usize, chars: &mut Vec<(char, Rgb, bool)>, upto: bool| {
        while let Some((place, _)) = inlays.peek()
            && (*place <= at || upto)
        {
            let (_, value) = inlays.next().expect("peeked");
            chars.push((' ', row.accent, false));
            chars.extend(value.chars().map(|ch| (ch, row.accent, false)));
            chars.push((' ', row.accent, false));
        }
    };
    for span in &row.spans {
        let ink = if context {
            legible(span.ink.mix(row.ground, CONTEXT_FADE), row.ground, LEGIBLE)
        } else {
            span.ink
        };
        for ch in span.text.chars() {
            put_values(at, &mut chars, false);
            if ch == '\t' {
                // To the next stop, as the window sets a tab.
                let column = chars.len();
                let to = (column / TAB + 1) * TAB;
                chars.extend(std::iter::repeat_n((' ', ink, span.italic), to - column));
            } else {
                chars.push((ch, ink, span.italic));
            }
            at += ch.len_utf8();
        }
    }
    put_values(at, &mut chars, true);
    while chars.last().is_some_and(|(ch, _, _)| *ch == ' ') {
        chars.pop();
    }
    let indent = chars.iter().take_while(|(ch, _, _)| *ch == ' ').count();
    let mut out = vec![Visual::Code {
        no: Some(row.no),
        kind: row.kind,
        ground: row.ground,
        accent: row.accent,
        lit: !row.lit_by.is_empty(),
        numerals: row.numerals.clone(),
        indent: 0,
        runs: runs_of(&chars),
    }];
    for value in row.values.iter().filter(|value| !value.keys.is_empty()) {
        out.push(Visual::Preview {
            ground: row.ground,
            indent,
            text: format!("↳ {} ▸ {}", value.at, value.is),
            accent: row.accent,
        });
    }
    out
}

/// Characters back into runs of one ink.
fn runs_of(chars: &[(char, Rgb, bool)]) -> Vec<(String, Rgb, bool)> {
    let mut runs: Vec<(String, Rgb, bool)> = Vec::new();
    for &(ch, ink, italic) in chars {
        match runs.last_mut() {
            Some((text, was, lean)) if *was == ink && *lean == italic => text.push(ch),
            _ => runs.push((ch.to_string(), ink, italic)),
        }
    }
    runs
}

/// `row` cut to `cols` columns: never clipped, each continuation hung two
/// columns in from the line's own indent and broken after a space where one
/// falls in the second half of the room, so a word is not cut where it need
/// not be.
pub(super) fn cut(row: &Visual, cols: usize) -> Vec<Visual> {
    let Visual::Code {
        no,
        kind,
        ground,
        accent,
        lit,
        numerals,
        runs,
        ..
    } = row
    else {
        return vec![row.clone()];
    };
    let cols = cols.max(8);
    let chars: Vec<(char, Rgb, bool)> = runs
        .iter()
        .flat_map(|(text, ink, italic)| text.chars().map(move |ch| (ch, *ink, *italic)))
        .collect();
    let lead = chars.iter().take_while(|(ch, _, _)| *ch == ' ').count();
    let hang = (lead + 2).min(cols / 2);
    let mut out = Vec::new();
    let mut rest: &[(char, Rgb, bool)] = &chars;
    let mut pad = 0;
    loop {
        let room = cols - pad;
        // As many characters as fit the room's columns, a wide one taking
        // two; never none, or a character wider than the room would stall.
        let mut used = 0;
        let mut fits = 0;
        for (ch, _, _) in rest {
            if used + column(*ch) > room && fits > 0 {
                break;
            }
            used += column(*ch);
            fits += 1;
        }
        let (piece, next) = if fits >= rest.len() {
            (rest, &rest[rest.len()..])
        } else {
            let at = rest[..fits]
                .iter()
                .rposition(|(ch, _, _)| *ch == ' ')
                .filter(|&at| at >= fits / 2)
                .map_or(fits, |at| at + 1);
            (&rest[..at], &rest[at..])
        };
        let first = out.is_empty();
        out.push(Visual::Code {
            no: if first { *no } else { None },
            kind: *kind,
            ground: *ground,
            accent: *accent,
            lit: *lit,
            numerals: if first { numerals.clone() } else { Vec::new() },
            indent: pad,
            runs: runs_of(piece),
        });
        // The spaces a line was cut at stay with it: in code a run of
        // spaces is the code, and a wrap must not eat it.
        rest = next;
        if rest.is_empty() {
            break;
        }
        pad = hang;
    }
    out
}

/// The widest line number a pane prints, in characters.
pub(super) fn number_width(code: &Code) -> usize {
    code.lines
        .iter()
        .filter_map(|line| match line {
            Line::Code(row) => Some(row.no.to_string().len()),
            Line::Gap { .. } => None,
        })
        .max()
        .unwrap_or(1)
}

/// The label a pane goes by: its name, or what it shows.
pub(super) fn pane_name(pane: &Pane) -> (Option<String>, String, Option<String>) {
    match pane {
        Pane::Code(code) => {
            let range = if code.range.first == code.range.last {
                format!("{}:{}", code.file, code.range.first)
            } else {
                format!("{}:{}-{}", code.file, code.range.first, code.range.last)
            };
            let what = match code.change {
                Some(deck_core::document::Change::Proposed) => format!("{range} · proposed"),
                Some(deck_core::document::Change::Made) => format!("{range} · changed"),
                None => range,
            };
            (code.name.clone(), what, code.note.clone())
        }
        Pane::Diagram(picture) => (
            picture.name.clone(),
            picture
                .note
                .clone()
                .unwrap_or_else(|| "diagram".to_string()),
            None,
        ),
        Pane::Page(page) => (
            page.name.clone(),
            page.note.clone().unwrap_or_else(|| "page".to_string()),
            None,
        ),
    }
}

/// The repo as a footer names it: `acme/ingest@a1b2c3d`.
pub(super) fn repo(document: &Document) -> Option<String> {
    let repo = document.repo.as_ref()?;
    let name = repo.remote.as_deref().map(|remote| {
        let remote = remote.trim_end_matches('/').trim_end_matches(".git");
        let parts: Vec<&str> = remote.rsplit(['/', ':']).take(2).collect();
        parts.into_iter().rev().collect::<Vec<_>>().join("/")
    });
    let commit = repo
        .commit
        .as_deref()
        .map(|commit| commit.chars().take(7).collect::<String>());
    match (name, commit) {
        (Some(name), Some(commit)) => Some(format!("{name}@{commit}")),
        (Some(name), None) => Some(name),
        (None, Some(commit)) => Some(commit),
        (None, None) => None,
    }
}

/// The day a deck was packed, as `20 Oct 2026`, never a time.
pub(super) fn day(created: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let year = created.get(..4)?;
    let month: usize = created.get(5..7)?.parse().ok()?;
    let date: u32 = created.get(8..10)?.parse().ok()?;
    Some(format!(
        "{date} {} {year}",
        MONTHS.get(month.checked_sub(1)?)?
    ))
}

/// Whether the document shows every group of its deck, numbered from one
/// with none missing: only then can a page say "of N".
pub(super) fn complete(document: &Document) -> bool {
    document
        .groups
        .iter()
        .enumerate()
        .all(|(ix, group)| usize::try_from(group.n).is_ok_and(|n| n == ix + 1))
}

/// The SVG's own size in its units, from its `width` and `height`.
pub(super) fn svg_size(svg: &str) -> (f32, f32) {
    let attr = |name: &str| {
        let key = format!(" {name}=\"");
        let at = svg.find(&key)? + key.len();
        let end = svg[at..].find('"')?;
        svg[at..at + end].parse::<f32>().ok()
    };
    (
        attr("width").unwrap_or(320.),
        attr("height").unwrap_or(240.),
    )
}

#[cfg(test)]
mod tests {
    use deck_core::document::{Row, RowKind, Span};
    use deck_core::theme::Rgb;

    use super::*;

    fn row(text: &str) -> Row {
        Row {
            no: 1,
            kind: RowKind::Spot,
            numerals: Vec::new(),
            lit_by: Vec::new(),
            ground: Rgb::new(0, 0, 0),
            accent: Rgb::new(255, 128, 0),
            spans: vec![Span {
                text: text.to_string(),
                ink: Rgb::new(200, 200, 200),
                italic: false,
            }],
            values: Vec::new(),
        }
    }

    fn text_of(rows: &[Visual]) -> Vec<String> {
        rows.iter()
            .filter_map(|row| match row {
                Visual::Code { runs, indent, .. } => Some(format!(
                    "{}{}",
                    " ".repeat(*indent),
                    runs.iter().map(|(t, _, _)| t.as_str()).collect::<String>()
                )),
                _ => None,
            })
            .collect()
    }

    /// A tab goes to the next stop, as the window sets it, and the spaces
    /// a long line is cut at stay with it.
    #[test]
    fn tabs_expand_and_wraps_keep_their_spaces() {
        let rows = whole(&row("\tif (x) {"));
        assert_eq!(text_of(&rows), ["    if (x) {"]);
        let rows = whole(&row("ab\tc"));
        assert_eq!(text_of(&rows), ["ab  c"]);
        let long = format!("{}    {}", "a".repeat(10), "b".repeat(10));
        let cut_rows = cut(&whole(&row(&long))[0], 12);
        let all: String = text_of(&cut_rows).join("|");
        assert!(all.matches(' ').count() >= 4, "{all}");
    }
}
