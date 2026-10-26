//! The sheet held in a named state, for taking its picture without pressing
//! anything. Debug builds only: `DECK_DEBUG_SHEET` names the state, and an
//! installed deck has none of this compiled in.
//!
//! The parts are joined with colons: `ready:pdf-landscape`,
//! `ready:html:groups=1,3:page=2`, `done:pdf-portrait:light`,
//! `ready:nocomments`, `asking`, and `loading`, which holds the pages from
//! being laid out so the sheet can be seen waiting for them.
//!
//! The sheet takes the pointer and the keyboard as it always does. A badge
//! on it says it is a snapshot, so nobody mistakes the state for one they
//! got to by hand.

use super::*;

/// The state a sheet was asked to show.
pub(super) struct Snapshot {
    /// What `DECK_DEBUG_SHEET` said.
    asked: String,
    /// Whether `asking` has opened its dialog yet.
    asked_once: bool,
}

impl ExportSheet {
    /// Put the sheet in the state `asked` names, said on stderr so a run
    /// that was not meant to have it can see where it came from.
    pub fn snapshot(&mut self, asked: &str, cx: &mut Context<Self>) {
        eprintln!("deck: DECK_DEBUG_SHEET: the export sheet is held at {asked}");
        for part in asked.split(':') {
            match part {
                "pdf-portrait" => {
                    self.choice.format = Format::Pdf;
                    self.choice.layout = Layout::Portrait;
                }
                "pdf-landscape" => {
                    self.choice.format = Format::Pdf;
                    self.choice.layout = Layout::Landscape;
                }
                "html" => self.choice.format = Format::Html,
                "light" => self.choice.theme = Theme::Light,
                "nocomments" => self.choice.comments = false,
                _ => {
                    if let Some(list) = part.strip_prefix("groups=")
                        && let Ok(numbers) = export::groups(list)
                    {
                        for (ix, on) in self.choice.groups.iter_mut().enumerate() {
                            *on = u32::try_from(ix + 1).is_ok_and(|n| numbers.contains(&n));
                        }
                    }
                    if let Some(n) = part.strip_prefix("page=") {
                        self.page = n.parse::<usize>().unwrap_or(1).saturating_sub(1);
                    }
                }
            }
        }
        self.debug = Some(Snapshot {
            asked: asked.to_string(),
            asked_once: false,
        });
        cx.notify();
    }

    fn asked(&self) -> Option<&str> {
        self.debug.as_ref().map(|shown| shown.asked.as_str())
    }

    /// Whether the pages are held from being laid out, for `loading`.
    pub(super) fn debug_holds_pages(&self) -> bool {
        self.asked()
            .is_some_and(|asked| asked.starts_with("loading"))
    }

    /// For `asking`: the save dialog asked for, once the sheet could export.
    pub(super) fn debug_render(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ready = matches!(self.scan, Scan::Clear) && self.current().is_some();
        let Some(shown) = self.debug.as_mut() else {
            return;
        };
        if shown.asked_once || !shown.asked.starts_with("asking") || !ready {
            return;
        }
        shown.asked_once = true;
        cx.defer_in(window, |sheet, window, cx| sheet.export(window, cx));
    }

    /// For `done`: the line a real export shows, from the real bytes, at the
    /// path the dialog would have offered. Nothing is written.
    pub(super) fn debug_laid(&mut self, cx: &mut Context<Self>) {
        if !self.asked().is_some_and(|asked| asked.starts_with("done"))
            || !matches!(self.phase, Phase::Ready)
        {
            return;
        }
        let (Some(packing), Some(laid)) = (self.packing.clone(), self.current()) else {
            return;
        };
        let painted = self.painted(&self.choice.theme);
        let options = self.choice.options(painted.dark);
        let shape = self.choice.shape();
        let pages = shape.map(|_| laid.pages.len());
        let path = model::start_folder(
            crate::state::remembered_export_dir(),
            self.source.project.as_deref(),
            deck_core::home::home(),
        )
        .join(model::file_name(&packing.name, &self.choice));
        self.phase = Phase::Writing;
        self.writing = cx.spawn(async move |this, cx| {
            let bytes = cx
                .background_executor()
                .spawn(async move {
                    crate::cli::written(
                        &packing.bundle,
                        &packing.name,
                        &painted.palette,
                        painted.syntax,
                        &options,
                        shape,
                    )
                    .map(|(bytes, _)| bytes.len())
                })
                .await;
            this.update(cx, |sheet, cx| {
                sheet.phase = match bytes {
                    Ok(bytes) => Phase::Saved(Saved { path, bytes, pages }),
                    Err(err) => Phase::Failed(format!("{err:#}")),
                };
                cx.notify();
            })
            .ok();
        });
    }

    /// The badge that says the sheet is a snapshot, and of what.
    pub(super) fn debug_badge(&self, palette: &Palette) -> Option<Div> {
        self.asked().map(|asked| {
            div()
                .absolute()
                .top(px(8.))
                .left(px(8.))
                .px(px(8.))
                .py(px(3.))
                .rounded(px(crate::radius::CAP))
                .bg(paint(palette.band))
                .border_1()
                .border_color(paint(palette.edge))
                .text_size(px(11.))
                .text_color(paint(palette.muted))
                .child(format!("debug snapshot: {asked}"))
        })
    }
}
