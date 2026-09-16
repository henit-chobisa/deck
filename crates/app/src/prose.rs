//! The narration, rendered.
//!
//! An agent writes markdown, so the band has to read markdown — but only the
//! inline part of it. A group's `say` is prose with a few words emphasised and
//! a few spelled as code; it is not a document, and it has no headings, lists
//! or tables to lay out.
//!
//! The library's markdown view was tried and it renders a *document*: its own
//! spacing, its own idea of a code chip, its own leading. Reaching into that to
//! match the deck's measurements is more work than reading the four marks that
//! actually appear, and the result still is not ours.
//!
//! So this parses those four marks and hands the result to [`StyledText`] —
//! the same primitive a code row uses. Wrapping, selection and measurement come
//! from GPUI rather than from a flex box full of word fragments.

use deck_core::LineRange;
use deck_core::theme::Palette;
use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette::paint;

/// A run of the paragraph that is styled the same way throughout.
pub struct Token {
    /// The text, with whatever whitespace followed it in the source. Keeping
    /// the space on the token is what lets the tokens be laid out with no gap
    /// between them, so a chip followed by a comma is followed by a comma.
    text: SharedString,
    mark: Option<Mark>,
    /// Where this word sits in the whole narration, counting from zero.
    ///
    /// A selection is a range of these. Words rather than characters: the
    /// narration is laid out as one element per word so that a paragraph can
    /// wrap between any two of them, and a range of elements is what that
    /// layout can answer questions about.
    at: usize,
    /// Which sentence of the whole narration this word belongs to.
    ///
    /// Carried on the word rather than wrapping each sentence in a container of
    /// its own, because a container would only wrap at its own edges and the
    /// paragraph has to keep wrapping between any two words. The number is what
    /// lets a click on one word light every word of its sentence.
    said: usize,
}

/// One paragraph, as the tokens it wraps at.
pub struct Paragraph {
    tokens: Vec<Token>,
}

/// Where one sentence ends and the next begins.
///
/// A full stop, question mark or exclamation followed by a space. Deliberately
/// naive: `e.g.` and `1.5` split a sentence in two here. The cost of that is
/// that a click lights half a sentence instead of all of it, and the comment
/// still carries the half the reader was pointing at — which is the thing that
/// matters, and is already far better than quoting the first line whatever they
/// meant.
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
}

impl Picking {
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
        }
    }
}

/// Every word of `say`, in order, each keeping the space that followed it.
///
/// A selection quotes `words[from..=to]` joined, which is why the spacing has
/// to ride along on the word rather than be put back afterwards.
#[must_use]
pub fn words(say: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let paragraphs = parse(say);
    let last = paragraphs.len().saturating_sub(1);

    for (ix, para) in paragraphs.into_iter().enumerate() {
        out.extend(para.tokens.into_iter().map(|token| token.text.to_string()));
        // The last word of a paragraph carries no trailing space — nothing
        // follows it on its own line. A selection that runs into the next
        // paragraph would join the two words together without one.
        if ix < last
            && let Some(end) = out.last_mut()
        {
            end.push(' ');
        }
    }
    out
}

/// The words of the sentence that `at` belongs to.
///
/// What a plain click selects. Clicking is not dragging, and selecting a single
/// word because somebody clicked once would be a selection nobody asked for.
#[must_use]
pub fn sentence_around(say: &str, at: usize) -> Option<(usize, usize)> {
    let tokens: Vec<(usize, usize)> = parse(say)
        .into_iter()
        .flat_map(|para| para.tokens)
        .map(|token| (token.at, token.said))
        .collect();
    let said = tokens.iter().find(|(ix, _)| *ix == at)?.1;
    let mut of_it = tokens.iter().filter(|(_, s)| *s == said).map(|(ix, _)| *ix);
    let first = of_it.next()?;
    Some((first, of_it.next_back().unwrap_or(first)))
}

fn ends_a_sentence(word: &str) -> bool {
    word.trim_end().ends_with(['.', '!', '?'])
}

/// Split `say` into paragraphs.
///
/// A blank line starts a new paragraph, which is the only block-level thing
/// this understands — and the only one narration uses.
#[must_use]
pub fn parse(say: &str) -> Vec<Paragraph> {
    tokenised(say, false)
}

/// [`parse`], saying whether beats survive it.
///
/// They must not reach the eye and must reach the ear, and both sides want the
/// same tokenising and the same marks spent — so the one difference is a flag
/// rather than a second parser that could drift from this one.
fn tokenised(say: &str, keep_beats: bool) -> Vec<Paragraph> {
    let (mut said, mut at) = (0, 0);
    say.split("\n\n")
        .map(str::trim)
        .filter(|para| !para.is_empty())
        .map(|para| {
            let parsed = paragraph(para, keep_beats, &mut said, &mut at);
            // A paragraph break always ends a sentence, whatever it ends with.
            said += 1;
            parsed
        })
        .collect()
}

/// The narration as something worth hearing.
///
/// Read the source aloud verbatim and the marks come with it: a listener hears
/// "backtick build payload backtick" and every code chip lands as punctuation
/// they have to discard. So this runs the same parse the band renders from, and
/// spends the marks instead of speaking them — the text of each token, with the
/// code ones said the way a person would say them.
///
/// The paragraph gaps matter as much as the words. A break in the narration is
/// a change of subject, and a voice that runs two subjects together turns an
/// argument back into a stream, which is the thing the groups exist to prevent.
/// `[[slnc n]]` is the system synthesiser's own instruction for a pause.
#[must_use]
pub fn spoken(say: &str, pause: u16) -> String {
    // Google's own spelling, used as the one neutral way to write a beat.
    // Every engine renders it differently and none of them show it, so there is
    // no reason for a second syntax that would have to be translated twice.
    let _ = pause;
    let between = " [pause long] ".to_string();
    tokenised(say, true)
        .into_iter()
        .map(|para| {
            para.tokens
                .iter()
                .map(|token| match token.mark {
                    Some(Mark::Code) => aloud(&token.text),
                    // Said as the words it is made of. `batch-2` is "batch 2".
                    Some(Mark::Pane) => aloud(&token.text.replace('-', " ")),
                    _ => token.text.to_string(),
                })
                .collect::<String>()
                .trim()
                .to_string()
        })
        .filter(|para| !para.is_empty())
        .collect::<Vec<_>>()
        .join(&between)
}

/// Every beat an agent may write, and what it is spelled.
///
/// Chirp 3 takes these in its `markup` field and decides how long each one
/// should be from what surrounds it. They are for the ear only: [`parse`]
/// takes them out, so a reader never sees stage directions in the prose.
pub const BEATS: &[&str] = &["[pause long]", "[pause short]", "[pause]"];

/// The text with every beat removed.
///
/// Called on the way into the band. An agent writing for the ear should not
/// have to write a second copy for the eye, so it writes one and each side
/// takes what it needs.
#[must_use]
pub fn unbeat(text: &str) -> String {
    let mut out = text.to_string();
    for beat in BEATS {
        out = out.replace(beat, "");
    }
    // A beat sat between two spaces, and removing it leaves both.
    while out.contains("  ") {
        out = out.replace("  ", " ");
    }
    out
}

/// How an agent says where it is pointing.
///
/// `[point 106-110]` inside the narration, or `[point 106]` for one line. It
/// stands beside the beats and is treated the same way: written once, never
/// shown and never said. What it does instead is light exactly those lines
/// inside the range the pane is already showing, for as long as the words
/// after it are being said.
///
/// The lines are the file's own, because that is what the agent has in front
/// of it and what the pane prints down its gutter. Counting from the top of
/// the shown range would mean the agent doing arithmetic to point at a line it
/// can already name.
pub const POINT: &str = "[point ";

/// `106-110`, or `106` on its own.
///
/// Anything else is not a point. An agent writing about pointing is writing
/// prose, and prose is left exactly as it was rather than swallowed by a
/// directive it never meant to give.
fn lines(text: &str) -> Option<LineRange> {
    let text = text.trim();
    let (first, last) = text.split_once('-').unwrap_or((text, text));
    Some(LineRange::new(
        first.trim().parse().ok()?,
        last.trim().parse().ok()?,
    ))
}

/// The text with every point removed.
///
/// A point is for the code pane. The eye gets the prose and the ear gets the
/// prose; neither should be handed the stage direction that came with it.
#[must_use]
pub fn unpoint(text: &str) -> String {
    let (mut out, mut rest) = (String::with_capacity(text.len()), text);
    while let Some(at) = rest.find(POINT) {
        let after = &rest[at + POINT.len()..];
        let Some(end) = after.find(']') else {
            break; // an opener with no closer is just text
        };
        if lines(&after[..end]).is_none() {
            out.push_str(&rest[..at + POINT.len()]);
            rest = after;
            continue;
        }
        out.push_str(&rest[..at]);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    // A point sat between two spaces, and taking it out leaves both.
    while out.contains("  ") {
        out = out.replace("  ", " ");
    }
    out
}

/// One stretch of narration, and where it points while it is being said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// The lines to light inside the pane, for as long as this is said.
    pub point: Option<LineRange>,
    /// The words, ready for an engine.
    pub text: String,
    /// How many words of the band this piece is.
    ///
    /// Counted the way the band counts, so the word being heard can be found
    /// on the page: the spoken text cannot be counted instead, because a code
    /// chip is one word on the page and several out loud.
    pub words: usize,
}

/// How many words the band draws for this text.
fn counted(text: &str) -> usize {
    parse(text).iter().map(|para| para.tokens.len()).sum()
}

/// The narration cut wherever its pointing changes.
///
/// This is the whole mechanism. The pieces are said in order, so the lit lines
/// move as each piece begins — the finger arrives with the sentence about it,
/// not four sentences early because the agent sent its commands faster than
/// anybody could listen to them.
///
/// Narration with no points comes back as a single piece, which is what
/// [`spoken`] returns on its own.
#[must_use]
pub fn pointed(say: &str, pause: u16) -> Vec<Said> {
    let mut out: Vec<Said> = Vec::new();
    let mut point: Option<LineRange> = None;
    let mut piece = String::new();
    let mut rest = say;

    while let Some(at) = rest.find(POINT) {
        let after = &rest[at + POINT.len()..];
        let Some(end) = after.find(']') else {
            break;
        };
        let Some(next) = lines(&after[..end]) else {
            piece.push_str(&rest[..at + POINT.len()]);
            rest = after;
            continue;
        };
        piece.push_str(&rest[..at]);
        rest = &after[end + 1..];

        // A paragraph break just before a point is a change of subject, and
        // cutting there would drop the gap the paragraph was going to get.
        let cut = piece.trim_end().len();
        if piece[cut..].matches('\n').count() >= 2 {
            piece.truncate(cut);
            piece.push_str(" [pause long]");
        }
        // A beat straight after a point belongs to the gap before it. Left at
        // the front of the next piece it is silence at the start of a clip,
        // which is exactly what is trimmed off when the pieces are joined.
        while let Some(beat) = BEATS
            .iter()
            .find(|beat| rest.trim_start().starts_with(**beat))
        {
            if !piece.trim().is_empty() {
                piece.push(' ');
                piece.push_str(beat);
            }
            rest = &rest.trim_start()[beat.len()..];
        }

        let text = spoken(&piece, pause);
        if !text.is_empty() {
            out.push(Said {
                point,
                text,
                words: counted(&piece),
            });
        }
        piece.clear();
        point = Some(next);
    }

    piece.push_str(rest);
    let text = spoken(&piece, pause);
    if !text.is_empty() {
        out.push(Said {
            point,
            text,
            words: counted(&piece),
        });
    }
    out
}

/// One code chip, as a person reading the code out would say it.
///
/// Identifiers are the whole problem. `base_url` spoken literally is "base
/// underscore url", and `buildPayload` is one long word a voice will put the
/// stress in the wrong place in. Both are two words that were written without a
/// space for the compiler's benefit, so the space goes back in.
fn aloud(code: &str) -> String {
    let (text, space) = split_trailing_space(code);

    // A path is said by its last part. "crates slash app slash src slash view
    // dot r s" is a sentence nobody needed; the file name is the thing being
    // pointed at, and the pane is already showing which file it is.
    let text = text.rsplit('/').next().unwrap_or(&text).to_string();
    // Empty strings appear in narration about exactly this kind of bug, and
    // "quote quote" is not what anybody says out loud.
    let text = text
        .replace("\"\"", "empty string")
        .replace("''", "empty string");
    let text = text.replace("::", " ").replace('_', " ");

    let mut said = String::with_capacity(text.len() + 4);
    let mut last = '\0';
    for ch in text.chars() {
        // The join in camelCase: an upper after a lower, or after a digit.
        if ch.is_uppercase() && (last.is_lowercase() || last.is_ascii_digit()) {
            said.push(' ');
        }
        said.push(ch);
        last = ch;
    }
    // A trailing colon is Python's, not the sentence's — `if credentials:` is
    // read "if credentials", and the pause after it belongs to the prose.
    let said = said.trim_end_matches(':').trim().to_string();
    format!("{said}{space}")
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
}

impl Look {
    /// The band: the thing being read.
    #[must_use]
    pub fn band(palette: &Palette) -> Self {
        Self {
            size: 13.2,
            leading: 21.,
            tone: palette.fg,
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
            leading: 18.6,
            tone: palette.fg.mix(palette.muted, 0.22),
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
    div().v_flex().gap(px(look.leading * 0.42)).children(
        paragraphs
            .into_iter()
            .map(|para| para.render(palette, mono.clone(), picking, look)),
    )
}

impl Paragraph {
    fn render(
        self,
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
            .line_height(px(look.leading))
            .text_color(paint(look.tone))
            .children(
                self.tokens
                    .into_iter()
                    .map(|token| token.render(palette, mono.clone(), picking)),
            )
    }
}

impl Token {
    fn render(self, palette: &Palette, mono: SharedString, picking: &Picking) -> AnyElement {
        // Every word answers to the pointer: down starts a selection, and the
        // window extends it to whichever word is under the pointer while the
        // button is held — the same shape the code rows use, one element per
        // word instead of one per line.
        //
        // Words rather than characters, and that is the layout speaking. The
        // narration is a row of separate elements so a paragraph can wrap
        // between any two of them, and a range of elements is the finest thing
        // that layout can be asked about.
        let at = self.at;
        let lit = picking
            .range
            .is_some_and(|(from, to)| at >= from.min(to) && at <= from.max(to));
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
        if self.mark == Some(Mark::Pane) {
            let (text, trailing) = split_trailing_space(&self.text);
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
        let wrap = move |inner: AnyElement| {
            div()
                .id(("say", at))
                .when(pickable, |this| this.cursor_pointer())
                .when(lit, |this| {
                    this.bg(paint(palette.accent.mix(palette.band, 0.74)))
                        .rounded(px(2.))
                })
                .when(!lit && heard > 0., |this| {
                    this.bg(paint(palette.band.mix(palette.accent, 0.2 * heard)))
                })
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    down(at, window, cx);
                })
                .on_hover(move |entered, window, cx| over(at, *entered, window, cx))
                .child(inner)
                .into_any_element()
        };

        wrap(match self.mark {
            Some(Mark::Code) => {
                // The chip: mono, on the pane's own ground, sized to its text.
                let (text, trailing) = split_trailing_space(&self.text);
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
            Some(Mark::Strong) => div().font_semibold().child(self.text).into_any_element(),
            // The accent, not italics. Emphasis in a deck means "this is the
            // word to look at", and the deck already has a colour for that.
            Some(Mark::Emphasis) => div()
                .text_color(paint(palette.accent))
                .child(self.text)
                .into_any_element(),
            None | Some(Mark::Pane) => div().child(self.text).into_any_element(),
        })
    }
}

/// The text without its trailing spaces, and those spaces.
///
/// A chip must not swallow the space after it, or the chip is drawn wider than
/// the words it holds.
fn split_trailing_space(text: &str) -> (String, String) {
    let trimmed = text.trim_end();
    (trimmed.to_string(), text[trimmed.len()..].to_string())
}

/// The four marks, in the order they are tried.
///
/// `**` before `*`, or every bold open would be read as an emphasis that never
/// closes.
const MARKS: &[(&str, Mark)] = &[
    ("**", Mark::Strong),
    ("`", Mark::Code),
    ("*", Mark::Emphasis),
    ("_", Mark::Emphasis),
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mark {
    Strong,
    Emphasis,
    Code,
    /// `[retry]`: the name of a pane.
    Pane,
}

/// A pane's name in brackets at the start of `text`, and how long it is.
fn pane_name_at(text: &str) -> Option<(&str, usize)> {
    let inside = text.strip_prefix('[')?;
    let end = inside.find(']')?;
    let name = &inside[..end];
    deck_core::protocol::valid_name(name).then_some((name, end + 2))
}

/// The text with each `[name]` said as the plain name.
///
/// For the places that show the agent's words without the band's parser: the
/// rail, the transcript. Brackets there are markup nobody asked to see.
#[must_use]
pub fn unname(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        match pane_name_at(&rest[open..]) {
            Some((name, len)) => {
                out.push_str(name);
                rest = &rest[open + len..];
            }
            None => {
                out.push('[');
                rest = &rest[open + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Whether a character is one a word is made of.
fn wordish(ch: Option<char>) -> bool {
    ch.is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
}

/// Where the mark opened at the start of `after_open` closes, if it does.
///
/// Every marker but `_` closes at the first repeat. `_` is the one that turns up
/// inside ordinary identifiers, and narration about code is full of them: a
/// group that mentions `base_url` twice would otherwise open an emphasis at the
/// first underscore, close it at the second, and render the whole sentence
/// between them in the accent colour with both underscores eaten — leaving
/// `baseurl` on screen and no way to tell it was ever two words.
///
/// So an underscore only marks when it is not flanked by a word: not opening
/// when the character before it belongs to one, and not closing when the
/// character after it does. This is CommonMark's rule, and it exists for
/// exactly this reason.
fn closes(after_open: &str, marker: &str, before: Option<char>) -> Option<usize> {
    if marker != "_" {
        return after_open.find(marker);
    }
    if wordish(before) {
        return None;
    }
    // Underscores inside a word are skipped rather than given up on, so
    // `_snake_case is one term_` still emphasises the whole phrase.
    let mut from = 0;
    while let Some(offset) = after_open[from..].find('_') {
        let at = from + offset;
        if !wordish(after_open[at + 1..].chars().next()) {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

/// One paragraph, split into the runs it can wrap between.
fn paragraph(para: &str, keep_beats: bool, said: &mut usize, at: &mut usize) -> Paragraph {
    // A newline inside a paragraph is the agent's line wrapping, not a break.
    // A point is an instruction about the code pane. It is taken out here
    // rather than only where the voice is fed, because the band renders from
    // this too and a reader who sees `[point 106]` mid-sentence has been given
    // the stage directions instead of the play.
    let flowed = unpoint(&para.replace('\n', " "));
    // Beats are for the ear. Left in, they render as literal brackets in the
    // middle of a sentence; taken out of the spoken copy, the agent's pacing is
    // lost. So each side gets what it needs from the same source.
    let source = if keep_beats { flowed } else { unbeat(&flowed) };
    // A beat or a point at the very start of a paragraph leaves the space that
    // followed it, which became a word of its own: an indent on the page, and
    // one word too many for the voice to count its way back to.
    let source = source.trim().to_string();
    let mut tokens: Vec<Token> = Vec::new();
    let mut plain = String::new();
    let mut rest = source.as_str();

    /// Plain text becomes one token per word, each keeping the whitespace that
    /// followed it — which is what lets the row wrap between words.
    ///
    /// The sentence counter advances *after* the word that closed one, so the
    /// full stop stays with the sentence it ends rather than opening the next.
    fn flush(plain: &mut String, tokens: &mut Vec<Token>, said: &mut usize, at: &mut usize) {
        let mut word = String::new();
        for ch in plain.chars() {
            word.push(ch);
            if ch == ' ' {
                let word = std::mem::take(&mut word);
                let ended = ends_a_sentence(&word);
                tokens.push(Token {
                    text: SharedString::from(word),
                    mark: None,
                    at: *at,
                    said: *said,
                });
                *at += 1;
                if ended {
                    *said += 1;
                }
            }
        }
        if !word.is_empty() {
            let ended = ends_a_sentence(&word);
            tokens.push(Token {
                text: SharedString::from(word),
                mark: None,
                at: *at,
                said: *said,
            });
            *at += 1;
            if ended {
                *said += 1;
            }
        }
        plain.clear();
    }

    'outer: while !rest.is_empty() {
        // A pane's name. Tried before the marks, and only for the exact shape a
        // name has, so a bracket in ordinary prose stays ordinary.
        if let Some((name, len)) = pane_name_at(rest) {
            flush(&mut plain, &mut tokens, said, at);
            rest = &rest[len..];
            let spacing: String = rest.chars().take_while(|c| *c == ' ').collect();
            rest = &rest[spacing.len()..];
            let text = format!("{name}{spacing}");
            let ended = ends_a_sentence(&text);
            tokens.push(Token {
                text: SharedString::from(text),
                mark: Some(Mark::Pane),
                at: *at,
                said: *said,
            });
            *at += 1;
            if ended {
                *said += 1;
            }
            continue 'outer;
        }
        for (marker, mark) in MARKS {
            let Some(after_open) = rest.strip_prefix(marker) else {
                continue;
            };
            let Some(len) = closes(after_open, marker, plain.chars().last()) else {
                continue; // an opener with no closer is just text
            };

            flush(&mut plain, &mut tokens, said, at);
            rest = &after_open[len + marker.len()..];

            // Any space after the mark rides along on the token, so the tokens
            // can be laid out with nothing between them.
            let spacing: String = rest.chars().take_while(|c| *c == ' ').collect();
            rest = &rest[spacing.len()..];

            let text = format!("{}{spacing}", &after_open[..len]);
            let ended = ends_a_sentence(&text);
            tokens.push(Token {
                text: SharedString::from(text),
                mark: Some(*mark),
                at: *at,
                said: *said,
            });
            *at += 1;
            if ended {
                *said += 1;
            }
            continue 'outer;
        }

        let ch = rest.chars().next().unwrap_or_default();
        plain.push(ch);
        rest = &rest[ch.len_utf8()..];
    }

    flush(&mut plain, &mut tokens, said, at);
    Paragraph { tokens }
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this module glob-imports GPUI, which
    // exports a `test` attribute of its own and would otherwise shadow the
    // standard one.
    use core::prelude::v1::test;

    use super::*;

    #[test]
    fn a_point_never_reaches_the_eye_or_the_ear() {
        // Both sides read the same source, so a direction left in either one
        // is a direction the reader is handed instead of the sentence.
        let said = "the guard is here. [point 106-110] and it waves it through.";
        assert!(!unpoint(said).contains("[point"), "{}", unpoint(said));
        assert!(
            !spoken(said, 420).contains("point"),
            "{}",
            spoken(said, 420)
        );
        let seen: String = parse(said)
            .into_iter()
            .flat_map(|para| para.tokens)
            .map(|token| token.text.to_string())
            .collect();
        assert!(!seen.contains('['), "{seen}");
    }

    #[test]
    fn the_narration_is_cut_where_the_pointing_changes() {
        let said = "first, the call. [point 106-110] then the guard. [point 140] \
                    and then nothing happens.";
        let out = pointed(said, 420);

        assert_eq!(out.len(), 3, "one piece per point, plus what came before");
        assert_eq!(out[0].point, None, "nothing was pointed at yet");
        assert_eq!(out[1].point, Some(LineRange::new(106, 110)));
        assert_eq!(out[2].point, Some(LineRange::new(140, 140)), "one line");
        assert!(out[1].text.starts_with("then the guard"), "{}", out[1].text);
    }

    #[test]
    fn a_pane_name_is_one_word_on_the_page_and_the_name_out_loud() {
        let said = "the answer is in [batch-2], not in [retry].";
        let out = parsed(said);
        assert!(!out[0].0.contains('['), "{}", out[0].0);
        assert_eq!(out[0].1, vec![Mark::Pane, Mark::Pane]);
        assert_eq!(spoken(said, 420), "the answer is in batch 2, not in retry.");
        assert_eq!(unname(said), "the answer is in batch-2, not in retry.");
    }

    #[test]
    fn a_bracket_that_is_not_a_name_stays_a_bracket() {
        assert_eq!(unname("see [1] and [the note]"), "see [1] and [the note]");
        assert!(parsed("see [the note]")[0].1.is_empty());
    }

    #[test]
    fn the_pieces_count_to_the_words_on_the_page() {
        // The word being heard is found on the page by adding up the pieces
        // before it. If the pieces counted differently from the page, the lit
        // sentence would drift further from the voice with every point.
        let said = "The guard is `base_url`. [point 12] It *asks* whether anything \
                    changed.\n\n[pause] [point 40] And [retry] waves it through.";
        let pieces: usize = pointed(said, 420).iter().map(|piece| piece.words).sum();
        assert_eq!(pieces, words(said).len());
    }

    #[test]
    fn a_beat_after_a_point_is_heard_before_it() {
        // Written after the point, the beat would open the next clip, and the
        // silence at the front of a clip is what gets trimmed when the pieces
        // are joined. Moved into the gap before, it survives.
        let out = pointed(
            "the guard is here. [point 140] [pause] and it is ignored.",
            420,
        );
        assert_eq!(out.len(), 2);
        assert!(out[0].text.ends_with("[pause]"), "{}", out[0].text);
        assert!(!out[1].text.contains("[pause]"), "{}", out[1].text);
    }

    #[test]
    fn a_paragraph_ending_at_a_point_keeps_its_gap() {
        // A paragraph break is a change of subject. Cutting a passage there
        // used to throw the gap away with the blank line.
        let out = pointed("the guard is here.\n\n[point 140] and it is ignored.", 420);
        assert!(out[0].text.ends_with("[pause long]"), "{}", out[0].text);
    }

    #[test]
    fn prose_about_pointing_is_still_prose() {
        // The directive is line numbers in brackets and nothing else. An agent
        // writing the words `[point at` in a sentence meant the words.
        let said = "it is worth a [point about the guard] here";
        assert_eq!(unpoint(said), said);
        let out = pointed(said, 420);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].point, None);
    }

    #[test]
    fn narration_with_no_points_is_one_piece() {
        // What `spoken` returned on its own, unchanged — otherwise every deck
        // written before pointing existed would be said differently.
        let said = "the guard is here.\n\nand it waves it through.";
        let out = pointed(said, 420);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, spoken(said, 420));
    }

    #[test]
    fn an_identifier_is_not_an_emphasis() {
        // The bug this exists for: narration naming `base_url` twice opened an
        // emphasis at the first underscore and closed it at the second, eating
        // both and rendering the sentence between them in the accent colour.
        // What reached the screen was `baseurl`.
        let said = "it pins a query to the connection's own base_url, and the \
                    base_url is exactly what just moved.";
        let out = parsed(said);
        assert!(
            out[0].0.contains("base_url"),
            "underscore survives: {}",
            out[0].0
        );
        assert!(out[0].1.is_empty(), "and nothing was emphasised");
    }

    #[test]
    fn an_underscore_pair_around_words_still_marks() {
        // The rule narrows `_`, it does not retire it.
        let out = parsed("the _whole form_, every time");
        assert_eq!(out[0].1, vec![Mark::Emphasis]);
    }

    #[test]
    fn a_mark_may_hold_an_identifier() {
        // `_` closes on the first underscore that does not sit inside a word,
        // so a phrase containing snake_case can still be emphasised whole.
        let out = parsed("_base_url is the one that moved_");
        assert_eq!(out[0].1, vec![Mark::Emphasis]);
        assert!(out[0].0.contains("base_url"), "{}", out[0].0);
    }

    #[test]
    fn a_beat_is_heard_and_never_seen() {
        // The agent writes one copy. The ear gets the pacing it asked for; the
        // eye never gets a stage direction in the middle of a sentence.
        let say = "It waves the empty password through. [pause] Then line 132.";
        assert!(spoken(say, 420).contains("[pause]"), "the ear keeps it");

        let seen: String = tokenised(say, false)
            .into_iter()
            .flat_map(|para| para.tokens)
            .map(|token| token.text.to_string())
            .collect();
        assert!(!seen.contains("[pause]"), "the eye never sees it: {seen}");
        assert!(
            !seen.contains("  "),
            "and it leaves no double space: {seen}"
        );
        assert!(seen.contains("through. Then"), "{seen}");
    }

    #[test]
    fn a_chip_is_said_the_way_a_person_would_say_it() {
        // Spoken verbatim these are "base underscore url" and one long word
        // with the stress in the wrong place. Both were written without a space
        // for the compiler, so the space goes back in.
        let said = spoken("`base_url` and `buildPayload` and `if credentials:`", 0);
        assert!(said.contains("base url"), "{said}");
        assert!(said.contains("build Payload"), "{said}");
        assert!(said.contains("if credentials"), "{said}");
        assert!(
            !said.contains(':'),
            "the colon is Python's, not the sentence's"
        );
        assert!(!said.contains('`'), "and no marker is ever read aloud");
    }

    #[test]
    fn a_path_is_said_by_its_last_part() {
        // "crates slash app slash src slash view dot r s" is a sentence nobody
        // needed — the pane already shows which file it is.
        let said = spoken("see `crates/app/src/view.rs`", 0);
        assert!(said.contains("view.rs"), "{said}");
        assert!(!said.contains("crates"), "{said}");
    }

    #[test]
    fn an_empty_string_is_called_one() {
        // The narration this came from was about exactly this bug, and "quote
        // quote" is not what anybody says out loud.
        assert!(spoken(r#"carries `password: ""`"#, 0).contains("empty string"));
    }

    #[test]
    fn paragraphs_are_held_apart() {
        // A break is a change of subject. Run two together and the argument is
        // a stream again, which is the thing the groups exist to prevent.
        //
        // Written in Google's spelling because one engine has to win, and that
        // is the one that understands a beat natively and judges its length
        // from what surrounds it. The others are told what to do with it.
        let said = spoken("First claim.\n\nSecond claim.", 400);
        assert!(said.contains("[pause long]"), "{said}");
        assert_eq!(said.matches("[pause").count(), 1, "one gap, not two");
    }

    /// Each paragraph as its whole text, and the marks it carries.
    fn parsed(say: &str) -> Vec<(String, Vec<Mark>)> {
        parse(say)
            .into_iter()
            .map(|para| {
                (
                    para.tokens.iter().map(|t| t.text.to_string()).collect(),
                    para.tokens.iter().filter_map(|t| t.mark).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn every_word_of_one_sentence_carries_the_same_number() {
        // The number is on the word rather than on a container around the
        // sentence, because a container would only wrap at its own edges and
        // the paragraph has to keep wrapping between any two words.
        let paras = parse("One runs twice. Two does not.");
        let said: Vec<usize> = paras[0].tokens.iter().map(|t| t.said).collect();
        assert_eq!(said, vec![0, 0, 0, 1, 1, 1]);
    }

    #[test]
    fn a_paragraph_break_ends_a_sentence_that_has_no_full_stop() {
        let paras = parse("no full stop here\n\nbut this is a new one");
        assert_eq!(paras[0].tokens[0].said, 0);
        assert_eq!(paras[1].tokens[0].said, 1);
    }

    #[test]
    fn a_selection_quotes_exactly_the_words_it_covers() {
        // The point of dragging: the comment carries what was dragged over,
        // not the first line of the say and not a whole sentence rounded up.
        let say = "The counter drops twice. That is the 63.";
        let said = words(say);
        assert_eq!(said[1..=3].concat().trim(), "counter drops twice.");
        assert_eq!(
            said.len(),
            8,
            "eight words, and the counting is what a range indexes"
        );
    }

    #[test]
    fn a_click_takes_the_sentence_the_word_is_in() {
        // A click is not a drag, and selecting one word because somebody
        // clicked once would be a selection they did not ask for.
        let say = "One runs twice. Two does not.";
        assert_eq!(sentence_around(say, 0), Some((0, 2)));
        assert_eq!(sentence_around(say, 4), Some((3, 5)));
        assert_eq!(sentence_around(say, 99), None);
    }

    #[test]
    fn a_selection_can_run_across_a_paragraph_break() {
        // The number is global to the narration rather than to a paragraph,
        // so a drag that starts in one and ends in the next is a range like
        // any other.
        let say = "First one here.\n\nSecond one there.";
        let said = words(say);
        assert_eq!(said[2..=3].concat().trim(), "here. Second");
    }

    #[test]
    fn markers_are_stripped_and_their_spans_recorded() {
        let got = parsed("carries a background and **nothing else**, so it survives");
        assert_eq!(got.len(), 1);
        assert_eq!(
            got[0].0,
            "carries a background and nothing else, so it survives"
        );
        assert_eq!(got[0].1, vec![Mark::Strong]);
    }

    #[test]
    fn a_blank_line_starts_a_paragraph_and_a_single_newline_does_not() {
        let got = parsed("one line\nwrapped by the agent\n\na second paragraph");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "one line wrapped by the agent");
        assert_eq!(got[1].0, "a second paragraph");
    }

    #[test]
    fn bold_is_read_before_emphasis() {
        // `**` has to be tried first: read left to right, the first `*` of a
        // bold opener otherwise starts an emphasis that swallows the rest.
        let got = parsed("**bold** and *lit*");
        assert_eq!(got[0].0, "bold and lit");
        assert_eq!(got[0].1, vec![Mark::Strong, Mark::Emphasis]);
    }

    #[test]
    fn an_opener_with_no_closer_is_left_as_text() {
        let got = parsed("2 * 3 is not emphasis");
        assert_eq!(got[0].0, "2 * 3 is not emphasis");
        assert!(got[0].1.is_empty());
    }

    #[test]
    fn code_spans_survive_punctuation_around_them() {
        let got = parsed("call `render.close()`, then stop");
        // The comma stays hard against the chip, and the space before `then`
        // rides on a token rather than becoming a gap between elements.
        assert_eq!(got[0].0, "call render.close(), then stop");
        assert_eq!(got[0].1, vec![Mark::Code]);
    }
}
