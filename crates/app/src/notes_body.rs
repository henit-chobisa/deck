//! The words and pictures of a release's notes, as a view of their own.
//!
//! The card around them belongs to the deck window; this does not. It is its
//! own entity so that scrolling it redraws it and nothing else. Drawn as part
//! of the window, every tick of a scroll repainted the whole deck — every
//! pane, the rail, the band — and parsed every paragraph of the notes again,
//! to move a list by a few points. Scrolling the card lagged, and felt broken.

use deck_core::theme::Palette;
use gpui_kit::component::{ActiveTheme as _, StyledExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::notes::{Block, Notes};
use crate::palette::paint;

/// The card is 560 wide with 24 either side; this is what a picture inside it
/// can have.
const TEXT: f32 = 512.;

pub struct NotesBody {
    version: String,
    notes: Notes,
    palette: Palette,
    scroll: ScrollHandle,
}

impl NotesBody {
    pub fn new(version: String, notes: Notes, palette: Palette) -> Self {
        Self {
            version,
            notes,
            palette,
            scroll: ScrollHandle::new(),
        }
    }

    /// The notes arrived, or did not.
    pub fn set(&mut self, notes: Notes, cx: &mut Context<Self>) {
        self.notes = notes;
        cx.notify();
    }

    /// What is showing now: the blocks, once the notes have been read.
    #[must_use]
    pub fn blocks(&self) -> Option<Vec<Block>> {
        match &self.notes {
            Notes::Read(blocks) => Some(blocks.clone()),
            _ => None,
        }
    }
}

impl Render for NotesBody {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = &self.palette;
        let mono = cx.theme().mono_font_family.clone();
        let look = crate::prose::Look {
            size: 13.,
            leading: 20.5,
            tone: palette.fg,
        };
        let prose = |text: &str| {
            crate::prose::render_look(
                crate::prose::parse(text),
                palette,
                mono.clone(),
                &crate::prose::Picking::quiet(Vec::new()),
                look,
            )
            .into_any_element()
        };
        let aside = |text: &'static str| {
            div()
                .text_size(px(12.5))
                .line_height(px(19.))
                .text_color(paint(palette.muted))
                .child(text)
                .into_any_element()
        };

        let body: Vec<AnyElement> = match &self.notes {
            Notes::Reading => vec![aside("Reading the notes…")],
            Notes::Away => vec![aside(
                "These notes are not kept here yet, and GitHub did not answer. \
                 They are on the release page whenever you are back online.",
            )],
            Notes::Read(blocks) if blocks.is_empty() => {
                vec![aside("This release came without notes.")]
            }
            Notes::Read(blocks) => blocks
                .iter()
                .enumerate()
                .map(|(ix, block)| match block {
                    Block::Heading(text) => div()
                        .when(ix > 0, |this| this.pt(px(10.)))
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(paint(palette.accent))
                        .child(text.to_uppercase())
                        .into_any_element(),
                    Block::Para(text) => prose(text),
                    Block::Item(text) => div()
                        .h_flex()
                        .items_start()
                        .gap(px(10.))
                        .child(
                            div()
                                .flex_none()
                                .mt(px(8.))
                                .size(px(4.))
                                .rounded_full()
                                .bg(paint(palette.accent)),
                        )
                        .child(div().flex_1().min_w_0().child(prose(text)))
                        .into_any_element(),
                    Block::Code(text) => div()
                        .px(px(12.))
                        .py(px(9.))
                        .rounded(px(6.))
                        .bg(paint(palette.wash))
                        .font_family(mono.clone())
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .child(text.clone())
                        .into_any_element(),
                    // As wide as the release asked, or half its pixels — the
                    // screenshots are taken on a retina screen — and never
                    // wider than the card. Not kept yet, it takes no room:
                    // the words are already readable without it.
                    Block::Image { url, width, .. } => {
                        match crate::notes::image(&self.version, url) {
                            Some((path, size)) => {
                                let wide = width
                                    .map(|width| width as f32)
                                    .or(size.map(|(width, _)| width as f32 / 2.))
                                    .unwrap_or(TEXT)
                                    .min(TEXT);
                                div()
                                    .py(px(4.))
                                    .child(
                                        img(path)
                                            .w(px(wide))
                                            .rounded(px(6.))
                                            .border_1()
                                            .border_color(paint(palette.edge)),
                                    )
                                    .into_any_element()
                            }
                            None => div().into_any_element(),
                        }
                    }
                })
                .collect(),
        };

        div()
            .id("notes-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .px(px(24.))
            .pt(px(16.))
            .pb(px(22.))
            .v_flex()
            .gap(px(10.))
            .children(body)
    }
}
