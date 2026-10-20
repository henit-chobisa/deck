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
    /// The sentences being heard, as word ranges, each with how lit it is.
    ///
    /// Two at most: the one coming up and the one going out.
    pub heard: Vec<((usize, usize), f32)>,
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
            pickable: false,
            shown: 1.,
            remarked: Vec::new(),
            lit_also: Vec::new(),
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
}

impl Look {
    /// The band: the thing being read.
    #[must_use]
    pub fn band(palette: &Palette) -> Self {
        Self {
            size: 12.5,
            leading: 21.5,
            // A step off the page's ink, so the group's title is the
            // brightest line in the band and the prose reads as its body.
            tone: palette.fg.mix(palette.band, 0.22),
            // Close: a group's paragraphs are beats of one claim, and a wide
            // gap between them read as separate statements.
            spacing: 5.,
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
            size: 12.2,
            leading: 17.6,
            tone: palette.fg.mix(palette.muted, 0.22),
            spacing: 18.6 * 0.42,
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
    div().v_flex().gap(px(look.spacing)).children(
        paragraphs
            .into_iter()
            .map(|para| render_paragraph(para, palette, mono.clone(), picking, look)),
    )
}

/// One paragraph, as a row that wraps between any two of its words.
fn render_paragraph(
    para: Paragraph,
    palette: &Palette,
    mono: SharedString,
    picking: &Picking,
    look: Look,
) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .text_size(px(look.size))
        // Each word stands in its line with `LIT_INSET` of clear space
        // above and below it, so the line's pitch is still `leading`.
        .line_height(px(look.leading - 2. * LIT_INSET))
        .text_color(paint(look.tone))
        .children(
            para.tokens
                .into_iter()
                .map(|token| render_token(token, palette, mono.clone(), picking)),
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
                    .rounded(px(3.))
                    .text_color(paint(palette.accent))
                    .bg(paint(
                        palette
                            .accent
                            .mix(palette.band, if on { 0.72 } else { 0.88 }),
                    ))
                    .border_1()
                    .border_color(paint(
                        palette.accent.mix(palette.band, if on { 0.3 } else { 0.7 }),
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
    let wrap = move |inner: AnyElement| {
        // Two boxes. The outer one is the word's whole share of its line,
        // and takes the pointer, so there is no dead strip between lines.
        // The inner one is what is painted, set in from the top and bottom.
        let bar = div()
            .when(lit > 0., |this| {
                // Firm enough to see at a glance. Three quarters of the way
                // to the band was a tint you could hold a whole sentence in
                // and not notice, which reads as the click having done
                // nothing — and a reader who thinks nothing happened does
                // not try again.
                let firm = palette.accent.mix(palette.band, 0.55);
                // The bar starts a little before its first word, as far
                // as the space after its last word reaches past the end,
                // so the words sit inside it rather than against its edge.
                // Pulled left by as much as it is padded, so lighting a
                // sentence moves no word.
                this.bg(paint(palette.band.mix(firm, lit)))
                    .when(first, |this| {
                        this.ml(px(-LIT_LEAD)).pl(px(LIT_LEAD)).rounded_l(px(4.))
                    })
                    .when(last, |this| this.rounded_r(px(4.)))
            })
            .when(lit <= 0. && heard > 0., |this| {
                this.bg(paint(palette.band.mix(palette.accent, 0.2 * heard)))
            })
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
            .my(px(LIT_INSET))
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

    wrap(match token.mark {
        Some(Mark::Code) => {
            // The chip: mono, on the pane's own ground, sized to its text.
            let (text, trailing) = split_trailing_space(&token.text);
            div()
                .flex()
                .items_center()
                .child(
                    div()
                        .font_family(mono)
                        .text_size(px(11.6))
                        .px(px(4.))
                        .py(px(1.))
                        .rounded(px(3.))
                        .bg(paint(palette.wash))
                        .child(text),
                )
                .child(trailing)
                .into_any_element()
        }
        Some(Mark::Strong) => div()
            .font_semibold()
            .child(SharedString::from(token.text))
            .into_any_element(),
        // The accent, not italics. Emphasis in a deck means "this is the
        // word to look at", and the deck already has a colour for that.
        Some(Mark::Emphasis) => div()
            .text_color(paint(palette.accent))
            .child(SharedString::from(token.text))
            .into_any_element(),
        None | Some(Mark::Pane) => div()
            .child(SharedString::from(token.text))
            .into_any_element(),
    })
}

/// The clear space above and below a lit sentence's bar, inside its line.
const LIT_INSET: f32 = 3.;

/// How far a lit sentence's bar reaches before its first word: about the
/// width of the space after its last, which is the bar's margin at that end.
const LIT_LEAD: f32 = 4.;
