//! The window: a narration band, and the panes that show what it is talking
//! about.
//!
//! The spotlight is the whole visual idea, and it is one rule: **the paper
//! moves, not the ink.** Code is never recoloured to dim it. The pane's ground
//! is washed back and the lit range is painted with the page's own brightness,
//! so the range reads as un-dimmed rather than highlighted.
//!
//! Which means the highlight is a background and nothing else. Syntax colours
//! are left exactly as the highlighter found them.

use deck_core::layout::{Arrange, Layout as GridSpec};
use deck_core::protocol::{Group, Ref};
use deck_core::theme::Palette;
use deck_core::{LineRange, Relocated};
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::chart::Chart;
use crate::load::{Deck, read_source};
use crate::palette::{current, paint};
use crate::pane::{Away, Mark, Pane};
use crate::sheet::{Sheet, Slot};

gpui_kit::actions!(
    deck,
    [
        NextGroup, PrevGroup, Comment, Rotate, Hide, Submit, Discard, ZoomIn, ZoomOut, ZoomReset,
        Close
    ]
);

/// The keys, and what the legend says about them.
///
/// One list, so a binding and its legend entry cannot drift apart. The legend
/// is built from this at render time rather than written out beside it.
const KEYS: &[(&str, &str, &str)] = &[
    ("n", "next", "next group"),
    ("p", "prev", "previous group"),
    ("c", "comment", "comment"),
    ("r", "rotate", "turn the panes"),
    ("h", "hide", "put it away"),
    ("s", "submit", "submit"),
    ("q", "close", "close"),
];

/// The bindings, in the context the window claims.
#[must_use]
pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("n", NextGroup, Some("Deck")),
        KeyBinding::new("p", PrevGroup, Some("Deck")),
        KeyBinding::new("c", Comment, Some("Deck")),
        KeyBinding::new("r", Rotate, Some("Deck")),
        KeyBinding::new("h", Hide, Some("Deck")),
        KeyBinding::new("s", Submit, Some("Deck")),
        KeyBinding::new("q", Close, Some("Deck")),
        KeyBinding::new("cmd-q", Close, None),
        // Zoom is bound window-wide, not to the deck's context: needing the
        // text bigger is not a thing that should stop working because a
        // composer has the keyboard.
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-0", ZoomReset, None),
        // Escape reaches here because the input's own Escape handler ends in
        // `cx.propagate()`. Saving does not: the input binds `secondary-enter`
        // in a context deeper than this one and handles it itself, so ⌘⏎ is
        // picked up from the event it emits instead. See `open_composer`.
        KeyBinding::new("escape", Discard, Some("DeckComposer")),
    ]
}

/// Roughly how many characters of code fit across the window.
///
/// `min_pane_width` is a count of columns, because that is the unit a person
/// means by "too narrow to read code in". Pixels are divided down by an
/// approximate character width — the answer only decides how many panes stand
/// beside each other, so being a character or two out costs nothing.
fn columns_across(window: &Window) -> u32 {
    const CHAR_PX: f32 = 8.0;
    let px = f32::from(window.viewport_size().width).max(0.0);
    // Saturating rather than a bare `as`: that truncates toward zero on a NaN
    // width and would silently claim the window is no columns wide.
    #[allow(clippy::cast_possible_truncation)]
    let columns = (px / CHAR_PX) as i64;
    u32::try_from(columns).unwrap_or(0)
}

/// What a quarter turn of the page means.
///
/// The arrangement, and whether the panes read backwards. Four turns, the way
/// a sheet of paper has four: side by side, one above the other, side by side
/// the other way round, one above the other the other way round.
fn quarter(turn: u8) -> (Arrange, bool) {
    let turn = turn % 4;
    let arrange = if turn.is_multiple_of(2) {
        Arrange::Columns
    } else {
        Arrange::Stacked
    };
    (arrange, turn >= 2)
}

/// Follow every pin to where its line is now.
///
/// Free of the view so the bookkeeping can be checked: pins are grouped by
/// file, followed one file at a time because the diff is per file, and then
/// married back to the remark each came from. Getting that pairing wrong would
/// put one reader's comment on another one's line, which is the worst thing
/// this program could do quietly.
///
/// A file with no snapshot is skipped rather than guessed at — its remark keeps
/// the range it was written with and says nothing about how far to trust it,
/// which is the honest answer to a question nobody can answer.
fn follow(
    pins: &[(usize, &std::path::Path, LineRange)],
    snapshots: &std::collections::HashMap<std::path::PathBuf, std::sync::Arc<str>>,
    read: impl Fn(&std::path::Path) -> String,
) -> std::collections::HashMap<usize, Relocated> {
    use std::collections::HashMap;

    let mut by_file: HashMap<&std::path::Path, Vec<(usize, LineRange)>> = HashMap::new();
    for (ix, file, range) in pins {
        by_file.entry(file).or_default().push((*ix, *range));
    }

    let mut found = HashMap::new();
    for (file, pins) in by_file {
        let Some(snapshot) = snapshots.get(file) else {
            continue;
        };
        let current = read(file);
        let ranges: Vec<LineRange> = pins.iter().map(|(_, range)| *range).collect();

        for ((ix, _), moved) in pins
            .iter()
            .zip(deck_core::relocate(snapshot, &current, &ranges))
        {
            found.insert(*ix, moved);
        }
    }
    found
}

/// A seam's rule while the pointer is on it, or has hold of it.
///
/// Generic over what it is styling because it is asked for twice: once on the
/// element outright, and once inside a hover rule, which is handed a style
/// rather than an element.
fn lit<S: Styled>(rule: S, sideways: bool, accent: Hsla) -> S {
    if sideways {
        rule.w(px(3.)).ml(px(-1.)).bg(accent)
    } else {
        rule.h(px(3.)).mt(px(-1.)).bg(accent)
    }
}

/// Which seam is being dragged.
#[derive(Clone, Copy, PartialEq)]
enum Divide {
    /// Between the narration and the code.
    Band,
    /// Between the pane row at this index and the one below it.
    Panes(usize),
    /// Between the pane at this index and the one to its right.
    ///
    /// Named by pane rather than by column, because the panes either side of
    /// it are the two that trade width and they are what the drag has to
    /// reach. Only ever drawn between two panes of the same row.
    Columns(usize),
}

/// The band's height before anyone drags it — enough for a title and a few
/// lines of prose, which is what most groups carry.
const BAND_NATURAL: f32 = 168.;

/// What a remark is about.
///
/// Either a line of code, or the group's claim itself. The second is what a
/// reader reaches for when the objection is to the argument rather than to any
/// particular line, and it is the remark most worth carrying back.
#[derive(Clone, PartialEq)]
enum About {
    /// Lines of one pane's file. Carries the pane, so the composer can name it.
    Lines { pane: usize, range: LineRange },
    /// A diagram, and whichever node of it is picked.
    ///
    /// No range, because a picture has no lines. What the remark is pinned to
    /// is the node's own label, which travels back in the comment's `quote`.
    Drawn { pane: usize },
    /// The narration: what the group claims.
    Claim,
}

/// A remark the reader has written, before it goes back.
#[derive(Clone)]
struct Remark {
    group: SharedString,
    ref_id: Option<SharedString>,
    file: Option<std::path::PathBuf>,
    range: Option<LineRange>,
    /// The text the remark was pinned to, so the far side can find it again if
    /// the file has moved underneath.
    quote: String,
    text: String,
}

/// Everything the deck is holding, so it survives being put away.
///
/// Hiding the window has to be free — a reader who has written four remarks
/// and then wants their editor back cannot be made to choose between the two.
/// So the window hands this to the bar on the way out and takes it back on the
/// way in, and nothing about what comes back says it has been anywhere.
///
/// Layout comes with it for the same reason. Turning the page and dragging a
/// seam are decisions the reader made about *this* deck; putting the window
/// away is not a reason to make them again.
pub struct Session {
    /// The deck, and whatever has landed of it so far.
    pub deck: Deck,
    /// How panes are arranged, as the config asked.
    layout: GridSpec,
    /// Every file this deck has shown, as it was the first time it was seen.
    ///
    /// The session's, not a pane's. A remark may be about a file whose pane is
    /// two groups back and no longer built, and it is still a remark about the
    /// text the reader was looking at. Taken once and never replaced: the whole
    /// point is that it does not follow the file.
    snapshots: std::collections::HashMap<std::path::PathBuf, std::sync::Arc<str>>,
    remarks: Vec<Remark>,
    group_ix: usize,
    turn: Option<u8>,
    band_height: Option<f32>,
    shares: Vec<f32>,
    widths: Vec<f32>,
    folded: std::collections::HashSet<usize>,
}

impl Session {
    /// A deck nobody has read yet.
    #[must_use]
    pub fn fresh(deck: Deck) -> Self {
        Self {
            deck,
            layout: GridSpec::default(),
            snapshots: std::collections::HashMap::new(),
            remarks: Vec::new(),
            group_ix: 0,
            // Whatever the reader last turned a page to. `None` only for
            // somebody who has never turned one, and then the page arranges
            // itself.
            turn: crate::state::remembered_turn(),
            band_height: None,
            shares: Vec::new(),
            widths: Vec::new(),
            folded: std::collections::HashSet::new(),
        }
    }

    /// Arrange this deck's panes the way the reader asked.
    #[must_use]
    pub fn arranged(mut self, layout: GridSpec) -> Self {
        self.layout = layout;
        self
    }

    /// How many remarks have been written so far.
    #[must_use]
    pub fn said(&self) -> usize {
        self.remarks.len()
    }
}

/// The window.
pub struct DeckView {
    deck: Deck,
    /// See [`Session::snapshots`].
    snapshots: std::collections::HashMap<std::path::PathBuf, std::sync::Arc<str>>,
    palette: Palette,
    grid: GridSpec,
    /// Panes for the group being read. Rebuilt when the group changes, which is
    /// the only time the content is genuinely new.
    panes: Vec<Sheet>,
    group_ix: usize,
    focus: FocusHandle,
    /// Everything written so far, across every group.
    remarks: Vec<Remark>,
    /// The composer, while one is open, and what it is about.
    ///
    /// The subscription rides along so it lives exactly as long as the composer
    /// does: dropping it is what stops the old textarea being listened to.
    composing: Option<(About, Entity<TextareaState>, Subscription)>,
    /// Whether the narration is the thing a comment would land on.
    /// Which sentence of the narration is picked, if any.
    ///
    /// A comment on the claim used to quote the first line of the say whatever
    /// the reader had in mind, so an objection to the third sentence came back
    /// answering the first. The sentence they clicked is the one they meant.
    picked_said: Option<(usize, usize)>,
    /// The word a narration drag started on.
    said_from: Option<usize>,
    /// The word the pointer is over, so a drag has somewhere to reach.
    said_over: Option<usize>,
    /// Whether the pointer moved between going down and coming up.
    ///
    /// A click is not a drag. Selecting the single word somebody clicked would
    /// be a selection they did not ask for, so a click with no movement takes
    /// the whole sentence instead.
    said_dragged: bool,
    /// Where a drag began: the pane, and the line the pointer went down on.
    ///
    /// The selection is always measured from here, never grown from wherever
    /// it happens to be. Growing was wrong in a way that only shows up when
    /// you come back: drag 10 to 15, move back to 14, and 15 stays selected
    /// because a range that only ever widens has no way to let go.
    drag_from: Option<(usize, u32)>,
    /// Whether the waiting panel has opened out to two ghosts.
    ///
    /// It starts as one and spreads after a beat. Not for the sake of the
    /// animation: a deck holds files *and* pictures, and showing both at once
    /// on the first frame is a diagram of the product rather than a window
    /// filling up. One, then the other, is how a deck actually arrives.
    spread: bool,
    /// The beat before it does.
    spreading: Task<()>,
    /// The loop watching the deck directory fill, while there is more coming.
    ///
    /// Dropping it stops it, which is what should happen when the window
    /// closes — a deck that went on polling a directory nobody is reading
    /// would be the daemon this whole design exists to avoid.
    tailing: Task<()>,
    /// The pane the wheel is carrying, and where it is carrying it to.
    ///
    /// `None` when nothing is moving. Holding the target rather than the
    /// remaining distance is what lets a wheel event that arrives mid-travel
    /// simply move it: the loop following it does not need to be told.
    drifting: Option<(usize, f32)>,
    /// The animation carrying a pane back to its range, while one is running.
    /// Dropping it stops it, which is what should happen if another starts.
    gliding: Task<()>,
    /// How each row's width is shared between the panes standing in it.
    ///
    /// One entry per pane rather than one per column: a seam moves width
    /// between the two panes it sits between, and those are what the reader
    /// has hold of.
    widths: Vec<f32>,
    /// A quarter turn of the page, if the reader has asked for one.
    ///
    /// Four positions, the way turning a sheet of paper has four: side by
    /// side, one above the other, side by side the other way round, one above
    /// the other the other way round. Which is to say an axis and an order,
    /// but nobody thinks of it that way while they are pressing the key — they
    /// think of it as turning the page until it looks right.
    ///
    /// `None` while the page is arranging itself, which it does well enough
    /// that most decks never need this. The first press takes over from
    /// whatever the page had chosen, and it is a preference from then on.
    turn: Option<u8>,
    /// The quarter turn the last frame actually drew.
    ///
    /// The automatic answer depends on the window's width, so the only place
    /// it is known is inside a frame. Recorded so a press of `r` can advance
    /// from what is on screen rather than from nothing.
    turn_now: u8,
    /// How many panes stood beside each other on the last frame.
    ///
    /// A seam has to know which panes share its row before it can move width
    /// between them, and that answer comes from the window's width — which
    /// only a frame knows. Recorded on the way past.
    cols: usize,
    /// The diagram being dragged, where the pointer took hold, and where the
    /// drawing was scrolled to when it did.
    ///
    /// A picture bigger than its pane has to be reachable, and a wheel is a
    /// poor way to travel in two directions at once. Measured from where the
    /// drag began rather than accumulated, for the same reason a line
    /// selection is: a position that is only ever added to cannot go back.
    panning: Option<(usize, Point<Pixels>, Point<Pixels>)>,
    /// How the page's height is shared between its panes.
    ///
    /// One share each, so two panes start even. A drag moves height from one
    /// neighbour to the other and leaves every other pane alone, which is what
    /// makes a divider feel like it belongs to the two panes it sits between
    /// rather than to the page.
    shares: Vec<f32>,
    /// The divider being dragged, and where the pointer was when it started.
    sizing: Option<(Divide, Pixels)>,
    /// How tall the narration band is, in pixels. `None` until it is dragged,
    /// so an untouched band is whatever its prose needs.
    band_height: Option<f32>,
    /// Remarks folded down to their header, by index.
    ///
    /// A card sits over the code, so a long one hides the lines under it. The
    /// reader needs to put it away without throwing it away — which is a
    /// different act from deleting it, and so a different control.
    folded: std::collections::HashSet<usize>,
    /// The row the pointer is over, if any.
    ///
    /// Hover is the only thing that knows *which* row — a mouse-move handler
    /// is not bounds-checked and fires for every row at once. But hover cannot
    /// tell whether a button is held, and a flag that tried to remember dragged
    /// or not kept sticking on, so afterwards merely crossing the code
    /// rewrote the selection. So hover records the row and nothing else, and
    /// whether this is a drag is read from the move event itself, which cannot
    /// be stale.
    hovered: Option<(usize, u32)>,
}

impl DeckView {
    /// Open a deck, where it was left — which for a deck nobody has opened
    /// yet is the beginning of it.
    pub fn resume(session: Session, cx: &mut Context<Self>) -> Self {
        let dark = cx.theme().mode.is_dark();
        let palette = current(dark);
        // Before any pane is built: the editors read their ground and their
        // syntax colours from the global, so it has to be ours by then.
        crate::palette::install(&palette, dark, cx);
        let Session {
            deck,
            layout,
            snapshots,
            remarks,
            group_ix,
            turn,
            band_height,
            shares,
            widths,
            folded,
        } = session;

        let mut view = Self {
            deck,
            snapshots,
            palette,
            grid: layout,
            panes: Vec::new(),
            group_ix,
            focus: cx.focus_handle(),
            remarks,
            composing: None,
            picked_said: None,
            said_from: None,
            said_over: None,
            said_dragged: false,
            drag_from: None,
            hovered: None,
            shares,
            widths,
            cols: 1,
            turn,
            turn_now: turn.unwrap_or(0),
            panning: None,
            sizing: None,
            band_height,
            folded,
            drifting: None,
            gliding: Task::ready(()),
            tailing: Task::ready(()),
            spread: false,
            spreading: Task::ready(()),
        };
        view.build_panes(cx);
        view.tail(cx);
        view.spread_later(cx);
        view
    }

    /// Open the waiting panel out, a beat after it appears.
    ///
    /// Only while there is nothing to read: a deck that arrived whole never
    /// shows the panel, and should not be running a timer about it.
    fn spread_later(&mut self, cx: &mut Context<Self>) {
        const BEAT: std::time::Duration = std::time::Duration::from_millis(2000);

        if !self.deck.groups().is_empty() {
            return;
        }
        self.spreading = cx.spawn(async move |view, cx| {
            cx.background_executor().timer(BEAT).await;
            let _ = view.update(cx, |deck, cx| {
                deck.spread = true;
                cx.notify();
            });
        });
    }

    /// Watch the deck for the groups that have not been written yet.
    ///
    /// Polled rather than subscribed to. A file-event crate would do this too,
    /// and would cost a dependency, a channel into the window's own scheduler,
    /// and a class of platform bug that is hard to reproduce; what is being
    /// watched is one small directory being written by a process this window
    /// was started by. Four times a second is under the threshold at which a
    /// group appears to arrive late, and a listing of a dozen names costs
    /// nothing.
    ///
    /// It stops the moment the deck is sealed, so a finished deck — which is
    /// most of them, most of the time — polls nothing at all.
    fn tail(&mut self, cx: &mut Context<Self>) {
        const EVERY: std::time::Duration = std::time::Duration::from_millis(250);

        if self.deck.sealed() {
            return;
        }

        self.tailing = cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(EVERY).await;

                let more = view.update(cx, |deck, cx| {
                    // Panes belong to the group being read, and a group
                    // landing behind it changes nothing on screen but the
                    // count. The one exception is the first group of all:
                    // until it arrives there is nothing to have built, and the
                    // reader is standing on a group that does not exist yet.
                    let nothing_to_read = deck.deck.groups().is_empty();
                    if deck.deck.refresh() {
                        if nothing_to_read {
                            deck.build_panes(cx);
                        }
                        cx.notify();
                    }
                    !deck.deck.sealed()
                });

                if !matches!(more, Ok(true)) {
                    return;
                }
            }
        });
    }

    fn group(&self) -> Option<&Group> {
        self.deck.groups().get(self.group_ix)
    }

    /// Walk to the group `step` away, if there is one.
    ///
    /// Walking off either end does nothing rather than wrapping: a deck is a
    /// story, and arriving back at the beginning by pressing `n` reads as
    /// having lost your place.
    fn walk(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(next) = self.group_ix.checked_add_signed(step) else {
            return;
        };
        if next >= self.deck.groups().len() {
            return;
        }
        self.group_ix = next;
        self.build_panes(cx);
        cx.notify();
    }

    fn on_next(&mut self, _: &NextGroup, _window: &mut Window, cx: &mut Context<Self>) {
        self.walk(1, cx);
    }

    fn on_prev(&mut self, _: &PrevGroup, _window: &mut Window, cx: &mut Context<Self>) {
        self.walk(-1, cx);
    }

    /// Close this deck without answering it.
    ///
    /// The deck is dropped rather than put back: `q` is the reader saying they
    /// are done with it, and a deck that reappeared on the bar after being
    /// closed would be impossible to get rid of. Putting one away for later is
    /// `h`, and that is a different key for a different thing.
    fn on_close(&mut self, _: &Close, window: &mut Window, cx: &mut Context<Self>) {
        // The platform's should-close hook does not run when the window is
        // taken away from inside, so the shape is written here.
        if let WindowBounds::Windowed(bounds) = window.window_bounds() {
            crate::state::remember_bounds(bounds);
        }
        self.stand_down(window, cx);
    }

    /// Make the code bigger or smaller.
    ///
    /// Every measurement in a pane — the row height, the gutter, where the lit
    /// range sits — is computed from the monospace size, so moving that one
    /// number moves the whole pane in step. The prose scales with it, since
    /// somebody who needs larger code needs larger prose too.
    fn zoom(&mut self, by: f32, cx: &mut Context<Self>) {
        let theme = Theme::global_mut(cx);
        theme.mono_font_size = px((f32::from(theme.mono_font_size) + by).clamp(9., 28.));
        theme.font_size = px((f32::from(theme.font_size) + by).clamp(11., 34.));
        Theme::sync_base(cx);
    }

    fn on_zoom_in(&mut self, _: &ZoomIn, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom(1., cx);
        self.keep_place(window, cx);
    }

    fn on_zoom_out(&mut self, _: &ZoomOut, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom(-1., cx);
        self.keep_place(window, cx);
    }

    /// Give the keyboard back and put every pane back on its range.
    ///
    /// Resizing the text changes every row height, so the scroll position that
    /// was showing the lit range now shows something else. The reader did not
    /// ask to be moved, only to be able to read.
    fn keep_place(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composing.is_none() {
            self.focus.focus(window, cx);
        }
        for pane in &self.panes {
            pane.show_range();
        }
        cx.notify();
    }

    fn on_zoom_reset(&mut self, _: &ZoomReset, window: &mut Window, cx: &mut Context<Self>) {
        let theme = Theme::global_mut(cx);
        theme.mono_font_size = px(12.);
        theme.font_size = px(15.);
        Theme::sync_base(cx);
        self.keep_place(window, cx);
    }

    /// Pick the line a comment would land on.
    ///
    /// Called from a row's own click handler rather than through an action,
    /// because it carries where it was clicked — which must not depend on
    /// whichever pane happens to hold the keyboard.
    /// The pointer went down on a row: start a selection, or extend one.
    ///
    /// Shift keeps the anchor where it was, so shift-clicking reaches from the
    /// line the selection started on rather than from its nearest edge.
    pub fn start_pick(&mut self, pane_ix: usize, line: u32, extend: bool, cx: &mut Context<Self>) {
        let anchor = match self.drag_from {
            Some((had_pane, had_line)) if extend && had_pane == pane_ix => had_line,
            _ => line,
        };
        self.drag_from = Some((pane_ix, anchor));
        self.pick(pane_ix, line, cx);
    }

    /// Remember which row the pointer is over. Selects nothing by itself.
    pub fn hover_row(&mut self, pane_ix: usize, line: u32, entered: bool) {
        if entered {
            self.hovered = Some((pane_ix, line));
        } else if self.hovered == Some((pane_ix, line)) {
            self.hovered = None;
        }
    }

    /// Extend the selection to the hovered row, while a button is held.
    fn drag_to_hovered(&mut self, cx: &mut Context<Self>) {
        if let Some((pane_ix, line)) = self.hovered
            && self.drag_from.is_some_and(|(pane, _)| pane == pane_ix)
        {
            self.pick(pane_ix, line, cx);
        }
    }

    /// Select from the drag's anchor to `line`.
    pub fn pick(&mut self, pane_ix: usize, line: u32, cx: &mut Context<Self>) {
        self.picked_said = None;
        self.said_from = None;
        let anchor = match self.drag_from {
            Some((pane, from)) if pane == pane_ix => from,
            _ => line,
        };
        for (ix, pane) in self.panes.iter_mut().enumerate() {
            match pane {
                Sheet::Code(code) => {
                    code.selected =
                        (ix == pane_ix).then(|| LineRange::new(anchor.min(line), anchor.max(line)));
                }
                // One selection in the window, whatever kind of pane it is in.
                Sheet::Drawn(chart) => chart.selected = None,
            }
        }
        cx.notify();
    }

    /// Pick a node of a diagram, or unpick it if it was already picked.
    ///
    /// A diagram has no lines, so this is the only way to say *which part* of a
    /// picture a remark is about. Clicking the same node again lets go, which
    /// is what turns a remark about one box back into a remark about the whole
    /// drawing.
    pub fn pick_node(&mut self, pane_ix: usize, node_ix: usize, cx: &mut Context<Self>) {
        let already = self
            .panes
            .get(pane_ix)
            .and_then(Sheet::chart)
            .is_some_and(|chart| chart.selected == Some(node_ix));

        self.picked_said = None;
        // A click on a node is not the start of a line drag, and leaving the
        // anchor behind would make the next move over some code extend a
        // selection the reader never began.
        self.drag_from = None;

        for (ix, pane) in self.panes.iter_mut().enumerate() {
            match pane {
                Sheet::Code(code) => code.selected = None,
                Sheet::Drawn(chart) => {
                    chart.selected = (ix == pane_ix && !already).then_some(node_ix);
                }
            }
        }
        cx.notify();
    }

    /// What a remark on pane `ix` would be about.
    fn about(&self, ix: usize) -> About {
        match self.panes.get(ix) {
            Some(Sheet::Code(code)) => About::Lines {
                pane: ix,
                range: code.comment_range(),
            },
            Some(Sheet::Drawn(_)) => About::Drawn { pane: ix },
            None => About::Claim,
        }
    }

    /// The pointer went down on a word of the narration.
    fn start_say_pick(&mut self, at: usize, cx: &mut Context<Self>) {
        self.said_from = Some(at);
        self.said_dragged = false;
        self.picked_said = Some((at, at));
        for pane in &mut self.panes {
            pane.unpick();
        }
        cx.notify();
    }

    /// Remember which word the pointer is over. Selects nothing by itself.
    fn hover_say(&mut self, at: usize, entered: bool) {
        if entered {
            self.said_over = Some(at);
        } else if self.said_over == Some(at) {
            self.said_over = None;
        }
    }

    /// Extend the narration selection to the hovered word, while held.
    fn drag_say_to_hovered(&mut self, cx: &mut Context<Self>) {
        if let (Some(from), Some(over)) = (self.said_from, self.said_over)
            && self.picked_said != Some((from, over))
        {
            self.said_dragged |= over != from;
            self.picked_said = Some((from, over));
            cx.notify();
        }
    }

    /// The pointer came up. A click that never moved takes the sentence.
    fn end_say_pick(&mut self, cx: &mut Context<Self>) {
        if self.said_from.is_none() {
            return;
        }
        if !self.said_dragged
            && let Some(group) = self.group()
            && let Some(found) = self
                .picked_said
                .and_then(|(at, _)| crate::prose::sentence_around(&group.say, at))
        {
            self.picked_said = Some(found);
            cx.notify();
        }
        self.said_from = None;
    }

    /// Open the composer on whatever is picked.
    ///
    /// With nothing picked it falls to the lit range, since the common case is
    /// a remark about the thing the deck is already pointing at.
    /// Turn the page a quarter.
    ///
    /// Deliberately not "swap" or "flip". Either of those is two controls, and
    /// the reader would have to know which one they wanted before pressing
    /// anything. One key that always does the same thing gets to all four
    /// arrangements in at most three presses, and the way back is to keep
    /// going.
    fn on_rotate(&mut self, _: &Rotate, _window: &mut Window, cx: &mut Context<Self>) {
        let turn = (self.turn_now + 1) % 4;
        self.turn = Some(turn);
        // Kept, because it is a preference about how the reader likes to read
        // rather than about this deck. A deck is opened by an agent, so the
        // reader never gets to arrange the window before it appears; arriving
        // the way they last left it is the only way it arrives right.
        crate::state::remember_turn(turn);
        cx.notify();
    }

    fn on_comment(&mut self, _: &Comment, window: &mut Window, cx: &mut Context<Self>) {
        if self.composing.is_some() {
            return;
        }
        let about = if self.picked_said.is_some() || self.panes.is_empty() {
            About::Claim
        } else {
            let pane = self.panes.iter().position(Sheet::is_picked).unwrap_or(0);
            self.about(pane)
        };
        self.open_composer(about, window, cx);
    }

    fn open_composer(&mut self, about: About, window: &mut Window, cx: &mut Context<Self>) {
        let asking = match about {
            About::Lines { .. } => "what you want to say about these lines",
            About::Drawn { .. } => "what you want to say about this",
            About::Claim => "what you want to say about this group",
        };
        let state = cx.new(|cx| TextareaState::new(window, cx).placeholder(asking));
        state.update(cx, |state, cx| state.focus(window, cx));

        // Saving arrives as an event from the textarea, not as a key binding.
        //
        // A binding never fired: the input handles Enter itself and reports it
        // as `PressEnter`, with `secondary` set when cmd was held. Anything
        // bound over the top of that is bound to a key the input has already
        // taken. `shift` still inserts a newline, which is what makes a long
        // remark possible.
        let listen = cx.subscribe_in(&state, window, |deck, _, event: &InputEvent, window, cx| {
            if matches!(
                event,
                InputEvent::PressEnter {
                    secondary: true,
                    ..
                }
            ) {
                deck.save_remark(window, cx);
            }
        });

        self.composing = Some((about, state, listen));
        cx.notify();
    }

    fn on_discard(&mut self, _: &Discard, window: &mut Window, cx: &mut Context<Self>) {
        self.composing = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// `Submit` means two different things depending on what has the keyboard,
    /// which is why the composer claims its own key context: inside it, save
    /// the remark; outside it, send the review back.
    fn on_submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        if self.composing.is_some() {
            self.save_remark(window, cx);
        } else {
            self.submit_review(window, cx);
        }
    }

    fn save_remark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((about, state, _listen)) = self.composing.take() else {
            return;
        };
        let said = state.read(cx).value().trim().to_string();
        self.focus.focus(window, cx);

        if said.is_empty() {
            cx.notify();
            return;
        }
        let Some(group) = self.group() else { return };
        let group_id: SharedString = group.id.clone().into();
        let claim = group.say.clone();

        let remark = match &about {
            About::Drawn { pane } => {
                let Some(chart) = self.panes.get(*pane).and_then(Sheet::chart) else {
                    return;
                };
                Remark {
                    group: group_id,
                    ref_id: Some(chart.ref_id.clone()),
                    // A diagram is in no file, so there is nothing to relocate
                    // and nothing to point an editor at.
                    file: None,
                    range: None,
                    quote: chart.quote(),
                    text: said,
                }
            }
            About::Lines { pane, range } => {
                let Some(pane) = self.panes.get(*pane).and_then(Sheet::code) else {
                    return;
                };
                Remark {
                    group: group_id,
                    ref_id: Some(pane.ref_id.clone()),
                    file: Some(pane.file.clone()),
                    range: Some(*range),
                    quote: pane.lines_of(*range),
                    text: said,
                }
            }
            About::Claim => Remark {
                group: group_id,
                ref_id: None,
                file: None,
                range: None,
                // The sentence being answered, so the remark reads on its own
                // — the one that was clicked, not whichever came first.
                quote: self.picked_said.map_or_else(
                    || claim.lines().next().unwrap_or_default().to_string(),
                    |(from, to)| {
                        let said = crate::prose::words(&claim);
                        let (a, b) = (from.min(to), from.max(to).min(said.len().saturating_sub(1)));
                        said.get(a..=b)
                            .unwrap_or_default()
                            .concat()
                            .trim()
                            .to_string()
                    },
                ),
                text: said,
            },
        };

        self.remarks.push(remark);
        cx.notify();
    }

    /// Follow every pinned remark from the text it was written against to the
    /// file as it is now.
    ///
    /// Done here, at the last moment, rather than while the deck is open. The
    /// window is not the reader's editor and has no part in the edits; what it
    /// has is the text it showed and the text on disk, and the only moment the
    /// second one matters is the moment the review leaves.
    ///
    /// Batched by file because the diff is per file: every pin in one file is
    /// followed through one comparison.
    fn follow_the_files(&self) -> std::collections::HashMap<usize, Relocated> {
        let pins: Vec<(usize, &std::path::Path, LineRange)> = self
            .remarks
            .iter()
            .enumerate()
            .filter_map(|(ix, remark)| Some((ix, remark.file.as_deref()?, remark.range?)))
            .collect();

        let base = self.deck.base();
        follow(&pins, &self.snapshots, |file| read_source(&base, file))
    }

    /// Take everything worth keeping, so the window can be put away.
    fn pack(&self) -> Session {
        Session {
            deck: self.deck.clone(),
            layout: self.grid,
            snapshots: self.snapshots.clone(),
            remarks: self.remarks.clone(),
            group_ix: self.group_ix,
            turn: self.turn,
            band_height: self.band_height,
            shares: self.shares.clone(),
            widths: self.widths.clone(),
            folded: self.folded.clone(),
        }
    }

    /// Put the deck away, leaving the bar it came from.
    ///
    /// Not the same as closing. Closing ends the command, and the agent gets
    /// whatever review was submitted; this keeps everything and gives the
    /// reader their screen back, which is what they actually wanted when they
    /// reached for the corner of the window.
    fn on_hide(&mut self, _: &Hide, window: &mut Window, cx: &mut Context<Self>) {
        if let WindowBounds::Windowed(bounds) = window.window_bounds() {
            crate::state::remember_bounds(bounds);
        }
        // Back on the queue, and the bar comes up over it. The window goes
        // after, because closing the last one ends the command.
        crate::open_pill(self.pack(), cx);
        window.remove_window();
    }

    /// Write the review and close.
    ///
    /// The file is written beside the deck under its id, and written to a
    /// temporary name first: the far side only tests that the file exists, so a
    /// half-written one would be indistinguishable from a finished review.
    /// Leave, and let whatever is still waiting take the screen.
    fn stand_down(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if crate::queue::len(cx) > 0 {
            crate::open_pill_over(Vec::new(), cx);
        }
        window.remove_window();
    }

    fn submit_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let found = self.follow_the_files();
        let comments: Vec<deck_core::Comment> = self
            .remarks
            .iter()
            .enumerate()
            .map(|(ix, remark)| {
                let moved = found.get(&ix).copied();
                deck_core::Comment {
                    group: remark.group.to_string(),
                    ref_id: remark.ref_id.as_ref().map(ToString::to_string),
                    file: remark.file.clone(),
                    range: moved.map_or(remark.range, |found| Some(found.range)),
                    source: moved.map(|found| found.source),
                    kind: deck_core::Kind::Question,
                    quote: remark.quote.clone(),
                    text: remark.text.clone(),
                }
            })
            .collect();

        let review = deck_core::Review {
            v: deck_core::VERSION,
            deck: self.deck.header.id.clone(),
            comments,
        };

        let path = self
            .deck
            .root
            .parent()
            .unwrap_or(&self.deck.root)
            .join(format!("{}.review", self.deck.header.id));
        let tmp = path.with_extension("review.tmp");

        let wrote = serde_json::to_string(&review)
            .map_err(std::io::Error::other)
            .and_then(|json| std::fs::write(&tmp, json))
            .and_then(|()| std::fs::rename(&tmp, &path));

        match wrote {
            // Answered, so it does not come back. Whatever else is waiting
            // does: the bar returns, and the command only ends once the queue
            // is empty.
            Ok(()) => self.stand_down(window, cx),
            Err(err) => eprintln!("deck: could not write {}: {err}", path.display()),
        }
    }

    /// Rebuild the panes for the current group.
    ///
    /// Called only when the group changes. Everything else — selection, a
    /// comment being written — repaints without coming through here, because a
    /// rebuild throws away where the reader had scrolled to.
    fn build_panes(&mut self, cx: &mut App) {
        let base = self.deck.base();
        let Some(group) = self.deck.groups().get(self.group_ix) else {
            self.panes = Vec::new();
            return;
        };

        self.panes = group
            .refs
            .iter()
            .map(|entry| match entry {
                Ref::Code(code) => {
                    // Read once per file and kept, so a pane rebuilt after the
                    // reader put the deck away shows the same text the comments
                    // in it were pinned against.
                    let source = self
                        .snapshots
                        .entry(code.file.clone())
                        .or_insert_with(|| std::sync::Arc::from(read_source(&base, &code.file)))
                        .clone();
                    Sheet::Code(Pane::new(code, &source, cx))
                }
                Ref::Diagram(drawn) => Sheet::Drawn(Chart::new(drawn)),
            })
            .collect();
    }

    fn render_band(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Nothing to read yet is a real state, not a blank. The agent opens
        // the window and then writes, so an empty deck is the first moment of
        // most decks' lives — and a window that shows nothing at all in it
        // looks broken rather than early.
        let say = self.group().map_or_else(
            || {
                if self.deck.sealed() {
                    "This deck has no groups in it.".to_string()
                } else {
                    "The agent is writing this deck. Groups appear as they land — \
                     you can start on the first without waiting for the last."
                        .to_string()
                }
            },
            |group| group.say.clone(),
        );

        // Two columns, not a stack with a legend tucked beside the prose. The
        // divider is the edge of the keys column, so it runs the whole height
        // of the band.
        div()
            .h_flex()
            .flex_none()
            .items_stretch()
            .bg(paint(self.palette.band))
            .when_some(self.band_height, |this, height| this.h(px(height)))
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .gap(px(9.))
                    .pl(px(18.))
                    .pr(px(18.))
                    .pt(px(15.))
                    .pb(px(14.))
                    .child(
                        div().h_flex().items_baseline().gap(px(14.)).child(
                            div()
                                .text_size(px(15.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(paint(self.palette.fg))
                                .child(self.deck.title()),
                        ),
                    )
                    .child(
                        // The prose is pickable too. An objection to what the
                        // group claims has to be able to land on the claim,
                        // not on some line of code chosen for want of anywhere
                        // better to put it.
                        div()
                            .id("deck-claim")
                            .max_w(px(560.))
                            .text_size(px(13.2))
                            .text_color(paint(self.palette.fg))
                            .child(crate::prose::render(
                                crate::prose::parse(&say),
                                &self.palette,
                                cx.theme().mono_font_family.clone(),
                                &crate::prose::Picking {
                                    range: self.picked_said,
                                    down: {
                                        let deck = cx.entity().downgrade();
                                        std::rc::Rc::new(move |at, _window, cx| {
                                            deck.update(cx, |deck, cx| deck.start_say_pick(at, cx))
                                                .ok();
                                        })
                                    },
                                    over: {
                                        let deck = cx.entity().downgrade();
                                        std::rc::Rc::new(move |at, entered, _window, cx| {
                                            deck.update(cx, |deck, _| deck.hover_say(at, entered))
                                                .ok();
                                        })
                                    },
                                },
                            )),
                    ),
            )
            .child(self.render_legend(cx))
    }

    /// Move the seam between the narration and the code.
    ///
    /// The band holds prose, so unlike a pane it has a height its content
    /// wants. Dragging overrides that; until then it is left to size itself.
    fn resize_band(&mut self, by: Pixels, cx: &mut Context<Self>) {
        const LEAST: f32 = 64.;
        const MOST: f32 = 520.;

        let from = self.band_height.unwrap_or(BAND_NATURAL);
        self.band_height = Some((from + f32::from(by)).clamp(LEAST, MOST));
        cx.notify();
    }

    /// Move height across the divider below pane `ix`.
    ///
    /// The two panes either side trade what one gains, so the page keeps its
    /// total and nothing else on it moves.
    fn resize_panes(&mut self, ix: usize, by: Pixels, height: Pixels, cx: &mut Context<Self>) {
        // Two lines of code is the least a pane can usefully show; below that
        // a drag has stopped resizing and started deleting.
        const LEAST: f32 = 0.08;

        let total: f32 = self.shares.iter().sum();
        if ix + 1 >= self.shares.len() || height <= px(0.) || total <= 0. {
            return;
        }

        let delta = f32::from(by) / f32::from(height) * total;
        let (a, b) = (self.shares[ix] + delta, self.shares[ix + 1] - delta);
        if a < LEAST * total || b < LEAST * total {
            return;
        }

        self.shares[ix] = a;
        self.shares[ix + 1] = b;
        cx.notify();
    }

    /// Move width across the seam to the right of pane `ix`.
    ///
    /// The same trade the row seam makes, along the other axis: the two panes
    /// either side swap what one gains, so the row keeps its total and no
    /// other row on the page moves.
    fn resize_columns(&mut self, ix: usize, by: Pixels, width: Pixels, cx: &mut Context<Self>) {
        // Narrower than this and a pane has stopped being a pane. Wider than
        // the row seam's floor because width is what a pane is short of.
        const LEAST: f32 = 0.12;

        let cols = self.cols.max(1);
        if ix + 1 >= self.widths.len() || width <= px(0.) {
            return;
        }

        // Share is only comparable within a row, so the total is this row's.
        let first = ix / cols * cols;
        let last = (first + cols).min(self.widths.len());
        let total: f32 = self.widths[first..last].iter().sum();
        if total <= 0. {
            return;
        }

        let delta = f32::from(by) / f32::from(width) * total;
        let (a, b) = (self.widths[ix] + delta, self.widths[ix + 1] - delta);
        if a < LEAST * total || b < LEAST * total {
            return;
        }

        self.widths[ix] = a;
        self.widths[ix + 1] = b;
        cx.notify();
    }

    /// Take hold of a diagram, so moving the pointer moves the drawing.
    pub fn start_pan(&mut self, pane_ix: usize, at: Point<Pixels>) {
        if let Some(chart) = self.panes.get(pane_ix).and_then(Sheet::chart) {
            self.panning = Some((pane_ix, at, chart.scroll().offset()));
        }
    }

    /// Carry the drawing to where the pointer has got to.
    ///
    /// Clamped to what there is: a picture that fits its pane has nowhere to
    /// go on that axis, and one that does not stops at its own edge rather
    /// than sliding out of the window.
    fn pan_to(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((pane_ix, from, was)) = self.panning else {
            return;
        };
        let Some(chart) = self.panes.get(pane_ix).and_then(Sheet::chart) else {
            return;
        };

        let scroll = chart.scroll();
        let slack = scroll.max_offset();
        let held = |wanted: Pixels, slack: Pixels| wanted.clamp(-slack.max(px(0.)), px(0.));

        scroll.set_offset(point(
            held(was.x + (at.x - from.x), slack.x),
            held(was.y + (at.y - from.y), slack.y),
        ));
        cx.notify();
    }

    /// Fold a remark down to its header, or open it again.
    pub fn toggle_remark(&mut self, remark_ix: usize, cx: &mut Context<Self>) {
        if !self.folded.remove(&remark_ix) {
            self.folded.insert(remark_ix);
        }
        cx.notify();
    }

    /// Take a remark back.
    ///
    /// A comment written by mistake, or answered by reading further, should be
    /// removable — a review is meant to carry what the reader still means.
    pub fn drop_remark(&mut self, remark_ix: usize, cx: &mut Context<Self>) {
        if remark_ix < self.remarks.len() {
            self.remarks.remove(remark_ix);
            // Folded state is held by index, so removing one shifts every
            // remark after it. Rebuild rather than leave the set pointing at
            // whatever moved up into the gap.
            self.folded = self
                .folded
                .iter()
                .filter(|ix| **ix != remark_ix)
                .map(|ix| if *ix > remark_ix { ix - 1 } else { *ix })
                .collect();
            cx.notify();
        }
    }

    /// A seam, and the handle for moving it.
    ///
    /// One shape for all three of the deck's seams — under the band, between
    /// two rows, and between two panes standing side by side. The mock draws
    /// them differently: the band's fills with washed accent, the panes' is a
    /// rule, on the reasoning that the band already meets the code at a change
    /// of colour and needs no line of its own. True of the seam; not true of
    /// the *handle*, which is a control, and controls that do the same thing
    /// should not answer the pointer in different ways.
    ///
    /// Held counts as hovered. Hover alone ends the moment the pointer leaves
    /// the few pixels of the handle, which during a drag is immediately — so
    /// it went quiet while it was still being dragged, and the reader lost the
    /// one thing telling them what they had hold of.
    fn render_seam(&self, what: Divide, cx: &mut Context<Self>) -> AnyElement {
        /// How much of the page the handle claims. Wider than the rule it
        /// draws, because a one-pixel target is not a target.
        const GRIP: f32 = 7.;

        let (id, sideways): (ElementId, bool) = match what {
            Divide::Band => ("seam-band".into(), false),
            Divide::Panes(ix) => (("seam-panes", ix).into(), false),
            Divide::Columns(ix) => (("seam-columns", ix).into(), true),
        };
        let group = SharedString::from(format!("{id:?}"));
        let held = self.sizing.map(|(held, _)| held) == Some(what);
        let accent = paint(self.palette.accent);

        let rule = div()
            .map(|this| {
                if sideways {
                    this.w(px(1.)).h_full()
                } else {
                    this.h(px(1.)).w_full()
                }
            })
            .bg(paint(self.palette.edge))
            .when(held, |this| lit(this, sideways, accent))
            .when(!held, |this| {
                this.group_hover(group.clone(), move |this| lit(this, sideways, accent))
            });

        div()
            .id(id)
            .group(group)
            .flex_none()
            // The page shows through the gap, so two panes read as two sheets
            // rather than one surface with a crack in it.
            .bg(paint(self.palette.bg))
            .map(|this| {
                if sideways {
                    this.w(px(GRIP))
                        .h_full()
                        .h_flex()
                        .justify_start()
                        .pl(px(3.))
                        .cursor(CursorStyle::ResizeLeftRight)
                } else {
                    this.h(px(GRIP))
                        .w_full()
                        .v_flex()
                        .justify_start()
                        .pt(px(3.))
                        .cursor(CursorStyle::ResizeUpDown)
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |deck, event: &MouseDownEvent, _window, _cx| {
                    deck.sizing = Some((
                        what,
                        if sideways {
                            event.position.x
                        } else {
                            event.position.y
                        },
                    ));
                }),
            )
            .child(rule)
            .into_any_element()
    }

    /// The button that goes back to the lit range, while it is off screen.
    ///
    /// Nothing to press when the range is already in front of you, so it is
    /// not drawn — the label has other things to say and this would only ever
    /// be one of them.
    fn render_focus_button(&self, pane_ix: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let away = self.panes.get(pane_ix)?.range_away()?;

        Some(
            div()
                .id(("focus", pane_ix))
                .flex_none()
                .px(px(6.))
                .py(px(3.))
                .rounded(px(4.))
                .border_1()
                .border_color(paint(self.palette.accent))
                .text_size(px(10.))
                .text_color(paint(self.palette.accent))
                .hover(|this| this.bg(paint(self.palette.focus)))
                .on_click(cx.listener(move |deck, _, _window, cx| deck.glide_to(pane_ix, cx)))
                .child(match away {
                    Away::Above => "↑ focus",
                    Away::Below => "↓ focus",
                })
                .into_any_element(),
        )
    }

    /// Take a wheel's worth of movement, and start the pane travelling.
    ///
    /// The wheel moves a *target*, not the pane. Every event that arrives while
    /// the pane is still catching up moves the target further, so a long
    /// trackpad flick is one continuous movement rather than forty of them, and
    /// a mouse notch is a glide rather than a jump.
    pub fn wheel(&mut self, pane_ix: usize, by: Pixels, cx: &mut Context<Self>) {
        let Some(pane) = self.panes.get(pane_ix).and_then(Sheet::code) else {
            return;
        };
        let scroll = pane.scroll();
        let handle = scroll.0.borrow().base_handle.clone();

        // Where it would end up if it kept going, held inside the file.
        let slack = f32::from(handle.max_offset().y).max(0.);
        let from = match self.drifting {
            Some((at, to)) if at == pane_ix => to,
            _ => f32::from(handle.offset().y),
        };
        let to = (from + f32::from(by)).clamp(-slack, 0.);

        // Already travelling to this pane: the target moved, and the loop that
        // is following it will pick that up on its next step.
        if self.drifting.is_some_and(|(at, _)| at == pane_ix) {
            self.drifting = Some((pane_ix, to));
            return;
        }
        self.drifting = Some((pane_ix, to));

        self.gliding = cx.spawn(async move |view, cx| {
            /// How much of what is left is closed each step. An exponential
            /// approach rather than a fixed run: it starts quickly, settles
            /// without stopping dead, and — the reason it is this shape — has
            /// no end to interrupt when the next wheel event arrives.
            const FOLLOW: f32 = 0.28;
            /// Below this it has arrived, and a pixel of drift left over is
            /// worth less than the frames spent closing it.
            const ENOUGH: f32 = 0.4;

            loop {
                let stepped = view.update(cx, |deck, cx| {
                    let Some((at, to)) = deck.drifting else {
                        return false;
                    };
                    if at != pane_ix {
                        return false;
                    }

                    let now = f32::from(handle.offset().y);
                    if (to - now).abs() < ENOUGH {
                        handle.set_offset(point(handle.offset().x, px(to)));
                        deck.drifting = None;
                        cx.notify();
                        return false;
                    }
                    handle.set_offset(point(handle.offset().x, px(now + (to - now) * FOLLOW)));
                    cx.notify();
                    true
                });

                if !matches!(stepped, Ok(true)) {
                    return;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(12))
                    .await;
            }
        });
    }

    /// Carry the pane back to its lit range over a few frames.
    ///
    /// Jumping there works and reads as the pane having been replaced: the
    /// reader has to find their bearings again in code that did not appear to
    /// move. Travelling shows which way it went and how far, so the range
    /// arrives somewhere the eye already is.
    fn glide_to(&mut self, pane_ix: usize, cx: &mut Context<Self>) {
        const STEPS: usize = 26;

        // Whatever the wheel was doing, it is not what the reader asked for
        // now. Two things moving one pane would fight over every frame.
        self.drifting = None;

        let Some(pane) = self.panes.get(pane_ix).and_then(Sheet::code) else {
            return;
        };
        let Some(target) = pane.focus_offset() else {
            return;
        };
        let scroll = pane.scroll();
        let from = scroll.0.borrow().base_handle.offset();

        self.gliding = cx.spawn(async move |view, cx| {
            for step in 1..=STEPS {
                // Ease out: most of the distance early, so it settles rather
                // than stopping.
                #[allow(clippy::cast_precision_loss)]
                let t = step as f32 / STEPS as f32;
                let eased = 1. - (1. - t).powi(3);
                let y = from.y + (target - from.y) * eased;

                let moved = view.update(cx, |_, cx| {
                    scroll.0.borrow().base_handle.set_offset(point(from.x, y));
                    cx.notify();
                });
                if moved.is_err() {
                    return;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(12))
                    .await;
            }
        });
    }

    /// The composer, when one is open.
    ///
    /// It claims its own key context so that `s` types an `s` instead of
    /// submitting the review, and so `cmd-enter` and `escape` mean something
    /// here and nothing outside.
    fn render_composer(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (about, state, _) = self.composing.as_ref()?;
        let where_at = match about {
            About::Lines { pane, range } => self
                .panes
                .get(*pane)
                .and_then(Sheet::code)
                .map(|pane| format!("comment on {}:{range}", pane.file.display()))
                .unwrap_or_default(),
            About::Drawn { pane } => self
                .panes
                .get(*pane)
                .and_then(Sheet::chart)
                .map(|chart| format!("comment on {}", chart.quote()))
                .unwrap_or_default(),
            About::Claim => "comment on this group".to_string(),
        };
        let ref_id = match about {
            About::Lines { pane, .. } | About::Drawn { pane } => {
                self.panes.get(*pane).map(|pane| pane.ref_id().clone())
            }
            About::Claim => self.group().map(|g| SharedString::from(g.id.clone())),
        };

        Some(
            div()
                .v_flex()
                .flex_none()
                .key_context("DeckComposer")
                .pl(px(16.))
                .pr(px(16.))
                .pt(px(13.))
                .pb(px(14.))
                .border_t_1()
                .border_color(paint(self.palette.edge))
                // The handlers live here, not only on the root. An action
                // dispatches up the focus chain from the element that has the
                // keyboard, and the composer is the nearest thing to it that
                // knows what saving means — putting them at the root meant the
                // textarea swallowed the key before anything heard it.
                .on_action(cx.listener(Self::on_submit))
                .on_action(cx.listener(Self::on_discard))
                .gap(px(9.))
                .bg(paint(self.palette.band))
                .child(
                    div()
                        .h_flex()
                        .justify_between()
                        .gap(px(12.))
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(11.5))
                        .text_color(paint(self.palette.muted))
                        .child(where_at)
                        .children(ref_id),
                )
                // Sized against the narration it answers, not against the
                // labels around it. What the reader types here is prose, and
                // it was set smaller than every other piece of prose in the
                // window — which read as a footnote to the deck rather than as
                // the half of it that is theirs.
                .child(
                    div()
                        .text_size(px(13.2))
                        .line_height(px(20.))
                        .child(Textarea::new(state).h(px(72.))),
                )
                // Under the box, where a hint belongs: beside the location it
                // competes with the one thing the reader needs to read.
                .child(
                    div()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(11.))
                        .text_color(paint(self.palette.muted))
                        .h_flex()
                        .gap(px(14.))
                        .child("⌘⏎ save")
                        .child("esc discard"),
                ),
        )
    }

    /// The strip along the bottom: what is still coming, and which deck this
    /// is. Quiet, and always in the same place.
    fn render_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let said = self.remarks.len();
        // How much of the story can be read right now.
        let here = u32::try_from(self.deck.groups().len()).unwrap_or(1);
        let total = self.deck.header.total.unwrap_or(here).max(here).max(1);
        // What the deck says it will be, never less than what it already is: a
        // count that a group can arrive and make a lie of is worse than none.
        let writing = !self.deck.sealed();
        div()
            .h_flex()
            .flex_none()
            .justify_between()
            .items_center()
            .gap(px(14.))
            .pl(px(14.))
            .pr(px(14.))
            .py(px(7.))
            .bg(paint(self.palette.band))
            .border_t_1()
            .border_color(paint(self.palette.edge))
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(10.5))
            .text_color(paint(self.palette.muted))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(14.))
                    // Where you are lives down here with the other counts,
                    // rather than beside the title. The title says what the
                    // group is about; the strip says where you are in the deck
                    // and what you have said so far, and those belong together.
                    .child(format!("group {}/{}", self.group_ix + 1, total))
                    .when(writing, |this| {
                        this.child(
                            div()
                                .h_flex()
                                .items_center()
                                .gap(px(6.))
                                .text_color(paint(self.palette.accent))
                                // Breathing, not lit. A steady dot says a
                                // state; a moving one says a process, and this
                                // is a process.
                                .child(
                                    div()
                                        .size(px(5.))
                                        .rounded_full()
                                        .bg(paint(self.palette.accent))
                                        .with_animation(
                                            "writing",
                                            Animation::new(std::time::Duration::from_millis(1900))
                                                .repeat()
                                                .with_easing(pulsating_between(0.25, 1.0)),
                                            |this, breath| this.opacity(breath),
                                        ),
                                )
                                // Which group is being waited on, when that
                                // can be said. It is the next one the story
                                // needs — not the next file to land, which
                                // may be one that arrived early and is being
                                // held behind a gap.
                                .child(if here < total {
                                    format!("claude is writing group {}", here + 1)
                                } else {
                                    "still writing".to_string()
                                }),
                        )
                    }),
            )
            .child(
                div()
                    .h_flex()
                    .gap_4()
                    .child(format!(
                        "{said} comment{}",
                        if said == 1 { "" } else { "s" }
                    ))
                    .child(self.deck.header.id.clone())
                    // Putting the deck away has to be reachable without
                    // knowing a key. It sits at the quiet end of the strip
                    // rather than as a control in the header: it is a way out,
                    // not something to be tempted by.
                    .child(
                        div()
                            .id("hide")
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(px(4.))
                            .text_color(paint(self.palette.muted))
                            .hover(|this| {
                                this.bg(paint(self.palette.wash))
                                    .text_color(paint(self.palette.fg))
                            })
                            .on_click(cx.listener(|deck, _, window, cx| {
                                deck.on_hide(&Hide, window, cx);
                            }))
                            .child("hide"),
                    ),
            )
    }

    /// The legend, built from [`KEYS`] so a rebound key is never stale on it.
    fn render_legend(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mono = cx.theme().mono_font_family.clone();

        div()
            .v_flex()
            .flex_none()
            .w(px(168.))
            .flex_none()
            .pl(px(14.))
            .pr(px(14.))
            .pt(px(15.))
            .pb(px(14.))
            // A divider, not a box. The keys are part of the band, set apart
            // from the prose rather than floated on top of it.
            .border_l_1()
            .border_color(paint(self.palette.edge))
            .child(
                div()
                    .pb(px(9.))
                    .font_family(mono.clone())
                    .text_size(px(9.5))
                    .text_color(paint(self.palette.muted))
                    .child("KEYS"),
            )
            .children(KEYS.iter().map(|(key, _, says)| {
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(9.))
                    .py(px(2.5))
                    .font_family(mono.clone())
                    .text_size(px(11.))
                    .text_color(paint(self.palette.fg.mix(self.palette.band, 0.28)))
                    // A key is a thing you press, so it is drawn as one — a
                    // cap with a thicker bottom edge, the way a key catches
                    // light.
                    .child(
                        div()
                            .min_w(px(19.))
                            .flex_none()
                            .px(px(4.))
                            .py(px(3.))
                            .text_center()
                            .rounded(px(4.))
                            .border_1()
                            .border_b_2()
                            .border_color(paint(self.palette.edge))
                            .bg(paint(self.palette.wash))
                            .text_size(px(10.5))
                            .line_height(px(10.5))
                            .text_color(paint(self.palette.fg))
                            .child(*key),
                    )
                    .child(*says)
            }))
    }
}

impl Render for DeckView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Built up front rather than inside `.children()`: rendering a pane
        // needs the context, and a closure holding it cannot also be handed to
        // the element it is building.
        // Rows call back into the view when clicked, and a weak handle is what
        // a closure that outlives this frame is allowed to hold.
        let me = cx.entity().downgrade();

        // The grid decides how many panes stand beside each other, from the
        // width actually available. `min_pane_width` is in the same unit, so
        // the two only have to agree with each other.
        //
        // A group with a picture in it is laid out sideways: a diagram wants
        // width and a file wants height, and stacking them gives each one the
        // other's shape.
        //
        // Worked out before a single pane is built, because a pane carries its
        // own share of the row and cannot be told afterwards.
        //
        // And only while the picture actually fits beside the code. A diagram
        // is put in a column because a diagram wants width; giving it half a
        // window and cutting the last box off it is the opposite of that. A
        // wide one gets the full width and the row below, where it fits and the
        // reader can see all of it without dragging.
        let count = self.panes.len();
        let drawings: Vec<f32> = self
            .panes
            .iter()
            .filter_map(Sheet::chart)
            .map(Chart::drawn_width)
            .collect();

        let across = columns_across(window);
        let panes_across = u32::try_from(count).unwrap_or(1);
        let settled = if drawings.is_empty() {
            self.grid
        } else {
            let beside = self.grid.beside_pictures();
            let cols = beside.grid(panes_across, across).cols.max(1);
            // What each column would come to, less the seam between them and
            // the scrollbar down the side of a pane.
            let each = f32::from(window.viewport_size().width) / cols as f32 - 18.;
            if drawings.iter().all(|width| *width <= each) {
                beside
            } else {
                self.grid
            }
        };

        // A quarter turn: an axis, and which pane leads.
        //
        // The reader's if they have turned the page, and otherwise whatever
        // the page settled on — recorded either way, so `r` advances from what
        // is actually on screen.
        let turn = self
            .turn
            .unwrap_or(u8::from(settled.arrange != Arrange::Columns));
        self.turn_now = turn;

        let (arrange, backwards) = quarter(turn);
        let spec = match self.turn {
            None => settled,
            Some(_) => GridSpec {
                arrange,
                ..self.grid
            },
        };
        let grid = spec.grid(panes_across, across);

        // Which pane stands where. Two of the four turns put the page's panes
        // in the order the group wrote them; the other two read the other way.
        let order: Vec<usize> = if backwards {
            (0..count).rev().collect()
        } else {
            (0..count).collect()
        };
        let cols = grid.cols.max(1) as usize;
        self.cols = cols;
        let row_count = count.div_ceil(cols).max(1);

        // One share per row for height, one per pane for width, kept in step
        // with however many there are.
        if self.shares.len() != row_count {
            self.shares = vec![1.; row_count];
        }
        if self.widths.len() != count {
            self.widths = vec![1.; count];
        }

        // Shares are held by *place*, not by pane: a seam moves width between
        // the two panes standing either side of it, and turning the page moves
        // panes between places without moving the places.
        let mut panes = Vec::with_capacity(count);
        for (place, &ix) in order.iter().enumerate() {
            // The remark's own index rides along, so a card can say which one
            // to drop when it is closed.
            let marks: Vec<Mark> = self
                .remarks
                .iter()
                .enumerate()
                .filter(|(_, remark)| remark.ref_id.as_ref() == Some(self.panes[ix].ref_id()))
                .filter_map(|(remark_ix, remark)| {
                    Some(Mark {
                        range: remark.range?,
                        text: SharedString::from(remark.text.clone()),
                        remark_ix,
                        folded: self.folded.contains(&remark_ix),
                    })
                })
                .collect();
            let slot = Slot {
                ix,
                share: self.widths.get(place).copied().unwrap_or(1.),
                palette: &self.palette,
                view: &me,
            };
            panes.push(self.panes[ix].render(&slot, &marks, self.render_focus_button(ix, cx), cx));
        }

        // The share goes on the pane itself, not on a wrapper around it. A
        // wrapper is a block, and a pane asking for its share inside one has
        // no flex context to ask — the panes came out with no size at all.
        let mut feed = panes.into_iter();
        let mut stacked: Vec<AnyElement> = Vec::with_capacity(row_count * 2);
        for row_ix in 0..row_count {
            if row_ix > 0 {
                stacked.push(self.render_seam(Divide::Panes(row_ix - 1), cx));
            }

            let first = row_ix * cols;
            let last = (first + cols).min(count);
            let mut across: Vec<AnyElement> = Vec::with_capacity((last - first) * 2);
            for ix in first..last {
                if ix > first {
                    across.push(self.render_seam(Divide::Columns(ix - 1), cx));
                }
                if let Some(pane) = feed.next() {
                    across.push(pane);
                }
            }

            stacked.push(
                div()
                    .h_flex()
                    // A zero basis makes the share the only thing deciding a
                    // row's height: otherwise a row holding a longer file
                    // would start out taller for no reason the reader asked
                    // for.
                    .flex_grow(self.shares.get(row_ix).copied().unwrap_or(1.))
                    .flex_basis(px(0.))
                    .min_h_0()
                    .children(across)
                    .into_any_element(),
            );
        }
        let rows = stacked;

        div()
            .size_full()
            .v_flex()
            .track_focus(&self.focus)
            // Only while nothing is being typed.
            //
            // GPUI matches a key against every context up the focus chain, so
            // an ancestor that always claimed "Deck" kept `q` bound to quit
            // even with the caret inside the composer — typing the letter shut
            // the window and threw the remark away. The deck's keys are only
            // the deck's keys when the deck has the keyboard.
            .when(self.composing.is_none(), |this| this.key_context("Deck"))
            .on_action(cx.listener(Self::on_next))
            .on_action(cx.listener(Self::on_prev))
            .on_action(cx.listener(Self::on_close))
            .on_action(cx.listener(Self::on_comment))
            .on_action(cx.listener(Self::on_rotate))
            .on_action(cx.listener(Self::on_hide))
            .on_action(cx.listener(Self::on_submit))
            .on_action(cx.listener(Self::on_discard))
            .on_action(cx.listener(Self::on_zoom_in))
            .on_action(cx.listener(Self::on_zoom_out))
            .on_action(cx.listener(Self::on_zoom_reset))
            // One handler for the whole window, asking the event whether a
            // button is down rather than remembering that it was. A remembered
            // flag is what kept sticking on, and then merely crossing the code
            // with the pointer rewrote the selection.
            .on_mouse_move(cx.listener(|deck, event: &MouseMoveEvent, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    deck.sizing = None;
                    deck.panning = None;
                    return;
                }
                // A divider being dragged owns the pointer; the code under it
                // must not also be selecting.
                if let Some((what, from)) = deck.sizing {
                    let along = if matches!(what, Divide::Columns(_)) {
                        event.position.x
                    } else {
                        event.position.y
                    };
                    let by = along - from;
                    match what {
                        Divide::Band => deck.resize_band(by, cx),
                        Divide::Panes(ix) => {
                            deck.resize_panes(ix, by, window.viewport_size().height, cx);
                        }
                        Divide::Columns(ix) => {
                            deck.resize_columns(ix, by, window.viewport_size().width, cx);
                        }
                    }
                    deck.sizing = Some((what, along));
                } else if deck.panning.is_some() {
                    deck.pan_to(event.position, cx);
                } else {
                    deck.drag_to_hovered(cx);
                    deck.drag_say_to_hovered(cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|deck, _, _window, cx| {
                    deck.sizing = None;
                    deck.panning = None;
                    deck.end_say_pick(cx);
                }),
            )
            .bg(paint(self.palette.bg))
            .border_1()
            .border_color(paint(self.palette.edge))
            .text_color(paint(self.palette.fg))
            .child(self.render_band(cx))
            .child(self.render_seam(Divide::Band, cx))
            .child(if self.deck.groups().is_empty() {
                crate::waiting::render(
                    &self.palette,
                    self.spread,
                    f32::from(window.viewport_size().width) >= crate::waiting::ROOM,
                )
                .into_any_element()
            } else {
                // Panes share the height between them and each scrolls its own
                // file. The list inside a pane has no height of its own, so the
                // chain from here down to it must be unbroken: a pane sized to
                // its content would leave the list nothing to fill and it would
                // render no rows at all.
                div()
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .children(rows)
                    .into_any_element()
            })
            .children(self.render_composer(cx))
            .child(self.render_strip(cx))
    }
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this module glob-imports GPUI, which
    // exports a `test` attribute of its own and would otherwise shadow the
    // standard one.
    use core::prelude::v1::test;

    use super::*;

    fn snapshots(
        of: &[(&str, &str)],
    ) -> std::collections::HashMap<std::path::PathBuf, std::sync::Arc<str>> {
        of.iter()
            .map(|(file, text)| (std::path::PathBuf::from(file), std::sync::Arc::from(*text)))
            .collect()
    }

    #[test]
    fn every_pin_comes_back_to_the_remark_it_came_from() {
        // Two files, pins interleaved between them. The pairing is what is
        // being checked: a remark that came back carrying another remark's line
        // would be the worst thing this program could do quietly.
        let was_a = "one\ntwo\nthree\n";
        let now_a = "added\nadded\none\ntwo\nthree\n";
        let same_b = "alpha\nbeta\n";

        let a = std::path::Path::new("a.rs");
        let b = std::path::Path::new("b.rs");
        let pins = [
            (0, a, LineRange::new(1, 1)),
            (1, b, LineRange::new(2, 2)),
            (2, a, LineRange::new(3, 3)),
        ];

        let found = follow(
            &pins,
            &snapshots(&[("a.rs", was_a), ("b.rs", same_b)]),
            |file| {
                if file == a {
                    now_a.to_string()
                } else {
                    same_b.to_string()
                }
            },
        );

        assert_eq!(
            found[&0].range,
            LineRange::new(3, 3),
            "`one` moved down two"
        );
        assert_eq!(found[&1].range, LineRange::new(2, 2), "`beta` did not move");
        assert_eq!(found[&2].range, LineRange::new(5, 5), "`three` moved too");
    }

    #[test]
    fn a_file_nobody_snapshotted_is_left_alone() {
        // Not guessed at. The remark keeps the range it was written with and
        // says nothing about how far to trust it, which is the honest answer to
        // a question nobody can answer.
        let pins = [(0, std::path::Path::new("gone.rs"), LineRange::new(4, 4))];
        let found = follow(&pins, &snapshots(&[]), |_| String::new());

        assert!(found.is_empty());
    }

    #[test]
    fn four_turns_are_four_different_pages() {
        let pages: Vec<(Arrange, bool)> = (0..4).map(quarter).collect();
        assert_eq!(
            pages,
            vec![
                (Arrange::Columns, false),
                (Arrange::Stacked, false),
                (Arrange::Columns, true),
                (Arrange::Stacked, true),
            ]
        );
    }

    #[test]
    fn turning_four_times_comes_back_to_where_it_started() {
        // Which is the whole reason there is one key rather than two: the way
        // out of an arrangement you did not want is to keep pressing it.
        for turn in 0..4 {
            assert_eq!(quarter(turn), quarter(turn + 4));
        }
    }

    #[test]
    fn a_half_turn_keeps_the_axis_and_swaps_the_panes() {
        // Turning twice is the same page read the other way round, not a
        // different shape of page.
        for turn in 0..2 {
            let (there, forwards) = quarter(turn);
            let (back, reversed) = quarter(turn + 2);
            assert_eq!(there, back);
            assert!(!forwards && reversed);
        }
    }
}
