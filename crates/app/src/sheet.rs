//! A pane is either read or drawn.
//!
//! Code panes and diagram panes have almost nothing in common inside — one is a
//! virtual list of highlighted rows, the other a painted canvas — but the deck
//! treats them as the same thing: a pane in the grid, wearing the same header,
//! able to be selected in and commented on. This is where the two meet.
//!
//! The forwarding is deliberately narrow. Only what the window genuinely asks
//! of *any* pane lives here; anything that only makes sense for one kind is
//! reached through [`Sheet::code`] or [`Sheet::chart`], so the compiler keeps
//! saying which is which rather than letting a line number leak into a picture.

use deck_core::theme::Palette;
use gpui_kit::*;

use crate::chart::Chart;
use crate::pane::{Away, Mark, Pane};
use crate::view::DeckView;

/// A pane's place on the page, and what it needs from the window to draw.
///
/// Gathered into one value rather than passed as four. Every one of them is
/// decided by the frame rather than by the pane — where it stands, how much of
/// the row it was given, what the window is painted in, and how to call back
/// into it — so they travel together and a pane that grows another need does
/// not change every signature between here and there.
pub struct Slot<'a> {
    /// Which pane this is, so a row can name it when the pointer lands on it.
    pub ix: usize,
    /// How folded this pane is, nought open and one folded to its spine.
    pub fold: f32,
    /// Whether this pane was brought in to answer a question.
    ///
    /// It gets a way to be closed, which an authored pane does not: the deck
    /// decides what an authored group shows, and the reader decides how long a
    /// borrowed pane stays.
    pub temporary: bool,
    /// Whether this pane may be folded at all.
    ///
    /// The last open pane may not: there would be nobody to give its width to.
    /// The control is not drawn rather than drawn and refused.
    pub foldable: bool,
    /// Whether it folds to the left edge of the window or the right one.
    ///
    /// A pane folds to the side it is already nearest, so the spine never
    /// crosses the code to get to its edge and the open panes stay in the
    /// middle of the window.
    pub fold_left: bool,
    /// How much of its row's width it takes.
    ///
    /// It goes on the pane's own root rather than on a wrapper, because a
    /// wrapper is a block and a pane asking for a share inside one has no flex
    /// context to ask.
    pub share: f32,
    /// The palette the whole window paints with.
    pub palette: &'a Palette,
    /// The window, for anything inside the pane to call back into.
    pub view: &'a WeakEntity<DeckView>,
    /// How fast a flow travels, for the button that says so.
    ///
    /// Handed down rather than read back out of the window. A pane asking the
    /// entity that is rendering it is a second borrow of something already
    /// borrowed, and it aborts — in a mouse handler, which cannot unwind, so
    /// it takes the process with it.
    pub pace: crate::view::Pace,
}

/// One pane of a group.
pub enum Sheet {
    /// A file, with a range lit inside it.
    Code(Pane),
    /// A picture of something that is in no file.
    Drawn(Chart),
}

impl Sheet {
    /// Whether the reader has chosen lines in this pane.
    #[must_use]
    pub fn reader_chose(&self) -> bool {
        match self {
            Self::Code(code) => code.reader_chose(),
            Self::Drawn(_) => false,
        }
    }

    /// The ref this pane shows, so a remark can name it.
    #[must_use]
    pub fn ref_id(&self) -> &SharedString {
        match self {
            Self::Code(code) => &code.ref_id,
            Self::Drawn(chart) => &chart.ref_id,
        }
    }

    /// What the prose calls this pane, when the deck named it.
    ///
    /// Both kinds answer to a name. A picture that could not be addressed by
    /// one was a pane the narration had no way to refer to, which left
    /// diagrams out of every sentence that named where it was pointing.
    #[must_use]
    pub fn name(&self) -> Option<&SharedString> {
        match self {
            Self::Code(code) => code.name.as_ref(),
            Self::Drawn(chart) => chart.name.as_ref(),
        }
    }

    /// The code pane, if this is one.
    #[must_use]
    pub fn code(&self) -> Option<&Pane> {
        match self {
            Self::Code(code) => Some(code),
            Self::Drawn(_) => None,
        }
    }

    /// The code pane, to move its live spotlight without rebuilding the group.
    #[must_use]
    pub fn code_mut(&mut self) -> Option<&mut Pane> {
        match self {
            Self::Code(code) => Some(code),
            Self::Drawn(_) => None,
        }
    }

    /// The diagram pane, if this is one.
    #[must_use]
    pub fn chart(&self) -> Option<&Chart> {
        match self {
            Self::Drawn(chart) => Some(chart),
            Self::Code(_) => None,
        }
    }

    /// The diagram pane, to change something about it.
    #[must_use]
    pub fn chart_mut(&mut self) -> Option<&mut Chart> {
        match self {
            Self::Drawn(chart) => Some(chart),
            Self::Code(_) => None,
        }
    }

    /// Whether the reader has picked something inside this pane.
    ///
    /// Lines in a code pane, a node in a diagram. Either way it is the answer
    /// to "is this the pane a comment would land on".
    #[must_use]
    pub fn is_picked(&self) -> bool {
        match self {
            Self::Code(code) => code.selected.is_some(),
            Self::Drawn(chart) => chart.selected.is_some(),
        }
    }

    /// Drop whatever was picked here.
    pub fn unpick(&mut self) {
        match self {
            Self::Code(code) => code.selected = None,
            Self::Drawn(chart) => chart.selected = None,
        }
    }

    /// Put the lit range back in view.
    ///
    /// A diagram has no range to lose, so it has nothing to do — which is not
    /// the same as this being meaningless for it. The window asks every pane
    /// after anything that changes row heights, and a pane that cannot drift is
    /// simply already where it should be.
    pub fn show_range(&self) {
        if let Self::Code(code) = self {
            code.show_range();
        }
    }

    /// Which way what this pane points at lies, or `None` if it is on screen.
    ///
    /// Always `None` for a diagram: the whole picture is the point of it, so
    /// there is nowhere to have scrolled away from and no button to offer.
    #[must_use]
    pub fn range_away(&self) -> Option<Away> {
        match self {
            Self::Code(code) => code.range_away(),
            Self::Drawn(_) => None,
        }
    }

    /// The pane folded down to its spine, or nothing for a picture.
    pub fn render_folded(&self, slot: &Slot, cx: &App) -> AnyElement {
        match self {
            Self::Code(code) => code.render_folded(slot, cx),
            // A picture has no name to put on a spine and nothing to read down
            // the side of it, so it folds away to nothing at all.
            Self::Drawn(_) => gpui_kit::div().into_any_element(),
        }
    }

    /// The pane, header and all.
    pub fn render(
        &self,
        slot: &Slot,
        marks: &[Mark],
        focus_button: Option<AnyElement>,
        cx: &App,
    ) -> AnyElement {
        match self {
            Self::Code(code) => code
                .render(slot, marks, focus_button, cx)
                .into_any_element(),
            Self::Drawn(chart) => chart.render(slot, cx).into_any_element(),
        }
    }
}
