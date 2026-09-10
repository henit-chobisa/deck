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
fn ends_a_sentence(word: &str) -> bool {
    word.trim_end().ends_with(['.', '!', '?'])
}

/// Split `say` into paragraphs.
///
/// A blank line starts a new paragraph, which is the only block-level thing
/// this understands — and the only one narration uses.
#[must_use]
pub fn parse(say: &str) -> Vec<Paragraph> {
    let mut said = 0;
    say.split("\n\n")
        .map(str::trim)
        .filter(|para| !para.is_empty())
        .map(|para| {
            let parsed = paragraph(para, &mut said);
            // A paragraph break always ends a sentence, whatever it ends with.
            said += 1;
            parsed
        })
        .collect()
}

/// Every sentence of `say`, in order — the text a comment on one of them quotes.
#[must_use]
pub fn sentences(say: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for para in parse(say) {
        for token in para.tokens {
            let at = token.said;
            while out.len() <= at {
                out.push(String::new());
            }
            out[at].push_str(&token.text);
        }
    }
    out.into_iter()
        .map(|said| said.trim().to_string())
        .filter(|said| !said.is_empty())
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
    picked: Option<usize>,
    pick: std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>,
) -> impl IntoElement {
    div().v_flex().gap(px(9.)).children(
        paragraphs
            .into_iter()
            .map(|para| para.render(palette, mono.clone(), picked, pick.clone())),
    )
}

impl Paragraph {
    fn render(
        self,
        palette: &Palette,
        mono: SharedString,
        picked: Option<usize>,
        pick: std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .text_size(px(13.2))
            .line_height(px(21.))
            .text_color(paint(palette.fg))
            .children(
                self.tokens.into_iter().enumerate().map(|(ix, token)| {
                    token.render(palette, mono.clone(), picked, ix, pick.clone())
                }),
            )
    }
}

impl Token {
    fn render(
        self,
        palette: &Palette,
        mono: SharedString,
        picked: Option<usize>,
        ix: usize,
        pick: std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>,
    ) -> AnyElement {
        // Every word answers to the pointer, and every word of one sentence
        // lights together. Picking by sentence rather than by character: the
        // comment carries the sentence it answers, and a half-selected phrase
        // would be a worse quote than a whole one.
        let said = self.said;
        let lit = picked == Some(said);
        let wrap = move |inner: AnyElement| {
            div()
                .id(("say", ix))
                .cursor_pointer()
                .when(lit, |this| {
                    this.bg(paint(palette.accent.mix(palette.band, 0.78)))
                        .rounded(px(2.))
                })
                .on_click(move |_, window, cx| pick(said, window, cx))
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
fn paragraph(para: &str, said: &mut usize) -> Paragraph {
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
    fn flush(plain: &mut String, tokens: &mut Vec<Token>, said: &mut usize) {
        let mut word = String::new();
        for ch in plain.chars() {
            word.push(ch);
            if ch == ' ' {
                let word = std::mem::take(&mut word);
                let ended = ends_a_sentence(&word);
                tokens.push(Token {
                    text: SharedString::from(word),
                    mark: None,
                    said: *said,
                });
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
                said: *said,
            });
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

            flush(&mut plain, &mut tokens, said);
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
                said: *said,
            });
            if ended {
                *said += 1;
            }
            continue 'outer;
        }

        let ch = rest.chars().next().unwrap_or_default();
        plain.push(ch);
        rest = &rest[ch.len_utf8()..];
    }

    flush(&mut plain, &mut tokens, said);
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
    fn a_sentence_can_be_named_on_its_own() {
        // What this is for: commenting on the narration used to quote the
        // first line of the say whatever the reader meant, so an objection to
        // the third sentence came back answering the first.
        let said = sentences("The counter drops twice. That is the 63.\n\nAnd a second one.");
        assert_eq!(
            said,
            vec![
                "The counter drops twice.",
                "That is the 63.",
                "And a second one."
            ]
        );
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
