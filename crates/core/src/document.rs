//! A deck as a document: one model every export writes from.
//!
//! A PDF, an HTML page, a Markdown file and a slide deck all have to say the
//! same things about a group — which panes it shows and in what order, which
//! lines its points land on and what number sits in the margin beside them,
//! what the lines held, what the reader and the agent said about it — and
//! each format answering those questions for itself is four answers that
//! drift apart. So they are answered once, here, from a frozen deck
//! ([`crate::bundle::Bundle`]), and every format only lays out what this
//! hands it.
//!
//! What it needs from outside is the ink: code coloured the way the window
//! colours it, and a diagram drawn the way the window draws it. Both live
//! where the window's renderer lives, so they come in through [`Paint`], and
//! this stays something every platform's tests can run.
//!
//! # The rules it carries
//!
//! - **Panes in the order the prose reaches them.** A reader looks where the
//!   sentence they are reading sends them; the first pane named or pointed at
//!   comes first, and the rest follow in the order the deck gave them.
//! - **One numeral per point, in reading order.** `¹` in the sentence and `¹`
//!   beside the lines it lands on, numbered within the group. The same point
//!   written twice keeps its number.
//! - **The spotlit range, a little around it, never the whole file.** Up to
//!   [`CONTEXT`] lines either side, as far as the snapshot reaches, each
//!   tagged with how far out it is, so a format shows as many as it has room
//!   for. A long stretch of the range with nothing on it folds to a gap.
//! - **The argument after its group.** The reader's remarks and the agent's
//!   answers, in the order they were said.
//! - **Every colour legible.** Each run of code is lifted to
//!   [`crate::theme::LEGIBLE`] on the ground it is printed on.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::bundle::{Bundle, Repo, Unreadable, snap::Snapshot};
use crate::diagram::Diagram;
use crate::line::LineRange;
use crate::prose::{self, Mark, Side, Spot};
use crate::protocol::{Confidence, Kind, Ref, RefSpec, Value, What};
use crate::theme::{LEGIBLE, Palette, Rgb, legible};

/// How many lines either side of a range the model carries, at most.
///
/// More than any page shows — a landscape page shows three, a portrait one —
/// so each format picks without asking for more.
pub const CONTEXT: u32 = 8;

/// A stretch of the range this long with no point and no value on it folds
/// to a gap, keeping [`FOLD_EDGE`] lines at each end.
pub const FOLD_AT: u32 = 7;

/// How many lines a fold keeps at each edge, so the lines a point lands on
/// still have their neighbours.
pub const FOLD_EDGE: u32 = 2;

/// The ink a document needs from the window's renderer.
pub trait Paint {
    /// `source` coloured as the language of `file`: runs over byte ranges,
    /// in order. Bytes no run covers, and runs with no colour, are written in
    /// the text's own colour.
    fn spans(&self, source: &str, file: &str) -> Vec<Painted>;

    /// `diagram` as an SVG, at rest.
    fn diagram(&self, diagram: &Diagram) -> String;
}

/// One coloured run, as [`Paint::spans`] gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Painted {
    /// Byte offsets into the source.
    pub range: std::ops::Range<usize>,
    /// Its colour, or `None` for the text's own.
    pub colour: Option<Rgb>,
    /// Set in italics.
    pub italic: bool,
}

/// A whole deck, ready to lay out.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Document {
    /// The claim the deck answers.
    pub title: String,
    /// Where it was written, when that is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<Repo>,
    /// When it was packed, as the manifest has it.
    pub created: String,
    /// One line per group, for a contents page.
    pub contents: Vec<Entry>,
    /// The groups, in order.
    pub groups: Vec<Group>,
    /// What was said that belongs to no group, for a closing section.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub general: Vec<Message>,
}

/// A group, as a contents page lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    /// Its number, from one.
    pub n: u32,
    /// Its title.
    pub title: String,
    /// How sure the agent was, and on what.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    /// Its panes' names, in the order the group shows them.
    pub panes: Vec<String>,
    /// How many things were said about it.
    pub remarks: usize,
}

/// One group, laid out.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    /// Its number, from one.
    pub n: u32,
    /// Its id in the deck, as `g2`.
    pub id: String,
    /// Its title, or the first words of its narration when it has none.
    pub title: String,
    /// How sure the agent was, and on what.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    /// The narration, paragraph by paragraph.
    pub prose: Vec<Vec<Run>>,
    /// The panes, in the order the prose reaches them.
    pub panes: Vec<Pane>,
    /// Every value the group's code held, for a ledger.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ledger: Vec<Held>,
    /// What the reader and the agent said about it, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub argument: Vec<Message>,
}

/// A run of narration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "run", rename_all = "lowercase")]
pub enum Run {
    /// Words, marked or not, with the space that followed them.
    Text {
        /// The words.
        text: String,
        /// How they are marked.
        #[serde(skip_serializing_if = "Option::is_none")]
        mark: Option<Marked>,
    },
    /// The numeral a point carries into the margin.
    Point {
        /// Its number in the group, from one.
        numeral: u32,
    },
}

/// How a run of narration is marked, as [`Mark`] has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Marked {
    /// Said with weight.
    Strong,
    /// The word to look at.
    Emphasis,
    /// Code, set as a chip.
    Code,
    /// The name of a pane.
    Pane,
}

impl From<Mark> for Marked {
    fn from(mark: Mark) -> Self {
        match mark {
            Mark::Strong => Self::Strong,
            Mark::Emphasis => Self::Emphasis,
            Mark::Code => Self::Code,
            Mark::Pane => Self::Pane,
        }
    }
}

/// One pane of a group.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "pane", rename_all = "lowercase")]
pub enum Pane {
    /// Lines of a file.
    Code(Code),
    /// A picture.
    Diagram(Picture),
    /// A page of the agent's own markup.
    Page(Page),
}

/// A code pane.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Code {
    /// The ref's id, as `g2r1`.
    pub id: String,
    /// What the prose calls it, when it has a name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Its label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The file, in the repo.
    pub file: String,
    /// The lines it lights, as the deck wrote them.
    pub range: LineRange,
    /// Whether the lines are a change, and which way.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<Change>,
    /// The rows to print, in order.
    pub lines: Vec<Line>,
    /// The file was not in the deck, so there are no lines to print.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub missing: bool,
}

/// Which way a code pane's change runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    /// A replacement proposed for the range: the range goes, `after` arrives.
    Proposed,
    /// A change already made: `before` went, the range arrived.
    Made,
}

/// A row of a code pane.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "row", rename_all = "lowercase")]
pub enum Line {
    /// One line of code.
    Code(Row),
    /// Lines left out: a stretch of the range with nothing on it.
    Gap {
        /// How many.
        lines: u32,
    },
}

/// One printed line of code.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// Its number in the file, or in the replacement for an arriving line.
    pub no: u32,
    /// What it is to the pane.
    pub kind: RowKind,
    /// The numeral of the point that lands on it, if one does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numeral: Option<u32>,
    /// The ground it is printed on.
    pub ground: Rgb,
    /// Its text in coloured runs, every colour legible on `ground`.
    pub spans: Vec<Span>,
    /// What it held, as the deck recorded it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Value>,
}

/// What a row is to its pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RowKind {
    /// Around the range, this many lines out from it.
    Context {
        /// How far out, from one.
        distance: u32,
    },
    /// In the range.
    Spot,
    /// In the range, going: the old half of a change.
    Going,
    /// Arriving: the new half of a change.
    Arriving,
}

/// A run of code in one colour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Span {
    /// The text.
    pub text: String,
    /// Its colour, legible on its row's ground.
    pub ink: Rgb,
    /// Set in italics.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
}

/// A picture pane.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Picture {
    /// The ref's id.
    pub id: String,
    /// What the prose calls it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Its label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The drawing, as SVG.
    pub svg: String,
    /// The blocks points land on, with their numerals.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<(String, u32)>,
}

/// A page pane.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Page {
    /// The ref's id.
    pub id: String,
    /// What the prose calls it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Its label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The markup, as written.
    pub html: String,
    /// The names points land on, with their numerals.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<(String, u32)>,
}

/// One value a line held, for a group's ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Held {
    /// The pane's name, or its file when it has none.
    pub pane: String,
    /// The line.
    pub line: u32,
    /// The numeral of the point on that line, if one lands there.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numeral: Option<u32>,
    /// The name on the line the value belongs to.
    pub at: String,
    /// What it held.
    pub is: String,
    /// Whether the claim turns on it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub lit: bool,
}

/// Something said about the deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    /// Who said it.
    pub author: Author,
    /// The pane it was about, if one.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    /// The lines it was about, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<LineRange>,
    /// What the reader had selected when they wrote it.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub quote: String,
    /// What kind of remark, when the reader said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    /// The words.
    pub text: String,
    /// When, in milliseconds from the start of the walk, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_ms: Option<u64>,
}

/// Who said something.
///
/// The roles, not names: a deck records no names, and a document that
/// invented one would be telling the reader something nobody said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Author {
    /// The person reading the deck.
    Reader,
    /// The agent that wrote it.
    Agent,
}

/// `bundle` as a document, painted by `paint` in `palette`.
///
/// # Errors
///
/// When a part of the bundle does not parse. A bundle that opened has
/// already been read once, so this fails only for one built by hand.
pub fn build(
    bundle: &Bundle,
    palette: &Palette,
    paint: &dyn Paint,
) -> Result<Document, Unreadable> {
    let header = bundle.header()?;
    let snapshots = bundle.snapshots()?;
    let review = bundle.review()?;
    let by_file: BTreeMap<&str, &Snapshot> = snapshots
        .iter()
        .map(|snapshot| (snapshot.file.path.as_str(), snapshot))
        .collect();

    let groups = bundle.groups()?;
    let (mut said, general) = argument(&groups, review.as_ref());
    let mut built = Vec::with_capacity(groups.len());
    for (ix, group) in groups.iter().enumerate() {
        let n = u32::try_from(ix + 1).unwrap_or(u32::MAX);
        let argument = said.remove(&group.id).unwrap_or_default();
        built.push(group_of(n, group, argument, &by_file, palette, paint));
    }
    let contents = built
        .iter()
        .map(|group| Entry {
            n: group.n,
            title: group.title.clone(),
            confidence: group.confidence.clone(),
            panes: group.panes.iter().map(pane_label).collect(),
            remarks: group.argument.len(),
        })
        .collect();
    Ok(Document {
        title: header.title,
        repo: bundle.manifest.repo.clone(),
        created: bundle.manifest.created.clone(),
        contents,
        groups: built,
        general,
    })
}

/// What a contents page calls a pane: its name, or what it shows.
fn pane_label(pane: &Pane) -> String {
    match pane {
        Pane::Code(code) => code.name.clone().unwrap_or_else(|| code.file.clone()),
        Pane::Diagram(picture) => picture
            .name
            .clone()
            .or_else(|| picture.note.clone())
            .unwrap_or_else(|| picture.id.clone()),
        Pane::Page(page) => page
            .name
            .clone()
            .or_else(|| page.note.clone())
            .unwrap_or_else(|| page.id.clone()),
    }
}

/// A point resolved: its numeral, and where it lands.
struct Landed {
    numeral: u32,
    spot: Spot,
    /// Index into the group's refs of each pane it lands in.
    panes: Vec<usize>,
}

fn group_of(
    n: u32,
    group: &crate::protocol::Group,
    argument: Vec<Message>,
    by_file: &BTreeMap<&str, &Snapshot>,
    palette: &Palette,
    paint: &dyn Paint,
) -> Group {
    // How long each file is, as far as the snapshot can tell: where a range
    // runs past the last line kept around it, the file ended there, since the
    // snapshot keeps context past a range wherever the file goes on.
    let line_count = |spec: &RefSpec| {
        let snapshot = by_file.get(path_of(spec).as_str())?;
        let span = snapshot
            .file
            .kept
            .iter()
            .find(|span| span.contains(spec.range.first))?;
        (spec.range.last > span.last).then_some(span.last)
    };

    let say = prose::unchipped(&group.say);
    let (prose, landed, reached) = read(&say, &group.refs, &line_count);

    // Panes in the order the prose reaches them, then the rest as written.
    let mut order: Vec<usize> = Vec::new();
    for ix in reached.into_iter().chain(0..group.refs.len()) {
        if !order.contains(&ix) {
            order.push(ix);
        }
    }
    let panes: Vec<Pane> = order
        .iter()
        .map(|&ix| pane_of(ix, &group.refs[ix], &landed, by_file, palette, paint))
        .collect();
    let ledger = panes.iter().flat_map(ledger_of).collect();
    let title = group.title.clone().unwrap_or_else(|| {
        let words: Vec<String> = prose::words(&prose::unpoint(&prose::unname(&say)));
        words
            .iter()
            .take(8)
            .map(String::as_str)
            .collect::<String>()
            .trim()
            .to_string()
    });
    Group {
        n,
        id: group.id.clone(),
        title,
        confidence: group.confidence.clone(),
        prose,
        panes,
        ledger,
        argument,
    }
}

/// The repo path a code ref names, with forward slashes.
fn path_of(spec: &RefSpec) -> String {
    spec.file.to_string_lossy().replace('\\', "/")
}

/// The narration as paragraphs of runs, every point it makes with its
/// numeral and its landing, and the order in which it reaches the refs.
fn read(
    say: &str,
    refs: &[Ref],
    line_count: &dyn Fn(&RefSpec) -> Option<u32>,
) -> (Vec<Vec<Run>>, Vec<Landed>, Vec<usize>) {
    let mut paragraphs: Vec<Vec<Run>> = Vec::new();
    let mut landed: Vec<Landed> = Vec::new();
    let mut reached: Vec<usize> = Vec::new();
    let named = |name: &str| {
        refs.iter().position(|r| {
            let own = match r {
                Ref::Code(code) => code.name.as_deref(),
                Ref::Diagram(diagram) => diagram.name.as_deref(),
                Ref::Page(page) => page.name.as_deref(),
            };
            own == Some(name)
        })
    };

    // The narration in pieces, cut at each point, the points between them.
    let mut pieces: Vec<(String, Option<Spot>)> = Vec::new();
    let mut rest = say;
    let mut piece = String::new();
    while let Some(at) = rest.find(prose::POINT) {
        let after = &rest[at + prose::POINT.len()..];
        let Some(end) = after.find(']') else {
            break;
        };
        match prose::spot(&after[..end]) {
            Some(spot) => {
                piece.push_str(&rest[..at]);
                pieces.push((std::mem::take(&mut piece), Some(spot)));
                rest = &after[end + 1..];
            }
            None => {
                piece.push_str(&rest[..at + prose::POINT.len()]);
                rest = after;
            }
        }
    }
    piece.push_str(rest);
    pieces.push((piece, None));

    for (text, spot) in pieces {
        let fresh = text.trim_start_matches([' ', '\t']).starts_with("\n\n");
        // The parse trims each piece, so the space between a point and the
        // sentence after it has to be put back, or the margin numeral runs
        // into the next word.
        let spaced = !fresh && text.starts_with(char::is_whitespace);
        for (ix, paragraph) in prose::parse(&text).into_iter().enumerate() {
            let continues = ix == 0 && !fresh && !paragraphs.is_empty();
            if !continues {
                paragraphs.push(Vec::new());
            }
            let current = paragraphs.last_mut().expect("pushed above");
            for (at, mut token) in paragraph.tokens.into_iter().enumerate() {
                if continues && spaced && at == 0 {
                    token.text.insert(0, ' ');
                }
                if token.mark == Some(Mark::Pane)
                    && let Some(ix) = named(token.text.trim())
                {
                    reached.push(ix);
                }
                // Words marked alike run together: a format sets a run,
                // not a word at a time.
                let mark = token.mark.map(Marked::from);
                match current.last_mut() {
                    Some(Run::Text { text, mark: was }) if *was == mark => {
                        text.push_str(&token.text);
                    }
                    _ => current.push(Run::Text {
                        text: token.text,
                        mark,
                    }),
                }
            }
        }
        let Some(spot) = spot else { continue };
        let numeral = match landed.iter().find(|l| l.spot == spot) {
            Some(before) => before.numeral,
            None => {
                let landing = prose::resolve_refs(&spot, refs, line_count);
                let panes: Vec<usize> = [landing.reading, landing.drawing, landing.paging]
                    .into_iter()
                    .flatten()
                    .collect();
                reached.extend(&panes);
                let numeral = u32::try_from(landed.len() + 1).unwrap_or(u32::MAX);
                landed.push(Landed {
                    numeral,
                    spot,
                    panes,
                });
                numeral
            }
        };
        if paragraphs.is_empty() {
            paragraphs.push(Vec::new());
        }
        paragraphs
            .last_mut()
            .expect("pushed above")
            .push(Run::Point { numeral });
    }
    paragraphs.retain(|paragraph| !paragraph.is_empty());
    (paragraphs, landed, reached)
}

fn pane_of(
    ix: usize,
    r: &Ref,
    landed: &[Landed],
    by_file: &BTreeMap<&str, &Snapshot>,
    palette: &Palette,
    paint: &dyn Paint,
) -> Pane {
    let marks = || -> Vec<(String, u32)> {
        landed
            .iter()
            .filter(|l| l.panes.contains(&ix))
            .filter_map(|l| l.spot.block.clone().map(|block| (block, l.numeral)))
            .collect()
    };
    match r {
        Ref::Code(spec) => Pane::Code(code_of(ix, spec, landed, by_file, palette, paint)),
        Ref::Diagram(diagram) => Pane::Diagram(Picture {
            id: diagram.id.clone(),
            name: diagram.name.clone(),
            note: diagram.note.clone(),
            svg: paint.diagram(&diagram.diagram),
            marks: marks(),
        }),
        Ref::Page(page) => Pane::Page(Page {
            id: page.id.clone(),
            name: page.name.clone(),
            note: page.note.clone(),
            html: page.page.clone(),
            marks: marks(),
        }),
    }
}

fn code_of(
    ix: usize,
    spec: &RefSpec,
    landed: &[Landed],
    by_file: &BTreeMap<&str, &Snapshot>,
    palette: &Palette,
    paint: &dyn Paint,
) -> Code {
    let file = path_of(spec);
    let change = match (&spec.after, &spec.before) {
        (Some(_), _) => Some(Change::Proposed),
        (None, Some(_)) => Some(Change::Made),
        (None, None) => None,
    };
    let mut code = Code {
        id: spec.id.clone(),
        name: spec.name.clone(),
        note: spec.note.clone(),
        file: file.clone(),
        range: spec.range,
        change,
        lines: Vec::new(),
        missing: false,
    };
    let Some(snapshot) = by_file.get(file.as_str()) else {
        code.missing = true;
        return code;
    };
    let Some(span) = snapshot
        .file
        .kept
        .iter()
        .copied()
        .find(|span| span.contains(spec.range.first))
    else {
        code.missing = true;
        return code;
    };
    let range = LineRange::new(spec.range.first, spec.range.last.min(span.last));

    // Numerals by line, for the lines points land on in this pane.
    let numeral_on = |line: u32, side: Side| {
        landed
            .iter()
            .filter(|l| l.panes.contains(&ix) && l.spot.side == side)
            .find(|l| l.spot.lines.is_some_and(|lines| lines.contains(line)))
            .map(|l| l.numeral)
    };
    let values_on = |line: u32, after: bool| -> Vec<Value> {
        spec.values
            .iter()
            .filter(|v| v.line == line && v.after == after)
            .cloned()
            .collect()
    };

    // The kept span's own text, coloured as one piece, so a string or a
    // comment that opens above the range is still one when it reaches it.
    let source = span_text(snapshot, span);
    let rows = colour(&source, &file, paint);
    let row_of = |no: u32, kind: RowKind, numeral, values| {
        let ground = ground_of(kind, palette);
        let at = usize::try_from(no - span.first).unwrap_or(usize::MAX);
        let spans = rows
            .get(at)
            .map(|runs| inked(runs, ground, palette))
            .unwrap_or_default();
        Line::Code(Row {
            no,
            kind,
            numeral,
            ground,
            spans,
            values,
        })
    };

    let first = range.first.saturating_sub(CONTEXT).max(span.first);
    let last = range.last.saturating_add(CONTEXT).min(span.last);
    for no in first..range.first {
        code.lines.push(row_of(
            no,
            RowKind::Context {
                distance: range.first - no,
            },
            None,
            Vec::new(),
        ));
    }

    let inside = match change {
        Some(Change::Made) => RowKind::Arriving,
        Some(Change::Proposed) => RowKind::Going,
        None => RowKind::Spot,
    };
    if let (Some(Change::Made), Some(before)) = (change, &spec.before) {
        // What went, above what arrived. Its lines have no number in the
        // file any more; they are numbered as the range was.
        push_text_rows(
            &mut code.lines,
            before,
            range.first,
            RowKind::Going,
            &file,
            palette,
            paint,
            &|_| None,
            &|_| Vec::new(),
        );
    }
    let marked = |no: u32| {
        let side = if inside == RowKind::Arriving {
            Side::Arriving
        } else {
            Side::Going
        };
        numeral_on(no, side).is_some() || !values_on(no, false).is_empty()
    };
    let mut no = range.first;
    while no <= range.last {
        // A long stretch with nothing on it folds, keeping its edges.
        let mut end = no;
        while end <= range.last && !marked(end) {
            end += 1;
        }
        let quiet = end - no;
        if quiet >= FOLD_AT {
            for line in (no..no + FOLD_EDGE).chain(end - FOLD_EDGE..end) {
                if line == end - FOLD_EDGE {
                    code.lines.push(Line::Gap {
                        lines: quiet - 2 * FOLD_EDGE,
                    });
                }
                code.lines.push(row_of(line, inside, None, Vec::new()));
            }
            no = end;
            continue;
        }
        let side = if inside == RowKind::Arriving {
            Side::Arriving
        } else {
            Side::Going
        };
        code.lines.push(row_of(
            no,
            inside,
            numeral_on(no, side),
            values_on(no, false),
        ));
        no += 1;
    }
    if let (Some(Change::Proposed), Some(after)) = (change, &spec.after) {
        push_text_rows(
            &mut code.lines,
            after,
            range.first,
            RowKind::Arriving,
            &file,
            palette,
            paint,
            &|line| numeral_on(line, Side::Arriving),
            &|line| values_on(line, true),
        );
    }
    for no in range.last + 1..=last {
        code.lines.push(row_of(
            no,
            RowKind::Context {
                distance: no - range.last,
            },
            None,
            Vec::new(),
        ));
    }
    code
}

/// Rows for text that is not in the file: the half of a change the deck
/// carries itself, numbered from `first`.
#[allow(clippy::too_many_arguments)]
fn push_text_rows(
    out: &mut Vec<Line>,
    text: &str,
    first: u32,
    kind: RowKind,
    file: &str,
    palette: &Palette,
    paint: &dyn Paint,
    numeral_on: &dyn Fn(u32) -> Option<u32>,
    values_on: &dyn Fn(u32) -> Vec<Value>,
) {
    if text.is_empty() {
        return;
    }
    let ground = ground_of(kind, palette);
    for (at, runs) in colour(text, file, paint).iter().enumerate() {
        let no = first + u32::try_from(at).unwrap_or(u32::MAX - first);
        out.push(Line::Code(Row {
            no,
            kind,
            numeral: numeral_on(no),
            ground,
            spans: inked(runs, ground, palette),
            values: values_on(no),
        }));
    }
}

/// The text of one kept span of a snapshot.
fn span_text(snapshot: &Snapshot, span: LineRange) -> String {
    (span.first..=span.last)
        .filter_map(|no| snapshot.line(no))
        .map(|line| format!("{}\n", line.strip_suffix('\r').unwrap_or(line)))
        .collect()
}

/// `source` coloured, as one list of runs per line: each run its text, its
/// colour if it has one of its own, and whether it leans.
fn colour(source: &str, file: &str, paint: &dyn Paint) -> Vec<Vec<(String, Option<Rgb>, bool)>> {
    let painted = paint.spans(source, file);
    let mut lines: Vec<Vec<(String, Option<Rgb>, bool)>> = vec![Vec::new()];
    let mut put = |text: &str, colour: Option<Rgb>, italic: bool| {
        for (ix, part) in text.split('\n').enumerate() {
            if ix > 0 {
                lines.push(Vec::new());
            }
            if !part.is_empty() {
                let line = lines.last_mut().expect("never empty");
                match line.last_mut() {
                    Some(last) if last.1 == colour && last.2 == italic => last.0.push_str(part),
                    _ => line.push((part.to_string(), colour, italic)),
                }
            }
        }
    };
    let mut at = 0;
    for run in painted {
        let (start, end) = (run.range.start.max(at), run.range.end.min(source.len()));
        if start >= end || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            continue;
        }
        put(&source[at..start], None, false);
        put(&source[start..end], run.colour, run.italic);
        at = end;
    }
    put(&source[at..], None, false);
    if source.ends_with('\n') {
        lines.pop();
    }
    lines
}

/// One line's runs with their colours settled and made legible on `ground`.
fn inked(runs: &[(String, Option<Rgb>, bool)], ground: Rgb, palette: &Palette) -> Vec<Span> {
    runs.iter()
        .map(|(text, colour, italic)| Span {
            text: text.clone(),
            ink: legible(colour.unwrap_or(palette.fg), ground, LEGIBLE),
            italic: *italic,
        })
        .collect()
}

/// The ground a row of this kind is printed on, as the window paints it.
fn ground_of(kind: RowKind, palette: &Palette) -> Rgb {
    match kind {
        RowKind::Context { .. } => palette.wash,
        RowKind::Spot => palette.focus,
        RowKind::Going => palette.gone,
        RowKind::Arriving => palette.fresh,
    }
}

fn ledger_of(pane: &Pane) -> Vec<Held> {
    let Pane::Code(code) = pane else {
        return Vec::new();
    };
    let label = code.name.clone().unwrap_or_else(|| code.file.clone());
    code.lines
        .iter()
        .filter_map(|line| match line {
            Line::Code(row) => Some(row),
            Line::Gap { .. } => None,
        })
        .flat_map(|row| {
            row.values.iter().map(|value| Held {
                pane: label.clone(),
                line: row.no,
                numeral: row.numeral,
                at: value.at.clone(),
                is: value.is.clone(),
                lit: value.lit,
            })
        })
        .collect()
}

/// What was said, by group, and what belongs to none.
///
/// A remark the reader wrote is recorded twice: as a comment, which carries
/// what was selected, the lines and the kind, and as a moment in the
/// transcript, which carries when. The comment is the remark; the moment
/// only lends it its time, matched by group, pane and words, in order. The
/// agent's answers are the moments it said. Moments of looking — what was
/// shown, a turn to the next group — are the walk, not the argument.
fn argument(
    groups: &[crate::protocol::Group],
    review: Option<&crate::protocol::Review>,
) -> (BTreeMap<String, Vec<Message>>, Vec<Message>) {
    let mut by_group: BTreeMap<String, Vec<Message>> = BTreeMap::new();
    let mut general = Vec::new();
    let Some(review) = review else {
        return (by_group, general);
    };
    let known = |id: &str| groups.iter().any(|g| g.id == id);
    let mut used = vec![false; review.transcript.len()];

    let mut messages: Vec<(Option<String>, Message)> = Vec::new();
    for comment in &review.comments {
        let at_ms = review
            .transcript
            .iter()
            .enumerate()
            .find(|(ix, m)| {
                !used[*ix]
                    && m.what == What::Wrote
                    && m.group.as_deref() == Some(comment.group.as_str())
                    && m.ref_id == comment.ref_id
                    && m.text.trim() == comment.text.trim()
            })
            .map(|(ix, m)| {
                used[ix] = true;
                m.at_ms
            });
        messages.push((
            Some(comment.group.clone()),
            Message {
                author: Author::Reader,
                ref_id: comment.ref_id.clone(),
                range: comment.range,
                quote: comment.quote.clone(),
                kind: Some(comment.kind),
                text: comment.text.clone(),
                at_ms,
            },
        ));
    }
    for (ix, moment) in review.transcript.iter().enumerate() {
        let author = match moment.what {
            What::Said => Author::Agent,
            What::Wrote | What::Reacted if !used[ix] => Author::Reader,
            _ => continue,
        };
        if moment.text.trim().is_empty() {
            continue;
        }
        messages.push((
            moment.group.clone(),
            Message {
                author,
                ref_id: moment.ref_id.clone(),
                range: moment.range,
                quote: String::new(),
                kind: moment.kind,
                text: prose::unbeat(&prose::unpoint(&moment.text)),
                at_ms: Some(moment.at_ms),
            },
        ));
    }
    // In the order things were said; what has no time keeps its place after
    // everything that does.
    messages.sort_by_key(|(_, message)| message.at_ms.unwrap_or(u64::MAX));
    for (group, message) in messages {
        match group {
            Some(id) if known(&id) => by_group.entry(id).or_default().push(message),
            _ => general.push(message),
        }
    }
    (by_group, general)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{CONTAINER, Manifest, snap};
    use crate::theme::{Imported, derive};

    /// Paint that colours every `let` in a shade far too close to the lit
    /// ground, so the test can see it lifted, and draws a picture as its
    /// node count.
    struct Faint(Rgb);

    impl Paint for Faint {
        fn spans(&self, source: &str, _file: &str) -> Vec<Painted> {
            source
                .match_indices("let")
                .map(|(at, word)| Painted {
                    range: at..at + word.len(),
                    colour: Some(self.0),
                    italic: false,
                })
                .collect()
        }

        fn diagram(&self, diagram: &Diagram) -> String {
            format!("<svg nodes=\"{}\"/>", diagram.nodes.len())
        }
    }

    fn palette() -> Palette {
        derive(Imported::plain(
            Rgb::from_hex("#282828").unwrap(),
            Rgb::from_hex("#ebdbb2").unwrap(),
            Rgb::from_hex("#fe8019").unwrap(),
        ))
    }

    /// The file the deck points into: line n says `line n`, and lines 143
    /// and 146 are the two decrements.
    fn batch_ts() -> String {
        (1..=200)
            .map(|n| match n {
                143 | 146 => "    let pending = pending - 1\n".to_string(),
                _ => format!("line {n}\n"),
            })
            .collect()
    }

    fn deck() -> Bundle {
        let source = batch_ts();
        let (kept, text) = snap::keep(
            &source,
            &[
                LineRange::new(139, 150),
                LineRange::new(10, 40),
                LineRange::new(190, 200),
            ],
            snap::CONTEXT,
        );
        let index = snap::Index {
            files: vec![snap::File {
                path: "src/batch.ts".to_string(),
                entry: "snap/1.txt".to_string(),
                sha256: crate::bundle::sha256(source.as_bytes()),
                commit: Some("a1b2c3d".to_string()),
                state: snap::State::Clean,
                kept,
            }],
        };
        let g1 = r#"{
            "id": "g1",
            "title": "The error path counts twice",
            "confidence": {"level": "medium", "why": "read both paths; did not run it"},
            "say": "A write that fails lands in the catch in [batch] and takes the counter down.[point 143] Nothing stops it there, so it goes on to the tail and takes it down again.[point 146]\n\nEvery failed write costs two. [path] follows one row through both, and the first decrement is the one to remove.[point 143] The [check] box is the question.[point check]",
            "refs": [
                {"id": "g1r1", "file": "src/batch.ts", "range": [190, 192], "name": "tail"},
                {"id": "g1d1", "name": "path", "diagram": {"nodes": [{"id": "check", "label": "write ok?", "role": "decision"}, {"id": "tail", "label": "tail"}], "edges": [{"from": "check", "to": "tail"}]}},
                {"id": "g1r2", "file": "src/batch.ts", "range": [139, 146], "name": "batch", "values": [
                    {"line": 143, "at": "pending", "is": "63", "lit": true},
                    {"line": 146, "at": "pending", "is": "62"}
                ]}
            ]
        }"#;
        let g2 = r#"{
            "id": "g2",
            "say": "A long stretch, pointed once in the middle.[point 25]",
            "refs": [{"id": "g2r1", "file": "src/batch.ts", "range": [10, 40]}]
        }"#;
        let g3 = r#"{
            "id": "g3",
            "title": "Count once",
            "say": "Count once, where the write settles.[point +143]",
            "refs": [{"id": "g3r1", "file": "src/batch.ts", "range": [143, 143], "name": "fix",
                      "after": "    } finally {\n        let pending = pending - 1\n"}],
            "asks": ["looks good"]
        }"#;
        let review = r#"{
            "v": 1, "deck": "d-1",
            "comments": [
                {"group": "g1", "ref": "g1r2", "range": [143, 143], "source": "extmark", "kind": "question",
                 "quote": "pending - 1", "text": "Why not decrement once, in a finally?"},
                {"group": "g3", "source": "stale", "kind": "nit", "text": "Name the test after the bug."}
            ],
            "transcript": [
                {"at_ms": 1000, "what": "shown", "group": "g1"},
                {"at_ms": 5000, "what": "wrote", "group": "g1", "ref": "g1r2", "text": "Why not decrement once, in a finally?"},
                {"at_ms": 9000, "what": "said", "group": "g1", "text": "A finally also runs on the retry branch.[point 143] [pause] Group three counts once."},
                {"at_ms": 12000, "what": "said", "text": "That is everything I had."},
                {"at_ms": 13000, "what": "turned", "group": "g2"}
            ]
        }"#;
        let mut files = BTreeMap::new();
        files.insert(
            "deck/deck.json".to_string(),
            br#"{"v":1,"id":"d-1","title":"Why the batch counter stalls at 63"}"#.to_vec(),
        );
        files.insert("deck/g1.json".to_string(), g1.as_bytes().to_vec());
        files.insert("deck/g2.json".to_string(), g2.as_bytes().to_vec());
        files.insert("deck/g3.json".to_string(), g3.as_bytes().to_vec());
        files.insert(
            "snap/index.json".to_string(),
            serde_json::to_vec(&index).unwrap(),
        );
        files.insert("snap/1.txt".to_string(), text.into_bytes());
        files.insert("review.json".to_string(), review.as_bytes().to_vec());
        let bundle = Bundle {
            manifest: Manifest {
                container: CONTAINER,
                protocol: 1,
                created: "2026-10-20T09:30:00Z".to_string(),
                generator: "deck 0.1.7".to_string(),
                repo: Some(Repo {
                    remote: Some("https://github.com/acme/ingest".to_string()),
                    commit: Some("a1b2c3d".to_string()),
                }),
                entries: BTreeMap::new(),
                allowed: Vec::new(),
            },
            files,
        };
        // Through the real format, so the model reads what a reader would.
        Bundle::open(&bundle.pack().expect("packs"))
            .expect("opens")
            .0
    }

    fn document() -> Document {
        let palette = palette();
        build(
            &deck(),
            &palette,
            &Faint(palette.focus.mix(palette.fg, 0.1)),
        )
        .expect("builds")
    }

    fn rows(pane: &Pane) -> Vec<&Row> {
        let Pane::Code(code) = pane else {
            panic!("a code pane")
        };
        code.lines
            .iter()
            .filter_map(|line| match line {
                Line::Code(row) => Some(row),
                Line::Gap { .. } => None,
            })
            .collect()
    }

    #[test]
    fn panes_come_in_the_order_the_prose_reaches_them() {
        let doc = document();
        // batch is named first, then path, then the picture's block; tail is
        // never reached, so it comes last.
        let ids: Vec<&str> = doc.groups[0]
            .panes
            .iter()
            .map(|pane| match pane {
                Pane::Code(code) => code.id.as_str(),
                Pane::Diagram(picture) => picture.id.as_str(),
                Pane::Page(page) => page.id.as_str(),
            })
            .collect();
        assert_eq!(ids, ["g1r2", "g1d1", "g1r1"]);
        assert_eq!(doc.contents[0].panes, ["batch", "path", "tail"]);
    }

    #[test]
    fn a_point_carries_one_numeral_to_the_sentence_and_the_lines() {
        let doc = document();
        let group = &doc.groups[0];
        let numerals: Vec<u32> = group
            .prose
            .iter()
            .flatten()
            .filter_map(|run| match run {
                Run::Point { numeral } => Some(*numeral),
                Run::Text { .. } => None,
            })
            .collect();
        // 143, 146, 143 again, then the picture's block.
        assert_eq!(numerals, [1, 2, 1, 3]);
        let batch = rows(&group.panes[0]);
        let on = |no: u32| {
            batch
                .iter()
                .find(|row| row.no == no)
                .and_then(|row| row.numeral)
        };
        assert_eq!((on(143), on(146), on(144)), (Some(1), Some(2), None));
        let Pane::Diagram(picture) = &group.panes[1] else {
            panic!()
        };
        assert_eq!(picture.marks, [("check".to_string(), 3)]);
        assert_eq!(picture.svg, r#"<svg nodes="2"/>"#);
    }

    #[test]
    fn a_point_keeps_the_space_after_it() {
        let doc = document();
        let after = doc.groups[0].prose[0]
            .iter()
            .skip_while(|run| !matches!(run, Run::Point { .. }))
            .nth(1);
        assert!(matches!(after, Some(Run::Text { text, .. }) if text.starts_with(" Nothing")));
    }

    #[test]
    fn the_prose_keeps_its_paragraphs_and_marks() {
        let doc = document();
        let prose = &doc.groups[0].prose;
        assert_eq!(prose.len(), 2);
        assert!(prose[0].iter().any(|run| matches!(
            run,
            Run::Text { text, mark: Some(Marked::Pane) } if text.trim() == "batch"
        )));
    }

    #[test]
    fn a_code_pane_shows_its_range_and_a_little_around_it() {
        let doc = document();
        let batch = rows(&doc.groups[0].panes[0]);
        assert_eq!(batch.first().unwrap().no, 139 - CONTEXT);
        assert_eq!(batch.last().unwrap().no, 146 + CONTEXT);
        let first = batch.first().unwrap();
        assert_eq!(first.kind, RowKind::Context { distance: CONTEXT });
        assert!(
            batch
                .iter()
                .filter(|r| (139..=146).contains(&r.no))
                .all(|r| r.kind == RowKind::Spot)
        );
        assert_eq!(
            batch.iter().find(|r| r.no == 143).unwrap().values[0].is,
            "63"
        );
    }

    #[test]
    fn a_range_past_the_end_of_the_file_stops_where_the_file_does() {
        // tail is 190-192 in a 200-line file; context runs to 200 and no
        // further, and the range is not clamped short.
        let doc = document();
        let tail = rows(&doc.groups[0].panes[2]);
        assert_eq!(tail.last().unwrap().no, 200);
    }

    #[test]
    fn a_long_quiet_stretch_folds_and_keeps_its_edges() {
        let doc = document();
        let Pane::Code(code) = &doc.groups[1].panes[0] else {
            panic!()
        };
        let gaps: Vec<u32> = code
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Gap { lines } => Some(*lines),
                Line::Code(_) => None,
            })
            .collect();
        // 10-24 is 15 quiet lines and 26-40 another 15: each keeps two at
        // each edge and folds eleven.
        assert_eq!(gaps, [11, 11]);
        let printed: Vec<u32> = rows(&doc.groups[1].panes[0])
            .iter()
            .filter(|r| r.kind == RowKind::Spot)
            .map(|r| r.no)
            .collect();
        assert_eq!(printed, [10, 11, 23, 24, 25, 26, 27, 39, 40]);
    }

    #[test]
    fn a_proposed_change_shows_what_goes_and_what_arrives() {
        let doc = document();
        let group = &doc.groups[2];
        let fix = rows(&group.panes[0]);
        let changed: Vec<(u32, RowKind, Option<u32>)> = fix
            .iter()
            .filter(|r| !matches!(r.kind, RowKind::Context { .. }))
            .map(|r| (r.no, r.kind, r.numeral))
            .collect();
        assert_eq!(
            changed,
            [
                (143, RowKind::Going, None),
                (143, RowKind::Arriving, Some(1)),
                (144, RowKind::Arriving, None)
            ]
        );
    }

    #[test]
    fn every_value_goes_in_the_ledger_with_its_numeral() {
        let doc = document();
        let ledger: Vec<(u32, Option<u32>, &str)> = doc.groups[0]
            .ledger
            .iter()
            .map(|held| (held.line, held.numeral, held.is.as_str()))
            .collect();
        assert_eq!(ledger, [(143, Some(1), "63"), (146, Some(2), "62")]);
    }

    #[test]
    fn a_remark_is_said_once_and_answers_follow_it() {
        let doc = document();
        let argument = &doc.groups[0].argument;
        // The remark from the comment, with the moment's time, then the
        // agent's answer with its point and beat taken out.
        assert_eq!(argument.len(), 2);
        assert_eq!(argument[0].author, Author::Reader);
        assert_eq!(argument[0].quote, "pending - 1");
        assert_eq!(argument[0].at_ms, Some(5000));
        assert_eq!(argument[1].author, Author::Agent);
        assert!(!argument[1].text.contains("[point"));
        assert!(!argument[1].text.contains("[pause"));
        // A remark with no moment still comes, after the timed ones.
        assert_eq!(
            doc.groups[2].argument[0].text,
            "Name the test after the bug."
        );
        assert_eq!(doc.contents[0].remarks, 2);
    }

    #[test]
    fn what_belongs_to_no_group_is_kept_for_the_end() {
        let doc = document();
        assert_eq!(doc.general.len(), 1);
        assert_eq!(doc.general[0].text, "That is everything I had.");
    }

    #[test]
    fn every_colour_reads_on_the_ground_it_is_printed_on() {
        let doc = document();
        let mut checked = 0;
        for group in &doc.groups {
            for pane in &group.panes {
                if let Pane::Code(_) = pane {
                    for row in rows(pane) {
                        for span in &row.spans {
                            assert!(
                                span.ink.contrast(row.ground) >= LEGIBLE,
                                "{:?} on line {}",
                                span.text,
                                row.no
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 50);
        // The faint `let` was lifted, not left.
        let batch = rows(&doc.groups[0].panes[0]);
        let row = batch.iter().find(|r| r.no == 143).unwrap();
        let keyword = row.spans.iter().find(|s| s.text == "let").unwrap();
        assert_ne!(keyword.ink, palette().focus.mix(palette().fg, 0.1));
    }

    #[test]
    fn a_file_not_in_the_deck_is_a_pane_with_nothing_to_print() {
        let mut bundle = deck();
        bundle.files.remove("snap/1.txt");
        bundle.files.remove("snap/index.json");
        let palette = palette();
        let doc = build(&bundle, &palette, &Faint(palette.fg)).expect("builds");
        let Pane::Code(code) = &doc.groups[0].panes[0] else {
            panic!()
        };
        assert!(code.missing && code.lines.is_empty());
    }

    /// The whole model of the fixture deck, pinned.
    ///
    /// Run with `UPDATE_GOLDEN=1` when a change is meant to move it, and read
    /// the diff: it is the shape every export writes from.
    #[test]
    fn the_fixture_deck_is_the_document_it_was() {
        let now = serde_json::to_string_pretty(&document()).expect("serialises") + "\n";
        let at =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/document.json");
        if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
            std::fs::write(&at, &now).expect("writable");
        }
        let was = std::fs::read_to_string(&at)
            .expect("the golden file exists; run with UPDATE_GOLDEN=1")
            .replace("\r\n", "\n");
        assert_eq!(
            now, was,
            "the document moved; if that was meant, run with UPDATE_GOLDEN=1"
        );
    }
}
