//! One code pane: a whole file, with its lit range findable inside it.
//!
//! The pane draws the code itself rather than embedding a text editor, and that
//! is the decision the rest of this module follows from.
//!
//! An editor was tried first. It brings tree-sitter highlighting for free, but
//! it owns its own scrolling and exposes no way to read or set the position. So
//! the gutter and the lit band, which have to be drawn beside and behind the
//! code, could only be siblings of it — and the moment the reader scrolled, the
//! code slid out from under them. Numbers pointed at the wrong lines and the
//! band sat still while the range moved away. No arrangement of three siblings
//! fixes that.
//!
//! Drawing the rows here makes each row one thing: its accent bar, its number
//! and its text, on one background. They cannot come apart, because there is
//! nothing to come apart. The list is virtual, so a whole file costs a
//! screenful of work, and the reader can scroll to line 1 or to the end of the
//! file like anywhere else.
//!
//! The highlighter is still the library's, driven through `deck_render::styled`
//! so that every export colours code with the same call.

use std::ops::Range;

use deck_core::line::LineRange;
use deck_core::protocol::{RefSpec, Value};
use deck_core::theme::{Palette, Rgb};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette::paint;
use crate::sheet::Slot;
use crate::view::DeckView;

// Row height, as a multiple of the monospace size: measured once, with the
// rest of the window, in `deck_render::metrics`.
use deck_render::metrics::LINE_HEIGHT;
// Where a value hangs on its line, shared with the exports so a value sits
// in the same place on paper.
use deck_render::{after_name, whole_word};

/// Lines of context shown above the range when the pane opens.
const LEAD_IN: u32 = 2;

/// How wide a pane is once it is folded down to its spine.
///
/// The same width the conversation folds to on the other side, so a window with
/// one of each has a matching margin down both edges.
///
/// Wide enough for a short word set the way words are set. It was narrower, and
/// the name had to be a single letter — which told nobody anything about which
/// pane they were looking at.
pub(crate) const SPINE: f32 = 58.;

/// How long a light takes to come up.
const RISE: std::time::Duration = std::time::Duration::from_millis(520);

/// How long a light takes to go out on its own, when nothing replaces it.
const FALL: std::time::Duration = std::time::Duration::from_millis(760);

/// How long it takes to go out when something else is arriving.
///
/// Much quicker, and this is the whole of the shimmer. A light leaving over
/// 760ms while the next arrives over 520 leaves two of them on at once for a
/// quarter of a second — three, if the reader is moving from claim to claim the
/// way they are meant to. What that looks like is the old lines flickering as
/// the new ones land, which is exactly what it is.
///
/// Leaving is not the same act as being replaced. Out of the way, and quickly.
const HAND_OVER: std::time::Duration = std::time::Duration::from_millis(200);

/// How long a value takes to open: the code below glides down to make room,
/// and the value's words come up into it.
///
/// Rows appearing at once pushed the rest of the file down in a jump, and a
/// jump is the reader losing their place — so it moves. But not slowly. A
/// value opens because a sentence reached it, at the same moment the pointed
/// lines light, and those take 520ms. A whole second here left the reader
/// looking at lit lines and an empty gap, waiting for the thing the sentence
/// was about. Under the point light, so the value is there when the light is.
pub(crate) const UNFOLD: std::time::Duration = std::time::Duration::from_millis(450);
/// How long it takes to close, a little quicker than it opened, so a value
/// put away is out of the way before the next sentence needs the room.
pub(crate) const REFOLD: std::time::Duration = std::time::Duration::from_millis(320);

/// How long a light the reader raised takes to come up.
///
/// Every timing above this one answers to a voice, and a voice is slow on
/// purpose. A light that answers a hand is a different thing: the reader
/// already knows what they did, and anything that takes half a second to agree
/// with them reads as the window thinking about it.
pub(crate) const HAND_RISE: std::time::Duration = std::time::Duration::from_millis(150);

/// And how long it takes to go out.
///
/// Longer than it took to arrive, on purpose. Coming up answers a gesture and
/// should feel caused by it; going out answers nothing, and a band that simply
/// stops existing reads as a glitch rather than as a hand being taken away.
pub(crate) const HAND_FALL: std::time::Duration = std::time::Duration::from_millis(420);

/// A pace, as the milliseconds a `Fade` stores it as.
fn ms(of: std::time::Duration) -> u16 {
    u16::try_from(of.as_millis()).unwrap_or(u16::MAX)
}

/// That pace back, or `fallback` when none was given.
fn span(stored: u16, fallback: std::time::Duration) -> std::time::Duration {
    if stored == 0 {
        fallback
    } else {
        std::time::Duration::from_millis(u64::from(stored))
    }
}

/// The width of the frame round the pane being talked about, and of the
/// margin a page keeps for it.
pub(crate) const FRAME: f32 = 2.;

/// The frame round the pane being talked about, at `level` of its fade.
///
/// One for every kind of pane, so a file, a picture and a page cannot drift
/// apart. Laid over the pane rather than drawn as its border, so lighting it
/// moves nothing, and with no handlers, so nothing under it stops being
/// clickable.
pub(crate) fn outline(palette: &Palette, level: f32) -> Div {
    div()
        .absolute()
        .inset_0()
        .border(px(FRAME))
        .border_color(paint(palette.wash.mix(palette.accent, 0.9 * level)))
}

/// A light easing between off and on, from wherever it was when it was told.
///
/// Time-based rather than a GPUI animation, for the reason the live room is:
/// an animation keyed to a list of rows that changes length between frames is
/// a bounds-check panic. A start instant cannot go out of step with the tree.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Fade {
    on: bool,
    /// The level it was at when it was last told to change.
    from: f32,
    since: Option<std::time::Instant>,
    /// Milliseconds a light of its own pace takes, or nought for the
    /// narration's.
    ///
    /// Milliseconds rather than a `Duration` because a `Pane` carries four of
    /// these and sits in an enum beside much smaller things: two `u16` cost
    /// four bytes where two `Option<Duration>` cost thirty-two, and no light
    /// in this window runs for a minute.
    fall_ms: u16,
    rise_ms: u16,
}

impl Fade {
    /// Head towards `on`. Says whether that was a change.
    pub(crate) fn set(&mut self, on: bool) -> bool {
        if self.on == on {
            return false;
        }
        // From the level it had reached, not from the end it was heading for.
        // A light turned back halfway would otherwise flash.
        self.from = self.level();
        self.on = on;
        self.since = Some(std::time::Instant::now());
        true
    }

    /// Stay on, but start rising again from no higher than `to`.
    ///
    /// For a light whose content changes in one frame while it is up: dipping
    /// it a little and letting it rise on its normal fade makes the change
    /// something seen rather than a cut.
    pub(crate) fn dip(&mut self, to: f32) {
        self.from = self.level().min(to);
        self.on = true;
        self.since = Some(std::time::Instant::now());
    }

    /// Give this light its own pace.
    ///
    /// The narration's timings answer to a voice. A light that answers to a
    /// hand wants different ones: quick enough in that it feels like the
    /// gesture caused it, slow enough out that letting go is something you
    /// watch rather than something that has already happened.
    #[must_use]
    pub(crate) fn paced(mut self, rise: std::time::Duration, fall: std::time::Duration) -> Self {
        self.rise_ms = ms(rise);
        self.fall_ms = ms(fall);
        self
    }

    /// Go out quickly, because something else is arriving.
    pub(crate) fn hand_over(&mut self) {
        self.fall_ms = ms(HAND_OVER);
        self.set(false);
    }

    /// Nought to one, eased at both ends.
    pub(crate) fn level(&self) -> f32 {
        let to = if self.on { 1. } else { 0. };
        let Some(since) = self.since else {
            return to;
        };
        let whole = if self.on {
            span(self.rise_ms, RISE)
        } else {
            span(self.fall_ms, FALL)
        };
        let along = (since.elapsed().as_secs_f32() / whole.as_secs_f32()).clamp(0., 1.);
        // Smoothstep: no snap at either end, which is the whole ask.
        let eased = along * along * (3. - 2. * along);
        self.from + (to - self.from) * eased
    }

    /// Head towards `on` from wherever the straight-time level has got to,
    /// for a light read with [`Fade::linear`]. Turned back halfway, `set`
    /// would restart from the eased level and the motion would jump.
    pub(crate) fn set_linear(&mut self, on: bool) -> bool {
        if self.on == on {
            return false;
        }
        self.from = self.linear();
        self.on = on;
        self.since = Some(std::time::Instant::now());
        true
    }

    /// Nought to one, straight with time: for a motion that brings its own
    /// curve and should not be eased twice.
    pub(crate) fn linear(&self) -> f32 {
        let to = if self.on { 1. } else { 0. };
        let Some(since) = self.since else {
            return to;
        };
        let whole = if self.on {
            span(self.rise_ms, RISE)
        } else {
            span(self.fall_ms, FALL)
        };
        let along = (since.elapsed().as_secs_f32() / whole.as_secs_f32()).clamp(0., 1.);
        self.from + (to - self.from) * along
    }

    /// Whether it is on, or on its way there.
    pub(crate) fn on(&self) -> bool {
        self.on
    }

    /// Whether it is still on its way somewhere.
    pub(crate) fn moving(&self) -> bool {
        let whole = if self.on {
            span(self.rise_ms, RISE)
        } else {
            span(self.fall_ms, FALL)
        };
        self.since.is_some_and(|since| since.elapsed() < whole)
    }
}

/// One line, ready to draw.
#[derive(Clone)]
struct Row {
    text: SharedString,
    /// Syntax runs, as byte ranges within `text`.
    runs: Vec<(Range<usize>, HighlightStyle)>,
    /// The line this is in the file, or `None` for a line that is not in it
    /// yet.
    number: Option<u32>,
    /// The file line this row answers for.
    ///
    /// Its own, for a line that is in the file. For a spliced one it is the
    /// nearest line of the range being changed — below it for an `after`,
    /// above it for a `before` — because that is the only real place a
    /// proposed line can be said to be.
    ///
    /// Everything that asks *is this row part of what is being talked about*
    /// asks it of this: the spotlight, a point, a selection, a comment mark.
    /// They all asked `number` once, and all answered no for a proposed line,
    /// so a point could not light the arriving half of a change and a diff
    /// could not be selected or commented on at all.
    pin: u32,
    /// The lines this row answers for, as a range.
    ///
    /// Its own single line, for a line that is in the file. The whole range
    /// being changed, for a spliced one — and that is the part that has to be a
    /// range rather than a point. A `[point 320-338]` inside a change of
    /// 315-340 shares no line with a single pin of 340, so the arriving half
    /// stayed dark while the sentence was about it.
    answers: LineRange,
    /// What the ref is proposing about this line, if anything.
    change: Option<Change>,
    /// A single value drawn right after a name on this line: where in the
    /// text it goes, what it says, and whether the claim turns on it.
    inlay: Vec<(usize, SharedString, bool, Who)>,
    /// What this row is when it is not a line of code: the preview of a value
    /// under the line that made it, or one key of that value, opened.
    extra: Option<Extra>,
}

/// What a pane's values were doing, carried across a rebuild.
pub struct Held {
    /// The values themselves.
    pub values: Vec<Value>,
    open: std::collections::HashSet<Who>,
    prose_open: std::collections::HashSet<Who>,
    shown: bool,
    reached: bool,
}

/// Which value a row belongs to: its line and the name it is written for.
///
/// Not the line alone. Two values with keys can hang under one line, and keyed
/// by line they opened and closed together.
pub(crate) type Who = (u32, SharedString);

/// A row that carries a value rather than a line of the file.
///
/// Under the line it belongs to, never beside it. A value written after a long
/// line is past the pane's edge, and one written over the code hides the code,
/// which is the one thing a pane is for.
#[derive(Clone)]
enum Extra {
    /// `↳ name ▶ Preview { key: value, … }` — the handle that opens the keys.
    Chip {
        who: Who,
        name: SharedString,
        preview: SharedString,
        open: bool,
        /// Characters in from the left, so it starts where the code does.
        indent: usize,
    },
    /// One key inside an opened value, or a run of keys left out.
    Key {
        /// The value this key is inside.
        who: Who,
        name: SharedString,
        is: SharedString,
        why: Option<SharedString>,
        lit: bool,
        more: bool,
        /// The width of the key column, in characters: the longest key named.
        width: usize,
        indent: usize,
        /// The blank row above the keys that gives them room to breathe.
        pad: bool,
    },
    /// Lines the deck did not keep: `lines 1–118 not in this deck`.
    ///
    /// One row, the height of a line of code, with no number — it is not a
    /// line — on a ground of its own, so the reader sees at once that the
    /// code goes on past it and that this copy stops here.
    Absent {
        /// The first line left out.
        first: u32,
        /// The last, or `None` when the deck does not say how long the file
        /// was.
        last: Option<u32>,
    },
}

impl Row {
    /// Whether a point aimed at `side` means this row.
    ///
    /// The arriving side is *only* the arriving rows. It is tempting to say an
    /// unchanged row belongs to both — there is only one of it, and pointing at
    /// code that is not changing is the same act either way — and that is wrong
    /// for one reason: the arriving rows are numbered as they will be, so they
    /// collide with the real lines further down the file. A change of five
    /// lines into six, at 340, gives green rows numbered 340 to 345 while the
    /// file below still has its own 345 and 346. `+340-346` then lit the six
    /// green rows and two ordinary lines under them.
    ///
    /// So `+` means the change and nothing else, and a bare point means the
    /// file as it stands — the replaced lines and every unchanged line around
    /// them, which is what pointing at code has always meant.
    fn takes(&self, side: crate::prose::Side) -> bool {
        match side {
            crate::prose::Side::Going => !matches!(self.change, Some(Change::New)),
            crate::prose::Side::Arriving => matches!(self.change, Some(Change::New)),
        }
    }
}

/// What a ref carrying an `after` or a `before` is saying about a line.
///
/// Two directions, and only one of them is a proposal. With `after`, the change
/// has not been made: the lit range is what would go, the replacement is what
/// would come, and deck never writes to the file — the agent showing its work
/// before doing it, which is the only way a reader gets to say *no* before it
/// happens rather than after. With `before`, the change is already on disk: the
/// lit range is what arrived and the spliced rows are what it replaced.
///
/// Either way the row without a number is the one that is not in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// A line that is going, or has already gone.
    Gone,
    /// A line that is arriving, or has already arrived.
    New,
}

/// Which way the lit range lies from what is on screen.
///
/// The button that goes back to it says so. It always said *down*, which is
/// wrong half the time and worse than saying nothing: an arrow is a promise
/// about where you are going, and one that points the wrong way is read before
/// it is disbelieved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Away {
    /// Scrolled past: the range is behind you.
    Above,
    /// Not reached yet.
    Below,
}

/// A remark, as the pane needs it to draw one.
pub struct Mark {
    /// The lines it is about.
    pub range: LineRange,
    /// What was said.
    pub text: SharedString,
    /// Which remark this is, so the card can name one to fold or drop.
    pub remark_ix: usize,
    /// Whether it is folded down to its header.
    pub folded: bool,
}

/// A word set on its side, as a picture.
///
/// Nothing in this toolkit can turn text: a glyph goes into the scene with the
/// identity transform and no way to pass another one. A *picture* can be turned,
/// and gpui rasterises SVG with the system fonts loaded — so the word goes into
/// a tiny SVG that already has it rotated, and that is painted where the text
/// would have been.
///
/// Left-hand spines read upwards and right-hand ones downwards, which is the way
/// vertical tabs are set everywhere else, and it keeps the start of the word
/// nearest the pane it belongs to.
///
/// The rendered picture is cached by path and size, so the path carries the word
/// and the colour: two spines the same size with different names would otherwise
/// be handed each other's picture.
pub fn sideways(
    text: &str,
    family: &str,
    size: f32,
    weight: u16,
    tone: deck_core::theme::Rgb,
    up: bool,
) -> impl IntoElement {
    // Roughly what the word will measure. It only has to be big enough: the
    // text is centred in the box, and a box a little long is invisible.
    #[allow(clippy::cast_precision_loss)]
    let length = (text.chars().count() as f32).mul_add(size * 0.68, 10.);
    let across = size * 1.5;
    let turn = if up { -90 } else { 90 };
    // The picture finds its own fonts, apart from the window's: a face the
    // window draws with can be one it cannot find, and then it fell back to a
    // serif. So the family goes with faces every platform has, and the
    // generic last, and a missing one falls back to a monospace.
    let family = family.replace(['"', '<', '>', '&'], "");
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{across}" height="{length}" viewBox="0 0 {across} {length}"><text x="{cx}" y="{cy}" transform="rotate({turn} {cx} {cy})" font-family="{family}, SF Mono, Menlo, Consolas, DejaVu Sans Mono, monospace" font-size="{size}" font-weight="{weight}" text-anchor="middle" dominant-baseline="central" fill="#000">{text}</text></svg>"##,
        across = across,
        length = length,
        cx = across / 2.,
        cy = length / 2.,
        turn = turn,
        family = family,
        size = size,
        weight = weight,
        text = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;"),
    );
    // Keyed by everything that changes the picture, because the atlas keys it
    // by this and the size alone.
    let key: SharedString = format!("deck-sideways:{turn}:{size}:{weight}:{text}").into();
    let data: std::sync::Arc<[u8]> = svg.into_bytes().into();

    gpui_kit::canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| {
            let _ = window.paint_svg(
                bounds,
                key.clone(),
                Some(&data),
                gpui_kit::TransformationMatrix::unit(),
                paint(tone),
                cx,
            );
        },
    )
    .w(px(across))
    .h(px(length))
}

/// One code pane.
pub struct Pane {
    /// What the pane says in place of code, when it has none to show.
    missing: Option<Missing>,
    /// The note at the end of the header, for a deck from a `.deck` file
    /// opened inside the repo it came from. See [`Aside`].
    pub aside: Option<Aside>,
    /// Shared rather than owned, because every frame hands the whole list to
    /// the closure that draws it, and a frame of a value opening is sixty of
    /// them a second: copying every line of the file, runs and all, to draw
    /// the forty on screen was most of what a frame cost.
    rows: std::rc::Rc<[Row]>,
    /// The rows of the file and its change, before any value rows go in.
    base: Vec<Row>,
    /// The values the ref carries, as written.
    values: Vec<Value>,
    /// The lines whose value the reader has opened.
    open: std::collections::HashSet<Who>,
    /// How far open each value is that has been touched, so opening and
    /// closing can be watched rather than happening between two frames.
    unfolding: Vec<(Who, Fade)>,
    /// Values coming in, going out, or out: written live and still arriving,
    /// or folded away because the narration is pointing somewhere else.
    ///
    /// A value with no entry here is simply there.
    presence: Vec<(Who, Fade)>,
    /// Whether the pane's values are out. One gate for all of them: they come
    /// and go together, by the eye in the header, never one line at a time.
    shown: bool,
    /// Whether the narration has reached this pane's values yet. A pane its
    /// prose points into starts with them away, and they come out once, the
    /// first time a sentence points at one of their lines.
    reached: bool,
    /// A value has been sent away and its rows are still drawn, to be taken
    /// out once it has finished going.
    hiding: bool,
    /// The sentence of the narration the reader has picked, as last seen.
    prose: Option<String>,
    /// Values the picked sentence opened, by naming one of their keys. They
    /// close again when the sentence is let go — unless the reader opened them
    /// by hand, which takes them off this list.
    prose_open: std::collections::HashSet<Who>,
    /// Which row each file line is drawn on.
    ///
    /// Value rows go in under their lines, wherever those are, so a line's row
    /// can no longer be worked out from one splice. It is looked up instead.
    ///
    /// As pairs of line and row, in line order, rather than a slot per line:
    /// the deck's copy of a file may hold lines near its far end, and a table
    /// as long as the file would be as long as the file.
    line_row: Vec<(u32, usize)>,
    /// The rows that stand for lines a deck's copy left out: the first line,
    /// the last when it is known, and the row.
    edges: Vec<(u32, Option<u32>, usize)>,
    /// The half of a change that is not on disk, whose numbers are not the
    /// file's: the arriving half of a proposal, the going half of a made one.
    unwritten: Option<Change>,
    /// The independently movable range the live walk asks the reader to see.
    spotlight: LineRange,
    /// The narrower range the narration is pointing at, inside the spotlight.
    ///
    /// The spotlight says which part of the file the reader should be looking
    /// at, and it holds still while several sentences are said about it. This
    /// is the agent's finger inside that, and it moves with the sentences.
    point: Option<(LineRange, crate::prose::Side)>,
    /// The lines coming up, and how far up they are.
    rising: Fade,
    /// The lines the finger just left, still going out.
    /// Lights on their way out: the range, which half of a change it meant,
    /// and how far gone it is.
    ///
    /// The side is not decoration. Without it a departing light landed on every
    /// row whose number it overlapped, both halves of a change alike — so
    /// leaving the going half lit the arriving half on its way out, and the
    /// reader saw a half they had never pointed at flash as they moved away
    /// from the other one.
    was: Vec<(LineRange, crate::prose::Side, Fade)>,
    /// Whether anything in the pane is pointed at, which is what dims the rest
    /// of the lit range's bar.
    pointing: Fade,
    /// Whether the agent is talking about this pane at all.
    heeded: Fade,
    /// Number of source lines, excluding proposed replacement rows.
    source_lines: u32,
    /// How many rows a change spliced into the file's own.
    ///
    /// Kept because everything else here counts in file lines, and a spliced
    /// row has no line to count. See [`Pane::row_of`].
    added: usize,
    /// The last file line that sits *above* those spliced rows.
    ///
    /// An `after` splices under the range and a `before` splices over it, so
    /// which lines are pushed down differs by which one was written. Without
    /// this the two cases share arithmetic that is only right for one of them,
    /// and the failure is a comment card landing a few lines off.
    added_at: u32,
    scroll: UniformListScrollHandle,
    /// The same rows, sideways.
    ///
    /// Code does not wrap, so a long line has to be reachable somehow. It used
    /// to be cut off at the pane's right edge with no way to see the rest,
    /// which on a narrow pane hid the end of most interesting lines.
    across: ScrollHandle,
    /// The longest line in the file, in characters.
    widest: usize,
    /// How far a proposal that just landed has come up.
    ///
    /// A change made during a walk replaces rows under the reader's eye. Coming
    /// up over half a second, it reads as an answer arriving; painted at once,
    /// it reads as the file having been different all along.
    arriving: Fade,
    /// The replacement this pane is currently drawing as a change, if any.
    ///
    /// Authored refs carry one from the start; a live answer can put one here
    /// and take it away again, which is why it is remembered rather than
    /// worked out from the rows.
    proposed: Option<String>,
    label: SharedString,
    note: Option<SharedString>,
    /// Where the lit range sits in the row list, and how tall a row is.
    ///
    /// Both are only knowable while drawing — a remark's own rows push the
    /// code below them down, and the row height follows the font size. They
    /// are recorded on the way past so the questions "is the range in view"
    /// and "where would it be" can be answered between frames.
    laid_out: std::cell::Cell<Option<(usize, f32)>>,
    /// The ref this pane shows, so a comment can name it.
    pub ref_id: SharedString,
    /// What the prose calls this pane, when the agent named it.
    pub name: Option<SharedString>,
    /// The file, for the review payload and for relocating later.
    pub file: std::path::PathBuf,
    /// The lines the reader has picked, if any.
    ///
    /// A range rather than a line: an objection is often about a block — a
    /// whole match arm, a whole guard — and pinning it to the first line of
    /// that block loses which block was meant.
    pub selected: Option<LineRange>,
    /// The rows the reader has hold of, as indices into `rows`.
    ///
    /// Selection is kept twice on purpose, and the two answer different
    /// questions. `selected` is where a comment lands, which has to be file
    /// lines, because that is the only thing an editor can be pointed at and
    /// the only thing a range can be relocated through.
    ///
    /// This is what is *drawn*, and it cannot be file lines. Every arriving
    /// line of a change shares one — they replace the same range, and none of
    /// them is in the file — so a selection kept only in lines could not tell
    /// the first new line from the fourth, and dragging across them lit all or
    /// nothing. Rows are the only thing that distinguishes them.
    picked: Option<(usize, usize)>,
    /// The pick as it is *drawn*: the rows, and how far their light has come
    /// up or gone down.
    ///
    /// Separate from `picked` because it has to outlive it. Letting go clears
    /// where a comment would land straight away — nothing should land on a
    /// range nobody is holding — while the band it leaves behind is still on
    /// screen, going out.
    held: Option<((usize, usize), Fade)>,
}

/// What a pane with no code to show says instead.
#[derive(Clone)]
struct Missing {
    /// The file, set in the code face.
    file: SharedString,
    /// The rest of the sentence.
    said: SharedString,
    /// What to offer instead: the reader's own copy, or the way back to the
    /// deck's.
    offer: Option<crate::load::Offer>,
}

/// The note a pane's header ends with when the deck came from a `.deck` file
/// and the reader is inside the repo it was written in.
///
/// Muted, and at the end of the header rather than across the pane: it is a
/// fact about where the code stands, not a thing the reader has to deal with
/// before reading on. The deck's copy is the default, because it is what they
/// were sent; the switch is there for the reader who wants to see where the
/// code went.
#[derive(Clone, PartialEq, Eq)]
pub enum Aside {
    /// Showing the deck's copy, and the checkout is at another commit:
    /// `code has moved since a1b2c3d · Show live`.
    Moved {
        /// The commit the deck was packed at, short.
        since: SharedString,
    },
    /// Showing the deck's copy, and the checkout is at the same commit but
    /// holds something else for this file — the deck was packed from changes
    /// nobody committed, or the reader has their own:
    /// `differs from the deck's copy · Show live`.
    Differs,
    /// Showing the checkout: `● live, from 9f8e7d6 · Show the deck's copy`.
    Live {
        /// The commit the checkout is at, short.
        head: SharedString,
    },
    /// Showing the checkout everywhere else, but this pane's lines could not
    /// be followed into it, so the pane keeps the deck's copy:
    /// `couldn't follow these lines into your checkout · Show the deck's copy`.
    Lost,
}

impl Pane {
    /// Build a pane for `code` out of `source`.
    ///
    /// A whole file is drawn whole. The deck's own copy is drawn as only the
    /// spans it kept, each still numbered as the file was, with a row at
    /// every edge saying which lines are not there — so the reader always
    /// knows that the code goes on, and how far, and that scrolling has
    /// reached the end of what this deck can show rather than the end of the
    /// file.
    pub fn new(code: &RefSpec, source: &crate::load::Source, cx: &App) -> Self {
        let missing = match source {
            crate::load::Source::Missing { file, said, offer } => Some(Missing {
                file: file.display().to_string().into(),
                said: said.to_string().into(),
                offer: *offer,
            }),
            _ => None,
        };
        let (mut rows, total, kept) = match source {
            crate::load::Source::Live(text) => {
                let lines: Vec<&str> = text.lines().collect();
                let total = u32::try_from(lines.len()).unwrap_or(u32::MAX);
                (highlight(text, &lines, &code.file, cx), total, None)
            }
            // Span by span, each highlighted on its own and numbered from
            // where it sat in the file. Nothing is padded out to the file's
            // length: a deck is a span's two numbers away from asking for any
            // number of lines, and only the ones it holds are laid out.
            crate::load::Source::Snapshot(snapshot) => {
                let lines = snapshot.lines();
                let mut rows = Vec::new();
                for span in lines.spans() {
                    let held: Vec<&str> = (span.first..=span.last)
                        .filter_map(|n| lines.get(n))
                        .map(|line| line.strip_suffix('\r').unwrap_or(line))
                        .collect();
                    let text: String = held.iter().map(|line| format!("{line}\n")).collect();
                    let mut some = highlight(&text, &held, &code.file, cx);
                    for (offset, row) in some.iter_mut().enumerate() {
                        let at = span.first + u32::try_from(offset).unwrap_or(0);
                        row.number = Some(at);
                        row.pin = at;
                        row.answers = LineRange::single(at);
                    }
                    rows.extend(some);
                }
                let last = lines.spans().last().map_or(0, |span| span.last);
                (rows, last, Some((lines.spans(), snapshot.file.lines)))
            }
            crate::load::Source::Missing { .. } => (Vec::new(), 0, None),
        };
        let total = total.max(1);
        let authored = code.range.clamp_to(total);
        let mut added = 0usize;
        // Where the authored range is in the rows. The same as its lines in a
        // whole file; in the deck's copy, wherever its span put it — and
        // nowhere, when the copy does not hold it, which leaves nothing to
        // mark.
        let marked = rows
            .iter()
            .position(|row| row.number == Some(authored.first))
            .zip(
                rows.iter()
                    .position(|row| row.number == Some(authored.last)),
            )
            .filter(|(first, last)| first <= last)
            .map(|(first, last)| first..last + 1);

        // A proposed change, drawn in place: the range marked as going, and the
        // replacement spliced in under it. Every row below then sits one lower
        // per line added, which is why nothing may look a row up by its line
        // number afterwards — see `row_of`.
        let mut added_at = authored.last;
        // A missing file has no rows to mark, and the range would index into
        // nothing.
        let drawn = missing.is_none() && marked.is_some();
        let marked = marked.unwrap_or(0..0);
        if let Some(after) = code.after.as_deref().filter(|_| drawn) {
            for row in &mut rows[marked.clone()] {
                row.change = Some(Change::Gone);
            }

            let coming: Vec<&str> = after.lines().collect();
            let mut fresh = highlight(after, &coming, &code.file, cx);
            for (offset, row) in fresh.iter_mut().enumerate() {
                // The number it will have once the edit lands, so the gutter
                // reads as the file afterwards rather than as a blank.
                //
                // A blank was the honest answer to "which line is this" and the
                // wrong one to live with: it left the arriving half of a change
                // with no address at all, so nothing could point at it, select
                // it or comment on it, and an agent walking a change could not
                // say *and this is what it becomes*.
                let at = authored.first + u32::try_from(offset).unwrap_or(0);
                row.number = Some(at);
                row.pin = at;
                row.answers = LineRange::single(at);
                row.change = Some(Change::New);
            }
            added = fresh.len();
            rows.splice(marked.end..marked.end, fresh);
        } else if let Some(before) = code.before.as_deref().filter(|_| drawn) {
            // The other direction, and the one that reads oddly until you see
            // why: here the *range* is the new code. It is on disk, so it keeps
            // its numbers, and what it replaced has none — the file does not
            // contain those lines any more, and pretending otherwise would put
            // numbers on the pane that nothing in the repository answers to.
            //
            // Empty falls out of this correctly and on purpose: the range is
            // marked as arrived and nothing is spliced over it, which is the
            // pane for code that replaced nothing. That is the commonest thing
            // an agent has to show — *here is what I added* — and the one a
            // highlight cannot say, because a highlight of new lines looks the
            // same as a highlight of lines that were always there.
            for row in &mut rows[marked.clone()] {
                row.change = Some(Change::New);
            }

            // Empty means it replaced nothing, so there is nothing to splice
            // and the marked range is the whole answer. Going through the rest
            // of this with no text was the crash: the highlighter reports a run
            // even for an empty document, and the line table it is indexed
            // against has no lines in it.
            if !before.is_empty() {
                let went: Vec<&str> = before.lines().collect();
                let mut gone = highlight(before, &went, &code.file, cx);
                for (offset, row) in gone.iter_mut().enumerate() {
                    // The number it had before the edit, for the same reason:
                    // the replaced half needs an address too, and the one it
                    // answered to is the one it used to have.
                    let at = authored.first + u32::try_from(offset).unwrap_or(0);
                    row.number = Some(at);
                    row.pin = at;
                    row.answers = LineRange::single(at);
                    row.change = Some(Change::Gone);
                }
                added = gone.len();
                // Over the range, not under it: a diff reads downwards, and
                // the half that is going goes first.
                added_at = authored.first - 1;
                rows.splice(marked.start..marked.start, gone);
            }
        }

        // The deck's copy: only the spans it kept, and a row at each edge.
        if let Some((kept, lines)) = kept {
            rows = trimmed(rows, kept, lines);
        }

        // Open on the range, a couple of lines above it so it does not start
        // hard against the top edge. The scroll is deferred to the next layout,
        // which is the only moment the list knows how tall it is.
        let scroll = UniformListScrollHandle::new();
        let widest = rows
            .iter()
            .map(|row| row.text.chars().count())
            .max()
            .unwrap_or(0);
        let top = authored.first.saturating_sub(LEAD_IN).max(1);
        scroll.scroll_to_item((top - 1) as usize, ScrollStrategy::Top);

        let unwritten = if code.after.is_some() {
            Some(Change::New)
        } else if code.before.is_some() {
            Some(Change::Gone)
        } else {
            None
        };
        let mut pane = Self {
            missing,
            aside: None,
            rows: std::rc::Rc::from(Vec::new()),
            base: rows,
            values: code.values.clone(),
            open: std::collections::HashSet::new(),
            unfolding: Vec::new(),
            presence: Vec::new(),
            shown: true,
            reached: true,
            hiding: false,
            prose: None,
            prose_open: std::collections::HashSet::new(),
            line_row: Vec::new(),
            edges: Vec::new(),
            unwritten,
            spotlight: authored,
            source_lines: total,
            added,
            added_at,
            scroll,
            across: ScrollHandle::new(),
            widest,
            label: format!("{}:{}", code.file.display(), authored).into(),
            note: code.note.clone().map(SharedString::from),
            laid_out: std::cell::Cell::new(None),
            ref_id: code.id.clone().into(),
            name: code.name.clone().map(SharedString::from),
            file: code.file.clone(),
            selected: None,
            picked: None,
            held: None,
            arriving: Fade::default(),
            proposed: code.after.clone(),
            point: None,
            rising: Fade::default(),
            was: Vec::new(),
            pointing: Fade::default(),
            heeded: Fade::default(),
        };
        pane.lay_out();
        pane.spotlight(authored);
        // A missing file has no lines to clamp the range to, and the header
        // still names the range the ref asked for.
        if pane.missing.is_some() {
            pane.label = format!("{}:{}", code.file.display(), code.range).into();
        }
        // Rows are not lines once some lines are left out, so the place to
        // open on is asked of the table from lines to rows. Strict, because
        // a copy this short often has its range in view already, and a
        // plain scroll leaves a list alone when the row is anywhere in sight
        // — which opened the pane on the edge row instead of the range.
        if kept.is_some() {
            pane.scroll
                .scroll_to_item_strict(pane.row_of(top), ScrollStrategy::Top);
        }
        pane
    }

    /// Put the value rows in under their lines, and the single values on them.
    ///
    /// Run again whenever a value is opened or closed: the rows below move, so
    /// the table from lines to rows is built afresh with them.
    fn lay_out(&mut self) {
        let mut rows = Vec::with_capacity(self.base.len() + self.values.len() * 2);
        for row in &self.base {
            let mut row = row.clone();
            // A row on disk carries what it held. A row of a proposal carries
            // what it would hold, when the agent worked that out: the fix's
            // values beside the bug's, so the reader sees both halves change.
            let on_disk =
                row.number.is_some() && (row.change.is_none() || row.change != self.unwritten);
            let proposed = row.number.is_some()
                && row.change == Some(Change::New)
                && self.unwritten == Some(Change::New);
            let here: Vec<&Value> = if on_disk || proposed {
                self.values
                    .iter()
                    .filter(|value| Some(value.line) == row.number && value.after == proposed)
                    .filter(|value| self.drawn(&who_of(value)))
                    .collect()
            } else {
                Vec::new()
            };
            // A single value goes inline, after its name; a value with keys
            // gets a row of its own under the line.
            let mut inlays: Vec<(usize, SharedString, bool, Who)> = here
                .iter()
                .filter(|value| value.keys.is_empty())
                .map(|value| {
                    (
                        after_name(&row.text, &value.at),
                        value.is.clone().into(),
                        value.lit,
                        who_of(value),
                    )
                })
                .collect();
            inlays.sort_by_key(|(at, _, _, _)| *at);
            row.inlay = inlays;
            let indent = row.text.len() - row.text.trim_start().len();
            rows.push(row.clone());
            for value in here.iter().filter(|value| !value.keys.is_empty()) {
                let who: Who = who_of(value);
                let open = self.open.contains(&who);
                // Still drawn while it closes: the code slides back over the
                // keys, and only then are they taken away.
                let shown = open
                    || self
                        .unfolding
                        .iter()
                        .any(|(at, light)| *at == who && light.moving());
                rows.push(Row {
                    text: SharedString::default(),
                    runs: Vec::new(),
                    number: None,
                    extra: Some(Extra::Chip {
                        who: who.clone(),
                        name: value.at.clone().into(),
                        preview: value.is.clone().into(),
                        open,
                        indent,
                    }),
                    inlay: Vec::new(),
                    ..row.clone()
                });
                if shown {
                    let width = value
                        .keys
                        .iter()
                        .filter_map(|key| key.key.as_ref().map(String::len))
                        .max()
                        .unwrap_or(4);
                    // The keys that matter, back to back, and one count at
                    // the end for the rest. A count between every pair broke
                    // the list into fragments the eye had to step over.
                    let left_out: u32 = value
                        .keys
                        .iter()
                        .filter(|key| key.key.is_none())
                        .map(|key| key.more.unwrap_or(0))
                        .sum();
                    let tail = (left_out > 0).then_some(deck_core::protocol::Key {
                        key: None,
                        is: None,
                        why: None,
                        lit: false,
                        more: Some(left_out),
                    });
                    // A blank row first. Every row in the list is one height,
                    // so the keys cannot be given a few pixels of their own;
                    // instead they are drawn half a row up into this one,
                    // which leaves half a row clear above them and half a row
                    // clear below.
                    rows.push(Row {
                        text: SharedString::default(),
                        runs: Vec::new(),
                        number: None,
                        extra: Some(Extra::Key {
                            who: who.clone(),
                            name: SharedString::default(),
                            is: SharedString::default(),
                            why: None,
                            lit: false,
                            more: false,
                            width,
                            indent: indent + 2,
                            pad: true,
                        }),
                        inlay: Vec::new(),
                        ..row.clone()
                    });
                    for key in value
                        .keys
                        .iter()
                        .filter(|key| key.key.is_some())
                        .chain(tail.as_ref())
                    {
                        let more = key.key.is_none();
                        rows.push(Row {
                            text: SharedString::default(),
                            runs: Vec::new(),
                            number: None,
                            extra: Some(Extra::Key {
                                who: who.clone(),
                                name: key.key.clone().map_or_else(
                                    || format!("+ {} more", key.more.unwrap_or(0)).into(),
                                    SharedString::from,
                                ),
                                is: key.is.clone().unwrap_or_default().into(),
                                why: key.why.clone().map(SharedString::from),
                                lit: key.lit,
                                more,
                                width,
                                indent: indent + 2,
                                pad: false,
                            }),
                            inlay: Vec::new(),
                            ..row.clone()
                        });
                    }
                }
            }
        }
        let mut line_row: Vec<(u32, usize)> = Vec::new();
        for (ix, row) in rows.iter().enumerate() {
            let real =
                row.extra.is_none() && (row.change.is_none() || row.change != self.unwritten);
            if let Some(number) = row.number.filter(|_| real) {
                line_row.push((number, ix));
            }
        }
        // The first row a line is drawn on is its row.
        line_row.sort_by_key(|(number, _)| *number);
        line_row.dedup_by_key(|(number, _)| *number);
        // In the deck's copy, a line it did not keep is found at the row that
        // says so: a scroll to it, or a card placed by it, lands on the edge
        // rather than off the end of the list.
        self.edges = rows
            .iter()
            .enumerate()
            .filter_map(|(ix, row)| match row.extra {
                Some(Extra::Absent { first, last }) => Some((first, last, ix)),
                _ => None,
            })
            .collect();
        self.line_row = line_row;
        // Wide enough for the longest thing drawn on a row, value included.
        self.widest = rows
            .iter()
            .map(|row| match &row.extra {
                Some(Extra::Chip {
                    name,
                    preview,
                    indent,
                    ..
                }) => indent + cells(name) + cells(preview) + 8,
                Some(Extra::Key {
                    is,
                    why,
                    width,
                    indent,
                    ..
                }) => indent + width + cells(is) + why.as_ref().map_or(0, |why| cells(why) + 2) + 4,
                Some(Extra::Absent { first, last }) => cells(&absent_words(*first, *last)),
                None => {
                    row.text.chars().count()
                        + row
                            .inlay
                            .iter()
                            .map(|(_, value, _, _)| cells(value) + 3)
                            .sum::<usize>()
                }
            })
            .max()
            .unwrap_or(0);
        self.rows = rows.into();
    }

    /// Open or close the value under `line`.
    ///
    /// The rows below move, so a selection held by row would land on
    /// different code: it is let go.
    pub fn toggle_value(&mut self, who: Who) {
        let opening = !self.open.remove(&who);
        if opening {
            self.open.insert(who.clone());
        }
        match self.unfolding.iter_mut().find(|(at, _)| *at == who) {
            // Turned back halfway, it goes back from where it had got to.
            Some((_, light)) => {
                light.set_linear(opening);
            }
            None => {
                let mut light = Fade::default().paced(UNFOLD, REFOLD);
                light.set_linear(opening);
                self.unfolding.push((who, light));
            }
        }
        self.lay_out();
        self.reselect();
    }

    /// After rows go in or come out, hold the same lines again.
    ///
    /// Selection is drawn by row, and every row under a value moves when it
    /// opens, closes or arrives, so the old indices would light other code.
    ///
    /// The band's light is carried, not restarted: the reader did nothing, so
    /// nothing should blink. With nothing selected, a band still going out is
    /// left to finish.
    fn reselect(&mut self) {
        let Some(lines) = self.selected else {
            return;
        };
        let rows: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| lines.contains(row.pin))
            .map(|(ix, _)| ix)
            .collect();
        let (Some(&first), Some(&last)) = (rows.first(), rows.last()) else {
            return;
        };
        self.picked = Some((first, last));
        match &mut self.held {
            Some((held, _)) => *held = (first, last),
            None => self.select_rows(first, last),
        }
    }

    /// The values this pane draws and which of them are open, so a pane built
    /// again — for a proposal — can carry them over.
    pub fn values_held(&self) -> Held {
        Held {
            values: self.values.clone(),
            open: self.open.clone(),
            prose_open: self.prose_open.clone(),
            shown: self.shown,
            reached: self.reached,
        }
    }

    /// Put back what a pane built again had: which values were open, which
    /// the prose opened, and whether the values were out.
    pub fn reopen(&mut self, held: Held) {
        self.open = held.open;
        self.prose_open = held.prose_open;
        self.reached = held.reached;
        if !held.shown {
            self.gate();
            self.reached = held.reached;
        }
        self.lay_out();
    }

    /// Write values onto lines already on screen, as an answer does.
    ///
    /// A value already on the same line for the same name is replaced; the
    /// rest stay. Each new one comes in rather than appearing: room opens for
    /// it first, then its words.
    pub fn set_values(&mut self, values: Vec<Value>) {
        for value in values {
            let line = value.line;
            let same = self
                .values
                .iter()
                .position(|old| old.line == line && old.at == value.at && old.after == value.after);
            if let Some(ix) = same {
                if self.values[ix] == value {
                    // The same value written again is still an answer, and
                    // the gate below opens for it.
                    continue;
                }
                self.values.remove(ix);
            }
            let who: Who = who_of(&value);
            self.values.push(value);
            self.presence.retain(|(at, _)| *at != who);
            // Room opens for it, then its words.
            let mut light = Fade::default().paced(UNFOLD, REFOLD);
            light.set_linear(true);
            self.presence.push((who, light));
        }
        // An answer is meant to be seen: the gate opens, and the pane's other
        // values come out with it.
        self.reached = true;
        self.set_shown(true);
        // A sentence already picked may name a key of one of these.
        self.prose = None;
        self.lay_out();
        self.reselect();
    }

    /// Open or close a value because the reader pressed it. Theirs from then
    /// on: a sentence being let go no longer closes it.
    pub fn hand_toggle(&mut self, who: Who) {
        self.prose_open.remove(&who);
        self.toggle_value(who);
    }

    /// Follow the sentence the reader has picked: a value whose key it names
    /// opens, and closes again when the sentence is let go.
    pub fn follow_prose(&mut self, said: Option<&str>) {
        if self.prose.as_deref() == said {
            return;
        }
        self.prose = said.map(str::to_string);
        let want: std::collections::HashSet<Who> = said.map_or_else(Default::default, |text| {
            self.values
                .iter()
                .filter(|value| {
                    value
                        .keys
                        .iter()
                        .filter_map(|key| key.key.as_deref())
                        .any(|key| has_word(text, key))
                })
                .map(who_of)
                .filter(|_| self.wants_value())
                .collect()
        });
        for who in self.prose_open.clone() {
            if !want.contains(&who) {
                if self.open.contains(&who) {
                    self.toggle_value(who.clone());
                }
                self.prose_open.remove(&who);
            }
        }
        for who in want {
            if !self.open.contains(&who) {
                self.toggle_value(who.clone());
                self.prose_open.insert(who);
            }
        }
    }

    /// Whether the pane's values are out right now: its one gate.
    fn wants_value(&self) -> bool {
        self.shown
    }

    /// Whether any of this pane's values sits on a line of `at`.
    #[must_use]
    pub fn holds_value_in(&self, at: LineRange) -> bool {
        self.values.iter().any(|value| at.contains(value.line))
    }

    /// Whether a value is drawn: out, or on its way in or out.
    fn drawn(&self, who: &Who) -> bool {
        self.presence
            .iter()
            .find(|(at, _)| at == who)
            .is_none_or(|(_, light)| light.on() || light.moving())
    }

    /// The narration has moved. The first time it points at a line holding
    /// one of this pane's values, they all come out together, once. Nothing
    /// here ever sends them away again: only the reader does, with the eye.
    pub fn follow_values(&mut self) {
        if self.reached {
            return;
        }
        let reaches = self
            .point
            .is_some_and(|(at, _)| self.values.iter().any(|value| at.contains(value.line)));
        if reaches {
            self.reached = true;
            self.set_shown(true);
        }
    }

    /// Start with the values away, until the narration reaches them. For a
    /// pane its prose points into: values ahead of the sentence being read are
    /// the answer before the question.
    pub fn gate(&mut self) {
        if self.values.is_empty() {
            return;
        }
        self.shown = false;
        self.reached = false;
        self.presence = self
            .values
            .iter()
            .map(|value| (who_of(value), Fade::default().paced(UNFOLD, REFOLD)))
            .collect();
        self.lay_out();
    }

    /// Whether this pane carries values at all, so it wears the eye.
    #[must_use]
    pub fn has_values(&self) -> bool {
        !self.values.is_empty()
    }

    /// Whether the pane's values are out, for the eye to say so.
    #[must_use]
    pub fn values_shown(&self) -> bool {
        self.shown
    }

    /// Bring every value out, or send every value away, together.
    pub fn set_shown(&mut self, on: bool) {
        self.shown = on;
        // A sentence already picked looks again: its chips could not open
        // while the values were away.
        self.prose = None;
        if on {
            self.reached = true;
        }
        let all: Vec<Who> = self.values.iter().map(who_of).collect();
        for who in all {
            match self.presence.iter_mut().find(|(at, _)| *at == who) {
                Some((_, light)) => {
                    light.set_linear(on);
                }
                None if !on => {
                    // Fully out until now: it leaves from all the way out.
                    let mut light = Fade::default().paced(UNFOLD, REFOLD);
                    light.on = true;
                    light.set_linear(false);
                    self.presence.push((who, light));
                }
                None => {}
            }
        }
        if on {
            self.lay_out();
            self.reselect();
        } else {
            self.hiding = true;
        }
    }

    /// Let go of values that have finished closing. Called once a frame.
    pub fn settle(&mut self) {
        // A pane with no value moving, closing or kept away has nothing to
        // settle, and that is nearly every pane on nearly every frame: this
        // runs for each of them whenever anything in the window moves.
        if !self.hiding && self.unfolding.is_empty() && self.presence.is_empty() {
            return;
        }
        // Nothing closing and nothing arriving: nothing to let go of, and no
        // reason to look through the rows on every frame.
        let gone = self.hiding
            && self
                .presence
                .iter()
                .all(|(_, light)| light.on() || !light.moving());
        if gone {
            self.hiding = false;
            self.lay_out();
            self.reselect();
        }
        if self
            .unfolding
            .iter()
            .all(|(who, _)| self.open.contains(who))
        {
            // Out and still: nothing to keep a record of.
            self.presence
                .retain(|(_, light)| light.moving() || !light.on());
            return;
        }
        let open = &self.open;
        let unfolding = &self.unfolding;
        let finished = self.rows.iter().any(|row| match row.extra {
            Some(Extra::Key { ref who, .. }) => {
                !open.contains(who)
                    && !unfolding
                        .iter()
                        .any(|(at, light)| at == who && light.moving())
            }
            _ => false,
        });
        self.unfolding
            .retain(|(who, light)| light.moving() || self.open.contains(who));
        self.presence
            .retain(|(_, light)| light.moving() || !light.on());
        if finished {
            self.lay_out();
            self.reselect();
        }
    }

    /// Which row of the list a file line is drawn on.
    ///
    /// The two are the same until a proposed replacement is spliced in, after
    /// which every line below it sits one row lower per line added. A comment
    /// card placed by line number and a scroll target measured in rows both
    /// have to come through here, or they land somewhere the reader did not
    /// point at.
    fn row_of(&self, number: u32) -> usize {
        if let Ok(at) = self
            .line_row
            .binary_search_by_key(&number, |(line, _)| *line)
        {
            return self.line_row[at].1;
        }
        self.edges
            .iter()
            .find(|(first, last, _)| *first <= number && last.is_none_or(|last| number <= last))
            .map_or_else(
                || row_of(number, self.added_at, self.added),
                |(_, _, ix)| *ix,
            )
    }

    /// Take hold of the rows between `from` and `to`, whichever way round.
    ///
    /// Draws by row and comments by line: the rows are kept as they are, and
    /// the file range is derived from what those rows answer for. Drag across
    /// three arriving lines and you see three lines held, and the comment lands
    /// on the range they replace — which is the only place it could land.
    pub fn select_rows(&mut self, from: usize, to: usize) {
        // Never across an edge of the deck's copy: a selection that ran over
        // `lines 153–212 not in this deck` would pin a comment, and quote,
        // lines the reader was never shown.
        let to = within_span(&self.rows, from, to);
        let (first, last) = (from.min(to), from.max(to));
        self.picked = Some((first, last));
        // A drag grows the range it started from, so the two always touch, and
        // the light carries on rather than restarting on every frame. A fresh
        // click lands somewhere disjoint and starts its own.
        //
        // Direction does not come into it: a light still falling after an
        // escape reverses from wherever it had got to, which is what `set`
        // does. Asking whether it was on would have thrown that away and made
        // the band jump to nothing before climbing again.
        let touching = self
            .held
            .is_some_and(|((was_first, was_last), _)| was_first <= last && first <= was_last);
        match &mut self.held {
            Some((rows, light)) if touching => {
                *rows = (first, last);
                light.set(true);
            }
            slot => {
                let mut light = Fade::default().paced(HAND_RISE, HAND_FALL);
                light.set(true);
                *slot = Some(((first, last), light));
            }
        }
        let pins = self
            .rows
            .get(first..=last.min(self.rows.len().saturating_sub(1)))
            .unwrap_or_default();
        self.selected = match (
            pins.iter().map(|row| row.pin).min(),
            pins.iter().map(|row| row.pin).max(),
        ) {
            (Some(first), Some(last)) => Some(LineRange::new(first, last)),
            _ => None,
        };
    }

    /// Let go.
    pub fn unpick(&mut self) {
        self.picked = None;
        self.selected = None;
        // The rows stay while the light goes down. They are not held any more
        // — `picked` and `selected` already say so — they are just still lit.
        if let Some((_, light)) = &mut self.held {
            light.set(false);
        }
    }

    /// Take hold of whatever rows answer for `range`.
    ///
    /// For a selection being put back rather than made — reopening a deck that
    /// was hidden with one in it. Rows are the truth on screen, so a remembered
    /// file range has to be turned back into them or it is remembered and
    /// invisible.
    pub fn select_lines(&mut self, range: LineRange) {
        let held: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| range.contains(row.pin))
            .map(|(ix, _)| ix)
            .collect();
        match (held.first(), held.last()) {
            (Some(&first), Some(&last)) => self.select_rows(first, last),
            _ => self.unpick(),
        }
    }

    /// Move only the live spotlight, leaving authored diff rows where they are.
    ///
    /// Revealing is separate so an owner can apply the state first and report
    /// layout visibility accurately after the next frame.
    pub fn spotlight(&mut self, range: LineRange) {
        self.spotlight = range.clamp_to(self.source_lines);
        // A new subject, so the old finger is pointing at nothing. Left in
        // place it would light a line of the new range for no stated reason.
        self.point_at(None);
        self.label = format!("{}:{}", self.file.display(), self.spotlight).into();
        self.laid_out.set(None);
    }

    /// The range currently presented as the live subject.
    #[must_use]
    pub fn spotlight_range(&self) -> LineRange {
        self.spotlight
    }

    /// Point at exactly these lines, or stop pointing.
    ///
    /// Nothing scrolls. A point belongs inside the range already on screen,
    /// and a pane that jumped every time the narration moved a sentence on
    /// would take the code out from under the reader mid-sentence.
    pub fn point_at(&mut self, range: Option<(LineRange, crate::prose::Side)>) {
        let range = range.map(|(at, side)| (at.clamp_to(self.source_lines), side));
        if range == self.point {
            return;
        }
        // The lines being left go out from wherever they had got to — and the
        // ones already on their way out keep going.
        //
        // This held one range, so pointing at a third place while the first was
        // still fading dropped that fade on the floor: the lines snapped to
        // nothing between frames, which reads as a shimmer rather than as a
        // light going down. Somebody moving quickly through a group sees it on
        // every step.
        let left_trace = self.point.map(|(at, side)| (at.first, at.last, side));
        self.was.retain(|(_, _, going)| going.moving());
        if let Some((left, side)) = self.point {
            let mut going = self.rising;
            // Handed over when something is taking its place, and given the
            // slow way out only when the pane stops being pointed at at all.
            if range.is_some() {
                going.hand_over();
            } else {
                going.set(false);
            }
            self.was.push((left, side, going));
        }
        self.point = range;
        self.rising = Fade::default();
        if range.is_some() {
            self.rising.set(true);
        }
        self.pointing.set(range.is_some());

        // Off unless asked for. Two guesses at a shimmer nobody but the reader
        // can see is two guesses too many: this says what actually reached the
        // pane, in what order, and how much of the last light was still on the
        // way out when the next one landed.
        if std::env::var_os("DECK_TRACE_POINT").is_some() {
            eprintln!(
                "point {:>8.3}s  {:?} -> {:?}  leaving={} rising={:.2}",
                std::time::UNIX_EPOCH
                    .elapsed()
                    .map_or(0., |at| at.as_secs_f32() % 1000.),
                left_trace,
                range.map(|(at, side)| (at.first, at.last, side)),
                self.was.len(),
                self.rising.level(),
            );
        }
    }

    /// Say whether the agent is talking about this pane.
    ///
    /// Drawn round the whole pane, because a finger on two lines is easy to
    /// miss in a group of four files, and which file is being talked about is
    /// the first thing a reader needs to know.
    pub fn heed(&mut self, on: bool) {
        self.heeded.set(on);
    }

    /// Whether a light in this pane is still coming up or going out.
    ///
    /// The window keeps asking for frames while this is true, and only then.
    #[must_use]
    pub fn fading(&self) -> bool {
        self.rising.moving()
            || self.pointing.moving()
            || self.heeded.moving()
            || self.arriving.moving()
            || self.was.iter().any(|(_, _, going)| going.moving())
            || self.held.is_some_and(|(_, light)| light.moving())
            || self.unfolding.iter().any(|(_, light)| light.moving())
            || self.presence.iter().any(|(_, light)| light.moving())
    }

    /// The lines the narration is pointing at, if any.
    #[must_use]
    pub fn pointed(&self) -> Option<(LineRange, crate::prose::Side)> {
        self.point
    }

    /// The text of line `number`, 1-based, as it was when the deck opened.
    #[must_use]
    pub fn line(&self, number: u32) -> String {
        self.rows
            .get(self.row_of(number))
            .filter(|row| row.number == Some(number))
            .map(|row| row.text.to_string())
            .unwrap_or_default()
    }

    /// Where the list would have to sit for the range to be at the top.
    ///
    /// Scroll offsets run negative as the content moves up, which is why this
    /// is the negation of the distance from the top.
    #[must_use]
    pub fn focus_offset(&self) -> Option<Pixels> {
        let (entry, row) = self.laid_out.get()?;
        // A row count times a row height: a file long enough to lose precision
        // here is one nobody is reviewing.
        #[allow(clippy::cast_precision_loss)]
        let top = entry as f32 * row;
        let lead = LEAD_IN as f32 * row;
        Some(px(-(top - lead).max(0.)))
    }

    /// Where the list would have to sit for the pointed lines to be in view,
    /// or `None` when they already are.
    ///
    /// In the upper third when they fit, so the lines after them are on screen
    /// too — an explanation usually goes on downwards. Top-aligned, under a
    /// couple of lines of lead, when they do not fit: a range taller than the
    /// pane is read from its start.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn point_offset(&self) -> Option<Pixels> {
        let point = self.point?;
        let (_, row) = self.laid_out.get()?;
        let state = self.scroll.0.borrow();
        let viewport = f32::from(state.base_handle.bounds().size.height);
        if viewport <= 0. {
            return None;
        }
        let seen = -f32::from(state.base_handle.offset().y);
        let furthest = f32::from(state.base_handle.max_offset().y).max(0.);
        let top = self.row_of(point.0.first) as f32 * row;
        let bottom = (self.row_of(point.0.last) + 1) as f32 * row;
        let height = bottom - top;

        let target = if height + 2. * row <= viewport {
            if top >= seen && bottom <= seen + viewport {
                return None;
            }
            top - (viewport - height) / 3.
        } else {
            let lead = LEAD_IN as f32 * row;
            if (top - lead - seen).abs() < row {
                return None;
            }
            top - lead
        };
        Some(px(-target.clamp(0., furthest)))
    }

    /// The list's own scroll handle, for driving it a frame at a time.
    #[must_use]
    pub fn scroll(&self) -> UniformListScrollHandle {
        self.scroll.clone()
    }

    /// Bring a proposal up rather than switching it on.
    ///
    /// Called by the window after building a pane for a live answer, because
    /// only the window knows a change arrived now instead of being authored.
    pub fn arrive(&mut self) {
        self.arriving.set(true);
    }

    /// What this pane is proposing the lit range should become.
    #[must_use]
    pub fn proposal(&self) -> Option<&str> {
        self.proposed.as_deref()
    }

    /// The sideways scroll handle, for the wheel.
    #[must_use]
    pub fn across(&self) -> ScrollHandle {
        self.across.clone()
    }

    /// Which way the lit range lies, or `None` while any of it is on screen.
    ///
    /// The button that goes back to it is only worth its space while there is
    /// an answer, and the answer is also which way its arrow points.
    #[must_use]
    pub fn range_away(&self) -> Option<Away> {
        let (entry, row) = self.laid_out.get()?;
        let state = self.scroll.0.borrow();
        let viewport = state.base_handle.bounds().size.height;
        if viewport <= px(0.) {
            // Nothing laid out yet, so nothing is lost yet.
            return None;
        }

        let top = f32::from(-state.base_handle.offset().y);
        let first = entry as f32 * row;
        let last = first + (self.spotlight_range().len() as f32 * row);

        if last <= top {
            Some(Away::Above)
        } else if first >= top + f32::from(viewport) {
            Some(Away::Below)
        } else {
            None
        }
    }

    /// Put the first line of a proposed change in view.
    ///
    /// A replacement is spliced in under the range it replaces, so showing the
    /// top of the range can leave the new code below the fold — which is the
    /// half the answer was about. With nothing proposed, the lit range is the
    /// subject as usual.
    pub fn show_change(&self) {
        let Some(first) = self
            .rows
            .iter()
            .position(|row| row.change == Some(Change::New))
        else {
            self.show_range();
            return;
        };
        self.scroll
            .scroll_to_item(first.saturating_sub(LEAD_IN as usize), ScrollStrategy::Top);
    }

    /// Put the lit range back in view.
    ///
    /// Asked for after anything that changes row heights, since a scroll
    /// position measured in rows means something different once the rows are a
    /// different size.
    pub fn show_range(&self) {
        let top = self.spotlight.first.saturating_sub(LEAD_IN).max(1);
        self.scroll
            .scroll_to_item(self.row_of(top), ScrollStrategy::Top);
    }

    /// The lines a comment would land on: whatever is selected, or the lit
    /// range when nothing is.
    #[must_use]
    pub fn comment_range(&self) -> LineRange {
        self.selected.unwrap_or(self.spotlight_range())
    }

    /// Whether the reader chose these lines, rather than the agent lighting them.
    ///
    /// The difference decides what a bare reaction is about. A lit range is the
    /// agent pointing; a selected one is the reader pointing. Treating the
    /// first as the second is how a reaction about a *sentence* ended up
    /// attached to whatever code happened to be on screen.
    #[must_use]
    pub fn reader_chose(&self) -> bool {
        self.selected.is_some()
    }

    /// The text of `range`, as it was when the deck opened.
    #[must_use]
    pub fn lines_of(&self, range: LineRange) -> String {
        (range.first..=range.last)
            .map(|number| self.line(number))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The pane, label and all.
    ///
    /// `marks` are the lines this pane already carries a comment on, with the
    /// text to show beside them.
    /// `marks` are the comments on this pane: the lines each one covers, and
    /// what it said.
    pub fn render(
        &self,
        slot: &Slot,
        marks: &[Mark],
        focus_button: Option<AnyElement>,
        cx: &App,
    ) -> impl IntoElement {
        let palette = slot.palette;
        let fold = slot.fold;
        let label = chrome(
            self.name.clone(),
            self.label.clone(),
            self.note.clone(),
            Controls {
                values: self.has_values().then(|| values_button(slot, self.shown)),
                // Not over a pane with no code: it says the same thing in its
                // own words, and offers the same switch.
                aside: self
                    .aside
                    .as_ref()
                    .filter(|_| self.missing.is_none())
                    .map(|aside| aside_note(aside, slot, cx)),
                ..controls(slot, focus_button)
            },
            palette,
            cx,
        );

        let heeded = self.heeded.level();

        div()
            .v_flex()
            .relative()
            .overflow_hidden()
            // Folding is a width, not a hiding. The pane gives up its share of
            // the row a frame at a time and keeps only its spine, and whatever
            // is beside it takes the room as it goes.
            .flex_grow(slot.share * (1. - fold))
            .flex_shrink(1.)
            // Nothing at all once it is folded. The spine it leaves behind is a
            // column of its own at the window's edge, and a basis here as well
            // meant the same width was set aside twice — a folded pane left a
            // pane-shaped hole beside its own spine.
            .flex_basis(px(0.))
            .h_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(paint(palette.wash))
            .child(
                div()
                    .v_flex()
                    .size_full()
                    .min_h_0()
                    .opacity(1. - fold)
                    .child(label)
                    .child(match &self.missing {
                        Some(missing) => missing_note(missing, slot, cx).into_any_element(),
                        None => self
                            .render_code(slot.ix, marks, palette, slot.view, cx)
                            .into_any_element(),
                    }),
            )
            // The pane being talked about, outlined. Drawn over the pane rather
            // than as its border, so lighting it moves nothing: a border that
            // appeared would push every row down by its width, mid-sentence.
            // It has no handlers, so nothing under it stops being clickable.
            .when(heeded > 0., |pane| pane.child(outline(palette, heeded)))
    }

    fn render_code(
        &self,
        pane_ix: usize,
        marks: &[Mark],
        palette: &Palette,
        view: &WeakEntity<DeckView>,
        cx: &App,
    ) -> impl IntoElement {
        // Whole pixels. The list places rows by the height it measures, and
        // layout rounds a row to the pixel, so 21.06 here and 21 there put
        // every line-to-position sum a little further out per hundred lines:
        // three thousand lines into a file, a scroll to the pointed lines
        // landed nine rows off, and the pane slid away from them. Rows that
        // are whole pixels measure exactly what this says they are.
        let row_px = px((f32::from(cx.theme().mono_font_size) * LINE_HEIGHT).round());
        let mono = cx.theme().mono_font_family.clone();
        let size = cx.theme().mono_font_size;

        // Everything a row needs is copied in: the closure outlives this call
        // and so cannot borrow the pane. The rows are shared, not copied.
        let rows = std::rc::Rc::clone(&self.rows);
        // For each value in motion: the last of its key rows, how many there
        // are, and how far open it is. Everything below a block is drawn that
        // much higher while it opens, so the code glides down to make room
        // rather than jumping; the keys under it are covered and uncovered as
        // it goes.
        // Room first, words after, but not one and then the other. The code
        // slides down over the first two thirds and the words come up from a
        // third of the way in, so by the time they are anything to look at the
        // gap is most of the way open; going away runs the other way. Moving
        // and appearing at exactly once was two things competing for the same
        // glance — and the two strictly in turn, the words waiting for the room
        // to finish, made a value the slowest thing on the screen.
        // Both eased in and out, the same curve a light uses, so a value comes
        // up the way a pointed line does. An ease-out run backwards for a
        // close sped up into its end and stopped hard, and words fading in
        // straight arrived with a corner at the end of them.
        let slide = |x: f32| {
            let x = x.clamp(0., 1.);
            x * x * (3. - 2. * x)
        };
        let steps = move |t: f32| (slide(t / 0.65), slide((t - 0.35) / 0.65));
        // Each stretch of rows in motion: its first and last row, how far the
        // room for it has opened, and how far its words are in.
        let mut blocks: Vec<(usize, usize, f32, f32)> = Vec::new();
        // A value coming in or going out: its preview row (and its keys, if
        // open) opens or closes the same way, and a single value grows its own
        // room on the line, then fills it — or empties it, then gives it back.
        // A value closed but not yet taken out is drawn at its end, not
        // skipped: skipping it for the frame between its light stopping and its
        // rows going drew it fully open for that frame.
        let mut arrivals: Vec<(Who, f32, f32)> = Vec::new();
        let mut whole: Vec<Who> = Vec::new();
        for (who, light) in self
            .presence
            .iter()
            .filter(|(_, light)| light.moving() || !light.on())
        {
            let (slide, show) = steps(light.linear());
            arrivals.push((who.clone(), slide, show));
            if let Some(ix) = self.rows.iter().position(
                |row| matches!(&row.extra, Some(Extra::Chip { who: at, .. }) if at == who),
            ) {
                let last = self
                    .rows
                    .iter()
                    .rposition(
                        |row| matches!(&row.extra, Some(Extra::Key { who: at, .. }) if at == who),
                    )
                    .unwrap_or(ix);
                blocks.push((ix, last, slide, show));
                whole.push(who.clone());
            }
        }
        // A value opening or closing, its keys only. Not one that is also
        // coming or going whole: its rows are already in that motion, and
        // moving them twice slid the code below over the lines above it.
        for (who, light) in self
            .unfolding
            .iter()
            .filter(|(who, light)| light.moving() || !self.open.contains(who))
            .filter(|(who, _)| !whole.contains(who))
        {
            let keys: Vec<usize> = self
                .rows
                .iter()
                .enumerate()
                .filter(
                    |(_, row)| matches!(&row.extra, Some(Extra::Key { who: at, .. }) if at == who),
                )
                .map(|(ix, _)| ix)
                .collect();
            if let (Some(first), Some(last)) = (keys.first(), keys.last()) {
                let (slide, show) = steps(light.linear());
                blocks.push((*first, *last, slide, show));
            }
        }
        let lit = self.spotlight;
        let point = self.point;
        let rising = self.rising.level();
        let was: Vec<(LineRange, crate::prose::Side, f32)> = self
            .was
            .iter()
            .map(|(range, side, going)| (*range, *side, going.level()))
            .collect();
        let pointing = self.pointing.level();
        // A change that was authored is simply there; one that just arrived
        // comes up out of the page.
        let arrived = if self.proposed.is_some() && self.arriving.on() {
            self.arriving.level()
        } else {
            1.
        };
        let palette = *palette;
        // A change comes up *through* the accent. The diff colours are pale by
        // design, and a pale colour rising out of paper is a change nobody sees
        // arrive; a warm flash that settles into the diff colour is one nobody
        // misses.
        let flash = palette.focus.mix(palette.accent, 0.3);
        // Quiet, but never so quiet it cannot be read: on a light editor
        // theme the fixed mix landed near 1.6:1. Lifted by the least that
        // reads at 3:1 on the pane and the page, which keeps it well behind
        // the code it numbers.
        let gutter_fg = deck_core::theme::legible(
            deck_core::theme::legible(palette.fg.mix(palette.wash, 0.6), palette.wash, 3.0),
            palette.bg,
            3.0,
        );
        let held = self.held;
        // A comment covers a range, so its bar covers the range: a mark on the
        // first line alone leaves the reader guessing how far down the remark
        // was meant to reach.
        let marked: std::collections::HashSet<u32> = marks
            .iter()
            .flat_map(|mark| mark.range.first..=mark.range.last)
            .collect();

        // A remark is a card floating over the code, not rows wedged between
        // it. Rows pushed the file apart and made the line under a remark hard
        // to find; the card sits on top, is any height it likes, and can be
        // closed. It is placed from the scroll offset, so it travels with the
        // line it belongs to.
        let lit_entry = self.row_of(self.spotlight.first);
        let view = view.clone();
        let wheeling = view.clone();

        self.laid_out.set(Some((lit_entry, f32::from(row_px))));
        let cards = self.render_cards(marks, row_px, &palette, &view, cx);

        // How wide the rows have to be for the longest line to fit, asked of the
        // font rather than guessed from its size. A monospace font is one
        // advance wide per character, every family has its own, and the guess
        // was right for the two fonts on this machine and nobody else's — and
        // when it is wrong the end of a long line is unreachable again. The
        // guess stays as the answer for a font that will not say.
        #[allow(clippy::cast_precision_loss)]
        let advance = cx
            .text_system()
            .em_advance(
                cx.text_system().resolve_font(&font(mono.clone())),
                px(f32::from(size)),
            )
            .unwrap_or_else(|_| px(f32::from(size) * 0.62));
        #[allow(clippy::cast_precision_loss)]
        let span = advance * self.widest as f32 + px(96.);
        // The values' smaller type, measured the same way, so a value's room
        // is exactly as wide as its words: an estimate left the room wider
        // than the box it handed over to, and the line jumped back at the end
        // of every opening.
        let small_advance = cx
            .text_system()
            .em_advance(
                cx.text_system().resolve_font(&font(mono.clone())),
                px(f32::from(size) * 0.92),
            )
            .unwrap_or(advance * 0.92);
        let across = self.across.clone();
        let sans = cx.theme().font_family.clone();

        let count = rows.len();
        let reach = blocks.clone();
        // One row, in its slot, drawn wherever the values in motion above it
        // have it right now.
        let draw = move |ix: usize| {
            let row = rows[ix].clone();
            let lift = lift_at(&blocks, ix);
            // A row inside a stretch in motion is only there once the
            // room has opened as far as it, ground and all, and its
            // words come in after the room is whole.
            let (cover, reveal) = blocks
                .iter()
                .find(|(first, last, _, _)| ix >= *first && ix <= *last)
                .map_or((1., 1.), |(first, last, slide, show)| {
                    #[allow(clippy::cast_precision_loss)]
                    let nth = (ix - first) as f32;
                    #[allow(clippy::cast_precision_loss)]
                    let room = (last - first + 1) as f32 * slide;
                    // Its ground is simply there once the gap reaches
                    // it — uncovered by the code sliding off it, not
                    // faded in, which read as the row swelling.
                    (if room > nth { 1. } else { 0. }, *show)
                });
            let grow: Vec<(f32, f32)> = row
                .inlay
                .iter()
                .map(|(_, _, _, who)| {
                    arrivals
                        .iter()
                        .find(|(at, _, _)| at == who)
                        .map_or((1., 1.), |(_, slide, show)| (*slide, *show))
                })
                .collect();
            // What a press on this row means.
            //
            // The gutter still shows nothing for a spliced line — it is
            // not in the file, and a number there would be a lie the
            // reader would quote back. This is the other question. Zero is not a line, so a spliced row answered
            // the mouse with a range nothing could contain: pressing a
            // proposed line selected nothing and a diff could not be
            // commented on at all.
            //
            // It stands in for the line the splice hangs off — the last
            // line going, for an `after`; the one above the arrival,
            // for a `before`. That is the honest pin, because a comment
            // about a line that is not in the file yet is a comment
            // about the range it would replace.
            let pick_at = row.pin;
            // All of these ask the same question — *is this row part of
            // what is being talked about* — and all of them used to
            // answer no for a spliced line, because they asked it in
            // file lines and a proposed line has none. So a point could
            // not light the half of a change that was arriving, which
            // is usually the half the sentence is about.
            let is_lit = lit.overlaps(row.answers);
            // By row, because every arriving line of a change answers
            // for the same file line and a range of those cannot tell
            // the first from the fourth.
            let holding = held.map_or(0., |((first, last), light)| {
                if ix >= first && ix <= last {
                    light.level()
                } else {
                    0.
                }
            });
            let has_mark = marked.contains(&pick_at);
            // The side as well as the lines. A pane showing a change
            // has two rows answering to each number — the one going and
            // the one arriving — so a point that only knew the numbers
            // matched both and lit red and green together. Walking a
            // change is two sentences, and wants two lights.
            let in_point =
                point.is_some_and(|(at, side)| at.overlaps(row.answers) && row.takes(side));
            // The brightest of everything still on its way out. Several
            // ranges can be fading at once — somebody moving quickly
            // leaves a trail of them — and a row caught by two should
            // go out from the higher, not flicker between them.
            let leaving = was
                .iter()
                .filter(|(range, side, _)| range.overlaps(row.answers) && row.takes(*side))
                .map(|(_, _, level)| *level)
                .fold(0., f32::max);
            // How strongly this row is pointed at, right now. A line in
            // both the old range and the new one never dips: it was lit
            // and it stays lit.
            // Whichever of the two is brighter, and never a number of
            // its own.
            //
            // This used to answer a flat 1.0 for a row that was both
            // arriving and leaving, meaning to say *a line in both
            // ranges never dips*. It created the dip it was avoiding:
            // the row snapped to full, held while the old light
            // decayed, then dropped to however far the new one had
            // actually risen and climbed again. On, off, on — which is
            // why it only ever showed between two points that share
            // lines, and never between two that do not.
            // Added, not compared. Two smoothstep curves crossing
            // each other peak at a half apiece, so taking the brighter
            // still dips through the middle of a crossing — which is
            // the blink, one layer down from where it looked like it
            // was. Summed, a light handing over to another holds steady
            // the whole way across.
            let pointed = if in_point {
                (rising + leaving).min(1.)
            } else {
                leaving
            };

            // What the row *is*: going, arriving, lit, or plain.
            let base = match row.change {
                Some(Change::Gone) => flash.mix(palette.gone, arrived),
                Some(Change::New) => flash.mix(palette.fresh, arrived),
                None if is_lit => palette.focus,
                None => palette.wash,
            };
            // Then what is being *said* about it, over the top.
            //
            // A pointed line deepens into more of its own colour rather
            // than taking the accent. Washing orange over the arriving
            // green gave a muddy brown and took away the one thing the
            // row was saying — that it is the half arriving. Red goes to
            // a darker red, green to a darker green, and only ordinary
            // paper takes the accent, where there is no meaning of its
            // own to drown.
            let said = match row.change {
                Some(Change::Gone) => base.mix(palette.gone_deep, pointed),
                Some(Change::New) => base.mix(palette.fresh_deep, pointed),
                None => base.mix(palette.accent, 0.26 * pointed),
            };
            let ground = said.mix(palette.accent, 0.14 * holding);
            // The edge of the deck's copy sits on a ground of its
            // own, a step down toward the desk, so it reads as the
            // end of what is here rather than as an empty line.
            let absent = matches!(row.extra, Some(Extra::Absent { .. }));
            let ground = if absent {
                palette.wash.mix(palette.ground, 0.45)
            } else {
                ground
            };
            // A key row is drawn half a row up, all of it — ground, bar
            // and words — into the blank row above the first key. That
            // leaves half a row clear above the keys and, under the last
            // one, half a row its own ground fills, so every band still
            // runs the width of the pane and lines up with its words.
            let keyed = matches!(row.extra, Some(Extra::Key { pad: false, .. }));
            let half = if keyed { row_px * 0.5 } else { px(0.) };
            // Where the trail runs: past the bar and the gutter, at
            // the key's indent.
            #[allow(clippy::cast_precision_loss)]
            let trail_at = match row.extra {
                Some(Extra::Key { indent, .. }) => px(56.) + advance * indent as f32,
                _ => px(0.),
            };
            let last_key = keyed
                && !matches!(
                    rows.get(ix + 1).and_then(|next| next.extra.as_ref()),
                    Some(Extra::Key { pad: false, .. })
                );
            // The row's slot holds still and what is drawn in it moves.
            // The list places each row itself and ignores an offset on
            // the row, so moving the row did nothing and the code below
            // a value jumped instead of gliding.
            div()
                .h(row_px)
                .w_full()
                .relative()
                .child(
                    div()
                        .id(("row", ix))
                        .h(row_px)
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(row_px * -lift - half)
                        .opacity(cover)
                        // The row's ground has to span the pane, not stop
                        // where its text does: a lit range that ends ragged
                        // down its right edge reads as damage, not emphasis.
                        .w_full()
                        .h_flex()
                        .items_center()
                        .font_family(mono.clone())
                        .text_size(size)
                        // Press, drag, release — the way selecting lines works
                        // everywhere else. Shift extends an existing selection
                        // without dragging.
                        .on_mouse_down(MouseButton::Left, {
                            let view = view.clone();
                            let handle = match row.extra {
                                Some(Extra::Chip { ref who, .. }) => Some(who.clone()),
                                _ => None,
                            };
                            move |event: &MouseDownEvent, _window, cx| {
                                // Lines that are not here cannot be
                                // picked or commented on.
                                if absent {
                                    return;
                                }
                                let extend = event.modifiers.shift;
                                let _ = view.update(cx, |deck, cx| {
                                    // A value's handle opens it; it is not code to pick.
                                    if let Some(who) = handle.clone() {
                                        deck.toggle_value(pane_ix, who, cx);
                                    } else {
                                        deck.start_pick(pane_ix, ix, extend, cx);
                                    }
                                });
                            }
                        })
                        // Hover, not mouse-move. A move handler is not
                        // bounds-checked: it runs for every move anywhere in
                        // the window, on every row at once, so whichever row
                        // was laid out last decided what was selected. Hover
                        // fires only for the row under the pointer — and it
                        // records that row rather than acting on it, because
                        // hover cannot tell whether a button is held.
                        .on_hover({
                            let view = view.clone();
                            move |entered: &bool, _window, cx| {
                                let entered = *entered;
                                let _ = view.update(cx, |deck, _| {
                                    deck.hover_row(pane_ix, ix, entered);
                                });
                            }
                        })
                        // One row, one background. The bar, the number and the
                        // code sit on it together, so the lit range can never
                        // drift away from the lines it lights.
                        // A picked range is one band, not a box round every
                        // row. Outlining each row drew a ladder of rectangles
                        // down the pane and hid the very thing being selected;
                        // the mock only ever outlined a single line, so the
                        // multi-line case had no design and I invented a bad
                        // one. A continuous ground reads as one selection
                        // however many lines it covers.
                        // A proposed change outranks the spotlight: the range
                        // is lit *because* it is going, and painting it with
                        // the ordinary focus colour would say the opposite of
                        // what the ref came to say. Mixed into the ground
                        // rather than filled with — a diff that shouts is a
                        // diff nobody reads the code of.
                        // A point is drawn by contrast, not by a colour of
                        // its own. A quarter of the accent rather than a
                        // tenth: a tenth is legible on paper and all but gone
                        // on a dark theme, where the lit ground is already
                        // close to the page. The lit ground already fills the range; the
                        // pointed lines take a little of the accent into it and
                        // the rest of the range gives up its bar, so the eye is
                        // pulled to the sentence being said without the page
                        // gaining a third kind of highlight to learn.
                        .bg(paint(ground))
                        // A dashed hairline between the edge and the code it
                        // borders, on whichever side the code is: dashed, as
                        // a cut is drawn, and not the solid rule between
                        // panes.
                        .when(absent && ix > 0, |row| {
                            row.border_t_1()
                                .border_dashed()
                                .border_color(paint(palette.edge))
                        })
                        .when(absent && ix + 1 < rows.len(), |row| {
                            row.border_b_1()
                                .border_dashed()
                                .border_color(paint(palette.edge))
                        })
                        .child(
                            div()
                                .w(px(12.))
                                .flex_none()
                                .text_center()
                                .text_color(paint(match row.change {
                                    // The bar takes the point too, so a
                                    // sub-highlight inside a diff has an edge
                                    // as well as a ground.
                                    Some(Change::Gone) => palette
                                        .accent
                                        .mix(palette.del, arrived)
                                        .mix(palette.accent, pointed),
                                    Some(Change::New) => palette
                                        .accent
                                        .mix(palette.add, arrived)
                                        .mix(palette.accent, pointed),
                                    None if has_mark => palette.add,
                                    None if holding > 0. => {
                                        palette.wash.mix(palette.accent, holding.max(pointed))
                                    }
                                    // While the narration is pointing, the rest
                                    // of the lit range steps back. Two ranges
                                    // with the same bar is two claims about
                                    // where to look.
                                    None if is_lit => {
                                        palette.accent.mix(palette.muted, pointing * (1. - pointed))
                                    }
                                    None => palette.wash.mix(palette.accent, pointed),
                                }))
                                .child(match row.change {
                                    Some(Change::Gone) => "−",
                                    Some(Change::New) => "+",
                                    None if absent => "",
                                    None => "▌",
                                }),
                        )
                        .child(
                            div()
                                .w(px(44.))
                                .flex_none()
                                .pr(px(12.))
                                .text_right()
                                .text_color(paint(if is_lit { palette.muted } else { gutter_fg }))
                                // Blank for a line that is not in the file.
                                .child(
                                    row.number
                                        .map(|number| number.to_string())
                                        .unwrap_or_default(),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .pr(px(18.))
                                .h_flex()
                                .items_center()
                                .gap_3()
                                .overflow_hidden()
                                // The words come in, the ground under them was
                                // already there: a gap the colour of the lit
                                // range, not a hole in it.
                                .opacity(reveal)
                                .child(row_content(
                                    &row,
                                    &sans,
                                    &palette,
                                    advance,
                                    small_advance,
                                    size,
                                    &grow,
                                )),
                        ),
                )
                // Under the last key, the half row the keys moved up
                // out of, in the same ground, with the trail running on.
                .when(last_key, |slot| {
                    slot.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(row_px * (0.5 - lift))
                            .h(row_px * 0.5)
                            .opacity(cover)
                            .bg(paint(ground))
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left(trail_at)
                                    .w(px(1.))
                                    .bg(paint(palette.edge.mix(palette.muted, 0.55))),
                            ),
                    )
                })
        };
        let list = uniform_list("deck-code", count, move |visible, _window, _cx| {
            let end = visible.end;
            visible
                .map(|ix| {
                    let slot = draw(ix);
                    let lift = lift_at(&reach, end);
                    if ix + 1 != end || lift <= 0. {
                        return slot.into_any_element();
                    }
                    // The rows just past the bottom edge, hung under the last
                    // one the list asked for. While a value opens, everything
                    // under it is drawn higher than its slot, and the list only
                    // builds the rows whose slots are on screen — so the rows
                    // that should have slid up into the bottom of the pane were
                    // never built, and a blank strip sat there, shrinking as
                    // the room opened. As many as the code is lifted, and one
                    // for the part of a row.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let under = lift.ceil() as usize + 1;
                    slot.children((1..=under).filter(|k| ix + k < count).map(|k| {
                        #[allow(clippy::cast_precision_loss)]
                        let down = row_px * k as f32;
                        draw(ix + k).absolute().left_0().right_0().top(down)
                    }))
                    .into_any_element()
                })
                .collect()
        })
        .track_scroll(&self.scroll)
        .h_full()
        // At least as wide as the longest line, and never narrower than the
        // pane. Given the line width alone, a file of short lines left the lit
        // ground stopping in the middle of the pane with bare paper beside it.
        .w_full()
        .min_w(span);

        div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .id(("code-across", pane_ix))
                    .size_full()
                    .overflow_x_scroll()
                    .track_scroll(&across)
                    .child(list),
            )
            // The wheel, taken before the list gets it.
            //
            // A list applies a wheel delta to its offset outright, which on a
            // trackpad is a stack of tiny jumps and on a mouse is one large
            // one. Neither reads as movement, and a reader who cannot see the
            // page move has to find their place again at the end of it.
            //
            // Painted after the list and so registered after it, and GPUI runs
            // bubble-phase mouse handlers in reverse registration order — which
            // is what lets this one answer first and stop the list from
            // answering at all. The hitbox is ordinary, so nothing under it
            // stops being hovered or clickable.
            .child(div().absolute().inset_0().on_scroll_wheel({
                let view = wheeling;
                move |event: &ScrollWheelEvent, window, cx| {
                    let by = event.delta.pixel_delta(window.line_height());
                    let _ = view.update(cx, |deck, cx| deck.wheel(pane_ix, by, cx));
                    cx.stop_propagation();
                }
            }))
            .children(cards)
            // A pane holds a whole file, so it needs to say how much of it is
            // off screen — and give a way to get there that is not the wheel.
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
                        Scrollbar::horizontal(&self.across)
                            .viewport_from_layout()
                            .max_fps(60),
                    ),
            )
    }

    /// The pane folded down to its spine.
    ///
    /// A badge with the pane's initial and a hairline running the height of it.
    /// The name was set standing on end, one letter above the next, and read as
    /// a column of letters rather than as a word — nothing here can turn text on
    /// its side, so the answer is not to try. The whole name is a hover away.
    ///
    /// Clicking anywhere on it opens the pane again.
    pub fn render_folded(&self, slot: &Slot, cx: &App) -> AnyElement {
        // Its name if the agent gave it one, and the file's own name if not:
        // `protocol` says more than `protocol.rs` does, and far more than `p`.
        let title: SharedString = self.name.clone().unwrap_or_else(|| {
            self.file
                .file_stem()
                .map_or_else(
                    || self.label.to_string(),
                    |stem| stem.to_string_lossy().to_string(),
                )
                .into()
        });
        // The whole path under it, not just the file. Two panes of `mod.rs` in
        // a big tree are the same word on two spines, and the reader is left
        // opening both to find out which is which.
        let file: SharedString = self.file.display().to_string().into();
        let said = self.label.clone();

        spine(slot, &title, Some(&file), said, cx)
    }

    /// The remark cards, placed against the code they belong to.
    #[allow(clippy::cast_precision_loss)]
    fn render_cards(
        &self,
        marks: &[Mark],
        row_px: Pixels,
        palette: &Palette,
        view: &WeakEntity<DeckView>,
        cx: &App,
    ) -> Vec<AnyElement> {
        let top = f32::from(-self.scroll.0.borrow().base_handle.offset().y);
        let row = f32::from(row_px);

        marks
            .iter()
            .map(|mark| {
                let remark_ix = mark.remark_ix;
                let (fold_view, drop_view) = (view.clone(), view.clone());
                // Just under the last line the remark covers — counted in
                // rows, not in line numbers, because a proposed replacement
                // puts rows between the two.
                // Under everything that belongs to the line, its value rows
                // included, or the card covers the value it is about.
                let below = self
                    .rows
                    .iter()
                    .rposition(|r| r.pin == mark.range.last)
                    .unwrap_or_else(|| self.row_of(mark.range.last));
                let y = (below + 1) as f32 * row - top;

                // One surface, quiet chrome.
                //
                // It was three stacked bars — a header, the text, a footer —
                // and the chrome ended up with more weight than the remark.
                // The tools that do this well go the other way: a hairline
                // border, no shadow, tight spacing on a 4px scale, and the
                // words as the only thing with any colour to them. So the
                // metadata is one small muted line and everything else is the
                // comment.
                // Icons, sized to the metadata line beside them and quiet
                // until the pointer finds them. A hit area larger than the
                // glyph, so they are not a game of precision.
                // The hover colour is an argument, not something a caller
                // chains on afterwards: GPUI keeps one hover style per element
                // and panics outright on a second, so a button that wanted its
                // own hover took the whole window down with it.
                let action = |id: &'static str, icon: IconName, lit: Rgb| {
                    div()
                        .id((id, remark_ix))
                        .flex_none()
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .text_color(paint(palette.muted))
                        .hover(move |this| this.bg(paint(palette.wash)).text_color(paint(lit)))
                        .child(Icon::new(icon).size(px(12.)))
                };

                div()
                    .absolute()
                    .top(px(y + 4.))
                    .left(px(58.))
                    .right(px(18.))
                    .p(px(12.))
                    .rounded(px(8.))
                    .bg(paint(palette.bg))
                    .border_1()
                    .border_color(paint(palette.edge))
                    // The one coloured edge, tying the card to the range it is
                    // about.
                    .border_l_2()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .pb(px(if mark.folded { 0. } else { 6. }))
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(px(10.))
                            .text_color(paint(palette.muted))
                            .child(format!("lines {}", mark.range))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap(px(2.))
                                    .child(
                                        action(
                                            "fold-remark",
                                            if mark.folded {
                                                IconName::ChevronRight
                                            } else {
                                                IconName::ChevronDown
                                            },
                                            palette.fg,
                                        )
                                        .on_click(
                                            move |_, _window, cx| {
                                                let _ = fold_view.update(cx, |deck, cx| {
                                                    deck.toggle_remark(remark_ix, cx);
                                                });
                                            },
                                        ),
                                    )
                                    .child(
                                        // The one act that cannot be undone
                                        // turns the colour of the thing it
                                        // does, before it is committed to.
                                        action("close-remark", IconName::Delete, palette.del)
                                            .on_click(move |_, _window, cx| {
                                                let _ = drop_view.update(cx, |deck, cx| {
                                                    deck.drop_remark(remark_ix, cx);
                                                });
                                            }),
                                    ),
                            ),
                    )
                    .when(!mark.folded, |this| {
                        this.child(
                            div()
                                .v_flex()
                                .gap(px(2.))
                                .font_family(cx.theme().font_family.clone())
                                .text_size(px(12.5))
                                .line_height(px(19.))
                                .text_color(paint(palette.fg))
                                // A line each, so a remark written as a list
                                // still reads as one.
                                .children(
                                    mark.text.lines().map(|line| div().child(line.to_string())),
                                ),
                        )
                    })
                    .into_any_element()
            })
            .collect()
    }
}

/// Split the file into rows, each carrying its own syntax runs.
///
/// The highlighter works over the whole file and reports runs as byte offsets
/// into it, so each run is cut to the line it falls on and re-based to that
/// line's start — which is the form [`StyledText`] wants.
fn highlight(source: &str, lines: &[&str], file: &std::path::Path, cx: &App) -> Vec<Row> {
    let mut rows: Vec<Row> = lines
        .iter()
        .enumerate()
        .map(|(ix, line)| Row {
            text: SharedString::from((*line).to_string()),
            runs: Vec::new(),
            number: u32::try_from(ix + 1).ok(),
            pin: u32::try_from(ix + 1).unwrap_or(u32::MAX),
            answers: LineRange::single(u32::try_from(ix + 1).unwrap_or(u32::MAX)),
            change: None,
            inlay: Vec::new(),
            extra: None,
        })
        .collect();

    // The one highlighter every export uses too, so code in a PDF is coloured
    // exactly as it is here.
    let styles = deck_render::styled(source, file, &cx.theme().highlight_theme);

    // Nothing to lay runs against. The highlighter answers for an empty
    // document with a run all the same, and every index below is into a line
    // table that has no lines in it.
    if lines.is_empty() {
        return rows;
    }

    // Where each line begins, so a byte offset can be turned into a line.
    let mut starts = Vec::with_capacity(lines.len());
    let mut at = 0usize;
    for line in lines {
        starts.push(at);
        at += line.len() + 1; // the newline the iterator dropped
    }

    for (range, style) in styles {
        // The line this run starts on. A run never spans lines: the highlighter
        // reports one per token.
        let ix = starts.partition_point(|&start| start <= range.start).max(1) - 1;
        let (start, len) = (starts[ix], rows[ix].text.len());
        let from = range.start.saturating_sub(start);
        let to = (range.end - start).min(len);
        if from < to {
            rows[ix].runs.push((from..to, style));
        }
    }

    rows
}

/// The controls any pane wears in its header: a fold for one the deck put
/// there, a close for one that was brought in.
///
/// Every kind of pane, not only a file. A picture or a page with neither was
/// a pane the reader could not get out of the way (#31), and one brought in to
/// answer a question could not be sent back (#32).
///
/// A borrowed pane offers no fold, only a close. Folding one would leave a
/// spine for something that is not part of the deck, and when it is the only
/// thing open the fold has to be refused anyway — which reads as the way back
/// being barred.
pub(crate) fn controls(slot: &Slot, focus_button: Option<AnyElement>) -> Controls {
    let open = slot.fold <= 0.;
    Controls {
        aside: None,
        focus_button,
        values: None,
        fold: (open && slot.foldable && !slot.temporary).then(|| fold_button(slot)),
        close: (open && slot.temporary).then(|| close_button(slot)),
    }
}

/// A link in a note: the accent, underlined faintly, and pressing it switches
/// the deck between its own copy of the code and the reader's.
fn switch(id: (&'static str, usize), words: &'static str, slot: &Slot) -> impl IntoElement {
    let palette = slot.palette;
    let view = slot.view.clone();
    let mut line = paint(palette.accent);
    line.a = 0.4;
    div()
        .id(id)
        .flex_none()
        .cursor_pointer()
        .text_color(paint(palette.accent))
        .border_b_1()
        .border_color(line)
        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
            let _ = view.update(cx, |deck, cx| deck.toggle_live(cx));
        })
        .child(words)
}

/// The note at the end of a pane's header: where the code stands against the
/// reader's checkout, and the way to the other copy.
fn aside_note(aside: &Aside, slot: &Slot, cx: &App) -> AnyElement {
    let palette = slot.palette;
    let mono = cx.theme().mono_font_family.clone();
    let commit = |sha: &SharedString| div().font_family(mono.clone()).child(sha.clone());
    let note = div()
        .font_family(cx.theme().font_family.clone())
        .text_size(px(11.))
        .h_flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .whitespace_nowrap()
        .text_color(paint(palette.muted));
    match aside {
        Aside::Moved { since } => note
            .child("code has moved since")
            .child(commit(since))
            .child("·")
            .child(switch(("show-live", slot.ix), "Show live", slot)),
        Aside::Differs => note
            .child("differs from the deck's copy")
            .child("·")
            .child(switch(("show-live", slot.ix), "Show live", slot)),
        Aside::Lost => note
            .child("couldn't follow these lines into your checkout")
            .child("·")
            .child(switch(("show-deck", slot.ix), "Show the deck's copy", slot)),
        // The dot says this is not the deck's copy any more, in the accent,
        // so a reader glancing at the header can tell which they are reading.
        Aside::Live { head } => note
            .child(div().size(px(5.)).rounded_full().bg(paint(palette.accent)))
            .child("live, from")
            .child(commit(head))
            .child("·")
            .child(switch(("show-deck", slot.ix), "Show the deck's copy", slot)),
    }
    .into_any_element()
}

/// What a pane says in place of code it does not have: the file, in the code
/// face, and why — `src/flush.ts is not in this deck.` — with the reader's
/// own copy offered when their checkout has one.
fn missing_note(missing: &Missing, slot: &Slot, cx: &App) -> impl IntoElement {
    let palette = slot.palette;
    let mono = cx.theme().mono_font_family.clone();
    div()
        .flex_1()
        .min_h_0()
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .px(px(20.))
        .py(px(28.))
        .child(
            div()
                .h_flex()
                .flex_wrap()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .text_color(paint(palette.muted))
                .child(
                    div()
                        .font_family(mono)
                        .text_color(paint(palette.fg.mix(palette.wash, 0.2)))
                        .child(missing.file.clone()),
                )
                .child(missing.said.clone())
                .when_some(missing.offer, |this, offer| {
                    this.child(div().px(px(5.)).child("·")).child(switch(
                        ("missing-switch", slot.ix),
                        match offer {
                            crate::load::Offer::Live => "Show live",
                            crate::load::Offer::Deck => "Show the deck's copy",
                        },
                        slot,
                    ))
                }),
        )
}

/// The control that closes a pane brought in to answer a question.
fn close_button(slot: &Slot) -> AnyElement {
    let palette = slot.palette;
    let ix = slot.ix;
    let view = slot.view.clone();
    div()
        .id(("close-pane", ix))
        .size(px(18.))
        .flex_none()
        .rounded(px(5.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_size(px(12.))
        .text_color(paint(palette.muted))
        .hover(|style| style.bg(paint(palette.band)).text_color(paint(palette.fg)))
        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
            let _ = view.update(cx, |deck, cx| deck.close_brought(ix, cx));
        })
        .child("×")
        .into_any_element()
}

/// The control that folds a pane away.
/// The eye: every value in the pane out, or every value away.
///
/// Open when they are out and crossed when they are away, so it says what the
/// pane is showing now — and pressing it is the one way they leave once out.
fn values_button(slot: &Slot, shown: bool) -> AnyElement {
    let palette = slot.palette;
    let ix = slot.ix;
    let view = slot.view.clone();
    div()
        .id(("values-pane", ix))
        .size(px(18.))
        .flex_none()
        .rounded(px(5.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(paint(if shown { palette.accent } else { palette.muted }))
        .hover(|style| style.bg(paint(palette.band)))
        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
            let _ = view.update(cx, |deck, cx| deck.toggle_values_shown(ix, cx));
        })
        .child(
            Icon::new(if shown {
                IconName::Eye
            } else {
                IconName::EyeOff
            })
            .size(px(12.)),
        )
        .into_any_element()
}

fn fold_button(slot: &Slot) -> AnyElement {
    let palette = slot.palette;
    let ix = slot.ix;
    let view = slot.view.clone();
    div()
        .id(("fold-pane", ix))
        .size(px(18.))
        .flex_none()
        .rounded(px(5.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_size(px(12.))
        .text_color(paint(palette.muted))
        .hover(|style| style.bg(paint(palette.band)))
        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
            let _ = view.update(cx, |deck, cx| deck.fold_pane(ix, true, cx));
        })
        .child(if slot.fold_left { "‹" } else { "›" })
        .into_any_element()
}

/// What a picture's or a page's spine says: what it is called, and under
/// that whatever says more.
///
/// Its name over its title when it has both. When the two are the same word —
/// a page is labelled by its name — the note it was given stands under it
/// instead, so two unnamed pages are not both just "page". Nothing stands
/// under it that only says it again.
pub(crate) fn spine_words(
    name: Option<&SharedString>,
    label: &SharedString,
    note: Option<&SharedString>,
) -> (SharedString, Option<SharedString>) {
    let title = name.unwrap_or(label).clone();
    let under = if same(label, &title) {
        note
    } else {
        Some(label)
    };
    let under = under.filter(|under| !same(under, &title));
    (title, under.cloned())
}

/// What a pane's header says, each part only once: its name, its label unless
/// that is just the name (a named page's always is), and its note unless that only
/// repeats one of the two (a picture whose note is its own title).
pub(crate) fn header_words(
    name: Option<SharedString>,
    label: SharedString,
    note: Option<SharedString>,
) -> (
    Option<SharedString>,
    Option<SharedString>,
    Option<SharedString>,
) {
    let label = Some(label).filter(|label| name.as_ref().is_none_or(|name| !same(name, label)));
    let note = note.filter(|note| {
        label.as_ref().is_none_or(|label| !same(note, label))
            && name.as_ref().is_none_or(|name| !same(note, name))
    });
    (name, label, note)
}

/// The same words, whatever space is around them.
fn same(a: &str, b: &str) -> bool {
    a.trim() == b.trim()
}

/// A header's label cut where the file's name begins: the folders, ending in
/// their slash, and the name with its range. A label with no slash is all
/// name, and has no folders.
fn split_path(label: &str) -> (String, String) {
    match label.rfind('/') {
        Some(at) => (label[..=at].to_string(), label[at + 1..].to_string()),
        None => (String::new(), label.to_string()),
    }
}

/// A pane folded down to its spine at the window's edge.
///
/// `title` is what the prose calls it, `under` whatever says which one it is
/// when two share a name, and `said` the whole of it for the tooltip. Clicking
/// anywhere on it opens the pane again.
pub(crate) fn spine(
    slot: &Slot,
    title: &SharedString,
    under: Option<&SharedString>,
    said: SharedString,
    cx: &App,
) -> AnyElement {
    let palette = slot.palette;
    let mono = cx.theme().mono_font_family.clone();
    let ix = slot.ix;
    let fold = slot.fold;
    let left = slot.fold_left;
    let view = slot.view.clone();

    div()
        .flex_none()
        // The spine grows as the pane gives up its place in the grid, so
        // the two movements are the one movement.
        .w(px(SPINE * fold))
        .h_full()
        .overflow_hidden()
        .child(
            div()
                .id(("pane-spine", ix))
                .w(px(SPINE))
                .h_full()
                .v_flex()
                .items_center()
                .pt(px(12.))
                .pb(px(14.))
                .bg(paint(palette.wash))
                .cursor_pointer()
                .hover(|style| style.bg(paint(palette.wash.mix(palette.band, 0.55))))
                .tooltip(move |_window, cx| {
                    cx.new(|_| gpui_kit::component::tooltip::Tooltip::new(said.clone()))
                        .into()
                })
                .on_mouse_down(MouseButton::Left, {
                    let view = view.clone();
                    move |_, _window, cx| {
                        let _ = view.update(cx, |deck, cx| deck.fold_pane(ix, false, cx));
                    }
                })
                .child(
                    div()
                        .size(px(18.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.))
                        .text_color(paint(palette.muted))
                        // Pointing the way the pane will open.
                        .child(if left { "›" } else { "‹" }),
                )
                // The name the prose calls it, and the file underneath, both
                // turned on their side — which the panel could not do until the
                // words went through a picture.
                .child(
                    div()
                        .mt(px(10.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .child(sideways(title, &mono, 12.5, 700, palette.accent, left)),
                )
                .children(under.map(|under| {
                    div()
                        .mt(px(2.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .child(sideways(under, &mono, 11.5, 500, palette.fg, left))
                }))
                // The spine itself: a line down the rest of the pane, which is what
                // the word means and all the room there is for it.
                .child(div().mt(px(12.)).w(px(1.)).flex_1().bg(paint(palette.edge))),
        )
        .into_any_element()
}

/// The label row a pane wears: what it is showing, and what to make of it.
///
/// Shared with the diagram pane. The two draw entirely different bodies, but a
/// pane is recognisable by its header — a deck where one kind of pane wore a
/// different hat would read as two tools sharing a window.
pub fn chrome(
    name: Option<SharedString>,
    label: SharedString,
    note: Option<SharedString>,
    controls: Controls,
    palette: &Palette,
    cx: &App,
) -> impl IntoElement {
    let Controls {
        aside,
        focus_button,
        values,
        fold,
        close,
    } = controls;
    let (name, label, note) = header_words(name, label, note);
    div()
        .h_flex()
        .flex_none()
        .justify_between()
        .items_center()
        .gap_3()
        .pl(px(14.))
        .pr(px(14.))
        .pt(px(7.))
        .pb(px(6.))
        // Opaque, so the code passes beneath it rather than through it.
        .bg(paint(palette.wash))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(10.5))
        .text_color(paint(palette.muted))
        // Paths and notes can be arbitrarily long; they must give up width
        // before the recovery control does. A single unbounded row pushed
        // Focus beyond the pane's clip, precisely when it was needed.
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(3.))
                // The name first, in the accent: it is the word the prose uses,
                // so it is the word a reader's eye goes looking for up here.
                .child(
                    div()
                        .h_flex()
                        .gap(px(8.))
                        .min_w_0()
                        // Alone on the row — a page — the name gives up width
                        // like the label would have.
                        .children(name.map(|name| {
                            div()
                                .when(label.is_some(), |this| this.flex_none())
                                .when(label.is_none(), |this| this.min_w_0().truncate())
                                .text_color(paint(palette.accent))
                                .child(name)
                        }))
                        // The folders give way and the file never does: a path
                        // cut from its end lost the file's name and its range,
                        // the two things a reader looks up here for. A label
                        // with no folders is cut from its end, as before.
                        .children(label.map(|label| {
                            let (folders, file) = split_path(&label);
                            if folders.is_empty() {
                                div().min_w_0().truncate().child(file).into_any_element()
                            } else {
                                div()
                                    .h_flex()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .child(div().min_w_0().truncate().child(folders))
                                    .child(div().flex_none().whitespace_nowrap().child(file))
                                    .into_any_element()
                            }
                        })),
                )
                .children(
                    note.as_deref()
                        .map(|note| div().overflow_hidden().child(emphasise(note, palette))),
                ),
        )
        .child(
            div()
                .h_flex()
                .flex_none()
                .items_center()
                .gap(px(6.))
                .children(aside)
                .children(focus_button)
                .children(values)
                .children(fold)
                .children(close),
        )
}

/// What a pane's header offers, beside its name.
#[derive(Default)]
pub struct Controls {
    /// A note about where the code stands, before the buttons. See [`Aside`].
    pub aside: Option<AnyElement>,
    /// The way back to the lit range, when it has been scrolled away from.
    pub focus_button: Option<AnyElement>,
    /// The eye that shows or hides a pane's values, when it has any.
    pub values: Option<AnyElement>,
    /// The way to fold the pane down to its spine.
    pub fold: Option<AnyElement>,
    /// The way to close a pane that was brought in to answer a question.
    pub close: Option<AnyElement>,
}

/// A note, with its emphasised words picked out in the accent.
///
/// The accent is for the word the agent wants looked at — `extends the page
/// list *in place*` — not for the whole note. Painting every note in it spends
/// the one colour the design reserves for "look here" on text that is only a
/// label, and then nothing stands out because everything does.
fn emphasise(note: &str, palette: &Palette) -> impl IntoElement {
    let mut spans: Vec<AnyElement> = Vec::new();
    let mut accented = false;

    for part in note.split('*') {
        if !part.is_empty() {
            let colour = if accented {
                palette.accent
            } else {
                palette.muted
            };
            spans.push(
                div()
                    .flex_none()
                    .text_color(paint(colour))
                    .child(part.to_string())
                    .into_any_element(),
            );
        }
        accented = !accented;
    }

    div().h_flex().flex_none().children(spans)
}

/// Which row of the list a file line is drawn on.
///
/// Free of the pane so the arithmetic can be checked without a window, which
/// matters more here than usual: it is off-by-one arithmetic that fails
/// silently, by putting a comment card or a scroll target a couple of lines
/// from where the reader pointed.
fn row_of(number: u32, added_at: u32, added: usize) -> usize {
    let ix = (number.max(1) - 1) as usize;
    if number > added_at { ix + added } else { ix }
}

/// How many rows above its own slot row `ix` is drawn, for the values in
/// motion over it: each stretch above it still opening, or closing, lifts it
/// by however much of the stretch's room is not there yet. Nought when
/// nothing above it is moving.
#[allow(clippy::cast_precision_loss)]
fn lift_at(blocks: &[(usize, usize, f32, f32)], ix: usize) -> f32 {
    blocks
        .iter()
        .filter(|(_, last, _, _)| ix > *last)
        .map(|(first, last, slide, _)| (last - first + 1) as f32 * (1. - slide))
        .sum()
}

/// Where a pick from row `from` towards row `to` stops: at `to`, or at the
/// last row before an edge of the deck's copy, whichever comes first.
fn within_span(rows: &[Row], from: usize, to: usize) -> usize {
    let edge = |ix: &usize| {
        rows.get(*ix)
            .is_some_and(|row| matches!(row.extra, Some(Extra::Absent { .. })))
    };
    if to > from {
        (from + 1..=to).find(edge).map_or(to, |ix| ix - 1)
    } else {
        (to..from).rev().find(edge).map_or(to, |ix| ix + 1)
    }
}

/// The stretches of a file a deck's copy leaves out, given the spans it kept
/// and, when it is known, how many lines the file had.
///
/// One before the first span when it does not start at line one, one between
/// each pair, and one after the last unless the file ends there. The last is
/// open — `None` — when the deck does not say where the file ends. Nothing at
/// all for a copy that kept nothing: there is no edge to mark.
fn gaps(kept: &[LineRange], lines: Option<u32>) -> Vec<(u32, Option<u32>)> {
    let mut gaps = Vec::new();
    let mut next = 1u32;
    for span in kept {
        if span.first > next {
            gaps.push((next, Some(span.first - 1)));
        }
        next = span.last.saturating_add(1);
    }
    if kept.is_empty() {
        return gaps;
    }
    match lines {
        Some(lines) if lines < next => {}
        Some(lines) => gaps.push((next, Some(lines))),
        None => gaps.push((next, None)),
    }
    gaps
}

/// What the row for a stretch left out says.
fn absent_words(first: u32, last: Option<u32>) -> String {
    match last {
        Some(last) if last == first => format!("line {first} not in this deck"),
        Some(last) => format!("lines {first}–{last} not in this deck"),
        // Where the file ends is not known, so the row says only what is: the
        // copy stops here.
        None => format!(
            "nothing past line {} is in this deck",
            first.saturating_sub(1)
        ),
    }
}

/// `rows` cut down to the spans a deck's copy kept, with a row for each
/// stretch left out, where it was.
///
/// A row of a proposed change is kept whatever its number says: it is not a
/// line of the file, and its range was kept with the ref it belongs to.
fn trimmed(rows: Vec<Row>, kept: &[LineRange], lines: Option<u32>) -> Vec<Row> {
    let inside = |row: &Row| row.change.is_some() || kept.iter().any(|span| span.contains(row.pin));
    let mut gaps = gaps(kept, lines).into_iter().peekable();
    let mut out = Vec::with_capacity(rows.len() + kept.len() + 1);
    for row in rows.into_iter().filter(inside) {
        while let Some((first, last)) =
            gaps.next_if(|(first, _)| row.change.is_none() && *first <= row.pin)
        {
            out.push(absent(first, last));
        }
        out.push(row);
    }
    out.extend(gaps.map(|(first, last)| absent(first, last)));
    out
}

/// The row that says lines `first` to `last` are not in this deck.
fn absent(first: u32, last: Option<u32>) -> Row {
    Row {
        text: SharedString::default(),
        runs: Vec::new(),
        number: None,
        pin: first,
        answers: LineRange::new(first, last.unwrap_or(first).max(first)),
        change: None,
        inlay: Vec::new(),
        extra: Some(Extra::Absent { first, last }),
    }
}

/// What a row shows to the right of its number: the code, with a single value
/// after its name, or a value's handle, or one of its keys.
fn row_content(
    row: &Row,
    sans: &SharedString,
    palette: &Palette,
    advance: Pixels,
    small_advance: Pixels,
    size: Pixels,
    grows: &[(f32, f32)],
) -> AnyElement {
    let faint = palette.muted.mix(palette.wash, 0.35);
    let soft = palette.fg.mix(palette.wash, 0.25);
    let small = size * 0.92;
    // The trail from a value down its keys: brighter than a pane edge, so the
    // keys read as hanging from the line above them.
    let trail = palette.edge.mix(palette.muted, 0.55);
    let gap = |chars: usize| {
        #[allow(clippy::cast_precision_loss)]
        let wide = advance * chars as f32;
        div().w(wide).flex_none()
    };
    match &row.extra {
        None => {
            if row.inlay.is_empty() {
                return StyledText::new(row.text.clone())
                    .with_highlights(row.runs.clone())
                    .into_any_element();
            }
            // The line cut at each value, so each one sits right after its own
            // name and the code between them keeps its colours.
            let piece = |from: usize, to: usize| {
                let runs: Vec<_> = row
                    .runs
                    .iter()
                    .filter(|(range, _)| range.start < to && range.end > from)
                    .map(|(range, style)| {
                        (
                            range.start.max(from) - from..range.end.min(to) - from,
                            *style,
                        )
                    })
                    .collect();
                StyledText::new(row.text[from..to].to_string()).with_highlights(runs)
            };
            let mut line = div().h_flex().items_center().flex_none();
            let mut from = 0;
            for (ix, (at, value, lit, who)) in row.inlay.iter().enumerate() {
                // Worked out for a proposal rather than seen: set in italic,
                // so it reads as *would be* beside the values that *were*.
                let predicted = who.1.starts_with('+');
                let grow = grows.get(ix).copied().unwrap_or((1., 1.));
                let at = (*at).clamp(from, row.text.len());
                line = line.child(piece(from, at));
                // Soft tint: faint words on a ground that is barely there,
                // and no border. A box read as code; this reads as a note on
                // it. Written live, it makes its own room first — the rest of
                // the line eases right — and then its words come in.
                // Always this wide, moving or still: the words plus their
                // padding, measured from the font. A room that switched from
                // an estimate to its natural width at rest jumped as it did.
                #[allow(clippy::cast_precision_loss)]
                let room = (small_advance * cells(value) as f32 + px(10.)) * grow.0;
                line = line.child(
                    div()
                        .flex_none()
                        .w(room)
                        .overflow_hidden()
                        .ml(px(5.) * grow.0)
                        .mr(px(2.) * grow.0)
                        .px(px(5.) * grow.0)
                        .rounded(px(3.))
                        .text_size(small)
                        // The room opens empty; the tint arrives with the words,
                        // so nothing is seen growing.
                        .bg({
                            // See-through until the words come: a solid ground
                            // at nought was the pane's own colour, a dark box
                            // on a lit line.
                            // And a wash over whatever is under it rather than a
                            // colour of its own, so on a lit line it lightens
                            // the light instead of cutting a darker patch in it.
                            let mut tint = paint(if *lit { palette.accent } else { palette.fg });
                            tint.a = if *lit { 0.16 } else { 0.10 } * grow.1;
                            tint
                        })
                        // Readable at a glance: the value is the evidence, and
                        // a faint grey made it the one thing on the line that
                        // had to be looked for. The lit one takes the accent.
                        .text_color(paint(if *lit {
                            palette.accent
                        } else {
                            palette.fg.mix(palette.wash, 0.18)
                        }))
                        .child(
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .opacity(grow.1)
                                .when(predicted, |this| this.italic())
                                .child(value.clone()),
                        ),
                );
                from = at;
            }
            line.child(piece(from, row.text.len())).into_any_element()
        }
        // The same button open or closed, words and all: only the caret turns.
        // A button that changed what it said as it opened read as a different
        // button, and the reader had to find their place on it again.
        Some(Extra::Chip {
            name,
            preview,
            open,
            indent,
            ..
        }) => {
            let preview = first_key(preview);
            div()
                .h_flex()
                .items_center()
                .flex_none()
                .child(gap(*indent))
                .child(div().text_color(paint(trail)).child("↳ "))
                .child(
                    div()
                        .text_color(paint(palette.muted))
                        .mr(px(8.))
                        .child(name.clone()),
                )
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(7.))
                        .px(px(7.))
                        .py(px(1.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(paint(palette.edge))
                        .bg(paint(palette.wash.mix(palette.fg, 0.04)))
                        .text_size(small)
                        .cursor_pointer()
                        .child(
                            // A fixed box, so the preview beside it does not
                            // move under the pointer when the caret turns; and
                            // the small carets every other disclosure uses.
                            div()
                                .flex_none()
                                .w(px(9.))
                                .flex()
                                .justify_center()
                                .text_color(paint(palette.accent))
                                .text_size(size * 0.7)
                                .child(if *open { "▾" } else { "▸" }),
                        )
                        .child(div().text_color(paint(soft)).child(preview)),
                )
                .into_any_element()
        }
        // Said quietly, in the prose face rather than the code's: it is a note
        // about the code, not a line of it. Upright: the window embeds no
        // italic of that face, and a slant the renderer has to fake reads as
        // a mistake rather than a voice.
        Some(Extra::Absent { first, last }) => div()
            .flex_none()
            .whitespace_nowrap()
            .font_family(sans.clone())
            .text_size(small)
            .text_color(paint(palette.muted))
            .child(absent_words(*first, *last))
            .into_any_element(),
        // The lit key's ground runs to the pane's edge, as a lit line does.
        Some(Extra::Key {
            pad: true, indent, ..
        }) => div()
            .h_flex()
            .flex_1()
            .h_full()
            .child(gap(*indent))
            .child(div().h_full().border_l_1().border_color(paint(trail)))
            .into_any_element(),
        Some(Extra::Key {
            name,
            is,
            why,
            lit,
            more,
            width,
            indent,
            ..
        }) => div()
            .h_flex()
            .items_center()
            .flex_1()
            .h_full()
            .child(gap(*indent))
            .child(
                // The trail down the whole row, the key and its value beside it.
                div()
                    .flex_1()
                    .h_full()
                    .border_l_1()
                    .border_color(paint(trail))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .relative()
                            .h_flex()
                            .items_center()
                            .pl(advance)
                            // A wash of the accent over whatever is under it. A
                            // fixed colour mixed from the page's ground went dull
                            // on a lit or pointed line, darker than the row it sat on.
                            .when(*lit, |this| {
                                let mut wash = paint(palette.accent);
                                wash.a = 0.18;
                                this.bg(wash)
                            })
                            .child(
                                div()
                                    .w({
                                        #[allow(clippy::cast_precision_loss)]
                                        let cols = *width as f32 + 2.;
                                        advance * cols
                                    })
                                    .flex_none()
                                    .text_color(paint(if *more {
                                        faint
                                    } else if *lit {
                                        palette.accent
                                    } else {
                                        palette.muted
                                    }))
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .text_color(paint(if *lit { palette.fg } else { soft }))
                                    .child(is.clone()),
                            )
                            .when_some(why.clone(), |this, why| {
                                this.child(
                                    div()
                                        .ml(px(10.))
                                        .text_size(small)
                                        .text_color(paint(faint))
                                        .child(why),
                                )
                            }),
                    ),
            )
            .into_any_element(),
    }
}

/// A value's identity: its line, its name, and which half of a change it is
/// about. A proposal's value and the code's own can share a line number and a
/// name, and they are two different values.
fn who_of(value: &Value) -> Who {
    let at: SharedString = if value.after {
        format!("+{}", value.at).into()
    } else {
        value.at.clone().into()
    };
    (value.line, at)
}

/// Whether `name` appears in `text` as a whole word.
fn has_word(text: &str, name: &str) -> bool {
    !name.is_empty() && whole_word(text, name).is_some()
}

/// How many monospace cells `text` takes: two for a wide character, four for
/// a tab. Counting characters clipped a value written in Japanese or with an
/// emoji in it, because its room was half as wide as its words.
fn cells(text: &str) -> usize {
    text.chars()
        .map(|ch| {
            if ch == '\t' {
                4
            } else {
                unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0)
            }
        })
        .sum()
}

/// A preview cut after its first key: `Session { a: 1, b: 2, … }` reads
/// `Session { a: 1, … }`. The first key is the one the claim is about, and the
/// rest made the button as wide as the line it hangs under.
fn first_key(preview: &str) -> SharedString {
    let Some(open) = preview.find(" {") else {
        return preview.to_string().into();
    };
    let mut quoted = false;
    let mut depth = 0i32;
    for (ix, ch) in preview[open + 2..].char_indices() {
        match ch {
            '"' => quoted = !quoted,
            '{' | '[' if !quoted => depth += 1,
            '}' | ']' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                let end = open + 2 + ix;
                return format!("{}, … }}", &preview[..end]).into();
            }
            _ => {}
        }
    }
    preview.to_string().into()
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this module glob-imports GPUI, which
    // exports a `test` attribute of its own and would otherwise shadow the
    // standard one.
    use core::prelude::v1::test;

    use super::*;

    #[test]
    fn a_header_label_splits_at_the_file_name() {
        assert_eq!(
            split_path("apps/web/store/option.store.ts:217-228"),
            (
                "apps/web/store/".to_string(),
                "option.store.ts:217-228".to_string()
            )
        );
        assert_eq!(
            split_path("README.md:1-4"),
            (String::new(), "README.md:1-4".to_string())
        );
    }

    /// A plain line of the file, numbered `n`.
    fn line(n: u32) -> Row {
        Row {
            text: format!("line {n}").into(),
            runs: Vec::new(),
            number: Some(n),
            pin: n,
            answers: LineRange::single(n),
            change: None,
            inlay: Vec::new(),
            extra: None,
        }
    }

    #[test]
    fn a_value_is_open_by_the_time_its_line_is_lit() {
        // A value opens at the same moment the sentence lights its line. Any
        // longer than the light and the reader is left looking at lit code
        // and an empty gap, waiting for the thing the sentence is about.
        assert!(UNFOLD <= RISE, "{UNFOLD:?} against {RISE:?}");
        assert!(REFOLD < UNFOLD);
    }

    #[test]
    fn only_the_rows_under_a_moving_value_are_lifted() {
        // Rows 3 to 5 opening, a quarter of the way: the room for them is a
        // quarter there, so everything under them is drawn three quarters of
        // three rows higher than its slot, and nothing above them moves.
        let blocks = [(3, 5, 0.25, 0.)];
        assert!(lift_at(&blocks, 2).abs() < f32::EPSILON);
        assert!(lift_at(&blocks, 5).abs() < f32::EPSILON);
        assert!((lift_at(&blocks, 6) - 2.25).abs() < 1e-5);
        assert!((lift_at(&blocks, 400) - 2.25).abs() < 1e-5);
        // Two at once add up below both.
        let blocks = [(3, 5, 0.25, 0.), (10, 10, 0., 0.)];
        assert!((lift_at(&blocks, 11) - 3.25).abs() < 1e-5);
        // And a value at rest lifts nothing.
        assert!(lift_at(&[(3, 5, 1., 1.)], 9).abs() < f32::EPSILON);
    }

    /// What a pane's rows say, one word per row: the number of a line, or the
    /// words of an edge.
    fn said(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| match row.extra {
                Some(Extra::Absent { first, last }) => absent_words(first, last),
                _ => row
                    .number
                    .map_or_else(|| "+".to_string(), |n| n.to_string()),
            })
            .collect()
    }

    #[test]
    fn the_deck_copy_shows_only_what_it_kept_with_an_edge_either_side() {
        let rows: Vec<Row> = (1..=12).map(line).collect();
        let kept = [LineRange::new(3, 4), LineRange::new(8, 9)];

        assert_eq!(
            said(&trimmed(rows, &kept, Some(20))),
            [
                "lines 1–2 not in this deck",
                "3",
                "4",
                "lines 5–7 not in this deck",
                "8",
                "9",
                "lines 10–20 not in this deck",
            ],
            "every line keeps its real number, and every stretch left out is one row"
        );
    }

    #[test]
    fn a_copy_that_reaches_either_end_of_the_file_has_no_edge_there() {
        let rows: Vec<Row> = (1..=5).map(line).collect();
        assert_eq!(
            said(&trimmed(rows, &[LineRange::new(1, 5)], Some(5))),
            ["1", "2", "3", "4", "5"]
        );
        assert_eq!(gaps(&[LineRange::new(1, 5)], Some(5)), []);
        assert_eq!(
            gaps(&[LineRange::new(2, 5)], Some(6)),
            [(1, Some(1)), (6, Some(6))]
        );
        assert_eq!(absent_words(6, Some(6)), "line 6 not in this deck");
    }

    #[test]
    fn a_deck_that_does_not_say_how_long_the_file_was_says_only_where_it_stops() {
        assert_eq!(
            gaps(&[LineRange::new(3, 9)], None),
            [(1, Some(2)), (10, None)]
        );
        assert_eq!(
            absent_words(10, None),
            "nothing past line 9 is in this deck"
        );
        assert_eq!(gaps(&[], None), [], "a copy that kept nothing has no edges");
    }

    #[test]
    fn a_pick_stops_at_the_edge_of_the_copy() {
        let rows = trimmed(
            (1..=12).map(line).collect(),
            &[LineRange::new(3, 4), LineRange::new(8, 9)],
            Some(12),
        );
        // edge, 3, 4, edge, 8, 9, edge
        assert_eq!(within_span(&rows, 1, 5), 2, "down: stops on line 4");
        assert_eq!(within_span(&rows, 5, 0), 4, "up: stops on line 8");
        assert_eq!(within_span(&rows, 4, 5), 5, "inside one span, as asked");
        assert_eq!(within_span(&rows, 2, 1), 1);
    }

    #[test]
    fn a_proposed_change_survives_the_cut() {
        // The arriving lines of a change carry the numbers they will have,
        // and are kept with their range rather than judged by those numbers.
        let mut rows: Vec<Row> = (1..=6).map(line).collect();
        let mut arriving = line(4);
        arriving.number = None;
        arriving.change = Some(Change::New);
        rows.insert(4, arriving);

        assert_eq!(
            said(&trimmed(rows, &[LineRange::new(3, 4)], Some(6))),
            [
                "lines 1–2 not in this deck",
                "3",
                "4",
                "+",
                "lines 5–6 not in this deck"
            ]
        );
    }

    #[test]
    fn a_value_goes_after_the_whole_name_and_not_inside_another() {
        let line = "export async function markPaid(id: string, cents: number) {";
        // `id` is inside `markPaid` first; the value belongs after the parameter.
        assert_eq!(
            &line[..after_name(line, "id")],
            "export async function markPaid(id"
        );
        assert_eq!(
            &line[..after_name(line, "cents")],
            "export async function markPaid(id: string, cents"
        );
        // A name made of several words is matched as written.
        let check = "  if (rowCount === 0) return;";
        assert_eq!(
            &check[..after_name(check, "rowCount === 0")],
            "  if (rowCount === 0"
        );
        // A name that is not there puts the value at the end of the line.
        assert_eq!(after_name(check, "missing"), check.len());
    }

    #[test]
    fn a_value_is_measured_in_cells_not_characters() {
        assert_eq!(cells("\"pay_12A\""), 9);
        // Wide characters take two cells, and a tab four.
        assert_eq!(cells("注文"), 4);
        assert_eq!(cells("a\tb"), 6);
    }

    #[test]
    fn a_sentence_names_a_key_only_as_a_whole_word() {
        let said = "Stripe carried it in `client_reference_id`, and metadata was empty.";
        assert!(has_word(said, "client_reference_id"));
        assert!(has_word(said, "metadata"));
        // Inside another word is not naming it.
        assert!(!has_word("the reference was set", "ref"));
        assert!(!has_word(said, "id"));
        assert!(!has_word(said, ""));
        // A line that merely ends in the name is not naming it.
        assert!(!has_word("the field is client_id", "id"));
    }

    #[test]
    fn a_preview_keeps_its_first_key_and_counts_the_rest() {
        assert_eq!(
            first_key(r#"Session { client_reference_id: "ord_8F2kQ71", metadata: {}, … }"#)
                .as_ref(),
            r#"Session { client_reference_id: "ord_8F2kQ71", … }"#
        );
        // A comma inside a string or a nested value is not the end of the key.
        assert_eq!(
            first_key(r#"Event { note: "a, b", tags: [1, 2] }"#).as_ref(),
            r#"Event { note: "a, b", … }"#
        );
        assert_eq!(first_key("Empty { }").as_ref(), "Empty { }");
    }

    #[test]
    fn a_spine_says_the_name_and_then_whatever_says_more() {
        let words = |name: Option<&str>, label: &str, note: Option<&str>| {
            let (name, note) = (name.map(SharedString::from), note.map(SharedString::from));
            let (title, under) =
                spine_words(name.as_ref(), &SharedString::from(label), note.as_ref());
            (title.to_string(), under.map(|under| under.to_string()))
        };
        // A named picture: its name, over its title.
        assert_eq!(
            words(Some("flow"), "how it travels", Some("the path")),
            ("flow".into(), Some("how it travels".into()))
        );
        // A page is labelled by its name, so the note stands under it.
        assert_eq!(
            words(Some("rows"), "rows", Some("which row wins")),
            ("rows".into(), Some("which row wins".into()))
        );
        // Nothing but a label.
        assert_eq!(words(None, "page", None), ("page".into(), None));
        // A note that is its own title is not said twice.
        assert_eq!(
            words(None, "search", Some(" search")),
            ("search".into(), None)
        );
        assert_eq!(
            words(Some("rows"), "rows", Some("rows")),
            ("rows".into(), None)
        );
    }

    #[test]
    fn a_header_says_each_thing_once() {
        let words = |name: Option<&str>, label: &str, note: Option<&str>| {
            let (name, label, note) = header_words(
                name.map(SharedString::from),
                SharedString::from(label),
                note.map(SharedString::from),
            );
            let text = |word: Option<SharedString>| word.map(|word| word.to_string());
            (text(name), text(label), text(note))
        };
        let some = |word: &str| Some(word.to_string());
        // A page is labelled by its name: the name, once, and its note.
        assert_eq!(
            words(Some("shape"), "shape", Some("the query")),
            (some("shape"), None, some("the query"))
        );
        // A picture whose note is its title.
        assert_eq!(
            words(None, "search, end to end", Some("search, end to end")),
            (None, some("search, end to end"), None)
        );
        // A named picture with its own title: both.
        assert_eq!(
            words(Some("map"), "search", Some("one query")),
            (some("map"), some("search"), some("one query"))
        );
        // A note that only repeats the name, and space around the words.
        assert_eq!(
            words(Some("map"), "map ", Some(" map")),
            (some("map"), None, None)
        );
        // A page with no name: still "page", and its note.
        assert_eq!(
            words(None, "page", Some("which row wins")),
            (None, some("page"), some("which row wins"))
        );
        // A code pane: its path, and its note.
        assert_eq!(
            words(None, "src/a.rs:1-9", Some("why")),
            (None, some("src/a.rs:1-9"), some("why"))
        );
    }

    #[test]
    fn without_a_proposed_change_a_line_is_its_own_row() {
        for line in 1..8 {
            assert_eq!(row_of(line, 4, 0), (line - 1) as usize);
        }
    }

    #[test]
    fn a_replacement_pushes_down_only_what_is_below_it() {
        // Lines 3..=5 are going, and two lines come in their place. Everything
        // above and including the range is where it was; everything after it
        // is two rows lower.
        let row = |line| row_of(line, 5, 2);

        assert_eq!(row(1), 0);
        assert_eq!(row(5), 4, "the last line of the range has not moved");
        assert_eq!(row(6), 7, "the line after it is past the two new ones");
        assert_eq!(row(7), 8);
    }

    #[test]
    fn rows_are_held_one_at_a_time_and_commented_on_as_a_range() {
        // The two halves of a selection, and why it is kept twice. Arriving
        // lines all answer for the same file range — they replace it, and none
        // of them is in the file — so a selection kept only in lines lit all of
        // them or none. Held by row, a reader can take three of five.
        //
        // And the comment still lands on the range they replace, because that
        // is the only place in the file a proposed line can be said to be.
        let pins = [140_u32, 148, 148, 148, 148, 149];

        // Three arriving rows out of four.
        let held = 2..=4;
        assert_eq!(held.clone().count(), 3, "three rows, distinctly");

        let lines: Vec<u32> = held.map(|ix| pins[ix]).collect();
        assert_eq!(
            (lines.iter().min(), lines.iter().max()),
            (Some(&148), Some(&148)),
            "and one line to hang the comment on"
        );
    }

    #[test]
    fn the_arriving_side_stops_at_the_end_of_the_change() {
        // The bleed that post-change numbering buys you. Five lines at 340
        // becoming six gives green rows numbered 340..=345, while the file
        // underneath still has its own 345 and 346 — so `+340-346` overlaps two
        // ordinary lines below the change as well as the six it meant.
        //
        // Being part of the change is what settles it, not the number.
        let arriving = Row {
            text: "new".into(),
            runs: Vec::new(),
            number: Some(345),
            pin: 345,
            answers: LineRange::single(345),
            change: Some(Change::New),
            inlay: Vec::new(),
            extra: None,
        };
        let unchanged = Row {
            change: None,
            ..arriving.clone()
        };
        let going = Row {
            change: Some(Change::Gone),
            ..arriving.clone()
        };

        assert!(arriving.takes(crate::prose::Side::Arriving));
        assert!(!unchanged.takes(crate::prose::Side::Arriving), "the bleed");
        assert!(!going.takes(crate::prose::Side::Arriving));

        // And a bare point is the file as it stands: what is there now, plus
        // what is being taken out of it.
        assert!(unchanged.takes(crate::prose::Side::Going));
        assert!(going.takes(crate::prose::Side::Going));
        assert!(!arriving.takes(crate::prose::Side::Going));
    }

    #[test]
    fn a_proposed_line_is_numbered_as_it_will_be() {
        // A spliced line used to carry no number, and so no address: nothing
        // could point at it, select it or comment on it, and an agent walking a
        // change could not say *and this is what it becomes*.
        //
        // It takes the number it will have once the edit lands, counted from
        // the first line of the range it replaces.
        let range = LineRange::new(968, 970);
        let arriving: Vec<u32> = (0..4).map(|offset| range.first + offset).collect();

        assert_eq!(arriving, vec![968, 969, 970, 971]);
        assert!(
            arriving.last().is_some_and(|&last| last > range.last),
            "a change that adds lines runs past the range it replaces"
        );
    }

    #[test]
    fn what_a_change_replaced_pushes_down_the_range_itself() {
        // The other direction, and the one the shared arithmetic used to get
        // wrong. With `before`, the rows go in *above* the range, so the range
        // moves down too — where an `after` leaves it exactly where it was.
        // Lines 3..=5 are what arrived, and the two lines they replaced sit
        // over them, so the splice happens after line 2.
        let row = |line| row_of(line, 2, 2);

        assert_eq!(row(1), 0);
        assert_eq!(row(2), 1, "the line above the splice has not moved");
        assert_eq!(row(3), 4, "the range itself is past the two old lines");
        assert_eq!(row(5), 6);
        assert_eq!(row(6), 7);
    }

    #[test]
    fn a_light_leaving_one_half_does_not_touch_the_other() {
        // The one the reader found by watching which half flashed. Moving from
        // the going half to a line below the change made the *arriving* half
        // blink — a half that was not in either point.
        //
        // A departing light remembered its range and not its side, so it landed
        // on every row whose number it overlapped. Both halves of a change
        // answer to the same numbers, so leaving one always lit the other.
        let row = |change| Row {
            text: "x".into(),
            runs: Vec::new(),
            number: Some(331),
            pin: 331,
            answers: LineRange::single(331),
            change,
            inlay: Vec::new(),
            extra: None,
        };

        let leaving = LineRange::new(330, 332);
        assert!(
            leaving.overlaps(row(Some(Change::New)).answers),
            "same numbers"
        );

        // And the side is what keeps them apart.
        assert!(row(Some(Change::Gone)).takes(crate::prose::Side::Going));
        assert!(!row(Some(Change::New)).takes(crate::prose::Side::Going));
        assert!(row(Some(Change::New)).takes(crate::prose::Side::Arriving));
        assert!(!row(Some(Change::Gone)).takes(crate::prose::Side::Arriving));
    }

    #[test]
    fn a_light_on_its_way_out_reverses_rather_than_restarting() {
        // Raised in review: the guards asked whether a light was *on*, which is
        // false for one still falling. Pressing escape and reselecting the same
        // sentence before the fall finished threw that light away and started
        // again, so the band dropped to nothing and climbed, instead of simply
        // coming back from where it had got to.
        let mut light = Fade::default();
        light.set(true);
        light.since = Some(std::time::Instant::now() - RISE);
        assert!((light.level() - 1.).abs() < 0.01, "all the way up");

        // Escape: on its way down, and partway there.
        light.set(false);
        light.since = Some(std::time::Instant::now() - FALL / 2);
        let falling = light.level();
        assert!((0.05..0.95).contains(&falling), "halfway out: {falling}");

        // Reselected before it finished.
        light.set(true);
        assert!(
            (light.level() - falling).abs() < 0.01,
            "comes back from {falling} rather than from nothing"
        );
    }

    #[test]
    fn a_light_the_reader_raised_arrives_quicker_than_it_leaves() {
        // The ask, as a pair of numbers: coming up should feel caused by the
        // gesture, going out should be something you watch.
        assert!(
            HAND_RISE < HAND_FALL,
            "a band that leaves as fast as it arrived reads as a glitch"
        );
        assert!(
            HAND_RISE < RISE,
            "and quicker in than the narration, which answers to a voice"
        );
        // Reported as sluggish when the sentences were on the narration pace.
        // Half a second to agree with a click reads as the window thinking
        // about it, whatever the easing does.
        assert!(HAND_RISE * 3 < RISE, "comfortably quicker, not marginally");

        let mut light = Fade::default().paced(HAND_RISE, HAND_FALL);
        light.set(true);
        // Halfway through its own rise, not through the narration's.
        light.since = Some(std::time::Instant::now() - HAND_RISE / 2);
        let halfway = light.level();
        assert!(
            (0.2..0.8).contains(&halfway),
            "eased, and already well up: {halfway}"
        );

        light.since = Some(std::time::Instant::now() - HAND_RISE);
        assert!(!light.moving(), "arrived in its own time, not in RISE");

        // And letting go takes the longer way out.
        light.set(false);
        light.since = Some(std::time::Instant::now() - HAND_RISE);
        assert!(
            light.moving(),
            "still leaving after as long as it took to come"
        );
    }

    #[test]
    fn a_line_in_both_points_never_dips() {
        // The tubelight blink, and why it only ever showed between two points
        // that share lines. A row that is arriving *and* leaving used to be
        // answered a flat 1.0 — so it snapped to full, held there while the old
        // light decayed, dropped to however far the new one had actually risen,
        // and climbed again. On, off, on.
        //
        // The brighter of the two is the whole rule: it can never dip, because
        // the outgoing one only falls once the incoming has passed it.
        let held = |rising: f32, leaving: f32| (rising + leaving).min(1.);

        // Taking the brighter of the two is the obvious answer and it dips:
        // two curves crossing peak at a half apiece in the middle.
        assert!((0.5_f32).max(0.5) < 1., "which is the blink");

        // Summed, the crossing holds steady the whole way over.
        for step in 0..=10u8 {
            let rising = f32::from(step) / 10.;
            let leaving = 1. - rising;
            assert!(
                (held(rising, leaving) - 1.).abs() < f32::EPSILON,
                "dipped at {rising}"
            );
        }

        // And neither one alone is ever pushed past full.
        assert!((held(1., 1.) - 1.).abs() < f32::EPSILON);
    }

    #[test]
    fn a_light_being_replaced_leaves_faster_than_one_that_just_ends() {
        // The shimmer, as arithmetic. A light out over 760ms while the next
        // comes up over 520 leaves both on together for a quarter of a second
        // — and three at once for a reader moving claim to claim, which is what
        // reads as the old lines flickering while the new ones land.
        //
        // Ending is not the same act as being replaced.
        assert!(
            HAND_OVER < RISE,
            "a replaced light has to be gone before its replacement is up"
        );
        assert!(FALL > RISE, "but one that simply ends still settles slowly");

        let mut handed = Fade::default();
        handed.set(true);
        handed.since = Some(std::time::Instant::now() - RISE);
        handed.hand_over();
        handed.since = Some(std::time::Instant::now() - HAND_OVER);
        assert!(!handed.moving(), "and it is finished within its own time");

        let mut ended = Fade::default();
        ended.set(true);
        ended.since = Some(std::time::Instant::now() - RISE);
        ended.set(false);
        ended.since = Some(std::time::Instant::now() - HAND_OVER);
        assert!(ended.moving(), "where the slow one is still going");
    }

    #[test]
    fn a_light_turned_back_halfway_does_not_flash() {
        // The light starts from the level it had reached. Starting from the
        // end it had been heading for would jump to full and fall from there.
        let mut light = Fade::default();
        light.set(true);
        light.since = Some(std::time::Instant::now() - RISE / 2);
        let reached = light.level();
        assert!(reached > 0.3 && reached < 0.7, "{reached}");

        light.set(false);
        assert!((light.level() - reached).abs() < 0.05, "{}", light.level());
    }

    #[test]
    fn a_settled_light_asks_for_no_frames() {
        // Frames are asked for only while a light is moving. A light that
        // reported moving for ever would repaint the window sixty times a
        // second for nothing, which was the sluggishness already paid for once.
        let mut light = Fade::default();
        assert!(!light.moving());
        light.set(true);
        assert!(light.moving());
        light.since = Some(std::time::Instant::now() - RISE);
        assert!(!light.moving());
        assert!((light.level() - 1.).abs() < f32::EPSILON);
    }

    #[test]
    fn a_live_spotlight_does_not_move_the_proposed_replacement() {
        // The insertion coordinate belongs to the authored ref. Moving a live
        // spotlight below it must not make the proposed rows jump below the
        // new subject or change where later source lines are drawn.
        let authored_last = 5;
        let _spotlight = LineRange::new(20, 22);

        assert_eq!(row_of(5, authored_last, 2), 4);
        assert_eq!(row_of(6, authored_last, 2), 7);
        assert_eq!(row_of(20, authored_last, 2), 21);
    }

    #[test]
    fn line_zero_is_treated_as_line_one() {
        // Nothing should ask for it — ranges are corrected on the way in — but
        // a subtraction that underflows here would panic in the middle of a
        // frame rather than fail a test.
        assert_eq!(row_of(0, 3, 4), 0);
    }
}
