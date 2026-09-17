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
use deck_core::{LineRange, LiveEffect, PauseReason, Relocated, Stage, StageId};
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
        NextGroup, PrevGroup, Comment, Rotate, Zen, Live, Follow, Hide, Submit, Discard, Noted,
        Asked, Wrong, ZoomIn, ZoomOut, ZoomReset, Close
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
    ("z", "zen", "lights off"),
    ("l", "live", "live walkthrough"),
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
        KeyBinding::new("z", Zen, Some("Deck")),
        KeyBinding::new("l", Live, Some("Deck")),
        KeyBinding::new("1", Noted, Some("Deck")),
        KeyBinding::new("2", Asked, Some("Deck")),
        KeyBinding::new("3", Wrong, Some("Deck")),
        KeyBinding::new("f", Follow, Some("Deck")),
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
fn resolved_path(base: &std::path::Path, file: &std::path::Path) -> std::path::PathBuf {
    let joined = if file.is_absolute() {
        file.to_path_buf()
    } else {
        base.join(file)
    };
    std::fs::canonicalize(&joined).unwrap_or(joined)
}

fn snapshot_identity(source: &str) -> String {
    use std::hash::{Hash as _, Hasher as _};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hash);
    format!("snapshot:{:016x}", hash.finish())
}

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
    /// Between the panes and the live rail.
    Rail,
    /// Between the pane at this index and the one to its right.
    ///
    /// Named by pane rather than by column, because the panes either side of
    /// it are the two that trade width and they are what the drag has to
    /// reach. Only ever drawn between two panes of the same row.
    Columns(usize),
}

impl Divide {
    fn along(self, at: Point<Pixels>) -> Pixels {
        // The rail is vertical too. Reading y made a leftward drag do
        // nothing, then jump as soon as the pointer moved up or down.
        if matches!(self, Self::Columns(_) | Self::Rail) {
            at.x
        } else {
            at.y
        }
    }
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
    /// Lines of one pane's immutable snapshot.
    Lines {
        group: SharedString,
        ref_id: SharedString,
        file: std::path::PathBuf,
        range: LineRange,
        quote: String,
    },
    /// A diagram and whichever node of it was picked.
    Drawn {
        group: SharedString,
        ref_id: SharedString,
        quote: String,
    },
    /// The selected narration, or the group's opening claim.
    Claim { group: SharedString, quote: String },
}

/// Live mode, and how far through its transition it is.
///
/// Time-based rather than a GPUI animation list, for a reason paid for once
/// already: a `with_animation` list whose length changes between frames is a
/// bounds-check panic, and the number of things moving here depends on how many
/// panes the group has. A start instant and a direction cannot go out of step
/// with the tree.
#[derive(Debug, Clone, Copy)]
struct Walking {
    since: std::time::Instant,
    /// True while sliding back out.
    going: bool,
}

/// How long the room takes to rearrange, each way.
///
/// Out is quicker than in. Arriving somewhere should feel like it settles;
/// leaving should feel like it gets out of your way.
const ARRIVE: std::time::Duration = std::time::Duration::from_millis(420);
const LEAVE: std::time::Duration = std::time::Duration::from_millis(260);

impl Walking {
    fn arriving() -> Self {
        Self {
            since: std::time::Instant::now(),
            going: false,
        }
    }

    fn leaving() -> Self {
        Self {
            since: std::time::Instant::now(),
            going: true,
        }
    }

    /// Nought to one, eased, where one is fully live.
    fn pace(self) -> f32 {
        let whole = if self.going { LEAVE } else { ARRIVE };
        let raw = (self.since.elapsed().as_secs_f32() / whole.as_secs_f32()).clamp(0., 1.);
        // Ease out cubic. Fast to start so it answers the key immediately, slow
        // to finish so nothing lands with a snap.
        let eased = 1. - (1. - raw).powi(3);
        if self.going { 1. - eased } else { eased }
    }

    /// Whether this transition has finished leaving and can be forgotten.
    fn spent(self) -> bool {
        self.going && self.since.elapsed() >= LEAVE
    }
}

/// A show request resolved entirely against authored groups and snapshots.
struct ResolvedShow {
    group_ix: usize,
    pane_ix: usize,
    stage: Stage,
    source_matches: bool,
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
    /// Whether it can wait.
    when: deck_core::When,
    /// What the reader wants done about it.
    ///
    /// Carried per remark rather than decided at submit time, because it is the
    /// reader's word and they said it when they wrote the remark. Every comment
    /// used to go out as `Question` whatever they meant, which made the agent
    /// guess tone from prose — the exact thing [`deck_core::Kind`] exists to
    /// stop.
    kind: deck_core::Kind,
}

struct ReadingPlace {
    ref_id: SharedString,
    selected: Option<LineRange>,
    offset: Point<Pixels>,
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
    /// The local command owner, retained while the deck waits or is hidden.
    live: crate::live::Handle,
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
    conversation: crate::conversation::Conversation,
    reading: Vec<ReadingPlace>,
    picked_said: Option<(usize, usize)>,
    band_offset: Point<Pixels>,
    rail_width: Option<f32>,
    rail_scroll: ScrollHandle,
    picked_reply: Option<usize>,
    walking: bool,
    draft: Option<(
        About,
        Entity<TextareaState>,
        deck_core::Kind,
        deck_core::When,
    )>,
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
    pub fn fresh(deck: Deck, live: crate::live::Handle) -> Self {
        Self {
            deck,
            live,
            layout: GridSpec::default(),
            snapshots: std::collections::HashMap::new(),
            remarks: Vec::new(),
            conversation: crate::conversation::Conversation::default(),
            reading: Vec::new(),
            picked_said: None,
            band_offset: point(px(0.), px(0.)),
            rail_width: None,
            rail_scroll: ScrollHandle::new(),
            picked_reply: None,
            walking: false,
            draft: None,
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
    /// The session owner whose status follows this view through hide/reopen.
    live: crate::live::Handle,
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
    /// What the remark being written is asking for.
    ///
    /// Held beside the composer rather than inside `About`, because `About` is
    /// *what the remark is pinned to* and this is *what the reader wants done*.
    /// Reset every time a composer opens: a must-fix should never be inherited
    /// by the next remark.
    composing_kind: deck_core::Kind,
    /// Whether the remark being written wants the walk to stop.
    composing_when: deck_core::When,
    /// Whether a voice is reading the deck aloud.
    aloud: bool,
    /// The timer that advances the voice, while there is anything to advance.
    talking_task: Option<Task<()>>,
    /// When the view last carried a pane back to the lit lines.
    ///
    /// Following is checked every tick, not only when the light moves, because
    /// a reader who scrolls away mid-sentence has to be brought back too. This
    /// keeps that from restarting the travel on every one of those ticks.
    followed: Option<std::time::Instant>,
    /// How many groups have had their voice made ahead of time.
    ///
    /// Groups arrive while the agent is still writing, so this is a count
    /// rather than a flag: each new group is sent to be made as it lands.
    ahead: usize,
    /// The pane name under the reader's pointer in the prose, if any.
    name_hovered: Option<SharedString>,
    /// The pane name the reader clicked in the prose, which stays lit.
    name_pinned: Option<SharedString>,
    /// Until when the agent holds the reader's attention on a pane.
    ///
    /// Pushed on by every show and every moment of speech, and let go a few
    /// seconds after both stop. The frame round a pane and the lit lines in it
    /// last exactly this long: they used to stay until something replaced
    /// them, which in practice was for ever.
    attending: Option<std::time::Instant>,
    /// Which panes were brought in to answer a question, rather than authored.
    ///
    /// They are ordinary panes in every other way — folded, pointed at,
    /// commented on — and they go when the reader turns to another group,
    /// because they belong to the question and not to the deck.
    temporary: std::collections::HashSet<usize>,
    /// How folded each pane is, by pane index.
    ///
    /// A pane folds to a spine on its own side of the row, the way the
    /// conversation folds to one on the right. Kept here rather than on the
    /// pane because it is a thing about the room, not about the file: what may
    /// fold depends on how many panes are open beside it.
    folds: Vec<crate::pane::Fade>,
    /// Whether the live rail is open, or folded down to its spine.
    ///
    /// Folded when live starts. The code is what the reader came for; the
    /// conversation is a click away.
    rail_open: crate::pane::Fade,
    /// The sentence being heard, and how far its light has come up.
    heard_now: Option<(crate::speech::Narration, (usize, usize), crate::pane::Fade)>,
    /// The sentence just heard, its light going out.
    heard_was: Option<(crate::speech::Narration, (usize, usize), crate::pane::Fade)>,
    conversation: crate::conversation::Conversation,
    /// How wide the reader has dragged the rail, if they have.
    ///
    /// Theirs once they touch it, and kept across hide and reopen with the
    /// other things they decided about this deck.
    rail_width: Option<f32>,
    rail_scroll: ScrollHandle,
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
    /// The loop applying live commands for as long as this view is visible.
    ///
    /// Separate from group tailing because sealing ends authored input, not the
    /// conversation. Hiding drops this task while the session owner remains.
    controlling: Task<()>,
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
    /// The flow being stepped through, while one is.
    ///
    /// Held so that starting a second flow drops the first: two paths lighting
    /// at once would be two answers to a question that has one.
    stepping: Task<()>,
    /// How fast a flow travels.
    ///
    /// A reader meeting a picture for the first time and one checking a path
    /// they already know want different speeds, and neither is wrong.
    pace: Pace,
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
    /// Where the reader has scrolled the narration to.
    ///
    /// A `say` is as long as the agent needed it to be and the band is as tall
    /// as the window can spare, so the two disagree often. What used to happen
    /// when they did was that the prose stopped mid-sentence at the bottom
    /// edge, with the rest of it unreachable.
    ///
    /// View state rather than something the session carries: it is where this
    /// reader has got to in this window, and a deck put away and brought back
    /// should start at the top of its prose.
    band_scroll: ScrollHandle,
    /// The voice, and whatever it is currently reading.
    ///
    /// Held by the window rather than the app so that closing a deck takes its
    /// voice with it — a reader who shut the window and kept hearing it would
    /// have no way left to make it stop.
    voice: crate::speech::Voice,
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

/// How fast a flow travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    /// For a picture being met for the first time.
    Slow,
    /// The default.
    Normal,
    /// For a path already known, being checked.
    Fast,
}

impl Pace {
    /// Seconds per step.
    fn per_step(self) -> f32 {
        match self {
            Self::Slow => 1.15,
            Self::Normal => 0.62,
            Self::Fast => 0.34,
        }
    }

    /// What the button says.
    pub fn label(self) -> &'static str {
        match self {
            Self::Slow => "slow",
            Self::Normal => "normal",
            Self::Fast => "fast",
        }
    }

    /// All three, in the order they are shown: slowest to quickest.
    pub const ALL: [Self; 3] = [Self::Slow, Self::Normal, Self::Fast];
}

/// Slow at both ends, quick through the middle.
///
/// A current that started at full speed reads as a jump, and one that stopped
/// dead reads as a dropped frame.
fn ease(along: f32) -> f32 {
    if along < 0.5 {
        2. * along * along
    } else {
        let back = -2. * along + 2.;
        1. - back * back / 2.
    }
}

impl DeckView {
    /// Open a deck, where it was left — which for a deck nobody has opened
    /// yet is the beginning of it.
    pub fn resume(session: Session, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dark = cx.theme().mode.is_dark();
        let palette = current(dark);
        // Before any pane is built: the editors read their ground and their
        // syntax colours from the global, so it has to be ours by then.
        crate::palette::install(&palette, dark, cx);
        let Session {
            deck,
            live,
            layout,
            snapshots,
            remarks,
            conversation,
            reading,
            picked_said,
            band_offset,
            rail_width,
            rail_scroll,
            picked_reply,
            walking,
            draft,
            group_ix,
            turn,
            band_height,
            shares,
            widths,
            folded,
        } = session;

        live.ready();
        let mut view = Self {
            deck,
            live,
            snapshots,
            palette,
            grid: layout,
            panes: Vec::new(),
            group_ix,
            focus: cx.focus_handle(),
            remarks,
            composing: None,
            composing_kind: deck_core::Kind::default(),
            composing_when: deck_core::When::default(),
            aloud,
            talking_task: None,
            followed: None,
            ahead: 0,
            name_hovered: None,
            name_pinned: None,
            heard_now: None,
            heard_was: None,
            attending: None,
            folds: Vec::new(),
            temporary: std::collections::HashSet::new(),
            rail_open: crate::pane::Fade::default(),
            rail_width,
            conversation,
            rail_scroll,
            picked_said,
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
            stepping: Task::ready(()),
            pace: Pace::Normal,
            sizing: None,
            band_height,
            band_scroll: ScrollHandle::new(),
            voice: crate::speech::Voice::default(),
            folded,
            drifting: None,
            gliding: Task::ready(()),
            tailing: Task::ready(()),
            controlling: Task::ready(()),
            spread: false,
            spreading: Task::ready(()),
        };
        if let Some((about, state, kind, when)) = draft {
            view.live.pause(PauseReason::Composer);
            let listen = Self::listen_composer(&state, window, cx);
            view.composing = Some((about, state, listen));
            view.composing_kind = kind;
            view.composing_when = when;
        }
        view.build_panes(cx);
        if let Some(stage) = view.live.stage()
            && view.group().is_some_and(|group| group.id == stage.group)
            && let Some(range) = stage.range
        {
            for pane in &mut view.panes {
                if let Some(code) = pane.code_mut()
                    && stage.ref_id.as_deref() == Some(code.ref_id.as_ref())
                {
                    code.spotlight(range);
                    code.show_range();
                }
            }
        }
        view.picked_said = picked_said;
        view.band_scroll.set_offset(band_offset);
        for place in reading {
            for pane in &mut view.panes {
                if let Some(code) = pane.code_mut()
                    && code.ref_id == place.ref_id
                {
                    code.selected = place.selected;
                    let scroll = code.scroll();
                    let mut scroll = scroll.0.borrow_mut();
                    scroll.deferred_scroll_to_item = None;
                    scroll.base_handle.set_offset(place.offset);
                }
            }
        }
        if let Some(asked) = view.conversation.asked_at {
            view.watch_answer(asked, cx);
        }
        view.tail(cx);
        view.control(cx);
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

    /// Apply live control independently of authored group tailing.
    ///
    /// Sealing stops new group discovery but must not close the conversation,
    /// so this loop has its own lifetime and runs only while the view is open.
    fn control(&mut self, cx: &mut Context<Self>) {
        use futures::StreamExt;

        let mut commands = self.live.subscribe();
        self.controlling = cx.spawn(async move |view, cx| {
            while commands.next().await.is_some() {
                if view
                    .update(cx, |deck, cx| deck.apply_live_show(cx))
                    .is_err()
                {
                    return;
                }
            }
        });
    }

    /// Put the agent's own words into the walk, and read them if asked.
    ///
    /// Recorded whether or not anybody heard them: a reader with the voice off,
    /// or no voice at all, still gets the sentence in the rail and in the
    /// transcript, which is where the conversation actually lives.
    fn said_live(&mut self, text: &str, aloud: bool, cx: &mut Context<Self>) {
        // The beats are for the ear. The agent writes one copy and the panel
        // showed it verbatim, so the reader watched "[pause]" scroll past in
        // the middle of a sentence — which is the one thing beats exist not to
        // do.
        let seen = crate::prose::unbeat(text);
        let anchor = self.pinned().map(|about| {
            Self::remark(
                about,
                String::new(),
                deck_core::Kind::default(),
                deck_core::When::Queue,
            )
        });
        if let Some(anchor) = anchor.as_ref() {
            self.note(
                deck_core::What::Said,
                None,
                deck_core::When::Queue,
                &seen,
                anchor,
            );
        }
        // The author answered, so nothing is owed.
        self.conversation.asked_at = None;
        if aloud {
            let speech = crate::speech::asked(cx);
            if speech.aloud {
                let said = crate::prose::spoken(text, speech.pause);
                self.voice.say(&said, &speech);
                self.keep_talking(cx);
            }
        }
        cx.notify();
    }

    fn apply_live_show(&mut self, cx: &mut Context<Self>) {
        let Some(command) = self.live.next_show() else {
            return;
        };
        if command.expired() {
            command.finish(crate::live::ShowAnswer::refused(
                deck_cli::live::ResponseStatus::NotFound,
                "the show request expired before it could be applied",
            ));
            return;
        }
        // Saying something does not move the reader, so it is not paced, not
        // refused by a pause, and not subject to a stage at all. It lands
        // beside whatever is on screen and goes into the walk.
        if let Some((text, aloud)) = command.saying() {
            let text = text.to_string();
            self.said_live(&text, aloud, cx);
            command.finish(crate::live::ShowAnswer::said());
            return;
        }
        let resolved = match self.resolve_show(&command) {
            Ok(resolved) => resolved,
            Err(answer) => {
                command.finish(answer);
                return;
            }
        };
        let mut stage = resolved.stage.clone();
        let effects = match self
            .live
            .apply_stage(stage.clone(), resolved.source_matches)
        {
            Ok(effects) => effects,
            Err(error) => {
                command.finish(crate::live::refusal(error));
                return;
            }
        };

        let mut changed = false;
        for effect in effects {
            match effect {
                LiveEffect::StageApplied(_) => changed = true,
                LiveEffect::StageUnchanged(id) => stage.id = id,
                _ => {}
            }
        }

        // The reader may have visited another group and rebuilt its panes
        // since this stage was applied. Reducer equality alone cannot prove
        // that the current view still contains the acknowledged evidence.
        changed |= self.group_ix != resolved.group_ix
            || self
                .panes
                .get(resolved.pane_ix)
                .and_then(Sheet::code)
                .is_none_or(|code| Some(code.spotlight_range()) != stage.range);
        if changed {
            if self.group_ix != resolved.group_ix {
                self.group_ix = resolved.group_ix;
                self.build_panes(cx);
            }
            let Some(code) = self
                .panes
                .get_mut(resolved.pane_ix)
                .and_then(Sheet::code_mut)
            else {
                command.finish(crate::live::ShowAnswer::refused(
                    deck_cli::live::ResponseStatus::NotFound,
                    "the resolved code pane is no longer present",
                ));
                return;
            };
            let Some(range) = stage.range else {
                return;
            };
            code.spotlight(range);
            code.show_range();
            // What the reader was shown is the claim the transcript makes, and
            // it was the one thing the transcript did not record: it held what
            // the agent said and what the reader pressed, and nothing about
            // where either of them was looking. A move is a moment.
            //
            // Kept out of the published stream on purpose. That stream is what
            // the *reader* did, and an agent being told about the move it just
            // asked for is an echo it would have to learn to ignore.
            self.conversation.transcript.push(deck_core::Moment {
                at_ms: u64::try_from(self.conversation.began.elapsed().as_millis())
                    .unwrap_or(u64::MAX),
                what: deck_core::What::Shown,
                group: Some(stage.group.clone()),
                ref_id: stage.ref_id.clone(),
                file: stage.file.clone(),
                range: stage.range,
                text: String::new(),
                kind: None,
                when: deck_core::When::Queue,
            });
            cx.notify();
        }
        command.finish(crate::live::ShowAnswer::shown(stage, changed));
    }

    fn resolve_show(
        &self,
        command: &crate::live::ShowCommand,
    ) -> Result<ResolvedShow, crate::live::ShowAnswer> {
        let Some((asked_file, range, asked_group, asked_pane)) = command.target() else {
            return Err(crate::live::ShowAnswer::refused(
                deck_cli::live::ResponseStatus::NotFound,
                "the request has no show target",
            ));
        };
        let group_ix = match asked_group {
            Some(id) => self
                .deck
                .groups()
                .iter()
                .position(|group| group.id == id)
                .ok_or_else(|| {
                    crate::live::ShowAnswer::refused(
                        deck_cli::live::ResponseStatus::NotFound,
                        format!("group `{id}` is not readable in this deck"),
                    )
                })?,
            None => self.group_ix,
        };
        let Some(group) = self.deck.groups().get(group_ix) else {
            return Err(crate::live::ShowAnswer::refused(
                deck_cli::live::ResponseStatus::NotFound,
                "the deck has no readable group",
            ));
        };

        let base = self.deck.base();
        let wanted = resolved_path(&base, asked_file);
        let candidates: Vec<(usize, &deck_core::protocol::RefSpec)> = group
            .refs
            .iter()
            .enumerate()
            .filter_map(|(ix, reference)| {
                let Ref::Code(code) = reference else {
                    return None;
                };
                (resolved_path(&base, &code.file) == wanted
                    && asked_pane.is_none_or(|pane| code.id == pane))
                .then_some((ix, code))
            })
            .collect();

        let (pane_ix, code) = match candidates.as_slice() {
            [] => {
                return Err(crate::live::ShowAnswer::refused(
                    deck_cli::live::ResponseStatus::NotFound,
                    "that file is not represented by a code pane in the selected group",
                ));
            }
            [one] => *one,
            _ => {
                return Err(crate::live::ShowAnswer::refused(
                    deck_cli::live::ResponseStatus::Ambiguous,
                    "more than one pane shows that file; name one with --pane",
                ));
            }
        };

        let snapshot = self.snapshots.get(&code.file).cloned().or_else(|| {
            std::fs::read_to_string(&wanted)
                .ok()
                .map(std::sync::Arc::<str>::from)
        });
        let Some(snapshot) = snapshot else {
            return Err(crate::live::ShowAnswer::refused(
                deck_cli::live::ResponseStatus::NotFound,
                format!("cannot read `{}`", wanted.display()),
            ));
        };
        let line_count = u32::try_from(snapshot.lines().count()).unwrap_or(u32::MAX);
        if line_count == 0 || range.last > line_count {
            return Err(crate::live::ShowAnswer::refused(
                deck_cli::live::ResponseStatus::OutOfRange,
                format!("the displayed snapshot has {line_count} lines, not {range}"),
            ));
        }
        let quote = snapshot
            .lines()
            .skip(range.first.saturating_sub(1) as usize)
            .take(range.len() as usize)
            .collect::<Vec<_>>()
            .join("\n");
        let current = std::fs::read_to_string(&wanted).ok();
        let source_matches = current.as_deref() == Some(snapshot.as_ref());

        Ok(ResolvedShow {
            group_ix,
            pane_ix,
            stage: Stage {
                id: StageId(command.id().to_string()),
                group: group.id.clone(),
                ref_id: Some(code.id.clone()),
                file: Some(code.file.clone()),
                range: Some(range),
                quote,
                snapshot: Some(snapshot_identity(&snapshot)),
            },
            source_matches,
        })
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
        self.live.pause(PauseReason::Navigation);
        self.group_ix = next;
        self.build_panes(cx);
        // Live means being walked through it. `build_panes` has just stopped
        // the voice mid-sentence because the group it belonged to left the
        // screen, so the new one picks up where the reader now is.
        if self.walking.is_some_and(|walking| !walking.going) {
            self.speak(cx);
        }
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
    /// Go live, or come back out.
    ///
    /// Live is not a second window and not a different deck — it is the same
    /// deck with the room rearranged around it. The narration comes forward,
    /// the rail carrying what you have said slides in beside it, the panes make
    /// space, and the voice picks up the current group. Pressing `l` again puts
    /// everything back where it was.
    ///
    /// One key for both directions, because there is only ever one thing the
    /// reader can want from it.
    fn on_walk(&mut self, _: &Walk, _window: &mut Window, cx: &mut Context<Self>) {
        self.aloud = !self.aloud;
        if self.aloud {
            // From the top of the group in front of them. A selection made
            // while reading was off was the reader pointing; once a voice is
            // reading, the voice is what points.
            self.picked_said = None;
            self.said_from = None;
            self.speak(cx);
        } else {
            self.voice.hush();
            self.flush_held();
            self.rest(cx);
        }
        cx.notify();
    }

    /// Hand the narration to the voice, a sentence's worth at a time.
    ///
    /// Cut wherever the agent moved its finger, so each piece carries the lines
    /// it is about. That is what keeps the light and the words together: the
    /// agent writes its commands as fast as it can, and a reader hears them one
    /// at a time.
    ///
    /// Runs with the voice off as well, because the points are worth having
    /// without one — then the pieces are walked at reading pace rather than
    /// spoken, and nothing is heard.
    fn narrate(
        &mut self,
        text: &str,
        of: Option<crate::speech::Narration>,
        speech: &deck_core::config::Speech,
        cx: &mut Context<Self>,
    ) {
        let said = crate::prose::pointed(text, speech.pause);
        // Nothing to carry and nobody to carry it to. A silent walk of prose
        // that never points would only keep a timer alive to change nothing.
        if !speech.aloud && !said.iter().any(|piece| piece.point.is_some()) {
            return;
        }
        // One passage, not a sentence at a time. Queued piece by piece, the
        // voice went quiet at every point while the next piece was fetched.
        self.voice.say(said, of, speech);
        self.attend();
        self.point_at(self.voice.pointing(), cx);
        self.keep_talking(cx);
    }

    /// Make the voice for groups the reader has not reached yet.
    ///
    /// From the render, because a render is what happens once the reader has
    /// the deck open: a deck still sitting on the bar is not worth paying for
    /// a voice nobody may hear. The group on screen first, then the ones after
    /// it, because that is the way a walk goes.
    fn look_ahead(&mut self, cx: &mut Context<Self>) {
        let groups = self.deck.groups();
        if groups.len() <= self.ahead {
            return;
        }
        let speech = crate::speech::asked(cx);
        let fresh = self.ahead..groups.len();
        let mut order: Vec<usize> = fresh.clone().filter(|ix| *ix >= self.group_ix).collect();
        order.extend(fresh.filter(|ix| *ix < self.group_ix));
        let passages = order
            .into_iter()
            .map(|ix| crate::prose::pointed(&groups[ix].say, speech.pause))
            .collect();
        self.ahead = groups.len();
        if speech.aloud {
            crate::speech::preload(passages, &speech);
        }
    }

    /// Follow the sentence being heard. Says whether it moved on.
    fn listen(&mut self) -> bool {
        let now = self.voice.hearing().and_then(|(of, word)| {
            let text = match of {
                crate::speech::Narration::Group(ix) => self.deck.groups().get(ix)?.say.as_str(),
                crate::speech::Narration::Answer(ix) => {
                    self.conversation.transcript.get(ix)?.text.as_str()
                }
            };
            Some((of, crate::prose::sentence_around(text, word)?))
        });
        if now == self.heard_now.map(|(of, range, _)| (of, range)) {
            return false;
        }
        // The last sentence goes out from wherever its light had got to.
        if let Some((of, range, mut light)) = self.heard_now.take() {
            light.set(false);
            self.heard_was = Some((of, range, light));
        }
        self.heard_now = now.map(|(of, range)| {
            let mut light = crate::pane::Fade::default();
            light.set(true);
            (of, range, light)
        });
        true
    }

    /// The sentences of `of` being heard, with how lit each one is.
    fn heard_in(&self, of: crate::speech::Narration) -> Vec<((usize, usize), f32)> {
        [self.heard_now, self.heard_was]
            .into_iter()
            .flatten()
            .filter(|(whose, _, _)| *whose == of)
            .map(|(_, range, light)| (range, light.level()))
            .filter(|(_, level)| *level > 0.)
            .collect()
    }

    /// Fold what the agent asked to fold, by name or the whole group.
    ///
    /// Silent about names it does not know: an agent folding `retry` in a group
    /// that has no `retry` has made a mistake about the room, and the room is
    /// not the place to argue about it.
    fn apply_fold(&mut self, names: &[String], group: bool, open: bool, cx: &mut Context<Self>) {
        let wanted: Vec<usize> = if group {
            // Everything the deck itself put there. What was brought in to
            // answer a question stays: folding it away with the group would
            // leave the window with nothing in it at all.
            (0..self.panes.len())
                .filter(|ix| !self.temporary.contains(ix))
                .collect()
        } else {
            names
                .iter()
                .filter_map(|name| {
                    self.panes.iter().position(|pane| {
                        pane.code()
                            .is_some_and(|code| code.name.as_deref() == Some(name.as_str()))
                    })
                })
                .collect()
        };
        for ix in wanted {
            if let Some(fold) = self.folds.get_mut(ix) {
                fold.set(!open);
            }
        }
        cx.notify();
    }

    /// Bring a file the group never showed into the room.
    fn apply_bring(&mut self, brought: Brought, cx: &mut Context<Self>) -> crate::live::ShowAnswer {
        let Brought {
            file,
            range,
            name,
            note,
            after,
            fold_group,
            fold,
        } = brought;
        let base = self.deck.base();
        let source = self
            .snapshots
            .entry(file.clone())
            .or_insert_with(|| std::sync::Arc::from(read_source(&base, &file)))
            .clone();
        if source.is_empty() {
            return crate::live::ShowAnswer::refused(
                deck_cli::live::ResponseStatus::NotFound,
                "there is nothing at that path to bring in",
            );
        }
        // A pane of its own, with an id nothing in the deck can collide with.
        let spec = deck_core::protocol::RefSpec {
            id: format!("brought-{}", self.panes.len()),
            file,
            range,
            note,
            name,
            after,
        };
        let pane = Pane::new(&spec, &source, cx);
        // What it displaces folds in the same movement, so the room moves once.
        self.apply_fold(&fold, fold_group, false, cx);
        self.panes.push(Sheet::Code(pane));
        self.folds.push(crate::pane::Fade::default());
        self.temporary.insert(self.panes.len() - 1);
        self.attend();
        self.keep_talking(cx);
        crate::live::ShowAnswer::said()
    }

    /// Close a pane that was brought in to answer a question.
    ///
    /// The group comes back with it: whatever was folded to make the room is
    /// opened again, because the room was only ever borrowed.
    pub fn close_brought(&mut self, ix: usize, cx: &mut Context<Self>) {
        if !self.temporary.remove(&ix) {
            return;
        }
        // Whatever the question borrowed, the group has back.
        self.panes.remove(ix);
        if ix < self.folds.len() {
            self.folds.remove(ix);
        }
        // Indices above it have all moved down by one.
        self.temporary = self
            .temporary
            .iter()
            .map(|at| if *at > ix { at - 1 } else { *at })
            .collect();
        if self.temporary.is_empty() {
            for fold in &mut self.folds {
                fold.set(false);
            }
        }
        cx.notify();
    }

    /// Fold a pane down to its spine, or open it again.
    ///
    /// Folding needs somewhere for the room to go: with one pane open there is
    /// nothing to give the width to, and a deck folded to nothing but spines is
    /// a window showing no code at all. Opening is always allowed.
    pub fn fold_pane(&mut self, ix: usize, away: bool, cx: &mut Context<Self>) {
        // Folding something borrowed is closing it. It was brought in to answer
        // a question, and a spine for it would sit among the deck's own panes
        // claiming to be one of them.
        if away && self.temporary.contains(&ix) {
            self.close_brought(ix, cx);
            return;
        }
        let open = self
            .folds
            .iter()
            .enumerate()
            .filter(|(at, fold)| *at != ix && !fold.on())
            .count();
        if away && open == 0 {
            return;
        }
        if let Some(fold) = self.folds.get_mut(ix) {
            fold.set(away);
            cx.notify();
        }
    }

    /// The reader pointed at a pane's name in the prose.
    ///
    /// Hovering lights the pane for as long as the pointer stays; clicking keeps
    /// it lit until clicked again, for a reader who wants to read the pane with
    /// the pointer somewhere else.
    fn on_name(
        &mut self,
        name: SharedString,
        naming: crate::prose::Naming,
        cx: &mut Context<Self>,
    ) {
        use crate::prose::Naming;
        match naming {
            Naming::Enter => self.name_hovered = Some(name),
            Naming::Leave => {
                if self.name_hovered.as_ref() == Some(&name) {
                    self.name_hovered = None;
                }
            }
            Naming::Click => {
                self.name_pinned = if self.name_pinned.as_ref() == Some(&name) {
                    None
                } else {
                    Some(name)
                };
            }
        }
        cx.notify();
    }

    /// Outline the pane the agent is talking about, and only that one.
    ///
    /// The pane it is pointing into, when it is pointing. Otherwise the pane it
    /// last moved the spotlight in. And only while live: a reader going through
    /// a deck alone is not being talked to about anything.
    ///
    /// Worked out from state every frame rather than set at each place that
    /// state changes. There are five of those, and a light left on by the one
    /// that was forgotten is worse than no light.
    fn heed(&mut self) {
        let pointed = self
            .panes
            .iter()
            .position(|pane| pane.code().is_some_and(|code| code.pointed().is_some()));
        let staged = self
            .live
            .stage()
            .filter(|stage| self.group().is_some_and(|group| group.id == stage.group));
        let shown = staged.and_then(|stage| {
            self.panes.iter().position(|pane| {
                pane.code()
                    .is_some_and(|code| stage.ref_id.as_deref() == Some(code.ref_id.as_ref()))
            })
        });
        // The reader asking beats the agent pointing. They put the pointer on a
        // name to find its pane, and showing them some other pane because a
        // sentence moved on would answer a question they did not ask.
        let asked = self
            .name_hovered
            .as_ref()
            .or(self.name_pinned.as_ref())
            .and_then(|name| {
                self.panes.iter().position(|pane| {
                    pane.code()
                        .is_some_and(|code| code.name.as_ref() == Some(name))
                })
            });
        let attending = self
            .attending
            .is_some_and(|until| std::time::Instant::now() < until);
        let about = asked.or(pointed.or(shown).filter(|_| attending));
        for (ix, pane) in self.panes.iter_mut().enumerate() {
            if let Some(code) = pane.code_mut() {
                code.heed(Some(ix) == about);
            }
        }
    }

    /// Light the lines the narration is pointing at, and say whether that moved.
    ///
    /// The pane is chosen by the lines themselves: a point belongs to whichever
    /// pane is already showing them. Picking the first pane instead would put
    /// the finger on a coincidence whenever a group shows two files.
    ///
    /// Lines no pane is showing light nothing. There is no honest place to put
    /// that finger, and guessing one means that moving the spotlight mid-walk
    /// leaves the old point burning somewhere it no longer belongs.
    fn point_at(&mut self, at: Option<LineRange>, cx: &mut Context<Self>) -> bool {
        let owner = at.and_then(|at| {
            self.panes.iter().position(|pane| {
                pane.code().is_some_and(|code| {
                    let lit = code.spotlight_range();
                    lit.first <= at.last && at.first <= lit.last
                })
            })
        });
        // The light is about to land in a pane that is folded away, so the
        // pane comes back. Lighting lines nobody can see is the same as
        // lighting nothing, and a walk that points into a spine has stopped
        // being a walk.
        if let Some(owner) = owner
            && let Some(fold) = self.folds.get_mut(owner)
        {
            fold.set(false);
        }
        let mut moved = false;
        for (ix, pane) in self.panes.iter_mut().enumerate() {
            let Some(code) = pane.code_mut() else {
                continue;
            };
            let want = if Some(ix) == owner { at } else { None };
            if code.pointed() != want {
                code.point_at(want);
                moved = true;
            }
        }
        // Follow the finger down the file. A point on lines scrolled out of
        // sight — below a tall range, or somewhere the reader wandered from —
        // lit nothing anybody could see. Not while the reader is holding the
        // pane still: their scroll is theirs until it lapses.
        if moved
            && !self.live.reader_holds()
            && let Some(owner) = owner
            && let Some(target) = self.panes[owner].code().and_then(Pane::point_offset)
        {
            self.glide(owner, target, cx);
        }
        moved
    }

    /// Keep the voice moving without repainting to do it.
    ///
    /// One utterance ends and the next begins; that is all this is watching
    /// for, and it changes every few seconds rather than every frame. Driving
    /// it from the paint meant redrawing every code pane sixty times a second
    /// to ask whether a child process had exited yet.
    fn keep_talking(&mut self, cx: &mut Context<Self>) {
        const EVERY: std::time::Duration = std::time::Duration::from_millis(120);

        if self.talking_task.is_some() {
            return;
        }
        self.talking_task = Some(cx.spawn(async move |deck, cx| {
            loop {
                cx.background_executor().timer(EVERY).await;
                let more = deck.update(cx, |deck, cx| {
                    let speech = crate::speech::asked(cx);
                    let was = deck.voice.talking();
                    // A queued remark takes the gap before another utterance
                    // starts, rather than depending on a lucky render between them.
                    let delivered = !was && deck.conversation.queued();
                    if !was {
                        deck.flush_held();
                    }
                    deck.voice.pump(&speech);
                    let now = deck.voice.talking();
                    if was != now || delivered {
                        cx.notify();
                    }
                    // The inverted queue check kept an idle task alive forever,
                    // and could retire it precisely when more speech was queued.
                    deck.voice.has_work()
                });
                match more {
                    Ok(true) => {}
                    _ => {
                        let _ = deck.update(cx, |deck, _| deck.talking_task = None);
                        return;
                    }
                }
            }
        }));
    }

    /// Draw a pane's lit range as a change to `after`, or back as it was.
    ///
    /// The whole pane is built again from the authored ref, with the
    /// replacement put in — the same path an authored `--after` takes, so a
    /// proposal made in an answer reads exactly like one written into a group.
    fn propose(&mut self, pane_ix: usize, range: LineRange, after: Option<String>, cx: &mut App) {
        let base = self.deck.base();
        let after_is_new = after.is_some();
        let Some(Ref::Code(code)) = self
            .deck
            .groups()
            .get(self.group_ix)
            .and_then(|group| group.refs.get(pane_ix))
        else {
            return;
        };
        let mut spec = code.clone();
        // The range being proposed about is the one on screen, not the one the
        // group was authored with: an answer proposes about what it just showed.
        spec.range = range;
        spec.after = after;
        let source = self
            .snapshots
            .entry(spec.file.clone())
            .or_insert_with(|| std::sync::Arc::from(read_source(&base, &spec.file)))
            .clone();
        if let Some(slot) = self.panes.get_mut(pane_ix) {
            let mut pane = Pane::new(&spec, &source, cx);
            // Authored changes are part of the page from the first frame. This
            // one arrived while the reader was looking, so it arrives.
            if after_is_new {
                pane.arrive();
            }
            *slot = Sheet::Code(pane);
        }
    }

    /// Carry a pane back to the lit lines when they have gone off screen.
    ///
    /// Checked while the voice is going rather than only when the light moves.
    /// A reader who scrolls away in the middle of a sentence used to be left
    /// there: the light was already where it belonged, so nothing noticed that
    /// it had stopped being visible.
    ///
    /// Their scroll still wins while it is theirs — `reader_holds` is true for
    /// a few seconds after they touch the pane, and this waits that out.
    fn follow_point(&mut self, cx: &mut Context<Self>) {
        /// Long enough that the travel finishes before it can be asked for
        /// again, so a pane never jitters between two of them.
        const AGAIN_AFTER: std::time::Duration = std::time::Duration::from_millis(1_100);

        if !self.voice.talking() || self.live.reader_holds() {
            return;
        }
        if self.followed.is_some_and(|at| at.elapsed() < AGAIN_AFTER) {
            return;
        }
        if let Some((ix, target)) = self.point_away() {
            self.followed = Some(std::time::Instant::now());
            self.glide(ix, target, cx);
        }
    }

    /// Hold the reader's attention on the agent's pane a little longer.
    fn attend(&mut self) {
        /// How long the frame and the lit lines outlast the last word.
        const LINGER: std::time::Duration = std::time::Duration::from_millis(3_000);
        self.attending = Some(std::time::Instant::now() + LINGER);
    }

    /// Let go: the frame round the pane and the lit lines in it fade out.
    ///
    /// Says whether there was anything to let go of.
    fn rest(&mut self, cx: &mut Context<Self>) -> bool {
        let held = self.attending.take().is_some();
        self.voice.forget_point();
        let moved = self.point_at(None, cx);
        held || moved
    }

    /// Read the current group aloud, if a voice is configured to do it.
    ///
    /// The group's prose only. The code is on screen, and a voice spelling out
    /// a line of it would be reading the one thing the reader can already see.
    fn speak(&mut self, cx: &mut Context<Self>) {
        let Some(group) = self.group() else {
            return;
        };
        let speech = crate::speech::asked(cx);
        if !speech.aloud {
            return;
        }
        let said = crate::prose::spoken(&group.say, speech.pause);
        self.voice.say(&said, &speech);
        self.keep_talking(cx);
    }

    /// *Noted.* The cheapest thing a reader can say, and the most common.
    fn on_noted(&mut self, _: &Noted, _window: &mut Window, cx: &mut Context<Self>) {
        self.react(FACES[0].1, deck_core::When::Queue, FACES[0].0, cx);
    }

    /// *Wait, what?* — the one that should make an agent stop and explain.
    fn on_asked(&mut self, _: &Asked, _window: &mut Window, cx: &mut Context<Self>) {
        self.react(FACES[3].1, deck_core::When::Queue, FACES[3].0, cx);
    }

    /// *That's wrong.* Blocking, and it should read as blocking.
    fn on_wrong(&mut self, _: &Wrong, _window: &mut Window, cx: &mut Context<Self>) {
        self.react(FACES[4].1, deck_core::When::Queue, FACES[4].0, cx);
    }

    /// Turn the rest of the screen down, or back up.
    ///
    /// The deck does not change. What changes is everything that was competing
    /// with it, which is the only thing wrong with reading on a screen that
    /// also has eleven other things on it.
    fn on_follow(&mut self, _: &Follow, _window: &mut Window, cx: &mut Context<Self>) {
        self.live.follow();
        cx.notify();
    }

    fn on_zen(&mut self, _: &Zen, window: &mut Window, cx: &mut Context<Self>) {
        // The deck's own handle, so it can be put back in front afterwards.
        //
        // The shade sits at the same window level as the deck — both are
        // popups — and within a level the last window ordered front is on top.
        // The shade is opened second, so without this it would cover the very
        // thing it is there to light.
        let deck = window.window_handle();
        cx.defer(move |cx| {
            if crate::shade::toggle(deck, cx) {
                let _ = deck.update(cx, |_, window, _| window.activate_window());
            }
        });
        cx.notify();
    }

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
        self.live.pause(PauseReason::Selection);
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
        self.picked_reply = None;
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
        self.live.pause(PauseReason::Selection);
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

    /// Capture what a remark on pane `ix` is about now.
    ///
    /// No pane index escapes this method. A live stage can move while the
    /// composer is open, so resolving the pane again on save would attach the
    /// reader's words to evidence they did not comment on.
    fn about(&self, ix: usize) -> Option<About> {
        let group: SharedString = self.group()?.id.clone().into();
        match self.panes.get(ix)? {
            Sheet::Code(code) => {
                let range = code.comment_range();
                Some(About::Lines {
                    group,
                    ref_id: code.ref_id.clone(),
                    file: code.file.clone(),
                    range,
                    quote: code.lines_of(range),
                })
            }
            Sheet::Drawn(chart) => Some(About::Drawn {
                group,
                ref_id: chart.ref_id.clone(),
                quote: chart.quote(),
            }),
        }
    }

    /// Capture the narration selection before a composer can outlive it.
    fn claim_about(&self) -> Option<About> {
        if let Some(moment) = self
            .picked_reply
            .and_then(|ix| self.conversation.transcript.get(ix))
        {
            return Some(About::Claim {
                group: moment.group.clone()?.into(),
                quote: moment.text.clone(),
            });
        }
        let group = self.group()?;
        let quote = self.picked_said.map_or_else(
            || group.say.lines().next().unwrap_or_default().to_string(),
            |(from, to)| {
                let said = crate::prose::words(&group.say);
                let (a, b) = (from.min(to), from.max(to).min(said.len().saturating_sub(1)));
                said.get(a..=b)
                    .unwrap_or_default()
                    .concat()
                    .trim()
                    .to_string()
            },
        );
        Some(About::Claim {
            group: group.id.clone().into(),
            quote,
        })
    }

    /// The pointer went down on a word of the narration.
    fn start_say_pick(&mut self, at: usize, cx: &mut Context<Self>) {
        self.picked_reply = None;
        self.live.pause(PauseReason::Selection);
        self.said_from = Some(at);
        self.said_dragged = false;
        self.picked_said = Some((at, at));
        for pane in &mut self.panes {
            pane.unpick();
        }
        self.follow_said(at, cx);
        cx.notify();
    }

    /// Light the code a sentence of the narration is about.
    ///
    /// The same thing the voice does, with the reader as the clock. A voice
    /// reaches a word and the code lights; a reader puts their pointer on a
    /// word and the code lights. Which one is driving is the only difference,
    /// and the light should not care.
    fn follow_said(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(point) = self.point_of(at) else {
            return;
        };
        self.attend();
        self.point_at(Some(point), cx);
        // Carried there even though the pane is held. Pressing on a sentence
        // sets a pause — it is reader activity, and the agent must not move
        // somebody who is busy reading — and that pause was then blocking the
        // one movement the reader had just asked for. A click is a request.
        if let Some((ix, target)) = self.point_away() {
            self.followed = Some(std::time::Instant::now());
            self.glide(ix, target, cx);
        }
    }

    /// The pane whose pointed lines are off screen, and where it would have to
    /// sit for them not to be.
    fn point_away(&self) -> Option<(usize, Pixels)> {
        self.panes.iter().enumerate().find_map(|(ix, pane)| {
            let code = pane.code()?;
            code.pointed()?;
            Some((ix, code.point_offset()?))
        })
    }

    /// Where the narration is pointing at word `at`.
    ///
    /// The narration is cut into pieces at its points, and every piece knows
    /// how many words of the page it is — so the word the reader touched falls
    /// inside exactly one of them, and that piece carries the lines.
    fn point_of(&self, at: usize) -> Option<LineRange> {
        let group = self.group()?;
        let mut seen = 0;
        // The pause only changes what a voice hears, and nothing here is
        // heard. Cutting is the same either way.
        for piece in crate::prose::pointed(&group.say, 0) {
            seen += piece.words;
            if at < seen {
                return piece.point;
            }
        }
        None
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
        let about =
            if self.picked_reply.is_some() || self.picked_said.is_some() || self.panes.is_empty() {
                self.claim_about()
            } else {
                let pane = self.panes.iter().position(Sheet::is_picked).unwrap_or(0);
                self.about(pane)
            };
        if let Some(about) = about {
            self.open_composer(about, window, cx);
        }
    }

    fn open_composer(&mut self, about: About, window: &mut Window, cx: &mut Context<Self>) {
        self.live.pause(PauseReason::Composer);
        let asking = match about {
            About::Lines { .. } => "what you want to say about these lines",
            About::Drawn { .. } => "what you want to say about this",
            About::Claim { .. } => "what you want to say about this group",
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
        let listen = Self::listen_composer(&state, window, cx);

        self.composing = Some((about, state, listen));
        self.composing_kind = deck_core::Kind::default();
        self.composing_when = deck_core::When::Queue;
        cx.notify();
    }

    fn listen_composer(
        state: &Entity<TextareaState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(state, window, |deck, _, event: &InputEvent, window, cx| {
            if matches!(
                event,
                InputEvent::PressEnter {
                    secondary: true,
                    ..
                }
            ) {
                deck.save_remark(window, cx);
            }
        })
    }

    fn on_discard(&mut self, _: &Discard, window: &mut Window, cx: &mut Context<Self>) {
        self.composing = None;
        // Thrown away rather than saved, but the anchor still goes back: the
        // hold belongs to the composer, not to whether it produced anything.
        self.live.composer_closed();
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

    /// What a remark made right now would be pinned to.
    ///
    /// The picked pane, or the first one. Shared by the composer and by a bare
    /// reaction so the two can never disagree about what the reader meant.
    fn pinned(&self) -> Option<About> {
        // A reaction is about **what is being said**. It only becomes a remark
        // on code when the reader themselves chose lines.
        //
        // This used to take the picked pane and fall back to pane zero, so
        // every reaction came out pinned to whatever the agent happened to be
        // lighting — a thumbs-up at a sentence arrived as a comment on a line
        // nobody was talking about. The lit range is the agent pointing; a
        // selected one is the reader pointing, and only the second is an
        // instruction about code.
        if let Some(ix) = self.panes.iter().position(Sheet::reader_chose) {
            return self.about(ix);
        }
        self.claim_about()
    }

    /// One anchor, one set of words, one kind.
    fn remark(about: About, text: String, kind: deck_core::Kind, when: deck_core::When) -> Remark {
        match about {
            About::Drawn {
                group,
                ref_id,
                quote,
            } => Remark {
                group,
                ref_id: Some(ref_id),
                // A diagram is in no file, so there is nothing to relocate and
                // nothing to point an editor at.
                file: None,
                range: None,
                quote,
                text,
                kind,
                when,
            },
            About::Lines {
                group,
                ref_id,
                file,
                range,
                quote,
            } => Remark {
                group,
                ref_id: Some(ref_id),
                file: Some(file),
                range: Some(range),
                quote,
                text,
                kind,
                when,
            },
            About::Claim { group, quote } => Remark {
                group,
                ref_id: None,
                file: None,
                range: None,
                quote,
                text,
                kind,
                when,
            },
        }
    }

    /// Record one moment of the walk, against a remark's own anchor.
    ///
    /// Written at the instant the thing happens rather than reconstructed at
    /// submit time, because the anchor is the point: a reaction three groups
    /// ago was about what was in front of the reader *then*.
    fn note(
        &mut self,
        what: deck_core::What,
        kind: Option<deck_core::Kind>,
        when: deck_core::When,
        text: &str,
        of: &Remark,
    ) {
        let moment = deck_core::Moment {
            at_ms: u64::try_from(self.conversation.began.elapsed().as_millis()).unwrap_or(u64::MAX),
            what,
            group: Some(of.group.to_string()),
            ref_id: of.ref_id.as_ref().map(ToString::to_string),
            file: of.file.clone(),
            range: of.range,
            text: text.to_string(),
            kind,
            when,
        };
        // The review keeps it either way. What `when` decides is the floor: a
        // reader who took it is heard at once and the voice stops; a reader who
        // waited is heard in the next gap.
        // Follow new turns only while the reader is at the tail. Reading an
        // older answer must not be interrupted by the next one arriving.
        if -self.rail_scroll.offset().y >= self.rail_scroll.max_offset().y - px(24.) {
            self.rail_scroll.scroll_to_bottom();
        }
        if when == deck_core::When::Interrupt {
            self.voice.hush();
        }
        for ready in self.conversation.record(moment, self.voice.talking()) {
            self.live.publish(ready);
        }
    }

    fn expect_answer(&mut self, cx: &mut Context<Self>) {
        let asked = std::time::Instant::now();
        self.conversation.asked_at = Some(asked);
        self.watch_answer(asked, cx);
    }

    fn watch_answer(&self, asked: std::time::Instant, cx: &mut Context<Self>) {
        let remaining = PATIENCE.saturating_sub(asked.elapsed());
        // A status label needs one deadline, not sixty full-window redraws a
        // second. An older deadline cannot expire a newer question.
        cx.spawn(async move |deck, cx| {
            cx.background_executor().timer(remaining).await;
            let _ = deck.update(cx, |deck, cx| {
                if deck.conversation.asked_at == Some(asked) {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn flush_held(&mut self) {
        for moment in self.conversation.release() {
            self.live.publish(moment);
        }
    }

    /// One keystroke, one reaction, pinned to what is on screen.
    ///
    /// The thing [`deck_core::Kind`] was written for and never had: *"Terse
    /// remarks read as neutral, and an agent left to infer tone from prose gets
    /// it wrong. Naming it costs the reader one keystroke."* A reaction is a
    /// remark with no words — the kind *is* the message — so it costs nothing
    /// to leave one, and a walk ends up dense with exactly where the reader
    /// agreed and where they did not.
    ///
    /// While a composer is open the same keys set the kind of the remark being
    /// written instead, because there the reader already has words.
    fn react(
        &mut self,
        kind: deck_core::Kind,
        when: deck_core::When,
        face: &str,
        cx: &mut Context<Self>,
    ) {
        if self.composing.is_some() {
            self.composing_kind = kind;
            cx.notify();
            return;
        }
        let Some(about) = self.pinned() else {
            return;
        };
        // The face is the message. Two faces can ask the agent for the same
        // thing and still not mean the same thing, so the one the reader
        // pressed travels with the remark rather than being flattened away.
        let remark = Self::remark(about, face.to_string(), kind, when);
        if kind != deck_core::Kind::Nit {
            self.expect_answer(cx);
        }
        self.note(deck_core::What::Reacted, Some(kind), when, face, &remark);
        self.remarks.push(remark);
        cx.notify();
    }

    fn save_remark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((about, state, _listen)) = self.composing.take() else {
            return;
        };
        // The anchor goes back the moment the composer does. It is held so that
        // nothing moves under somebody who is writing, and that hold does not
        // lapse on its own — so failing to say this once left the agent unable
        // to move anything for the rest of the session.
        self.live.composer_closed();
        let said = state.read(cx).value().trim().to_string();
        let kind = self.composing_kind;
        self.focus.focus(window, cx);

        if said.is_empty() {
            cx.notify();
            return;
        }
        let remark = Self::remark(about, said, kind, self.composing_when);
        let said = remark.text.clone();
        // Written words expect an answer, whichever way they waited for the
        // floor. Without this the panel said nothing back and a reader who had
        // just typed a question could not tell it had been sent.
        self.expect_answer(cx);
        self.note(
            deck_core::What::Wrote,
            Some(kind),
            remark.when,
            &said,
            &remark,
        );
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
            live: self.live.clone(),
            layout: self.grid,
            snapshots: self.snapshots.clone(),
            remarks: self.remarks.clone(),
            // A hidden window is replaced, not retained. Omitting these reset
            // the conversation and erased it from the eventual review too.
            conversation: self.conversation.clone(),
            reading: self
                .panes
                .iter()
                .filter_map(Sheet::code)
                .map(|code| ReadingPlace {
                    ref_id: code.ref_id.clone(),
                    selected: code.selected,
                    offset: code.scroll().0.borrow().base_handle.offset(),
                })
                .collect(),
            picked_said: self.picked_said,
            band_offset: self.band_scroll.offset(),
            rail_width: self.rail_width,
            rail_scroll: self.rail_scroll.clone(),
            picked_reply: self.picked_reply,
            walking: self.walking.is_some_and(|walk| !walk.going),
            draft: self.composing.as_ref().map(|(about, state, _)| {
                (
                    about.clone(),
                    state.clone(),
                    self.composing_kind,
                    self.composing_when,
                )
            }),
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
        // The lights come up first. A shade is drawn over every display and
        // the deck is the only thing above it, so a shade that outlived the
        // window it was dimming for would be a near-black screen with nothing
        // on it to press.
        crate::shade::lights_on(cx);
        self.voice.hush();
        self.flush_held();
        self.live.hidden();
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
        crate::shade::lights_on(cx);
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
                    kind: remark.kind,
                    when: remark.when,
                    quote: remark.quote.clone(),
                    text: remark.text.clone(),
                }
            })
            .collect();

        let review = deck_core::Review {
            v: deck_core::VERSION,
            // A failed write must leave the conversation intact for retry.
            transcript: self.conversation.transcript.clone(),
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
        // A new group starts at the top of its own narration. Carrying the
        // last one's scroll over means arriving halfway down a paragraph that
        // has not been read.
        self.band_scroll.set_offset(point(px(0.), px(0.)));
        // And stops talking, because what it is saying belongs to the group
        // that just left the screen.
        self.voice.hush();
        self.flush_held();
        self.picked_said = None;

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
                    .min_h_0()
                    .overflow_hidden()
                    .gap(px(9.))
                    .pb(px(14.))
                    // The horizontal and top padding belong to the children
                    // rather than to this column, so that the titlebar can
                    // include the gap above the title. Put here, that gap is a
                    // strip of band between the window's edge and the only
                    // thing that answers a double-click — which is exactly
                    // where somebody aiming at a titlebar clicks.
                    .child(
                        // The title stays. It is what the deck is *for*, and
                        // scrolling it away to read the middle of a long
                        // narration loses the one line that says what the
                        // narration is about.
                        // It is also the deck's titlebar, so it behaves like
                        // one: double-click zooms the window. The real
                        // titlebar is there but made invisible and its traffic
                        // lights are pushed off-screen, so a double-click
                        // where a titlebar would be landed on the band and did
                        // nothing — while every other window on the machine
                        // zooms.
                        div()
                            .id("deck-titlebar")
                            .h_flex()
                            .flex_none()
                            .items_baseline()
                            .gap(px(14.))
                            .pt(px(15.))
                            .pl(px(18.))
                            .pr(px(18.))
                            .on_click(|event, window, _| {
                                if event.click_count() >= 2 {
                                    window.zoom_window();
                                }
                            })
                            .child(
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
                            .pl(px(18.))
                            .pr(px(18.))
                            // Scrolls, because a `say` is as long as the agent
                            // needed it to be and the band is as tall as the
                            // window can spare. When the two disagreed the
                            // prose stopped mid-sentence at the bottom edge,
                            // and the rest of it could not be reached at all —
                            // dragging the seam is a way to make the band
                            // bigger, not a way to read past the end of it.
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.band_scroll)
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
    /// Widen or narrow the live rail.
    ///
    /// Dragging left makes it wider, which is why the delta is subtracted: the
    /// seam is on the rail's left edge and the rail grows toward the pointer.
    fn resize_rail(&mut self, by: Pixels, cx: &mut Context<Self>) {
        const LEAST: f32 = 150.;
        const MOST: f32 = 560.;

        let from = self.rail_width.unwrap_or(RAIL);
        self.rail_width = Some((from - f32::from(by)).clamp(LEAST, MOST));
        cx.notify();
    }

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
            self.panning = Some((pane_ix, at, chart.nudge));
        }
    }

    /// Carry the drawing to where the pointer has got to.
    ///
    /// Moved on the drawing rather than through the scroll container, which
    /// re-clamps its own offset to the overflow every frame: a picture that
    /// fitted its pane could not be moved at all, and one that did not could
    /// only be pulled two of the four ways. A hand that has taken hold of
    /// something expects it to come, so this simply carries it and holds it
    /// within reach of where it was.
    fn pan_to(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((pane_ix, from, was)) = self.panning else {
            return;
        };
        let Some(chart) = self.panes.get_mut(pane_ix).and_then(Sheet::chart_mut) else {
            return;
        };

        /// How far a drawing can be carried from where it was laid out.
        ///
        /// Generous, and the same in every direction. Its only job is to stop
        /// a picture being flung somewhere it cannot be found again.
        const REACH: f32 = 2000.;

        let held = |value: Pixels| value.clamp(px(-REACH), px(REACH));
        chart.nudge = point(held(was.x + (at.x - from.x)), held(was.y + (at.y - from.y)));
        cx.notify();
    }

    /// Take a diagram in or out.
    ///
    /// `by` is the wheel's travel in pixels, turned into a ratio: a wheel
    /// notch is a proportion of what is there, not a fixed number of pixels,
    /// or every step would be enormous when zoomed out and imperceptible when
    /// zoomed in.
    pub fn zoom_chart(&mut self, pane_ix: usize, by: f32, cx: &mut Context<Self>) {
        /// How much of a turn one pixel of wheel is worth.
        const PER_PIXEL: f32 = 0.004;

        if let Some(chart) = self.panes.get_mut(pane_ix).and_then(Sheet::chart_mut) {
            chart.zoom_by(1. + by * PER_PIXEL);
            cx.notify();
        }
    }

    /// Put a diagram back to the size it was drawn, and where.
    pub fn reset_zoom(&mut self, pane_ix: usize, cx: &mut Context<Self>) {
        if let Some(chart) = self.panes.get_mut(pane_ix).and_then(Sheet::chart_mut) {
            chart.zoom = 1.;
            // The position goes back with it. A picture put back to its own
            // size but left half off the pane has not been put back.
            chart.nudge = point(px(0.), px(0.));
            cx.notify();
        }
    }

    /// Play a flow through the diagram in `pane_ix`, or stop the one playing.
    ///
    /// Clicking the flow that is already running stops it, because the button
    /// is the only thing on screen that could — and a picture stuck part-way
    /// through a path it will not finish is worse than one at rest.
    pub fn play_flow(&mut self, pane_ix: usize, flow: usize, cx: &mut Context<Self>) {
        let Some(chart) = self.panes.get_mut(pane_ix).and_then(Sheet::chart_mut) else {
            return;
        };
        if chart.playing.is_some_and(|playing| playing.flow == flow) {
            chart.playing = None;
            self.stepping = Task::ready(());
            cx.notify();
            return;
        }

        let steps = chart.steps(flow);
        if steps == 0 {
            return;
        }
        chart.playing = Some(crate::chart::Playing { flow, front: 0. });
        cx.notify();

        // A current, not a slideshow.
        //
        // The front moves continuously and every frame is drawn from where it
        // has got to, so a box lights *through* rather than lighting up: at
        // 2.4 the third box is 40 percent lit and the arrow into it is 40
        // percent across. Stepping whole numbers on a timer was the same
        // information delivered as a flick-book, and it read like one.
        const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

        let over = self.pace.per_step() * steps as f32;
        self.stepping = cx.spawn(async move |deck, cx| {
            let began = std::time::Instant::now();
            loop {
                cx.background_executor().timer(FRAME).await;
                let along = (began.elapsed().as_secs_f32() / over).min(1.);
                // Eased at both ends: a current that started at full speed
                // would read as a jump, and one that stopped dead would read
                // as a frame dropped at the end.
                // Up to `steps`, not `steps - 1`. A node at index i is fully
                // lit once the front reaches i + 1, so a front that stopped
                // one short left the last node of every path dark — the one
                // the whole path was walked to arrive at.
                let front = ease(along) * steps as f32;

                let carried = deck.update(cx, |deck, cx| {
                    let Some(chart) = deck.panes.get_mut(pane_ix).and_then(Sheet::chart_mut) else {
                        return false;
                    };
                    // Somebody stopped it, or started another one.
                    if !chart.playing.is_some_and(|playing| playing.flow == flow) {
                        return false;
                    }
                    chart.playing = Some(crate::chart::Playing { flow, front });
                    cx.notify();
                    true
                });
                if !matches!(carried, Ok(true)) || along >= 1. {
                    return;
                }
            }
        });
    }

    /// Change the speed, and restart whatever is playing at the new one.
    pub fn set_pace(&mut self, pane_ix: usize, pace: Pace, cx: &mut Context<Self>) {
        self.pace = pace;
        // Restarted rather than adjusted part-way. The speed is being chosen
        // by somebody watching, and the way to see what you chose is to see it
        // from the beginning.
        let playing = self
            .panes
            .get(pane_ix)
            .and_then(Sheet::chart)
            .and_then(|chart| chart.playing);
        if let Some(playing) = playing {
            if let Some(chart) = self.panes.get_mut(pane_ix).and_then(Sheet::chart_mut) {
                chart.playing = None;
            }
            self.play_flow(pane_ix, playing.flow, cx);
        }
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
            Divide::Rail => ("seam-rail".into(), true),
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
                    deck.sizing = Some((what, what.along(event.position)));
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
        self.live.pause(PauseReason::Navigation);
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
        let Some(target) = self
            .panes
            .get(pane_ix)
            .and_then(Sheet::code)
            .and_then(Pane::focus_offset)
        else {
            return;
        };
        self.glide(pane_ix, target, cx);
    }

    /// Carry a pane's list to `target` over a few frames.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn glide(&mut self, pane_ix: usize, target: Pixels, cx: &mut Context<Self>) {
        /// A frame, near enough. Shorter than this and the timer is doing more
        /// work than the screen can show.
        const FRAME: std::time::Duration = std::time::Duration::from_millis(8);
        /// The shortest a move takes, however near it is.
        const AT_LEAST: f32 = 420.;
        /// And the longest, however far.
        const AT_MOST: f32 = 950.;
        /// Milliseconds per pixel between the two.
        const PER_PIXEL: f32 = 0.7;

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
            About::Lines { file, range, .. } => {
                format!("comment on {}:{range}", file.display())
            }
            About::Drawn { quote, .. } => format!("comment on {quote}"),
            About::Claim { .. } => "comment on this group".to_string(),
        };
        let ref_id = Some(match about {
            About::Lines { ref_id, .. } | About::Drawn { ref_id, .. } => ref_id.clone(),
            About::Claim { group, .. } => group.clone(),
        });

        Some(
            div()
                .v_flex()
                .flex_none()
                .key_context("DeckComposer")
                // Sized for whichever it is in. The panel can be two hundred
                // points wide; window padding inside it leaves no room to type.
                .map(|this| {
                    if self.walking.is_some_and(|walking| !walking.going) {
                        this.pt(px(9.)).pb(px(2.))
                    } else {
                        this.pl(px(16.))
                            .pr(px(16.))
                            .pt(px(13.))
                            .pb(px(14.))
                            .border_t_1()
                            .border_color(paint(self.palette.edge))
                    }
                })
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
                        .flex_wrap()
                        .justify_between()
                        .gap(px(8.))
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(10.5))
                        .text_color(paint(self.palette.muted))
                        .child(where_at)
                        .children(ref_id),
                )
                // Sized against the narration it answers, not against the
                // labels around it. What the reader types here is prose, and
                // it was set smaller than every other piece of prose in the
                // window — which read as a footnote to the deck rather than as
                // the half of it that is theirs.
                .child(div().text_size(px(13.2)).line_height(px(20.)).child(
                    Textarea::new(state).h(px(
                        if self.walking.is_some_and(|walking| !walking.going) {
                            56.
                        } else {
                            72.
                        },
                    )),
                ))
                // Under the box, where a hint belongs: beside the location it
                // competes with the one thing the reader needs to read.
                .child(
                    div()
                        .h_flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_size(px(10.))
                                .text_color(paint(self.palette.muted))
                                .h_flex()
                                .gap(px(14.))
                                .child("⌘⏎ save")
                                .child("esc discard"),
                        )
                        // Only while somebody is listening. Outside a live walk
                        // there is nobody to interrupt, so offering the choice
                        // would be a control that does nothing.
                        .children(self.render_urgency(cx)),
                ),
        )
    }

    /// Whether this remark can wait, offered only when it can matter.
    ///
    /// Queued is the default and the common case: the author reads it when the
    /// review comes back. Interrupting is the reader taking the floor — it
    /// wakes the agent out of `deck wait` with this one question, which is the
    /// whole of the back-and-forth.
    fn render_urgency(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.walking.is_none_or(|walking| walking.going) {
            return None;
        }
        let palette = &self.palette;
        let picked = self.composing_when;

        Some(
            div()
                .h_flex()
                .flex_none()
                .gap(px(3.))
                .children(
                    [
                        (
                            0usize,
                            deck_core::When::Queue,
                            "wait for a gap",
                            palette.muted,
                        ),
                        (
                            1usize,
                            deck_core::When::Interrupt,
                            "interrupt",
                            palette.accent,
                        ),
                    ]
                    .into_iter()
                    .map(move |(ix, when, label, tone)| {
                        let on = when == picked;
                        div()
                            .id(("urgency", ix))
                            .px(px(9.))
                            .py(px(4.))
                            .rounded(px(999.))
                            .cursor_pointer()
                            .text_size(px(10.5))
                            .text_color(paint(if on { palette.on_accent } else { palette.muted }))
                            .when(on, |this| this.bg(paint(tone)))
                            .hover(|style| style.bg(paint(palette.wash)))
                            .on_click(cx.listener(move |deck, _, _window, cx| {
                                deck.composing_when = when;
                                cx.notify();
                            }))
                            .child(label)
                    }),
                )
                .into_any_element(),
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

impl DeckView {
    /// How live the window currently is, nought to one.
    ///
    /// Also retires a transition that has finished leaving, so `walking` is
    /// `None` again and the next `l` is an arrival rather than a reversal.
    fn live_pace(&mut self) -> f32 {
        match self.walking {
            Some(walking) if walking.spent() => {
                self.walking = None;
                0.
            }
            Some(walking) => walking.pace(),
            None => 0.,
        }
    }

    /// The rail: everything you have said, in the order you said it.
    ///
    /// Slides in rather than appearing, and carries its own width so the panes
    /// beside it are squeezed by the same number in the same frame. A rail that
    /// popped into place would make the reader find their place again.
    ///
    /// Newest last, because a walk reads forwards.
    fn render_rail(&self, pace: f32, speaking: bool, cx: &mut Context<Self>) -> AnyElement {
        let wide = self.rail_width.unwrap_or(RAIL);
        let palette = &self.palette;
        let mono = cx.theme().mono_font_family.clone();
        let following = matches!(self.live.following(), deck_core::Following::Following);

        // One timeline, in the order things happened.
        //
        // This was two lists — everything the agent said, then everything the
        // reader said — so a question asked early appeared below an answer
        // given late and the conversation read backwards. The transcript is
        // already in order, so it *is* the rail; keeping a second list beside
        // it was what let the two disagree.
        let lines: Vec<AnyElement> = self
            .conversation
            .transcript
            .iter()
            .enumerate()
            .filter(|(_, moment)| {
                matches!(
                    moment.what,
                    deck_core::What::Said
                        | deck_core::What::Reacted
                        | deck_core::What::Wrote
                        | deck_core::What::Shown
                )
            })
            .map(|(ix, moment)| {
                if moment.what == deck_core::What::Shown {
                    let target = moment.file.as_ref().map_or_else(
                        || moment.group.clone().unwrap_or_default(),
                        |file| {
                            format!(
                                "{}{}",
                                file.display(),
                                moment
                                    .range
                                    .map_or_else(String::new, |range| format!(":{range}"))
                            )
                        },
                    );
                    return div()
                        .flex_none()
                        .px(px(8.))
                        .py(px(7.))
                        .font_family(mono.clone())
                        .text_size(px(9.5))
                        .text_color(paint(palette.muted))
                        .child(SharedString::from(format!("Showing {target}")))
                        .into_any_element();
                }
                let mine = moment.what == deck_core::What::Said;
                let reacted = moment.what == deck_core::What::Reacted;
                let tone = match moment.kind {
                    Some(deck_core::Kind::MustFix) => palette.del,
                    Some(deck_core::Kind::Question) => palette.accent,
                    _ => palette.muted,
                };
                div()
                    .h_flex()
                    .items_start()
                    .gap(px(7.))
                    .px(px(8.))
                    .py(px(9.))
                    .rounded(px(5.))
                    .when(self.picked_reply == Some(ix), |this| {
                        this.bg(paint(palette.wash))
                    })
                    .child(
                        div()
                            .flex_none()
                            .w(px(9.))
                            .text_size(px(10.))
                            .text_color(paint(tone))
                            // The agent's own lines carry no mark. Only what the
                            // reader put in needs pointing at.
                            .child(if mine || reacted { "" } else { "\u{2022}" }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            // A reaction was a face and wanted to be large. It
                            // is a word now, and a word set larger than the
                            // sentence beside it reads as shouting.
                            .v_flex()
                            .gap(px(4.))
                            .text_size(px(13.))
                            .when(reacted, |this| this.font_family(mono.clone()))
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_size(px(9.5))
                                    .text_color(paint(palette.muted))
                                    .child(SharedString::from(format!(
                                        "{} · {}{}",
                                        if mine { "agent" } else { "you" },
                                        moment.group.as_deref().unwrap_or("walk"),
                                        if moment.when == deck_core::When::Interrupt {
                                            " · interrupted"
                                        } else {
                                            ""
                                        },
                                    ))),
                            )
                            .text_color(paint(if reacted { tone } else { palette.fg }))
                            .child(SharedString::from(moment.text.clone())),
                    )
                    .id(("line", ix))
                    .cursor_pointer()
                    .hover(|style| style.bg(paint(palette.wash)))
                    .on_click(cx.listener(move |deck, _, _window, cx| {
                        if deck.composing.is_some() {
                            return;
                        }
                        deck.picked_reply = Some(ix);
                        deck.picked_said = None;
                        for pane in &mut deck.panes {
                            pane.unpick();
                        }
                        deck.live.pause(PauseReason::Selection);
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();

        div()
            .id("live-rail")
            // Clicking into the panel lets the code go. A selection is the
            // reader pointing at lines, and once they have turned to the
            // conversation they are not pointing at them any more — leaving the
            // highlight behind means the next reaction attaches to something
            // they stopped looking at minutes ago.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|deck, _, _window, cx| {
                    if deck.panes.iter().any(Sheet::reader_chose) {
                        for pane in &mut deck.panes {
                            pane.unpick();
                        }
                        cx.notify();
                    }
                }),
            )
            .flex_none()
            .w(px(wide * pace))
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .border_l_1()
            .border_color(paint(palette.edge))
            .bg(paint(palette.band))
            // Fades a little behind the slide, so it reads as arriving rather
            // than as the panes merely getting narrower.
            .opacity(pace.powi(2))
            .child(
                div()
                    .v_flex()
                    .w(px(wide))
                    .h_full()
                    .px(px(14.))
                    .pt(px(13.))
                    .pb(px(10.))
                    .gap(px(2.))
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .items_center()
                            .pb(px(9.))
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_size(px(9.5))
                                    .text_color(paint(palette.muted))
                                    .child("LIVE"),
                            )
                            .child(
                                div()
                                    .font_family(mono)
                                    .text_size(px(9.5))
                                    .text_color(paint(if following {
                                        palette.muted
                                    } else {
                                        palette.accent
                                    }))
                                    // The thing that was missing entirely: a
                                    // reader could pause movement by clicking
                                    // and had no way to know they had.
                                    .id("resume-following")
                                    .cursor_pointer()
                                    .on_click(cx.listener(|deck, _, _window, cx| {
                                        deck.live.follow();
                                        cx.notify();
                                    }))
                                    .child(if !following {
                                        "resume · f"
                                    } else if speaking {
                                        "voice active"
                                    } else {
                                        "following"
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .id("said-rail")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.rail_scroll)
                            .children(lines),
                    )
                    .child(
                        div()
                            .id("latest-reply")
                            .flex_none()
                            .py(px(5.))
                            .text_size(px(10.))
                            .text_color(paint(palette.muted))
                            .cursor_pointer()
                            .on_click(cx.listener(|deck, _, _window, cx| {
                                deck.picked_reply = None;
                                deck.rail_scroll.scroll_to_bottom();
                                cx.notify();
                            }))
                            .child("Latest ↓ · select a turn to reply"),
                    )
                    .children(self.render_pending())
                    // The composer belongs here while live, not floating over
                    // the code. A reader writing a reply is talking to the
                    // panel, and sending them to a box on top of the file they
                    // are reading pulls their eye off the thing they are
                    // replying about.
                    .children(self.render_composer(cx))
                    .child(self.render_reactions(cx)),
            )
            .into_any_element()
    }
}

impl DeckView {
    /// Distinguish feedback waiting behind speech from feedback awaiting an
    /// answer. Neither state proves that an agent is connected or thinking.
    fn render_pending(&self) -> Option<AnyElement> {
        let since = self.conversation.asked_at?.elapsed();
        let palette = &self.palette;
        // Still held means the reader waited for a gap and the gap has not come.
        // Saying so is better than "thinking", which would be a lie about who
        // is holding things up.
        let (dots, tone) = if self.conversation.queued() {
            ("waiting for a gap", palette.muted)
        } else if since >= PATIENCE {
            ("no answer yet", palette.muted)
        } else {
            // Publication is not proof that an agent is thinking, or even
            // connected. Do not turn transport uncertainty into fake presence.
            ("sent · waiting for agent", palette.accent)
        };

        Some(
            div()
                .flex_none()
                .pt(px(7.))
                .text_size(px(10.5))
                .text_color(paint(tone))
                .child(SharedString::from(dots))
                .into_any_element(),
        )
    }

    /// The reactions, under a rule at the foot of the panel.
    ///
    /// Words, not faces. Every other surface in this window is one ink and one
    /// accent; seven colour glyphs along the bottom fought all of it, and no
    /// arrangement of them fixed that — the problem was never which emoji.
    ///
    /// A word also says exactly what it means. `bug` is `bug`; a beetle is a
    /// beetle until the reader decides what you meant by it.
    fn render_reactions(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = &self.palette;
        let mono = cx.theme().mono_font_family.clone();

        div()
            .h_flex()
            .flex_wrap()
            .flex_none()
            .items_center()
            .gap(px(2.))
            .mt(px(9.))
            .pt(px(8.))
            .border_t_1()
            .border_color(paint(palette.edge))
            .children(FACES.iter().enumerate().map(|(ix, &(mark, kind))| {
                let tone = match kind {
                    deck_core::Kind::MustFix => palette.del,
                    deck_core::Kind::Question => palette.accent,
                    deck_core::Kind::Nit => palette.muted,
                };
                div()
                    .id(("react", ix))
                    .flex_none()
                    .px(px(6.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .font_family(mono.clone())
                    .text_size(px(10.5))
                    .text_color(paint(tone))
                    .cursor_pointer()
                    .hover(|style| style.bg(paint(palette.wash)))
                    .on_click(cx.listener(move |deck, _, _window, cx| {
                        let when = if mark == "stop" {
                            deck_core::When::Interrupt
                        } else {
                            deck_core::When::Queue
                        };
                        deck.react(kind, when, mark, cx);
                    }))
                    .child(mark)
            }))
            .into_any_element()
    }
}

/// The reactions, and what each one asks the agent to do.
///
/// In one place so the row and the transcript cannot drift apart: the mark is
/// the remark's whole text, so what the reader pressed is what the agent reads.
///
/// Words rather than faces, and short ones. They sit in a panel that can be
/// dragged narrow, they are coloured by what they ask for, and each says
/// exactly one thing — which a picture of a beetle does not.
const FACES: &[(&str, deck_core::Kind)] = &[
    ("+1", deck_core::Kind::Nit),
    ("nice", deck_core::Kind::Nit),
    ("looking", deck_core::Kind::Question),
    ("?", deck_core::Kind::Question),
    ("careful", deck_core::Kind::MustFix),
    ("bug", deck_core::Kind::MustFix),
    ("stop", deck_core::Kind::MustFix),
];

/// Report a prolonged wait without pretending to know why the agent is quiet.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(25);

/// Room for a readable reply without turning the evidence into a thumbnail.
const RAIL: f32 = 304.;

impl Render for DeckView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Built up front rather than inside `.children()`: rendering a pane
        // needs the context, and a closure holding it cannot also be handed to
        // the element it is building.
        // Rows call back into the view when clicked, and a weak handle is what
        // a closure that outlives this frame is allowed to hold.
        let me = cx.entity().downgrade();

        // Live is a time-based transition, so the window has to keep asking for
        // frames until it settles. Driven here rather than from a spawned timer
        // because the pace is already a function of the clock: one place decides
        // how live the room is, and everything below reads it.
        let live = self.live_pace();
        let speaking = self.voice.talking();
        // Only while something is genuinely moving.
        //
        // This used to ask for a frame whenever an answer was owed or a voice
        // was going — which meant the whole window, every code pane included,
        // repainted sixty times a second for as long as the reader waited. It
        // felt exactly as heavy as it was. The dots stop moving once the
        // indicator gives up, and the voice is pumped by a timer instead.
        if self.walking.is_some() && (live > 0.) && (live < 1.) {
            window.request_animation_frame();
        }

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
            // While live, a remark lives in the panel and nowhere else. Drawn
            // on the code as well it is the same sentence twice, and the copy
            // over the file covers the very lines being discussed — which is
            // the reason the composer moved out of there in the first place.
            let marks: Vec<Mark> = if live > 0. {
                Vec::new()
            } else {
                self.remarks
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
                    .collect()
            };
            let slot = Slot {
                pace: self.pace,
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
            .on_action(cx.listener(Self::on_zen))
            .on_action(cx.listener(Self::on_live))
            .on_action(cx.listener(Self::on_noted))
            .on_action(cx.listener(Self::on_asked))
            .on_action(cx.listener(Self::on_wrong))
            .on_action(cx.listener(Self::on_follow))
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
                    let along = what.along(event.position);
                    let by = along - from;
                    match what {
                        Divide::Band => deck.resize_band(by, cx),
                        Divide::Rail => deck.resize_rail(by, cx),
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
                //
                // In live mode the rail stands beside them and takes its width
                // out of theirs, so the two move as one thing rather than the
                // rail landing on top of the code.
                div()
                    .h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        // `h_full` is not decoration. Beside the rail these
                        // panes sit in a row, and a column in a row takes its
                        // width from the flex and its height from nothing at
                        // all — it collapsed and the code vanished, leaving the
                        // rail (which does say `h_full`) as the only thing with
                        // any height. The old warning one level up says the
                        // same thing about wrappers; this is that warning
                        // arriving a second time.
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .min_h_0()
                            .children(rows),
                    )
                    .when(live > 0., |this| {
                        this.child(self.render_seam(Divide::Rail, cx))
                            .child(self.render_rail(live, speaking, cx))
                    })
                    .into_any_element()
            })
            // Only when there is no panel to hold it. Live gives the
            // composer a home beside the conversation it belongs to.
            .when(live <= 0., |this| this.children(self.render_composer(cx)))
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

    #[test]
    fn a_live_rail_drag_uses_horizontal_motion_only() {
        // The rail divides left from right, like a column seam, and the match
        // that decided which axis to read only named columns. So a leftward
        // drag moved nothing and the panel jumped the moment the pointer
        // drifted up or down — which is exactly what it looked like.
        let from = point(px(800.), px(300.));
        let sideways = point(px(750.), px(300.));
        let down = point(px(800.), px(450.));
        assert_eq!(
            Divide::Rail.along(sideways) - Divide::Rail.along(from),
            px(-50.),
            "a horizontal drag moves it"
        );
        assert_eq!(
            Divide::Rail.along(down) - Divide::Rail.along(from),
            px(0.),
            "and a vertical one does not"
        );
    }

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
