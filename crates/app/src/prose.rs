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

/// How the narration answers the pointer.
pub struct Picking {
    /// The words currently selected, as an inclusive range.
    pub range: Option<(usize, usize)>,
    /// The pointer went down on this word.
    pub down: Down,
    /// The pointer entered or left this word.
    pub over: Over,
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
    let (mut said, mut at) = (0, 0);
    say.split("\n\n")
        .map(str::trim)
        .filter(|para| !para.is_empty())
        .map(|para| {
            let parsed = paragraph(para, &mut said, &mut at);
            // A paragraph break always ends a sentence, whatever it ends with.
            said += 1;
            parsed
        })
        .collect()
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
    div().v_flex().gap(px(9.)).children(
        paragraphs
            .into_iter()
            .map(|para| para.render(palette, mono.clone(), picking)),
    )
}

impl Paragraph {
    fn render(self, palette: &Palette, mono: SharedString, picking: &Picking) -> impl IntoElement {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .text_size(px(13.2))
            .line_height(px(21.))
            .text_color(paint(palette.fg))
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
        let (down, over) = (picking.down.clone(), picking.over.clone());
        let wrap = move |inner: AnyElement| {
            div()
                .id(("say", at))
                .cursor_pointer()
                .when(lit, |this| {
                    this.bg(paint(palette.accent.mix(palette.band, 0.74)))
                        .rounded(px(2.))
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
            None => div().child(self.text).into_any_element(),
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
}

/// One paragraph, split into the runs it can wrap between.
fn paragraph(para: &str, said: &mut usize, at: &mut usize) -> Paragraph {
    // A newline inside a paragraph is the agent's line wrapping, not a break.
    let source = para.replace('\n', " ");
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
        for (marker, mark) in MARKS {
            let Some(after_open) = rest.strip_prefix(marker) else {
                continue;
            };
            let Some(len) = after_open.find(marker) else {
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
