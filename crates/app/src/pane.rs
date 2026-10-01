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
//! The highlighter is still the library's, driven directly.

use std::ops::Range;

use deck_core::line::LineRange;
use deck_core::protocol::RefSpec;
use deck_core::theme::{Palette, Rgb};
use gpui_kit::component::highlighter::SyntaxHighlighter;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette::paint;
use crate::sheet::Slot;
use crate::view::DeckView;

/// Row height, as a multiple of the monospace size.
const LINE_HEIGHT: f32 = 1.62;
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
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{across}" height="{length}" viewBox="0 0 {across} {length}"><text x="{cx}" y="{cy}" transform="rotate({turn} {cx} {cy})" font-family="{family}" font-size="{size}" font-weight="{weight}" text-anchor="middle" dominant-baseline="central" fill="#000">{text}</text></svg>"##,
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
    rows: Vec<Row>,
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

impl Pane {
    /// Build a pane for `code` out of `source`, the whole file.
    pub fn new(code: &RefSpec, source: &str, cx: &App) -> Self {
        let lines: Vec<&str> = source.lines().collect();
        let total = u32::try_from(lines.len()).unwrap_or(u32::MAX).max(1);
        let authored = code.range.clamp_to(total);
        let mut rows = highlight(source, &lines, &code.file, cx);
        let mut added = 0usize;

        // A proposed change, drawn in place: the range marked as going, and the
        // replacement spliced in under it. Every row below then sits one lower
        // per line added, which is why nothing may look a row up by its line
        // number afterwards — see `row_of`.
        let mut added_at = authored.last;
        if let Some(after) = code.after.as_deref() {
            for row in &mut rows[(authored.first as usize - 1)..(authored.last as usize)] {
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
            rows.splice((authored.last as usize)..(authored.last as usize), fresh);
        } else if let Some(before) = code.before.as_deref() {
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
            for row in &mut rows[(authored.first as usize - 1)..(authored.last as usize)] {
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
                let at = (authored.first as usize) - 1;
                rows.splice(at..at, gone);
            }
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

        let mut pane = Self {
            rows,
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
        pane.spotlight(authored);
        pane
    }

    /// Which row of the list a file line is drawn on.
    ///
    /// The two are the same until a proposed replacement is spliced in, after
    /// which every line below it sits one row lower per line added. A comment
    /// card placed by line number and a scroll target measured in rows both
    /// have to come through here, or they land somewhere the reader did not
    /// point at.
    fn row_of(&self, number: u32) -> usize {
        row_of(number, self.added_at, self.added)
    }

    /// Take hold of the rows between `from` and `to`, whichever way round.
    ///
    /// Draws by row and comments by line: the rows are kept as they are, and
    /// the file range is derived from what those rows answer for. Drag across
    /// three arriving lines and you see three lines held, and the comment lands
    /// on the range they replace — which is the only place it could land.
    pub fn select_rows(&mut self, from: usize, to: usize) {
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
                focus_button,
                // A borrowed pane offers no fold, only a close. Folding one
                // would leave a spine for something that is not part of the
                // deck, and when it is the only thing open the fold has to be
                // refused anyway — which reads as the way back being barred.
                fold: (fold <= 0. && slot.foldable && !slot.temporary)
                    .then(|| self.render_fold(slot, cx)),
                close: (fold <= 0. && slot.temporary).then(|| self.render_close(slot)),
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
                    .child(self.render_code(slot.ix, marks, palette, slot.view, cx)),
            )
            // The pane being talked about, outlined. Drawn over the pane rather
            // than as its border, so lighting it moves nothing: a border that
            // appeared would push every row down by its width, mid-sentence.
            // It has no handlers, so nothing under it stops being clickable.
            .when(heeded > 0., |pane| {
                pane.child(
                    div()
                        .absolute()
                        .inset_0()
                        .border_2()
                        .border_color(paint(palette.wash.mix(palette.accent, 0.9 * heeded))),
                )
            })
    }

    fn render_code(
        &self,
        pane_ix: usize,
        marks: &[Mark],
        palette: &Palette,
        view: &WeakEntity<DeckView>,
        cx: &App,
    ) -> impl IntoElement {
        let row_px = px(f32::from(cx.theme().mono_font_size) * LINE_HEIGHT);
        let mono = cx.theme().mono_font_family.clone();
        let size = cx.theme().mono_font_size;

        // Everything a row needs is copied in: the closure outlives this call
        // and so cannot borrow the pane.
        let rows = self.rows.clone();
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
        let gutter_fg = palette.fg.mix(palette.wash, 0.6);
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
        let across = self.across.clone();

        let list = uniform_list("deck-code", rows.len(), move |visible, _window, _cx| {
            visible
                .map(|ix| {
                    let row = rows[ix].clone();
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

                    div()
                        .id(("row", ix))
                        .h(row_px)
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
                            move |event: &MouseDownEvent, _window, cx| {
                                let extend = event.modifiers.shift;
                                let _ = view.update(cx, |deck, cx| {
                                    deck.start_pick(pane_ix, ix, extend, cx);
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
                        .bg(paint({
                            // What the row *is*: going, arriving, lit, or plain.
                            let ground = match row.change {
                                Some(Change::Gone) => flash.mix(palette.gone, arrived),
                                Some(Change::New) => flash.mix(palette.fresh, arrived),
                                None if is_lit => palette.focus,
                                None => palette.wash,
                            };
                            // Then what is being *said* about it, over the top.
                            //
                            // A pointed line deepens into more of its own
                            // colour rather than taking the accent. Washing
                            // orange over the arriving green gave a muddy brown
                            // and took away the one thing the row was saying —
                            // that it is the half arriving. Red goes to a
                            // darker red, green to a darker green, and only
                            // ordinary paper takes the accent, where there is
                            // no meaning of its own to drown.
                            let ground = match row.change {
                                Some(Change::Gone) => ground.mix(palette.gone_deep, pointed),
                                Some(Change::New) => ground.mix(palette.fresh_deep, pointed),
                                None => ground.mix(palette.accent, 0.26 * pointed),
                            };
                            ground.mix(palette.accent, 0.14 * holding)
                        }))
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
                                .pr(px(18.))
                                .h_flex()
                                .items_center()
                                .gap_3()
                                .overflow_hidden()
                                .child(StyledText::new(row.text).with_highlights(row.runs)),
                        )
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
        let palette = slot.palette;
        let mono = cx.theme().mono_font_family.clone();
        let ix = slot.ix;
        let fold = slot.fold;
        let left = slot.fold_left;
        let view = slot.view.clone();
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
                            .child(sideways(&title, &mono, 12.5, 700, palette.accent, left)),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .flex_none()
                            .flex()
                            .justify_center()
                            .child(sideways(&file, &mono, 11.5, 500, palette.fg, left)),
                    )
                    // The spine itself: a line down the rest of the pane, which is what
                    // the word means and all the room there is for it.
                    .child(div().mt(px(12.)).w(px(1.)).flex_1().bg(paint(palette.edge))),
            )
            .into_any_element()
    }

    /// The control that closes a pane brought in to answer a question.
    fn render_close(&self, slot: &Slot) -> AnyElement {
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

    /// The control that folds this pane away.
    fn render_fold(&self, slot: &Slot, _cx: &App) -> AnyElement {
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
                let y = (self.row_of(mark.range.last) + 1) as f32 * row - top;

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
        })
        .collect();

    let mut highlighter = SyntaxHighlighter::new(language_of(file));
    highlighter.update(None, &Rope::from(source), None);
    let styles = highlighter.styles(&(0..source.len()), &*cx.theme().highlight_theme);

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
        focus_button,
        fold,
        close,
    } = controls;
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
                        .children(name.map(|name| {
                            div()
                                .flex_none()
                                .text_color(paint(palette.accent))
                                .child(name)
                        }))
                        .child(div().min_w_0().truncate().child(label)),
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
                .children(focus_button)
                .children(fold)
                .children(close),
        )
}

/// What a pane's header offers, beside its name.
#[derive(Default)]
pub struct Controls {
    /// The way back to the lit range, when it has been scrolled away from.
    pub focus_button: Option<AnyElement>,
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

/// The highlighter's name for the language in `file`.
///
/// A file extension is not a language name — the highlighter wants `rust`, and
/// the file is called `.rs`.
fn language_of(file: &std::path::Path) -> &'static str {
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        Some("ts" | "mts" | "cts") => "typescript",
        Some("tsx") => "tsx",
        Some("js" | "mjs" | "cjs") => "javascript",
        // Not "jsx", which is nobody's grammar. The highlighter answers for a
        // name it does not know with `Plain` rather than an error, so every
        // `.jsx` file came up as unpainted text and nothing anywhere said why.
        // Tsx is the grammar that parses JSX tags; plain javascript does not.
        Some("jsx") => "tsx",
        Some("py") => "python",
        Some("go") => "go",
        Some("rb") => "ruby",
        Some("lua") => "lua",
        Some("md" | "markdown") => "markdown",
        Some("yml" | "yaml") => "yaml",
        Some("sh" | "bash" | "zsh") => "bash",
        Some("json") => "json",
        Some("toml") => "toml",
        Some("html") => "html",
        Some("css") => "css",
        Some("c" | "h") => "c",
        Some("cpp" | "cc" | "hpp") => "cpp",
        Some("java") => "java",
        _ => "text",
    }
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this module glob-imports GPUI, which
    // exports a `test` attribute of its own and would otherwise shadow the
    // standard one.
    use core::prelude::v1::test;

    use super::*;

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
    fn every_language_we_name_is_one_the_highlighter_knows() {
        // The failure this catches is silent by construction: an unknown name
        // comes back as `Plain`, so the file renders, reads fine, and is simply
        // never painted. `.jsx` did that from the first release to this test.
        use gpui_kit::component::highlighter::Language;

        for file in [
            "a.rs",
            "a.ts",
            "a.mts",
            "a.cts",
            "a.tsx",
            "a.js",
            "a.mjs",
            "a.cjs",
            "a.jsx",
            "a.py",
            "a.go",
            "a.rb",
            "a.lua",
            "a.md",
            "a.markdown",
            "a.yml",
            "a.yaml",
            "a.sh",
            "a.bash",
            "a.zsh",
            "a.json",
            "a.toml",
            "a.html",
            "a.css",
            "a.c",
            "a.h",
            "a.cpp",
            "a.cc",
            "a.hpp",
            "a.java",
        ] {
            let named = language_of(std::path::Path::new(file));
            assert_ne!(
                Language::from_str(named),
                Language::Plain,
                "{file} is called `{named}`, which the highlighter does not know"
            );
        }

        // And the fallback still is what it says it is.
        assert_eq!(language_of(std::path::Path::new("a.unknown")), "text");
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
