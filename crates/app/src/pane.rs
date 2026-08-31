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

/// One line, ready to draw.
#[derive(Clone)]
struct Row {
    text: SharedString,
    /// Syntax runs, as byte ranges within `text`.
    runs: Vec<(Range<usize>, HighlightStyle)>,
    /// The line this is in the file, or `None` for a line that is not in it
    /// yet.
    number: Option<u32>,
    /// What the ref is proposing about this line, if anything.
    change: Option<Change>,
}

/// What a ref carrying an `after` is proposing about a line.
///
/// A change that has not been made: the lit range is what would go, and the
/// replacement is what would come. Deck never writes to the file — this is the
/// agent showing its work before doing it, which is the only way a reader gets
/// to say *no* before it happens rather than after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// A line the ref proposes to remove.
    Gone,
    /// A line the ref proposes to add. Has no number: it is not in the file.
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

/// One code pane.
pub struct Pane {
    rows: Vec<Row>,
    lit: LineRange,
    /// How many rows a proposed replacement added under the lit range.
    ///
    /// Kept because everything else here counts in file lines, and a spliced
    /// row has no line to count. See [`Pane::row_of`].
    added: usize,
    scroll: UniformListScrollHandle,
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
    /// The file, for the review payload and for relocating later.
    pub file: std::path::PathBuf,
    /// The lines the reader has picked, if any.
    ///
    /// A range rather than a line: an objection is often about a block — a
    /// whole match arm, a whole guard — and pinning it to the first line of
    /// that block loses which block was meant.
    pub selected: Option<LineRange>,
}

impl Pane {
    /// Build a pane for `code` out of `source`, the whole file.
    pub fn new(code: &RefSpec, source: &str, cx: &App) -> Self {
        let lines: Vec<&str> = source.lines().collect();
        let total = u32::try_from(lines.len()).unwrap_or(u32::MAX).max(1);
        let lit = code.range.clamp_to(total);
        let mut rows = highlight(source, &lines, &code.file, cx);
        let mut added = 0usize;

        // A proposed change, drawn in place: the range marked as going, and the
        // replacement spliced in under it. Every row below then sits one lower
        // per line added, which is why nothing may look a row up by its line
        // number afterwards — see `row_of`.
        if let Some(after) = code.after.as_deref() {
            for row in &mut rows[(lit.first as usize - 1)..(lit.last as usize)] {
                row.change = Some(Change::Gone);
            }

            let coming: Vec<&str> = after.lines().collect();
            let mut fresh = highlight(after, &coming, &code.file, cx);
            for row in &mut fresh {
                row.number = None;
                row.change = Some(Change::New);
            }
            added = fresh.len();
            rows.splice((lit.last as usize)..(lit.last as usize), fresh);
        }

        // Open on the range, a couple of lines above it so it does not start
        // hard against the top edge. The scroll is deferred to the next layout,
        // which is the only moment the list knows how tall it is.
        let scroll = UniformListScrollHandle::new();
        let top = lit.first.saturating_sub(LEAD_IN).max(1);
        scroll.scroll_to_item((top - 1) as usize, ScrollStrategy::Top);

        Self {
            rows,
            lit,
            added,
            scroll,
            label: format!("{}:{}", code.file.display(), lit).into(),
            note: code.note.clone().map(SharedString::from),
            laid_out: std::cell::Cell::new(None),
            ref_id: code.id.clone().into(),
            file: code.file.clone(),
            selected: None,
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
        row_of(number, self.lit.last, self.added)
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

    /// The list's own scroll handle, for driving it a frame at a time.
    #[must_use]
    pub fn scroll(&self) -> UniformListScrollHandle {
        self.scroll.clone()
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
        let last = first + (self.lit.len() as f32 * row);

        if last <= top {
            Some(Away::Above)
        } else if first >= top + f32::from(viewport) {
            Some(Away::Below)
        } else {
            None
        }
    }

    /// Put the lit range back in view.
    ///
    /// Asked for after anything that changes row heights, since a scroll
    /// position measured in rows means something different once the rows are a
    /// different size.
    pub fn show_range(&self) {
        let top = self.lit.first.saturating_sub(LEAD_IN).max(1);
        self.scroll
            .scroll_to_item(self.row_of(top), ScrollStrategy::Top);
    }

    /// The lines a comment would land on: whatever is selected, or the lit
    /// range when nothing is.
    #[must_use]
    pub fn comment_range(&self) -> LineRange {
        self.selected.unwrap_or(self.lit)
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
        let label = chrome(
            self.label.clone(),
            self.note.clone(),
            focus_button,
            palette,
            cx,
        );

        div()
            .v_flex()
            .flex_grow(slot.share)
            .flex_shrink(1.)
            .flex_basis(px(0.))
            .h_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(paint(palette.wash))
            .child(label)
            .child(self.render_code(slot.ix, marks, palette, slot.view, cx))
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
        let lit = self.lit;
        let palette = *palette;
        let gutter_fg = palette.fg.mix(palette.wash, 0.6);
        let selected = self.selected;
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
        let lit_entry = self.row_of(self.lit.first);
        let view = view.clone();
        let wheeling = view.clone();

        self.laid_out.set(Some((lit_entry, f32::from(row_px))));
        let cards = self.render_cards(marks, row_px, &palette, &view, cx);

        let list = uniform_list("deck-code", rows.len(), move |visible, _window, _cx| {
            visible
                .map(|ix| {
                    let row = rows[ix].clone();
                    // The file line, where there is one. A spliced-in line has
                    // none: it is not in the file, and giving it the number of
                    // whatever it was pushed past would be a lie the reader
                    // would then quote in a comment.
                    let number = row.number.unwrap_or(0);
                    let is_lit = row.number.is_some() && lit.contains(number);
                    let is_picked =
                        row.number.is_some() && selected.is_some_and(|s| s.contains(number));
                    let has_mark = marked.contains(&number);

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
                                    deck.start_pick(pane_ix, number, extend, cx);
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
                                    deck.hover_row(pane_ix, number, entered);
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
                        .bg(paint(match (row.change, is_picked, is_lit) {
                            (Some(Change::Gone), _, _) => palette.gone,
                            (Some(Change::New), _, _) => palette.fresh,
                            (None, true, _) => palette.focus.mix(palette.accent, 0.14),
                            (None, false, true) => palette.focus,
                            (None, false, false) => palette.wash,
                        }))
                        .child(
                            div()
                                .w(px(12.))
                                .flex_none()
                                .text_center()
                                .text_color(paint(match row.change {
                                    Some(Change::Gone) => palette.del,
                                    Some(Change::New) => palette.add,
                                    None if has_mark => palette.add,
                                    None if is_picked || is_lit => palette.accent,
                                    None => palette.wash,
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
        .size_full();

        div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(list)
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
                    let by = event.delta.pixel_delta(window.line_height()).y;
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
            change: None,
        })
        .collect();

    let mut highlighter = SyntaxHighlighter::new(language_of(file));
    highlighter.update(None, &Rope::from(source), None);
    let styles = highlighter.styles(&(0..source.len()), &*cx.theme().highlight_theme);

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
    label: SharedString,
    note: Option<SharedString>,
    focus_button: Option<AnyElement>,
    palette: &Palette,
    cx: &App,
) -> impl IntoElement {
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
        .child(label)
        .child(
            div()
                .h_flex()
                .items_center()
                .gap(px(10.))
                .min_w_0()
                .children(note.as_deref().map(|note| emphasise(note, palette)))
                .children(focus_button),
        )
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
fn row_of(number: u32, lit_last: u32, added: usize) -> usize {
    let ix = (number.max(1) - 1) as usize;
    if number > lit_last { ix + added } else { ix }
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
        Some("jsx") => "jsx",
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
