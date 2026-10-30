//! The narration, rendered.
//!
//! [`deck_core::prose`] reads a group's `say` into words, marks and sentences;
//! this draws them. The library's markdown view was tried and it renders a
//! *document*: its own spacing, its own idea of a code chip, its own leading.
//! Reaching into that to match the deck's measurements is more work than
//! drawing the four marks that actually appear, and the result still is not
//! ours.
//!
//! So each word becomes an element of its own. Wrapping, selection and
//! measurement come from GPUI rather than from a flex box full of word
//! fragments.

use deck_core::theme::Palette;
use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

// The reading, under the names the rest of the window has always called it by.
pub use deck_core::prose::{
    Mark, Paragraph, Said, Side, Spot, Token, parse, pointed, sentence_around, sentence_starts,
    split_trailing_space, unbeat, unname, unpoint, words,
};

use crate::palette::paint;

/// Told that the pointer went down on a word.
pub type Down = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Told that the pointer entered or left a word.
pub type Over = std::rc::Rc<dyn Fn(usize, bool, &mut Window, &mut App)>;

/// What the pointer did to a pane's name in the prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Naming {
    /// It came onto the name.
    Enter,
    /// It went off the name.
    Leave,
    /// It clicked the name.
    Click,
}

/// Told that the pointer did something to a pane's name.
pub type Name = std::rc::Rc<dyn Fn(SharedString, Naming, &mut Window, &mut App)>;

/// How the narration answers the pointer.
pub struct Picking {
    /// The words currently selected, as an inclusive range.
    pub range: Option<(usize, usize)>,
    /// The pointer went down on this word.
    pub down: Down,
    /// The pointer entered or left this word.
    pub over: Over,
    /// The names of the panes on screen. A bracketed word that is not one of
    /// them is drawn as the text it is.
    pub names: Vec<SharedString>,
    /// The pane name the reader has lit, if any.
    pub named: Option<SharedString>,
    /// The pointer did something to a pane's name.
    pub name: Name,
    /// The sentences being heard, as word ranges, each with how lit it is:
    /// the one coming up, and any still going out.
    pub heard: Vec<((usize, usize), f32)>,
    /// The paragraphs being heard, as word spans, each with how lit it is.
    ///
    /// What the voice dimming answers to, rather than `heard`. A sentence's
    /// light rises over half a second, so a short one never reached full
    /// before the voice moved on, and the paragraph it was in stepped back a
    /// third of the way and came forward again on every short sentence. Keyed
    /// by paragraph, a voice moving from one sentence to the next in the same
    /// paragraph leaves the light exactly where it was.
    pub heard_paragraphs: Vec<((usize, usize), f32)>,
    /// How much the voice is reading this prose at all, nought to one.
    ///
    /// One light, on for as long as the voice is hearing anything here, so
    /// the paragraphs it is not in step back once and stay back until it
    /// stops. Summing the sentences' lights did the same only while each
    /// sentence reached full before the next began.
    pub voiced: f32,
    /// Whether the words can be picked at all. The rail's record cannot, and a
    /// pointer hand over words that do nothing is a promise it breaks.
    pub pickable: bool,
    /// How far up the lit range has come, nought to one.
    ///
    /// A level rather than the boolean `range` alone, so a sentence the reader
    /// pressed arrives the way the sentence a voice is reading does. One of
    /// them easing in while the other snaps is two different windows.
    pub shown: f32,
    /// Word ranges somebody has already said something about.
    ///
    /// A comment on the narration used to leave the narration exactly as it
    /// was, so the only evidence it existed was a line in the rail — and if the
    /// rail was folded, none at all. A reader could write one and have no way
    /// of telling whether it had landed.
    pub remarked: Vec<(usize, usize)>,
    /// Sentences lit from somewhere else — the walk's track, under the
    /// pointer — each with how far up its light has come. Painted exactly as a
    /// pressed sentence is, because it is the same promise: *this one*.
    pub lit_also: Vec<((usize, usize), f32)>,
    /// The margin ticks the picked sentence has lit, as word spans, each
    /// with how far up its light has come: see [`Tick`].
    ///
    /// Their own light rather than `shown`, because the tick answers to the
    /// paragraph and the bar to the sentence. Empty for prose nobody keeps a
    /// [`Tick`] for, whose tick then simply follows `shown`.
    pub ticks: Vec<((usize, usize), f32)>,
}

impl Picking {
    /// Give a quiet piece of prose a hand: pressable words, and a lit range.
    ///
    /// The rail's turns are read rather than picked from, so they start quiet —
    /// but an agent's turn is *about* code, and pressing a sentence of it is
    /// the same act as pressing a sentence of the narration. Without this the
    /// reply that explained something was the one place in the window where
    /// putting a finger on a sentence did nothing at all.
    #[must_use]
    pub fn pressable(mut self, lit: Option<(usize, usize)>, shown: f32, down: Down) -> Self {
        self.pickable = true;
        self.range = lit;
        self.shown = shown;
        self.down = down;
        self
    }

    /// How the voice is reading this prose: see [`Picking::voiced`] and
    /// [`Picking::heard_paragraphs`].
    #[must_use]
    pub fn voiced(mut self, voiced: f32, paragraphs: Vec<((usize, usize), f32)>) -> Self {
        self.voiced = voiced;
        self.heard_paragraphs = paragraphs;
        self
    }

    /// The margin ticks a [`Tick`] has lit for this prose.
    #[must_use]
    pub fn ticked(mut self, ticks: Vec<((usize, usize), f32)>) -> Self {
        self.ticks = ticks;
        self
    }

    /// Prose that is read but not picked from: the rail's record of what was
    /// said. It still lights the sentence being heard.
    #[must_use]
    pub fn quiet(heard: Vec<((usize, usize), f32)>) -> Self {
        Self {
            range: None,
            down: std::rc::Rc::new(|_, _, _| {}),
            over: std::rc::Rc::new(|_, _, _, _| {}),
            names: Vec::new(),
            named: None,
            name: std::rc::Rc::new(|_, _, _, _| {}),
            heard,
            heard_paragraphs: Vec::new(),
            voiced: 0.,
            pickable: false,
            shown: 1.,
            remarked: Vec::new(),
            lit_also: Vec::new(),
            ticks: Vec::new(),
        }
    }
}

/// Render paragraphs into the band.
///
/// A wrapping row of tokens rather than one styled string.
///
/// The string was tried first and could not carry a code chip: a highlight run
/// sets colour and weight, but not a font family, a padding or a corner — so
/// `render.close()` came out in the prose face, square, and as tall as the
/// whole line. The chip is the point of the mark, so the mark has to be an
/// element that can have one.
#[must_use]
pub fn render(
    paragraphs: Vec<Paragraph>,
    palette: &Palette,
    mono: SharedString,
    picking: &Picking,
) -> impl IntoElement {
    render_look(paragraphs, palette, mono, picking, Look::band(palette))
}

/// How a stretch of prose is set.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// Text size.
    pub size: f32,
    /// Line height.
    pub leading: f32,
    /// The colour of ordinary words.
    pub tone: deck_core::theme::Rgb,
    /// The space between paragraphs.
    pub spacing: f32,
    /// Whether the first paragraph is set in the page's full ink: the claim,
    /// for a reader who drops in and reads only the title and one paragraph.
    pub lead: bool,
    /// Where the margin tick stands, from the prose's left edge: negative,
    /// out in the margin.
    ///
    /// The band has a clear margin, so the tick stands in it, clear of the
    /// lit bar's lead. The rail's prose hangs from the thread, which runs a
    /// few points to its left, and a tick between the two crowded both, so
    /// there the tick is laid on the thread itself: the thread lights beside
    /// the paragraph the reader picked from.
    pub tick: f32,
    /// What the margin tick comes up from: the colour under it at no light.
    ///
    /// The band's tick stands on the band's own ground, so it rises out of
    /// that. The rail's is laid on the thread, and rising out of the band
    /// there meant that a tick at low light painted the band over the thread
    /// and the thread went missing beside the very paragraph it was marking.
    /// Rising out of the thread's own colour, the tick at its lowest is the
    /// thread, and it lights from there. That holds on a turn that was just
    /// brought, too: its tinted ground is under the turn, but the thread is
    /// drawn in the same colour on every turn, so the tick still starts as
    /// the thread it covers.
    pub tick_from: deck_core::theme::Rgb,
}

impl Look {
    /// The band: the thing being read.
    #[must_use]
    pub fn band(palette: &Palette) -> Self {
        Self {
            size: deck_render::metrics::PROSE_SIZE,
            leading: deck_render::metrics::PROSE_LEADING,
            // A step off the page's ink, so the group's title is the
            // brightest line in the band and the prose reads as its body.
            tone: deck_render::reading::prose(palette.fg, palette.band),
            // A group's paragraphs are beats of one claim, so the gap is
            // modest; but it is half a line again on top of the leading, so a
            // new paragraph is never mistaken for the next line of the last.
            spacing: deck_render::metrics::PROSE_SPACING,
            lead: true,
            tick: -9.,
            tick_from: palette.band,
        }
    }

    /// The rail: a record of the conversation beside the thing being read.
    ///
    /// Smaller, and a shade off the page's own ink. Set the same as the band,
    /// a long answer in the rail reads as the subject rather than as a note
    /// about it.
    #[must_use]
    pub fn rail(palette: &Palette) -> Self {
        Self {
            size: deck_render::metrics::RAIL_SIZE,
            leading: deck_render::metrics::RAIL_LEADING,
            tone: deck_render::reading::rail(palette.fg, palette.band),
            spacing: 8.,
            lead: false,
            // The rail's prose starts 24 points into its turn and the thread
            // runs at 10, a point wide: two points centred on it.
            tick: -14.5,
            // The thread's own colour: see `tick_from`.
            tick_from: palette.edge,
        }
    }
}

/// [`render`], set the way the caller wants it.
#[must_use]
pub fn render_look(
    paragraphs: Vec<Paragraph>,
    palette: &Palette,
    mono: SharedString,
    picking: &Picking,
    look: Look,
) -> impl IntoElement {
    // While the voice is reading, the paragraphs it is not in step back, so
    // the eye stays where the voice is. Never otherwise: somebody reading on
    // their own needs the whole argument at full strength.
    //
    // By degrees, never by a switch. How far a paragraph steps back is how
    // much the voice is reading this prose at all, less how much of that is
    // in this paragraph. Both are lights that answer to something longer
    // than a sentence. The first is one light, on for as long as the voice is
    // hearing anything here; the second is one per paragraph, and a voice
    // moving between sentences of the same paragraph leaves it alone. They
    // were summed from the sentences' own lights, which stayed at full only
    // when the sentence the voice left had reached full before it left: a
    // short sentence never did, and every other paragraph brightened a third
    // of the way and dimmed again on each one.
    let heard = |para: &Paragraph| -> f32 {
        picking
            .heard_paragraphs
            .iter()
            .filter(|((from, to), _)| {
                para.tokens
                    .iter()
                    .any(|token| token.at >= *from && token.at <= *to)
            })
            .map(|(_, level)| *level)
            .sum::<f32>()
            .min(1.)
    };
    let voiced = picking.voiced;
    let rendered: Vec<AnyElement> = paragraphs
        .into_iter()
        .enumerate()
        .map(|(ix, para)| {
            let back = voiced * (1. - heard(&para));
            // The paragraph's full ink, for its stressed and emphasised words:
            // they step back with the words around them, or a paragraph the
            // voice has left would still have its loudest words shouting.
            let full = palette.fg.mix(palette.band, VOICED_BACK * back);
            // The first paragraph is set in the page's full ink for a reader
            // on their own, and comes down to the prose's tone as the voice
            // comes up, so the voice reading it is not reading the one bright
            // paragraph on the page.
            let tone = if look.lead && ix == 0 {
                palette.fg.mix(look.tone, voiced)
            } else {
                look.tone
            }
            .mix(palette.band, VOICED_BACK * back);
            let tick = tick_level(&para, picking);
            render_paragraph(
                para,
                palette,
                mono.clone(),
                picking,
                Look { tone, ..look },
                full,
                tick,
            )
            .into_any_element()
        })
        .collect();
    div().v_flex().gap(px(look.spacing)).children(rendered)
}

/// How far a paragraph the voice has left steps back toward the ground: about
/// 5:1 for a band's prose, there to be found rather than read.
const VOICED_BACK: f32 = 0.35;

/// How lit a paragraph's margin tick is.
///
/// The tick says *the sentence you are on is in here*, so it answers to the
/// paragraph, not to the sentence's bar. Moving from one sentence to the next
/// in the same paragraph lights a new bar from nothing, and a tick that
/// followed the bar went out and came back with it, flickering beside words
/// the reader had not left. So each light that touches the paragraph is
/// summed, never more than full: a [`Tick`] keeps one light for the
/// paragraph however many sentences in it are picked, and a sentence lit
/// from the walk's track hands over to the next one with the next rising as
/// the last falls, which together stay at full.
fn tick_level(para: &Paragraph, picking: &Picking) -> f32 {
    let touches = |(from, to): (usize, usize)| {
        let (from, to) = (from.min(to), from.max(to));
        para.tokens
            .iter()
            .any(|token| token.at >= from && token.at <= to)
    };
    let picked = if picking.ticks.is_empty() {
        picking
            .range
            .filter(|range| touches(*range))
            .map_or(0., |_| picking.shown)
    } else {
        picking
            .ticks
            .iter()
            .filter(|(span, _)| touches(*span))
            .map(|(_, level)| *level)
            .sum()
    };
    let also: f32 = picking
        .lit_also
        .iter()
        .filter(|(range, _)| touches(*range))
        .map(|(_, level)| *level)
        .sum();
    (picked + also).min(1.)
}

/// The words a pick spans, widened to the whole paragraphs it touches: what a
/// [`Tick`] is keyed by, so a pick that moves within a paragraph keeps its
/// tick.
#[must_use]
pub fn paragraphs_holding(
    paragraphs: &[Paragraph],
    (from, to): (usize, usize),
) -> Option<(usize, usize)> {
    let (from, to) = (from.min(to), from.max(to));
    let held: Vec<&Paragraph> = paragraphs
        .iter()
        .filter(|para| {
            para.tokens
                .iter()
                .any(|token| token.at >= from && token.at <= to)
        })
        .collect();
    let first = held.first()?.tokens.first()?.at;
    let last = held.last()?.tokens.last()?.at;
    Some((first, last))
}

/// Lights that hand over from one thing to the next: the one that is on, and
/// every one still going out.
///
/// Kept by whoever keeps what they mark, and keyed by what they mark. Told to
/// follow the same key again, nothing changes; told to follow another, the
/// light that was on starts to go out from wherever it had got to while the
/// new one comes up, so a light fades across rather than jumping; told to
/// follow nothing, the last one goes out on its own.
///
/// Every light going out is kept until it is out. Only the last one used to
/// be, and a reader who moved twice inside a fall — A, then B, then C — had
/// A's light thrown away halfway down: it vanished in a frame, which is the
/// snap every other light in the window is built not to do.
#[derive(Debug, Clone)]
pub(crate) struct Lights<K> {
    now: Option<(K, crate::pane::Fade)>,
    going: Vec<(K, crate::pane::Fade)>,
    /// A fresh light, at the pace these lights keep.
    fresh: crate::pane::Fade,
}

impl<K: Copy + PartialEq> Lights<K> {
    /// No light on, and each one that comes on rises and falls as `fresh`
    /// does.
    pub(crate) fn new(fresh: crate::pane::Fade) -> Self {
        Self {
            now: None,
            going: Vec::new(),
            fresh,
        }
    }

    /// What the light that is on marks, if one is.
    pub(crate) fn on(&self) -> Option<K> {
        self.now.as_ref().map(|(key, _)| *key)
    }

    /// Light `to`, or nothing. Says whether that was a change.
    pub(crate) fn follow(&mut self, to: Option<K>) -> bool {
        if self.on() == to {
            return false;
        }
        // Out is out: nothing is left to draw of a light that has gone all
        // the way down, so the list holds only the ones still falling.
        self.going.retain(|(_, light)| light.level() > 0.);
        // Back to something that is still going out: that light turns round
        // from wherever it had fallen to, rather than starting again from
        // nothing beside its own fading self.
        let back = to
            .and_then(|to| self.going.iter().position(|(key, _)| *key == to))
            .map(|at| self.going.remove(at).1);
        if let Some((key, mut light)) = self.now.take() {
            light.set(false);
            self.going.push((key, light));
        }
        let fresh = self.fresh;
        self.now = to.map(|key| {
            let mut light = back.unwrap_or(fresh);
            light.set(true);
            (key, light)
        });
        true
    }

    /// Every light with something left to show, and how lit it is.
    pub(crate) fn lit(&self) -> impl Iterator<Item = (K, f32)> + '_ {
        self.now
            .iter()
            .chain(&self.going)
            .map(|(key, light)| (*key, light.level()))
            .filter(|(_, level)| *level > 0.)
    }

    /// Whether any light is still on its way somewhere.
    pub(crate) fn moving(&self) -> bool {
        self.now
            .iter()
            .chain(&self.going)
            .any(|(_, light)| light.moving())
    }
}

impl<O: Copy + PartialEq> Lights<(O, (usize, usize))> {
    /// The spans lit in prose `of`, with how lit each one is: what
    /// [`Picking`] takes.
    pub(crate) fn levels(&self, of: O) -> Vec<((usize, usize), f32)> {
        self.lit()
            .filter(|((whose, _), _)| *whose == of)
            .map(|((_, span), level)| (span, level))
            .collect()
    }
}

/// The margin tick's own light, kept by whoever keeps the pick it marks.
///
/// Keyed by the prose it is in — a group, or a turn of the rail — and the
/// paragraphs the pick touches. A pick that moves to another sentence of the
/// same paragraphs leaves the light as it is; one that moves elsewhere lets
/// this light go out while the new one comes up; and letting go of the pick
/// lets it fall with the bar.
pub(crate) type Tick = Lights<(usize, (usize, usize))>;

impl Tick {
    /// A tick's lights, at the pace of a light the reader's hand raised.
    pub(crate) fn hand() -> Self {
        Self::new(
            crate::pane::Fade::default().paced(crate::pane::HAND_RISE, crate::pane::HAND_FALL),
        )
    }
}

/// One paragraph, as a row that wraps between any two of its words.
///
/// `full` is the paragraph's full ink, for its stressed and emphasised words,
/// and `tick` how lit its margin tick is.
fn render_paragraph(
    para: Paragraph,
    palette: &Palette,
    mono: SharedString,
    picking: &Picking,
    look: Look,
    full: deck_core::theme::Rgb,
    tick: f32,
) -> impl IntoElement {
    let dark = deck_render::reading::dark(palette.band);
    div()
        .relative()
        .flex()
        .flex_wrap()
        .items_center()
        // Light strokes on a dark ground look thinner than they are, so a
        // dark theme reads in Plex Sans Text, a step heavier than regular.
        .when(dark, |this| this.font_family("IBM Plex Sans Text"))
        // A tick in the margin beside a paragraph that holds the picked
        // sentence. A fill alone is lost the moment the reader looks away;
        // the tick is how they find their place again. It comes and goes
        // with its light, from what is under it up to the accent, as the bar
        // does: see `Look::tick_from`.
        .when(tick > 0., |this| {
            this.child(
                div()
                    .absolute()
                    .left(px(look.tick))
                    .top(px(2.))
                    .bottom(px(2.))
                    .w(px(2.))
                    .rounded(px(1.))
                    .bg(paint(look.tick_from.mix(palette.accent, tick))),
            )
        })
        .text_size(px(look.size))
        // A word's painted box is the line, less a point above and below
        // and the rule's pixel and its match: a lit sentence that wraps is
        // one shape with a hairline between its lines, and its words have
        // even room above and below them inside the bar.
        .line_height(px(word(look.leading)))
        .text_color(paint(look.tone))
        .children(
            para.tokens
                .into_iter()
                .map(|token| render_token(token, palette, mono.clone(), picking, look, full)),
        )
}

/// One word, answering to the pointer.
///
/// The text arrives as the core's `String` and goes on as a [`SharedString`],
/// which is what GPUI holds and what the word always was by the time it was
/// drawn.
fn render_token(
    token: Token,
    palette: &Palette,
    mono: SharedString,
    picking: &Picking,
    look: Look,
    full: deck_core::theme::Rgb,
) -> AnyElement {
    // Every word answers to the pointer: down starts a selection, and the
    // window extends it to whichever word is under the pointer while the
    // button is held — the same shape the code rows use, one element per
    // word instead of one per line.
    //
    // Words rather than characters, and that is the layout speaking. The
    // narration is a row of separate elements so a paragraph can wrap
    // between any two of them, and a range of elements is the finest thing
    // that layout can be asked about.
    let at = token.at;
    // How lit this word is as part of a picked sentence: the range says
    // which words, `shown` says how far up the light has come.
    let lit = if picking
        .range
        .is_some_and(|(from, to)| at >= from.min(to) && at <= from.max(to))
    {
        picking.shown
    } else {
        0.
    };
    // A picked sentence is one bar, not a row of pills: only its two outer
    // ends are rounded. Rounding every word notched the bar between each
    // pair of them.
    let (first, last) = picking.range.map_or((false, false), |(from, to)| {
        (at == from.min(to), at == from.max(to))
    });
    let lit = picking
        .lit_also
        .iter()
        .filter(|((from, to), _)| at >= *from && at <= *to)
        .map(|(_, level)| *level)
        .fold(lit, f32::max);
    // Being heard: a soft ground under the sentence the voice is on, which
    // rises and falls rather than jumping from one sentence to the next.
    let heard = picking
        .heard
        .iter()
        .filter(|((from, to), _)| at >= *from && at <= *to)
        .map(|(_, level)| *level)
        .fold(0., f32::max);
    // The ground the voice gives a word it is on, and the band's own where
    // it is on none.
    let heard_ground = palette.band.mix(palette.accent, 0.2 * heard);

    // A pane's name answers the pointer by lighting its pane. It is still a
    // word a selection can run across, so it keeps `over`; it does not take
    // `down`, because pressing on it means *that pane*, not *start
    // selecting here*.
    if token.mark == Some(Mark::Pane) {
        let (text, trailing) = split_trailing_space(&token.text);
        let name = SharedString::from(text.clone());
        if !picking.names.contains(&name) {
            return div()
                .child(format!("[{text}]{trailing}"))
                .into_any_element();
        }
        let on = picking.named.as_ref() == Some(&name);
        let (over, told, clicked) = (
            picking.over.clone(),
            picking.name.clone(),
            picking.name.clone(),
        );
        let (entered_name, clicked_name) = (name.clone(), name);
        return div()
            .id(("say", at))
            .flex()
            .items_center()
            .cursor_pointer()
            .on_hover(move |entered, window, cx| {
                over(at, *entered, window, cx);
                let naming = if *entered {
                    Naming::Enter
                } else {
                    Naming::Leave
                };
                told(entered_name.clone(), naming, window, cx);
            })
            .on_click(move |_, window, cx| {
                clicked(clicked_name.clone(), Naming::Click, window, cx);
            })
            .child(
                div()
                    .px(px(5.))
                    .rounded(px(crate::radius::CAP))
                    .text_color(paint(palette.accent))
                    .bg(paint(
                        palette
                            .accent
                            .mix(palette.band, if on { 0.72 } else { 0.88 }),
                    ))
                    .border_1()
                    .border_color(paint(
                        palette.accent.mix(palette.band, if on { 0.3 } else { 0.5 }),
                    ))
                    .child(text),
            )
            .child(trailing)
            .into_any_element();
    }
    let (down, over) = (picking.down.clone(), picking.over.clone());
    let pickable = picking.pickable;
    let remarked = picking
        .remarked
        .iter()
        .any(|(from, to)| at >= *from && at <= *to);
    // The bar a picked sentence is drawn on, at full light, and the ink that
    // reads on it. The prose's own tone is set a step off the page's ink so
    // the title leads; on the bar that step made the very words you picked
    // hard to read, so lit words take the page's own ink, the title's. The
    // bar is the one that gives way, stepping back toward the band until
    // that ink reads on it; the ink is never lifted, because asking for more
    // turned it white, which glared on the warm bar.
    let firm = deck_render::reading::lit_bar(palette.fg, palette.band, palette.accent);
    let ink = palette.fg;
    // The point of clear space above and below each word's box.
    let inset = 1.;
    let wrap = move |inner: AnyElement| {
        // Two boxes. The outer one is the word's whole share of its line,
        // and takes the pointer, so there is no dead strip between lines.
        // The inner one is what is painted, set in from the top and bottom.
        let bar = div()
            .when(lit > 0., |this| {
                // Seen at a glance, and quiet: see `lit_bar`. A tint you
                // could hold a whole sentence in and not notice reads as the
                // click having done nothing, and a reader who thinks nothing
                // happened does not try again; a block of the accent took
                // the words' contrast away.
                //
                // The bar starts a little before its first word, as far
                // as the space after its last word reaches past the end,
                // so the words sit inside it rather than against its edge.
                // Pulled left by as much as it is padded, so lighting a
                // sentence moves no word.
                // Up from the ground the word already had, not from the
                // band: a sentence picked while it is being heard used to
                // lose its tint in the first lit frame and start again from
                // bare band, a step down before it went up.
                this.bg(paint(heard_ground.mix(firm, lit)))
                    // The words come up with the bar, from the prose's tone
                    // to the page's ink, rather than switching halfway.
                    .text_color(paint(look.tone.mix(ink, lit)))
                    .when(first, |this| {
                        this.ml(px(-LIT_LEAD))
                            .pl(px(LIT_LEAD))
                            .rounded_l(px(crate::radius::CAP))
                    })
                    .when(last, |this| this.rounded_r(px(crate::radius::CAP)))
            })
            .when(lit <= 0. && heard > 0., |this| this.bg(paint(heard_ground)))
            // Something was said about these words. Underlined rather than
            // filled: the ground is already doing two jobs here — what is
            // selected, and what is being read aloud — and a third colour
            // competing with those is how a band of prose turns into a
            // paint chart. A rule under the words is quiet and unambiguous.
            //
            // The rule is always there and usually invisible. A border that
            // appears only when there is a comment adds a pixel to the
            // word's height the moment one lands, and the whole paragraph
            // reflows around it — the line the reader is looking at moves
            // under them as they write about it.
            // The rule's pixel, matched above it, so the words sit in the
            // middle of their ground when a sentence is lit instead of a
            // pixel high.
            .pt(px(1.))
            // Clear of the line above and the one below: a lit sentence is
            // a bar set in its line, not the whole height of it. Painted
            // edge to edge, a sentence that wraps became one block, and a
            // single line looked sliced off at the top and bottom.
            .my(px(inset))
            .border_b_1()
            .border_color(if remarked {
                paint(palette.comment)
            } else {
                // Transparent, not the band's colour.
                //
                // Painting it the band only hides it on the band, and this
                // same prose is drawn in the rail as well, on a different
                // ground — so every turn in the conversation came out with
                // a rule under it. Reserving the space costs nothing;
                // guessing what is behind it costs a line under everything.
                gpui_kit::transparent_black()
            })
            .child(inner);
        div()
            .id(("say", at))
            .when(pickable || remarked, |this| this.cursor_pointer())
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                down(at, window, cx);
            })
            .on_hover(move |entered, window, cx| over(at, *entered, window, cx))
            .child(bar)
            .into_any_element()
    };

    // A stressed or emphasised word's ink: the paragraph's full ink, and the
    // page's own on a lit bar, coming up with the bar as the words around it
    // do.
    let full = full.mix(palette.fg, lit);
    wrap(match token.mark {
        Some(Mark::Code) => {
            // The chip: mono, on a ground just off the band's (see
            // `reading::chip`), sized to its text.
            let (text, trailing) = split_trailing_space(&token.text);
            div()
                .flex()
                .items_center()
                .child(
                    div()
                        .font_family(mono)
                        // A step under the prose: see `chip_size`.
                        .text_size(px(deck_render::metrics::chip_size(look.size)))
                        // Exactly the word's box, padding and all, so a chip
                        // in a lit sentence sits inside the bar instead of
                        // standing out of it above and below.
                        .line_height(px(word(look.leading) - 2.))
                        .px(px(deck_render::reading::CHIP_PADDING))
                        .py(px(1.))
                        .rounded(px(crate::radius::CAP))
                        .bg(paint(deck_render::reading::chip(palette.band)))
                        .child(text),
                )
                .child(trailing)
                .into_any_element()
        }
        // Plex's own semibold, in the paragraph's full ink. Named, because a
        // dark theme's prose is in Plex Sans Text, which has no semibold.
        Some(Mark::Strong) => div()
            .font_family("IBM Plex Sans")
            .font_semibold()
            .text_color(paint(full))
            .child(SharedString::from(token.text))
            .into_any_element(),
        // A true italic in the paragraph's full ink. It was the accent, which
        // put orange on the line being read: orange vibrates on a dark
        // ground at reading size, and it is what deck uses for things that
        // can be pressed, so an emphasised word looked like a link.
        Some(Mark::Emphasis) => div()
            .italic()
            .text_color(paint(full))
            .child(SharedString::from(token.text))
            .into_any_element(),
        None | Some(Mark::Pane) => div()
            .child(SharedString::from(token.text))
            .into_any_element(),
    })
}

/// The height of a word's painted box in a line `leading` high: a point of
/// clear space above and below, and the rule's pixel and its match above.
fn word(leading: f32) -> f32 {
    leading - 4.
}

/// How far a lit sentence's bar reaches before its first word: about the
/// width of the space after its last, which is the bar's margin at that end.
const LIT_LEAD: f32 = 4.;

#[cfg(test)]
mod tests {
    use super::{Lights, Paragraph, Picking, paragraphs_holding, parse, tick_level};

    fn two_paragraphs() -> Vec<Paragraph> {
        parse("One two three. Four five.\n\nSix seven. Eight nine.")
    }

    #[test]
    fn a_pick_is_held_by_the_whole_paragraphs_it_touches() {
        let paragraphs = two_paragraphs();
        let first = paragraphs_holding(&paragraphs, (0, 2)).unwrap();
        // Another sentence of the same paragraph: the same span, so the tick
        // keeps its light.
        assert_eq!(paragraphs_holding(&paragraphs, (3, 4)), Some(first));
        // Across into the next: both paragraphs.
        let across = paragraphs_holding(&paragraphs, (4, 5)).unwrap();
        assert_eq!(across.0, first.0);
        assert!(across.1 > first.1);
    }

    #[test]
    fn a_tick_handing_over_inside_its_paragraph_stays_full() {
        let paragraphs = two_paragraphs();
        // A light going out and the next coming up, both in the first
        // paragraph: summed, the tick does not dip.
        let mut picking = Picking::quiet(Vec::new());
        picking.lit_also = vec![((0, 2), 0.4), ((3, 4), 0.7)];
        assert!((tick_level(&paragraphs[0], &picking) - 1.).abs() < f32::EPSILON);
        assert!(tick_level(&paragraphs[1], &picking) <= 0.);
        // A pick with no `Tick` kept for it follows its own light.
        picking.lit_also.clear();
        picking.range = Some((3, 4));
        picking.shown = 0.25;
        assert!((tick_level(&paragraphs[0], &picking) - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn every_light_going_out_is_kept_until_it_is_out() {
        // A, then B, then C, all inside one fall: A is still on its way down
        // when C comes up, and keeping only the last light going out threw
        // it away in a frame.
        let pace = std::time::Duration::from_millis(1000);
        let mut lights = Lights::new(crate::pane::Fade::default().paced(pace, pace));
        let rest = || std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(lights.follow(Some('a')));
        rest();
        assert!(lights.follow(Some('b')));
        rest();
        assert!(lights.follow(Some('c')));
        rest();
        let mut lit: Vec<char> = lights.lit().map(|(key, _)| key).collect();
        lit.sort_unstable();
        assert_eq!(lit, ['a', 'b', 'c']);
        assert_eq!(lights.on(), Some('c'));

        // The same key again changes nothing.
        assert!(!lights.follow(Some('c')));

        // Back to A turns its own light round from where it had fallen to,
        // rather than lighting a second A from nothing beside the one going
        // out.
        let before = lights
            .lit()
            .find(|(key, _)| *key == 'a')
            .map(|(_, level)| level);
        assert!(lights.follow(Some('a')));
        assert_eq!(lights.lit().filter(|(key, _)| *key == 'a').count(), 1);
        let after = lights
            .lit()
            .find(|(key, _)| *key == 'a')
            .map(|(_, level)| level);
        let (before, after) = (before.unwrap_or(0.), after.unwrap_or(0.));
        assert!((after - before).abs() < 0.01, "{after} from {before}");
    }

    #[test]
    fn a_voice_moving_inside_a_paragraph_does_not_dim_it() {
        // Keyed by paragraph, two sentences of the same one are the same key,
        // so the light the dimming answers to is never handed over.
        let paragraphs = two_paragraphs();
        let first = paragraphs_holding(&paragraphs, (0, 2));
        let second = paragraphs_holding(&paragraphs, (3, 4));
        let mut lights = Lights::new(crate::pane::Fade::default());
        assert!(lights.follow(first));
        assert!(!lights.follow(second));
    }
}
