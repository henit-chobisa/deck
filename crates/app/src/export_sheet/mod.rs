//! The export sheet: the deck written out as a PDF or an HTML file for
//! somebody who does not have deck.
//!
//! A sheet over the dimmed window, in two columns. On the left, what to
//! write — the format, which way up, the colours, which groups, whether what
//! was said goes with it — and what the reader should know before they do:
//! the notices the export itself has, and a secret, if the scanner finds
//! one. On the right, the pages, as the file will hold them.
//!
//! Three things it holds to.
//!
//! - **It is `deck export`.** Every control is one of the command's flags
//!   ([`model::Choice::args`]), the deck is packed or opened and scanned by
//!   the command's own code ([`crate::cli::export_packing`]), and the bytes
//!   are written by it too ([`crate::cli::written`]). The notices are the
//!   command's notices, from the same pages ([`deck_render::export::notices`]);
//!   a secret is refused in the command's words.
//! - **The preview is the file.** The pages are laid out by the export and
//!   painted box for box ([`preview`]), not drawn to look like them, and they
//!   are laid out again the moment an option changes: unticking a group
//!   takes it out of the pages, the counts, and the `.deck` inside. The
//!   HTML file is the exception: a browser sets it, so it is shown as the
//!   landscape pages, a group to a screen, and the sheet says so.
//! - **Where it goes is the reader's.** Export opens the system's own save
//!   dialog, named and placed by [`model::file_name`] and
//!   [`model::start_folder`], and writes only where it answers. There is
//!   no folder exports go to.

#[cfg(debug_assertions)]
mod debug;
mod model;
mod preview;
#[cfg(test)]
mod tests;
mod themes;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use deck_cli::pack::{Hit, Packing};
use deck_core::theme::{Palette, Rgb, Syntax};
use deck_render::export::{self, Page};
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

#[cfg(test)]
pub use model::quoted;
pub use model::{Borrowed, Choice, Format, Layout, Look, Saved, Theme};

use crate::palette::{hover_on_band, paint};

gpui_kit::actions!(
    deck_export,
    [
        CloseSheet,
        NextPage,
        PrevPage,
        NextControl,
        PrevControl,
        Choose,
        Hold
    ]
);

/// The sheet's keys, in the context it claims while it is up: escape puts it
/// away, the arrows page through the preview, tab and shift-tab move between
/// its controls, and space presses the one with the ring.
///
/// No letter: an export is chosen, never a key pressed on the way to
/// somewhere else. Return is held and does nothing, since gpui would
/// otherwise press whatever control has the focus with it.
#[must_use]
pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("escape", CloseSheet, Some("DeckExport")),
        KeyBinding::new("left", PrevPage, Some("DeckExport")),
        KeyBinding::new("right", NextPage, Some("DeckExport")),
        KeyBinding::new("tab", NextControl, Some("DeckExport")),
        KeyBinding::new("shift-tab", PrevControl, Some("DeckExport")),
        KeyBinding::new("space", Choose, Some("DeckExport")),
        KeyBinding::new("enter", Hold, Some("DeckExport")),
    ]
}

/// A control on the sheet that the keyboard can reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Control {
    Pdf,
    Html,
    Portrait,
    Landscape,
    Theme,
    Every,
    Group(usize),
    Comments,
    /// The filter over the theme list.
    Filter,
    /// One of deck's own themes in the list: the window's, dark, light.
    Own(usize),
    /// An editor's theme in the list, by its place among them.
    Row(usize),
    /// The first click on a secret's *let through*, by its place in the list.
    Allow(usize),
    /// The second, which lets it through.
    AllowSure(usize),
    /// Changing your mind between the two.
    AllowCancel(usize),
    Export,
    /// Once a file is saved: put the sheet away.
    Done,
    /// Once a file is saved: back to the choices, to write another.
    Again,
}

/// The answer a save dialog will give: the path chosen, `None` for Cancel,
/// or why no dialog could be shown.
type Answer = futures::future::LocalBoxFuture<'static, Result<Option<PathBuf>, String>>;

/// Ask where to save `name`, opening in `start`: the answer, and a way to
/// take the dialog down when the answer is no longer wanted, where there is
/// one.
///
/// On a Mac the panel hangs from the deck's own window as a sheet
/// ([`crate::mac::save_panel`]), since the deck floats above gpui's
/// free-standing panel, and the deck can end that sheet itself. Elsewhere it
/// is gpui's `prompt_for_new_path`, which gives nothing to take the dialog
/// down with: the wait for it can only be dropped.
fn ask_where(
    start: &std::path::Path,
    name: &str,
    window: &mut Window,
    cx: &mut App,
) -> (Answer, Option<Box<dyn FnOnce()>>) {
    #[cfg(target_os = "macos")]
    if let Some((answer, put_away)) = crate::mac::save_panel(window, start, name) {
        let answer: Answer = Box::pin(async move { Ok(answer.await.unwrap_or(None)) });
        return (answer, Some(put_away));
    }
    let _ = window;
    let answer = cx.prompt_for_new_path(start, Some(name));
    let answer: Answer = Box::pin(async move {
        match answer.await {
            Ok(Ok(path)) => Ok(path),
            Ok(Err(err)) => Err(format!("the save dialog would not open: {err}")),
            Err(_) => Ok(None),
        }
    });
    (answer, None)
}

/// How the window is painted, said once at start.
static WINDOW_LOOK: std::sync::OnceLock<Look> = std::sync::OnceLock::new();

/// What a window that never said is taken to be in: deck's own colours, the
/// machine's mode, grey paper — the config's defaults.
static UNTOLD: Look = Look {
    borrowed: None,
    mode: deck_core::theme::Mode::Auto,
    paper: deck_core::theme::Paper::Grey,
};

/// Say how the window is painted: which editor's theme it borrowed, `None`
/// when it is in deck's own palette, and the mode and paper it was opened in.
pub fn name_theme(look: Look) {
    let _ = WINDOW_LOOK.set(look);
}

fn window_look() -> &'static Look {
    WINDOW_LOOK.get().unwrap_or(&UNTOLD)
}

fn window_theme() -> Option<&'static Borrowed> {
    window_look().borrowed.as_ref()
}

/// The deck to export, as the window knows it.
#[derive(Debug, Clone)]
pub struct Source {
    /// The deck folder, or the `.deck` file it was opened from: what `deck
    /// export` would be handed.
    pub deck: PathBuf,
    /// The review as it stands in the window, comments not yet submitted and
    /// all, to carry in place of the one on disk or in the file; `None` when
    /// nothing has been said in the window, and what is there goes as it is.
    pub review: Option<deck_core::Review>,
    /// The project it was written against, or the folder deck was started in,
    /// for where the first save dialog opens.
    pub project: Option<PathBuf>,
    /// Its title.
    pub title: String,
    /// Each group's title, in order.
    pub titles: Vec<String>,
    /// Whether the machine is set to dark, which decides an editor's theme
    /// when it has one for each.
    pub machine_dark: bool,
}

/// The colours a page is painted in.
#[derive(Debug, Clone, Copy)]
pub struct Painted {
    /// The palette.
    pub palette: Palette,
    /// Whether it is dark.
    pub dark: bool,
    /// The syntax colours.
    pub syntax: Syntax,
}

/// Emitted when the sheet should be put away.
pub struct Closed;

/// What the scanner says of the groups chosen.
#[derive(Debug, Clone)]
enum Scan {
    /// Not read yet.
    Reading,
    /// Nothing in what would be sent looks like a secret.
    Clear,
    /// Things that do, nobody has let through.
    Found(Vec<Hit>),
    /// Refused for another reason, in the command's words: a file the deck
    /// points at that is not there, or a deck that would not read.
    Refused(String),
}

/// Where an export is.
#[derive(Debug, Clone)]
enum Phase {
    /// Nothing asked yet.
    Ready,
    /// The save dialog is up.
    Asking,
    /// Being written where the dialog said.
    Writing,
    /// Written.
    Saved(Saved),
    /// It went wrong, and this is why.
    Failed(String),
}

impl Phase {
    /// Where *Export again* goes: back to ready, with every choice as it
    /// was, from a file saved or an export that failed. Nothing else is
    /// taken back; a dialog or a write under way is not the reader's to
    /// restart.
    fn again(&self) -> Self {
        match self {
            Self::Saved(_) | Self::Failed(_) => Self::Ready,
            other => other.clone(),
        }
    }
}

/// How the sheet and the theme list come and go: the surface pace the cards
/// over the window have, in quick enough to feel like the press caused it,
/// out a little quicker, so putting it away is something you see happen.
fn surface() -> crate::pane::Fade {
    crate::pane::Fade::default().paced(
        std::time::Duration::from_millis(200),
        std::time::Duration::from_millis(160),
    )
}

/// How a thumbnail answers a pointer or a choice: the feedback pace.
fn feedback() -> crate::pane::Fade {
    crate::pane::Fade::default().paced(
        std::time::Duration::from_millis(120),
        std::time::Duration::from_millis(200),
    )
}

/// How far a small page is lit: chosen, and under the pointer.
#[derive(Debug, Clone, Copy)]
struct Thumb {
    chosen: crate::pane::Fade,
    hovered: crate::pane::Fade,
}

impl Thumb {
    fn new() -> Self {
        Self {
            chosen: feedback(),
            hovered: feedback(),
        }
    }

    /// How opaque it is drawn, from the level it is dimmed to when it is
    /// neither chosen nor under the pointer, to whole when it is either.
    fn opacity(&self) -> f32 {
        let lit = self.chosen.level().max(self.hovered.level());
        DIMMED + (1. - DIMMED) * lit
    }

    fn moving(&self) -> bool {
        self.chosen.moving() || self.hovered.moving()
    }
}

/// How dim a small page is while it is not the big one.
const DIMMED: f32 = 0.7;

/// One option set's pages, laid out.
struct Laid {
    pages: Vec<Arc<Page>>,
    labels: Vec<String>,
    notices: Vec<String>,
}

/// The pages one choice lays out, what each is called, and what the export
/// will say about them.
type Laying = (Vec<Page>, Vec<String>, Vec<String>);

/// What a set of pages is known by: everything that changes them.
type Key = (export::Format, Theme, Vec<u32>, bool, Format);

/// How many option sets' pages are kept laid out: enough to click back and
/// forth between a few without waiting, few enough that a long session of
/// clicking does not hold every layout it ever made.
const LAID_KEPT: usize = 6;

/// The sheet.
pub struct ExportSheet {
    focus: FocusHandle,
    /// The window's palette, which the sheet itself is painted in.
    palette: Palette,
    window: Painted,
    source: Source,
    choice: Choice,
    /// Which page of the preview is the big one.
    page: usize,
    /// Fingerprints let through from here, as `--allow` would.
    allow: Vec<String>,
    /// The deck, packed or opened, with everything let through so far.
    packing: Option<Arc<Packing>>,
    /// What was said about each group, and in all.
    said: Vec<usize>,
    remarks: usize,
    answers: usize,
    scan: Scan,
    /// A secret's fingerprint whose *let through* was clicked once, waiting
    /// on the second click that says it was checked.
    confirming: Option<String>,
    /// The last few option sets' pages, oldest first.
    laid: Vec<(Key, Arc<Laid>)>,
    /// Why the pages for an option set could not be laid out.
    unlaid: Option<(Key, String)>,
    /// Each control's focus, made the first time it is drawn.
    stops: HashMap<Control, FocusHandle>,
    /// The controls drawn this frame, in the order tab moves through them.
    order: Vec<Control>,
    /// The `[theme]` the config says, which an export is painted under.
    config: deck_core::config::Theme,
    /// Every editor's theme on this machine, as far as it has been read.
    editors: Vec<themes::Row>,
    /// The theme list, while it is open.
    picking: Option<themes::Picking>,
    /// Where the theme field is, for the list to hang from.
    field: Bounds<Pixels>,
    /// Whether the command line under the preview has just been copied.
    copied: bool,
    discovering: Task<()>,
    asking_nvim: Task<()>,
    /// Whether Neovim's answer, when it comes, is to be chosen: set when its
    /// row is clicked, and cleared when the list shuts, by a pick or not. A
    /// theme picked while Neovim was still starting is the reader's later
    /// word, and an answer that arrives after it must not take it back.
    nvim_on_answer: bool,
    /// The pages last shown, kept up — dimmed, and said to be updating —
    /// while the pages for a changed option are laid out, so the preview
    /// never goes blank between two of them.
    last: Option<Arc<Laid>>,
    drawings: Arc<preview::Drawings>,
    /// The row of small pages, kept scrolled to the big one: with twenty
    /// groups it runs well past the sheet's edge.
    thumbs: ScrollHandle,
    phase: Phase,
    /// How far the sheet has come in. Put away, it goes out on this, and is
    /// let go of by the window only once it is out.
    fade: crate::pane::Fade,
    /// How far the theme list has come in.
    list_fade: crate::pane::Fade,
    /// The theme list on its way out: shut, but still drawn until it has
    /// faded, with the filter it had.
    parting: Option<themes::Picking>,
    /// The small pages' lights, one for each.
    thumb_lights: Vec<Thumb>,
    /// Takes the save dialog down, while it is up and the deck has a way to:
    /// for when the reader stops waiting on it from the sheet.
    put_away: Option<Box<dyn FnOnce()>>,
    reading: Task<()>,
    laying: Task<()>,
    scanning: Task<()>,
    writing: Task<()>,
    /// The state it is held in for a screenshot.
    #[cfg(debug_assertions)]
    debug: Option<debug::Snapshot>,
}

impl EventEmitter<Closed> for ExportSheet {}

impl Focusable for ExportSheet {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ExportSheet {
    /// A sheet for `source`, painted in `palette`, exporting by default in
    /// the colours the window is `painted` in — exactly those, which "As the
    /// window" paints the preview and the file in, rather than working them
    /// out again from the config the window may have been told to go over.
    /// It starts reading the deck at once: packing a folder reads every file
    /// it points at.
    pub fn new(source: Source, palette: Palette, painted: Painted, cx: &mut Context<Self>) -> Self {
        preview::embed_faces(cx);
        let mut sheet = Self {
            focus: cx.focus_handle(),
            palette,
            window: painted,
            choice: Choice::new(source.titles.len()),
            source,
            page: 0,
            allow: Vec::new(),
            packing: None,
            said: Vec::new(),
            remarks: 0,
            answers: 0,
            scan: Scan::Reading,
            confirming: None,
            laid: Vec::new(),
            unlaid: None,
            stops: HashMap::new(),
            order: Vec::new(),
            config: deck_core::config::Theme::default(),
            editors: Vec::new(),
            picking: None,
            field: Bounds::default(),
            copied: false,
            discovering: Task::ready(()),
            asking_nvim: Task::ready(()),
            nvim_on_answer: false,
            last: None,
            drawings: Arc::new(preview::Drawings::new()),
            thumbs: ScrollHandle::new(),
            phase: Phase::Ready,
            fade: surface(),
            list_fade: surface(),
            parting: None,
            thumb_lights: Vec::new(),
            put_away: None,
            reading: Task::ready(()),
            laying: Task::ready(()),
            scanning: Task::ready(()),
            writing: Task::ready(()),
            #[cfg(debug_assertions)]
            debug: None,
        };
        sheet.fade.set(true);
        sheet.read(cx);
        sheet.discover(cx);
        sheet
    }

    /// Pack or open the deck, as `deck export` does, with whatever has been
    /// let through; then lay it out and scan it.
    fn read(&mut self, cx: &mut Context<Self>) {
        let deck = self.source.deck.clone();
        let review = self.source.review.clone();
        let allow = self.allow.clone();
        let painted = self.window;
        self.reading = cx.spawn(async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn(async move {
                    let packing = crate::cli::export_packing(&deck, &allow, false, review)?;
                    // What was said about each group, from the document the
                    // contents are written from.
                    let document = export::paged(
                        &packing.bundle,
                        &painted.palette,
                        painted.syntax,
                        &export::Options::default(),
                    )
                    .map_err(anyhow::Error::msg)?;
                    let said: Vec<(u32, usize, usize)> = document
                        .contents
                        .iter()
                        .map(|entry| (entry.n, entry.remarks, entry.answers))
                        .collect();
                    // What the command would paint under: read here, off
                    // the window's thread, like the deck.
                    let config = crate::config::read()
                        .map(|config| config.theme)
                        .unwrap_or_else(|err| {
                            eprintln!("deck: {err:#}");
                            deck_core::config::Theme::default()
                        });
                    anyhow::Ok((packing, said, config))
                })
                .await;
            this.update(cx, |sheet, cx| {
                match read {
                    Ok((packing, said, config)) => {
                        sheet.config = config;
                        sheet.said = vec![0; sheet.choice.groups.len()];
                        for (n, remarks, answers) in &said {
                            if let Some(slot) = usize::try_from(*n)
                                .ok()
                                .and_then(|n| sheet.said.get_mut(n.wrapping_sub(1)))
                            {
                                *slot = remarks + answers;
                            }
                        }
                        sheet.remarks = said.iter().map(|(_, r, _)| r).sum();
                        sheet.answers = said.iter().map(|(_, _, a)| a).sum();
                        sheet.packing = Some(Arc::new(packing));
                        sheet.lay_out(cx);
                        sheet.rescan(cx);
                    }
                    Err(err) => sheet.scan = Scan::Refused(format!("{err:#}")),
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// The palette, darkness and syntax `theme` paints in, as `deck export
    /// --theme` paints them ([`crate::cli::painted_for`]), from colours
    /// already read: painting never waits on a disk or a process.
    ///
    /// The window's own is the window's palette itself, not the config's
    /// answer for the theme it borrowed. The two part whenever the window was
    /// opened with `--mode` or `--paper` over the config — a `deck open --mode
    /// dark` on a machine whose config says light would otherwise preview
    /// and export light under a label that says dark. The command line it
    /// copies carries the same flags ([`Choice::args`]), so it paints this too.
    fn painted(&self, theme: &Theme) -> Painted {
        if *theme == Theme::Window {
            return self.window;
        }
        crate::cli::painted_for(
            &theme.flag(window_theme()),
            &self.config,
            self.source.machine_dark,
            |editor, name, _| self.imported(editor, name),
        )
    }

    fn key(&self) -> Key {
        (
            self.choice.previewed(),
            self.choice.theme.clone(),
            self.choice.numbers(),
            self.choice.comments,
            self.choice.format,
        )
    }

    /// The pages for what is asked now, laid out off the window's thread
    /// and kept, so going back to an option set is instant. Asked again
    /// before it is done, the last one asked wins: a click through four
    /// themes lays out the one it stops on.
    fn lay_out(&mut self, cx: &mut Context<Self>) {
        let Some(packing) = self.packing.clone() else {
            return;
        };
        #[cfg(debug_assertions)]
        if self.debug_holds_pages() {
            return;
        }
        let key = self.key();
        if self.laid.iter().any(|(laid, _)| *laid == key) {
            self.settle_page();
            return;
        }
        let painted = self.painted(&self.choice.theme);
        let choice = self.choice.clone();
        let titles = self.source.titles.clone();
        self.laying = cx.spawn(async move |this, cx| {
            // A moment for the next click, so a run of them lays out once.
            cx.background_executor()
                .timer(std::time::Duration::from_millis(40))
                .await;
            let laid = cx
                .background_executor()
                .spawn(async move { lay(&packing.bundle, painted, &choice, &titles) })
                .await;
            this.update(cx, |sheet, cx| {
                match laid {
                    Ok((pages, labels, notices)) => {
                        let drawn =
                            preview::draw_diagrams(&pages, &sheet.drawings, &cx.svg_renderer());
                        if !drawn.is_empty() {
                            let mut all = (*sheet.drawings).clone();
                            all.extend(drawn);
                            sheet.drawings = Arc::new(all);
                        }
                        let pages = pages.into_iter().map(Arc::new).collect();
                        if sheet.laid.len() >= LAID_KEPT {
                            sheet.laid.remove(0);
                        }
                        sheet.laid.push((
                            key,
                            Arc::new(Laid {
                                pages,
                                labels,
                                notices,
                            }),
                        ));
                        sheet.settle_page();
                        sheet.thumbs.scroll_to_item(sheet.page);
                        #[cfg(debug_assertions)]
                        sheet.debug_laid(cx);
                    }
                    Err(err) => sheet.unlaid = Some((key, err)),
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// Scan what an export of the chosen groups, with or without the
    /// comments, would carry, in the command's own way, and say what it
    /// found.
    fn rescan(&mut self, cx: &mut Context<Self>) {
        let Some(packing) = self.packing.clone() else {
            return;
        };
        let groups = self.choice.numbers();
        let comments = self.choice.comments;
        // Export stays off until this scan lands: the last one was of
        // other groups.
        self.scan = Scan::Reading;
        self.confirming = None;
        self.scanning = cx.spawn(async move |this, cx| {
            let scanned = cx
                .background_executor()
                .spawn(async move { crate::cli::carrying((*packing).clone(), &groups, comments) })
                .await;
            this.update(cx, |sheet, cx| {
                sheet.scan = scan_of(scanned);
                cx.notify();
            })
            .ok();
        });
    }

    /// Let `fingerprint` through, as `--allow` would, and read the deck
    /// again with it: the manifest of what is exported then records it.
    fn let_through(&mut self, fingerprint: String, cx: &mut Context<Self>) {
        if !self.allow.contains(&fingerprint) {
            self.allow.push(fingerprint);
        }
        self.confirming = None;
        self.scan = Scan::Reading;
        self.read(cx);
        cx.notify();
    }

    fn current(&self) -> Option<Arc<Laid>> {
        let key = self.key();
        self.laid
            .iter()
            .find(|(laid, _)| *laid == key)
            .map(|(_, laid)| laid.clone())
    }

    /// Why the pages for what is asked now could not be laid out.
    fn unlaid(&self) -> Option<&str> {
        self.unlaid
            .as_ref()
            .filter(|(key, _)| *key == self.key())
            .map(|(_, why)| why.as_str())
    }

    fn settle_page(&mut self) {
        let count = self.current().map_or(0, |laid| laid.pages.len());
        if self.page >= count {
            self.page = 0;
        }
    }

    /// Something was changed: what it changes is laid out, scanned again if
    /// it changes what the file carries — the groups, or the comments — and
    /// a saved line from before goes, since it is about a file this choice
    /// would not make.
    fn changed(&mut self, carried: bool, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Saved(_) | Phase::Failed(_)) {
            self.phase = Phase::Ready;
        }
        self.copied = false;
        self.page = 0;
        self.lay_out(cx);
        if carried {
            self.rescan(cx);
        }
        cx.notify();
    }

    /// What the sheet is still waiting on before it can export, said as the
    /// button says it: `None` once there is nothing left to wait for.
    fn waiting(&self) -> Option<&'static str> {
        if self.packing.is_none() && !matches!(self.scan, Scan::Refused(_)) {
            Some("Reading the deck…")
        } else if matches!(self.scan, Scan::Reading) {
            Some("Scanning for secrets…")
        } else if self.packing.is_some() && self.current().is_none() && self.unlaid().is_none() {
            Some("Laying out the pages…")
        } else {
            match self.phase {
                Phase::Asking => Some("Choosing where…"),
                Phase::Writing => Some("Writing it…"),
                _ => None,
            }
        }
    }

    /// Whether Export can be pressed, and what it says.
    fn button(&self) -> (String, bool) {
        let laid = self.current();
        let pages = laid.as_ref().map(|laid| laid.pages.len());
        let blocked = blocks(&self.scan) || matches!(self.phase, Phase::Asking | Phase::Writing);
        model::button(&self.choice, pages, blocked)
    }

    /// The button: ask where, then write it there.
    fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (_, ready) = self.button();
        let (Some(packing), Some(laid)) = (self.packing.clone(), self.current()) else {
            return;
        };
        if !ready {
            return;
        }
        let name = model::file_name(&packing.name, &self.choice);
        let start = model::start_folder(
            crate::state::remembered_export_dir(),
            self.source.project.as_deref(),
            deck_core::home::home(),
        );
        let painted = self.painted(&self.choice.theme);
        let options = self.choice.options(painted.dark);
        let shape = self.choice.shape();
        let pages = shape.map(|_| laid.pages.len());
        self.phase = Phase::Asking;
        let (answer, put_away) = ask_where(&start, &name, window, cx);
        self.put_away = put_away;
        self.writing = cx.spawn(async move |this, cx| {
            let answered = answer.await;
            // Answered: the dialog has gone by itself, and there is nothing
            // left to take down.
            this.update(cx, |sheet, _| sheet.put_away = None).ok();
            let asked = match answered {
                Ok(path) => path,
                Err(err) => {
                    this.update(cx, |sheet, cx| {
                        sheet.phase = Phase::Failed(err);
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let Some(asked) = asked else {
                // Cancel: nothing written, nothing said.
                this.update(cx, |sheet, cx| {
                    sheet.phase = Phase::Ready;
                    cx.notify();
                })
                .ok();
                return;
            };
            this.update(cx, |sheet, cx| {
                sheet.phase = Phase::Writing;
                cx.notify();
            })
            .ok();
            let saved = cx
                .background_executor()
                .spawn(async move {
                    // Scanned once more, as the command scans on its way to
                    // writing, so nothing let through only on screen leaves.
                    let packing = crate::cli::carrying(
                        (*packing).clone(),
                        &options.groups,
                        options.comments,
                    )?;
                    if let Some(why) = packing.refusal() {
                        anyhow::bail!(why);
                    }
                    let (bytes, _) = crate::cli::written(
                        &packing.bundle,
                        &packing.name,
                        &painted.palette,
                        painted.syntax,
                        &options,
                        shape,
                    )?;
                    model::save(&asked, &bytes, pages)
                })
                .await;
            this.update(cx, |sheet, cx| {
                sheet.phase = match saved {
                    Ok(saved) => {
                        if let Some(dir) = saved.path.parent() {
                            crate::state::remember_export_dir(dir);
                        }
                        Phase::Saved(saved)
                    }
                    Err(err) => Phase::Failed(format!("{err:#}")),
                };
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }

    /// Take the diagrams the preview painted out of `window`'s sprite atlas,
    /// for when the sheet is put away. gpui keeps an image painted with
    /// `paint_image` there until it is dropped by hand, and each opening of
    /// the sheet draws its diagrams afresh, under new ids: left in, they
    /// would pile up for as long as the window is open.
    pub fn forget_drawings(&self, window: &mut Window) {
        for image in self.drawings.values() {
            let _ = window.drop_image(image.clone());
        }
    }

    /// Put the sheet away: it starts to fade, and says so. It is the window
    /// that lets go of it, once [`Self::gone`].
    fn close(&mut self, cx: &mut Context<Self>) {
        self.leave(cx);
        cx.emit(Closed);
    }

    /// Start fading out, if it has not.
    pub fn leave(&mut self, cx: &mut Context<Self>) {
        if self.fade.set(false) {
            cx.notify();
        }
    }

    /// Whether it has faded out all the way, and can be let go of.
    #[must_use]
    pub fn gone(&self) -> bool {
        !self.fade.on() && !self.fade.moving()
    }

    /// Again: back to the choices, as they were.
    fn again(&mut self, cx: &mut Context<Self>) {
        self.phase = self.phase.again();
        cx.notify();
    }

    /// Whether a file is being asked for or written: the sheet stays up
    /// until it says how that went, since putting it away drops the task.
    fn busy(&self) -> bool {
        matches!(self.phase, Phase::Asking | Phase::Writing)
    }

    /// Escape, or a click on the shade: the theme list shut first, when it
    /// is up, since it is over the sheet; then the sheet put away — but not
    /// while it is waiting on a save dialog or writing a file.
    ///
    /// While a dialog is asked for, it stops the waiting instead, and the
    /// sheet stays up, ready to export again. This can only reach the sheet
    /// when no dialog is in front of it to take the key — on a Mac the panel
    /// hangs from the window and takes Escape itself — so it is the way out
    /// of a dialog that never came up, or one gpui's prompt left hanging,
    /// which would otherwise hold the sheet up for good. The wait is dropped,
    /// so an answer that does come later goes nowhere, and on a Mac the panel
    /// is ended too, if it is still standing, so no dialog is left behind
    /// whose Save would write nothing.
    ///
    /// While a file is being written, nothing: the write is the reader's
    /// answer already, and what came of it is said on the sheet when it ends.
    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // On its way out: the keys are still the sheet's, and do nothing.
        if !self.fade.on() {
            return;
        }
        if self.picking.is_some() {
            self.close_themes(window, cx);
            return;
        }
        match self.phase {
            Phase::Asking => {
                self.writing = Task::ready(());
                if let Some(put_away) = self.put_away.take() {
                    put_away();
                }
                self.phase = Phase::Ready;
                cx.notify();
            }
            Phase::Writing => {}
            _ => self.close(cx),
        }
    }

    fn turn_page(&mut self, by: isize, cx: &mut Context<Self>) {
        let count = self.current().map_or(0, |laid| laid.pages.len());
        if count == 0 {
            return;
        }
        let count = isize::try_from(count).unwrap_or(1);
        let at = isize::try_from(self.page).unwrap_or(0);
        self.page = usize::try_from((at + by).rem_euclid(count)).unwrap_or(0);
        self.thumbs.scroll_to_item(self.page);
        cx.notify();
    }

    /// The controls the sheet shows as it stands, in the order they are
    /// drawn, top to bottom: the order tab moves through them.
    fn tab_order(&self, cx: &App) -> Vec<Control> {
        if self.picking.is_some() {
            return self.theme_order(cx);
        }
        let mut order = vec![Control::Pdf, Control::Html];
        if self.choice.format == Format::Pdf {
            order.extend([Control::Portrait, Control::Landscape]);
        }
        order.extend([Control::Theme, Control::Every]);
        order.extend((0..self.source.titles.len()).map(Control::Group));
        order.push(Control::Comments);
        if let Scan::Found(hits) = &self.scan {
            for (ix, hit) in hits.iter().enumerate() {
                if self.confirming.as_deref() == Some(hit.fingerprint.as_str()) {
                    order.extend([Control::AllowSure(ix), Control::AllowCancel(ix)]);
                } else {
                    order.push(Control::Allow(ix));
                }
            }
        }
        if matches!(self.phase, Phase::Saved(_)) {
            order.extend([Control::Done, Control::Again]);
        } else {
            order.push(Control::Export);
        }
        order
    }

    /// `control`'s focus, made if it has none yet.
    fn stop(&mut self, control: Control, cx: &mut Context<Self>) -> FocusHandle {
        self.stops
            .entry(control)
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    /// The control that has the focus, if one does.
    fn focused(&self, window: &Window) -> Option<Control> {
        self.order.iter().copied().find(|control| {
            self.stops
                .get(control)
                .is_some_and(|stop| stop.is_focused(window))
        })
    }

    /// Tab, or shift-tab: the next control, or the one before, round from
    /// the last to the first, so the focus never leaves the sheet.
    fn step_focus(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let count = isize::try_from(self.order.len()).unwrap_or(0);
        if count == 0 {
            return;
        }
        let to = match self
            .focused(window)
            .and_then(|at| self.order.iter().position(|c| *c == at))
        {
            Some(at) => (isize::try_from(at).unwrap_or(0) + by).rem_euclid(count),
            None if by > 0 => 0,
            None => count - 1,
        };
        let control = self.order[usize::try_from(to).unwrap_or(0)];
        self.stop(control, cx).focus(window, cx);
        cx.notify();
    }

    /// What a click on `control` does, and space while it has the ring.
    ///
    /// Nothing while a file is asked for or written: a change made then
    /// would be to a choice the file being written no longer follows, and a
    /// secret let through then would be let through for an export already
    /// on its way. Paging the preview is not a press, and goes on.
    fn press(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || !self.fade.on() {
            return;
        }
        let fingerprint = |sheet: &Self, ix: usize| match &sheet.scan {
            Scan::Found(hits) => hits.get(ix).map(|hit| hit.fingerprint.clone()),
            _ => None,
        };
        match control {
            Control::Pdf | Control::Html => {
                self.choice.format = if control == Control::Pdf {
                    Format::Pdf
                } else {
                    Format::Html
                };
                self.changed(false, cx);
            }
            Control::Portrait | Control::Landscape => {
                self.choice.layout = if control == Control::Portrait {
                    Layout::Portrait
                } else {
                    Layout::Landscape
                };
                self.changed(false, cx);
            }
            Control::Theme => {
                if self.picking.is_some() {
                    self.close_themes(window, cx);
                } else {
                    self.open_themes(window, cx);
                }
            }
            Control::Filter => {}
            Control::Own(_) | Control::Row(_) => self.press_theme(control, window, cx),
            Control::Every => {
                let every = self.choice.chosen() == self.choice.groups.len();
                self.choice.groups.fill(!every);
                self.changed(true, cx);
            }
            Control::Group(ix) => {
                if let Some(on) = self.choice.groups.get_mut(ix) {
                    *on = !*on;
                }
                self.changed(true, cx);
            }
            Control::Comments => {
                self.choice.comments = !self.choice.comments;
                // A secret said only in a comment goes with the comments.
                self.changed(true, cx);
            }
            Control::Allow(ix) => {
                self.confirming = fingerprint(self, ix);
                // The ring follows the link to what it turned into, and
                // lands on changing your mind rather than on letting it
                // through, so space pressed twice lets nothing through.
                if let Some(next) = ring_after(control) {
                    self.stop(next, cx).focus(window, cx);
                }
                cx.notify();
            }
            Control::AllowSure(ix) => {
                if let Some(fingerprint) = fingerprint(self, ix) {
                    self.let_through(fingerprint, cx);
                }
            }
            Control::AllowCancel(_) => {
                self.confirming = None;
                if let Some(next) = ring_after(control) {
                    self.stop(next, cx).focus(window, cx);
                }
                cx.notify();
            }
            Control::Export => self.export(window, cx),
            Control::Done => self.close(cx),
            Control::Again => self.again(cx),
        }
    }

    /// What `theme` is called on the field and in the list.
    fn label_of(&self, theme: &Theme) -> String {
        model::label(theme, window_theme(), self.window.dark)
    }
}

/// Where the ring goes once `control` is pressed, for the presses that
/// take the control away and put another in its place: the first step of a
/// let-through hands it to *cancel*, the safe one of the two that follow, and
/// *cancel* hands it back to the first step. The rest keep the ring where it
/// is. Letting a secret through is always a choice made on purpose, never
/// where the focus happens to fall.
fn ring_after(control: Control) -> Option<Control> {
    match control {
        Control::Allow(ix) => Some(Control::AllowCancel(ix)),
        Control::AllowCancel(ix) => Some(Control::Allow(ix)),
        _ => None,
    }
}

/// The pages `choice` lays `bundle` out as, painted as `painted`, what each
/// is called, and what the person exporting should be told: the sheet's
/// whole preview, as one function a test can hold to `deck export`.
///
/// # Errors
///
/// When a part of the bundle does not parse.
fn lay(
    bundle: &deck_core::bundle::Bundle,
    painted: Painted,
    choice: &Choice,
    titles: &[String],
) -> Result<Laying, String> {
    let options = choice.options(painted.dark);
    let document = export::paged(bundle, &painted.palette, painted.syntax, &options)?;
    let mut pages = export::layout(&document, choice.previewed());
    // The HTML file has no pages to run over and no faces to borrow: the
    // browser sets it. What it is told is what `deck export` prints for it,
    // which is nothing.
    let notices = if choice.format == Format::Pdf {
        export::notices(&pages)
    } else {
        // And it is the window a group to a screen: each group's page, not
        // the argument pages landscape paper puts after it, which the window
        // says in its rail.
        pages.retain(|page| matches!(page.kind, export::Kind::Group { .. }));
        Vec::new()
    };
    let title = |n: u32| {
        usize::try_from(n)
            .ok()
            .and_then(|n| titles.get(n.wrapping_sub(1)).cloned())
    };
    let labels = pages
        .iter()
        .map(|page| model::page_label(page.kind, title))
        .collect();
    Ok((pages, labels, notices))
}

/// What a scan of the chosen groups comes to on the sheet: a secret first,
/// since it is the one thing the reader can let through from here, then any
/// other refusal in the command's words, and otherwise clear.
fn scan_of(scanned: anyhow::Result<Packing>) -> Scan {
    match scanned {
        Ok(packing) if !packing.hits.is_empty() => Scan::Found(packing.hits),
        Ok(packing) => packing.refusal().map_or(Scan::Clear, Scan::Refused),
        Err(err) => Scan::Refused(format!("{err:#}")),
    }
}

/// Whether what the scanner says leaves Export off.
fn blocks(scan: &Scan) -> bool {
    !matches!(scan, Scan::Clear)
}

/// `element` as a stop on the sheet's tab path, ringed when the keyboard
/// put the focus on it: two points of accent, a point clear of it.
fn ringed(element: Stateful<Div>, stop: &FocusHandle, palette: &Palette) -> Stateful<Div> {
    let ring = |colour: Rgb, spread: f32| BoxShadow {
        color: paint(colour),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(spread),
        inset: false,
    };
    let shadows = vec![ring(palette.accent, 4.), ring(palette.band, 2.)];
    element
        .track_focus(stop)
        .focus_visible(move |style| style.shadow(shadows))
}

/// A choice's look, on or off or not yet there.
fn choice(
    id: &'static str,
    title: &'static str,
    sub: &'static str,
    on: bool,
    disabled: bool,
    palette: &Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .v_flex()
        .gap(px(1.))
        .px(px(10.))
        .py(px(8.))
        .rounded(px(crate::radius::CONTROL))
        .border_1()
        .border_color(paint(if on { palette.accent } else { palette.edge }))
        .bg(paint(if on {
            palette.wash.mix(palette.accent, 0.12)
        } else {
            palette.wash
        }))
        .when(disabled, |this| this.opacity(0.45))
        .when(!disabled, |this| this.cursor_pointer())
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(paint(palette.fg))
                .child(title),
        )
        .child(
            div()
                .text_size(px(11.))
                .text_color(paint(palette.muted))
                .child(sub),
        )
}

/// `doing`, beside a dot that breathes: the strip's way of saying a process
/// is under way rather than a state, used for everything the sheet waits on.
fn breathing(id: &'static str, doing: &'static str, palette: &Palette) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(7.))
        .text_size(px(12.))
        .text_color(paint(palette.muted))
        .child(
            div()
                .flex_none()
                .size(px(6.))
                .rounded_full()
                .bg(paint(palette.accent))
                .with_animation(
                    id,
                    Animation::new(std::time::Duration::from_millis(1900))
                        .repeat()
                        .with_easing(pulsating_between(0.25, 1.0)),
                    |this, breath| this.opacity(breath),
                ),
        )
        .child(doing)
}

/// A box that is ticked, empty, or — `None` — a dash for some of what it
/// stands for.
fn tick_some(on: Option<bool>, palette: &Palette) -> Div {
    match on {
        Some(on) => tick(on, palette),
        None => ground(true, palette).child(
            div()
                .w(px(7.))
                .h(px(1.5))
                .rounded(px(1.))
                .bg(paint(palette.on_accent)),
        ),
    }
}

/// A box that is ticked or not.
fn tick(on: bool, palette: &Palette) -> Div {
    ground(on, palette).child(if on { "✓" } else { "" })
}

/// A tick box with no mark in it yet: filled in the accent, or an outline.
fn ground(filled: bool, palette: &Palette) -> Div {
    div()
        .flex_none()
        .mt(px(1.))
        .size(px(15.))
        .rounded(px(crate::radius::CAP))
        .border_1()
        .border_color(paint(if filled {
            palette.accent
        } else {
            palette.muted
        }))
        .when(filled, |this| this.bg(paint(palette.accent)))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.))
        .line_height(px(10.))
        .text_color(paint(if filled {
            palette.on_accent
        } else {
            palette.band
        }))
}

/// A field's label: small capitals, as the design draws them.
fn label(text: &'static str, palette: &Palette) -> Div {
    div()
        .text_size(px(10.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(paint(palette.muted))
        .child(text.to_uppercase())
}

/// A notice's sentence: a capital to start, a full stop to end.
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    let mut out: String = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// A notice box in `tone`.
fn notice(tone: Rgb, palette: &Palette) -> Div {
    div()
        .v_flex()
        .gap(px(3.))
        .rounded(px(crate::radius::CARD))
        .px(px(11.))
        .py(px(9.))
        .border_1()
        .border_color(paint(tone).opacity(0.45))
        .bg(paint(tone).opacity(0.09))
        .text_size(px(12.))
        .line_height(px(17.5))
        .text_color(paint(palette.fg))
}

impl Render for ExportSheet {
    #[allow(clippy::too_many_lines)]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(debug_assertions)]
        self.debug_render(window, cx);
        let palette = self.palette;
        // Out all the way: the window may let go of it now. Said from here,
        // where the last frame of the fade has just been worked out, rather
        // than by a timer that could land before or after it.
        if self.gone() {
            cx.emit(Closed);
        }
        // The theme list, once it has faded, is let go with its filter.
        if self.picking.is_none() && !self.list_fade.moving() {
            self.parting = None;
        }
        let shown_sheet = self.fade.level();
        if self.fade.moving()
            || self.list_fade.moving()
            || self.thumb_lights.iter().any(Thumb::moving)
        {
            window.request_animation_frame();
        }
        self.order = self.tab_order(cx);
        let stops: HashMap<Control, FocusHandle> = self
            .order
            .clone()
            .into_iter()
            .map(|control| (control, self.stop(control, cx)))
            .collect();
        // A control that had the focus and is gone — the Export button
        // once a file is saved — leaves it nowhere, and the sheet's own keys
        // with it: the sheet takes it back.
        if self
            .stops
            .iter()
            .any(|(control, stop)| !stops.contains_key(control) && stop.is_focused(window))
        {
            let focus = self.focus.clone();
            cx.defer_in(window, move |_, window, cx| focus.focus(window, cx));
        }
        let ring = |element: Stateful<Div>, control: Control| match stops.get(&control) {
            Some(stop) => ringed(element, stop, &palette),
            None => element,
        };
        #[cfg(debug_assertions)]
        let badge = self.debug_badge(&palette);
        #[cfg(not(debug_assertions))]
        let badge: Option<Div> = None;
        let mono = cx.theme().mono_font_family.clone();
        let warn = crate::keys::level_colour(deck_core::protocol::Level::Medium, &palette);
        let bad = palette.del;
        let good = palette.add;
        let fresh = self.current();
        if fresh.is_some() {
            self.last.clone_from(&fresh);
        }
        // What the preview shows: the pages for what is asked, or the last
        // ones while those are laid out. Not when they could not be: the
        // last pages are of something else, and would pass for these.
        let unlaid = self.unlaid().map(str::to_string);
        let stale = fresh.is_none() && self.last.is_some() && unlaid.is_none();
        let laid = if unlaid.is_some() {
            None
        } else {
            fresh.clone().or_else(|| self.last.clone())
        };
        let waiting = self.waiting();
        let count = self.choice.groups.len();
        let (says, ready) = self.button();
        let pdf = self.choice.format == Format::Pdf;

        // The options, top to bottom as the design has them.
        let formats = div()
            .v_flex()
            .gap(px(6.))
            .child(
                div()
                    .h_flex()
                    .items_stretch()
                    .gap(px(6.))
                    .child(
                        ring(
                            choice(
                                "x-pdf",
                                "PDF",
                                "to send, print, attach",
                                pdf,
                                false,
                                &palette,
                            ),
                            Control::Pdf,
                        )
                        .on_click(cx.listener(|sheet, _, window, cx| {
                            sheet.press(Control::Pdf, window, cx);
                        })),
                    )
                    .child(
                        ring(
                            choice(
                                "x-html",
                                "HTML",
                                "one file, plays in a browser",
                                !pdf,
                                false,
                                &palette,
                            ),
                            Control::Html,
                        )
                        .on_click(cx.listener(|sheet, _, window, cx| {
                            sheet.press(Control::Html, window, cx);
                        })),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .items_stretch()
                    .gap(px(6.))
                    .child(choice("x-md", "Markdown", "coming", false, true, &palette))
                    .child(choice(
                        "x-slides", "Slides", "coming", false, true, &palette,
                    )),
            );
        let portrait = self.choice.layout == Layout::Portrait;
        // Side by side, one height, whatever each says under its name.
        let layouts = div()
            .h_flex()
            .items_stretch()
            .gap(px(6.))
            .child(
                ring(
                    choice(
                        "x-portrait",
                        "Portrait",
                        "a cover, a group a page",
                        portrait,
                        false,
                        &palette,
                    ),
                    Control::Portrait,
                )
                .on_click(cx.listener(|sheet, _, window, cx| {
                    sheet.press(Control::Portrait, window, cx);
                })),
            )
            .child(
                ring(
                    choice(
                        "x-landscape",
                        "Landscape",
                        "each page is the window",
                        !portrait,
                        false,
                        &palette,
                    ),
                    Control::Landscape,
                )
                .on_click(cx.listener(|sheet, _, window, cx| {
                    sheet.press(Control::Landscape, window, cx);
                })),
            );
        let shown = self.painted(&self.choice.theme).palette;
        let field = self.field;
        let this = cx.entity().downgrade();
        let theme = div()
            .on_prepaint(move |bounds, _, cx| {
                if bounds != field {
                    // The list hangs from the field, and was drawn this frame
                    // from where the field was last frame. Moved while the
                    // list is open — the sheet resized, the options scrolled —
                    // the list has to be drawn again to follow it, or it sits
                    // where the field used to be until something else asks for
                    // a frame.
                    this.update(cx, |sheet, cx| {
                        sheet.field = bounds;
                        if sheet.picking.is_some() {
                            cx.notify();
                        }
                    })
                    .ok();
                }
            })
            .child(
                ring(div().id("x-theme"), Control::Theme)
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(crate::radius::CONTROL))
                    .border_1()
                    .border_color(paint(if self.picking.is_some() {
                        palette.accent
                    } else {
                        palette.edge
                    }))
                    .bg(paint(palette.wash))
                    .cursor_pointer()
                    // On the way down, so the list's own click-outside does
                    // not shut it only for this to open it again.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|sheet, _, window, cx| {
                            cx.stop_propagation();
                            sheet.press(Control::Theme, window, cx);
                        }),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .h_flex()
                            .items_center()
                            .gap(px(9.))
                            .child(div().flex_none().h_flex().gap(px(3.)).children(
                                [shown.bg, shown.fg, shown.accent].map(|colour| {
                                    div()
                                        .size(px(12.))
                                        .rounded(px(crate::radius::CAP))
                                        .border_1()
                                        .border_color(paint(palette.edge))
                                        .bg(paint(colour))
                                }),
                            ))
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .text_size(px(13.))
                                    .child(self.label_of(&self.choice.theme)),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(paint(palette.accent))
                            .child(Icon::new(IconName::ChevronDown).size(px(12.))),
                    ),
            );
        let list = self.theme_list(&stops, cx);
        let groups = div()
            .id("x-groups")
            .v_flex()
            .gap(px(7.))
            .max_h(px(150.))
            .overflow_y_scroll()
            .pr(px(4.))
            // Select all: ticked when every group is, a dash when some are,
            // and pressed it ticks them all, or clears them once they are.
            .child({
                let chosen = self.choice.chosen();
                let every = chosen == self.choice.groups.len();
                ring(div().id("x-every"), Control::Every)
                    .h_flex()
                    .gap(px(9.))
                    .items_start()
                    .rounded(px(crate::radius::CONTROL))
                    .cursor_pointer()
                    .text_size(px(12.5))
                    .line_height(px(17.5))
                    .on_click(cx.listener(|sheet, _, window, cx| {
                        sheet.press(Control::Every, window, cx);
                    }))
                    .child(tick_some(
                        if every {
                            Some(true)
                        } else if chosen == 0 {
                            Some(false)
                        } else {
                            None
                        },
                        &palette,
                    ))
                    .child("Select all")
            })
            .children(self.source.titles.iter().enumerate().map(|(ix, title)| {
                let on = self.choice.groups.get(ix).copied().unwrap_or(false);
                let said = self.said.get(ix).copied().unwrap_or(0);
                ring(div().id(("x-group", ix)), Control::Group(ix))
                    .h_flex()
                    .gap(px(9.))
                    .items_start()
                    .rounded(px(crate::radius::CONTROL))
                    .cursor_pointer()
                    .text_size(px(12.5))
                    .line_height(px(17.5))
                    .on_click(cx.listener(move |sheet, _, window, cx| {
                        sheet.press(Control::Group(ix), window, cx);
                    }))
                    .child(tick(on, &palette))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(format!("{} · {title}", ix + 1)),
                    )
                    .when(said > 0, |this| {
                        this.child(
                            div()
                                .flex_none()
                                .text_size(px(11.5))
                                .text_color(paint(palette.muted))
                                .child(model::comments(said)),
                        )
                    })
            }));
        let comments = ring(div().id("x-comments"), Control::Comments)
            .h_flex()
            .gap(px(9.))
            .items_start()
            .rounded(px(crate::radius::CONTROL))
            .cursor_pointer()
            .text_size(px(12.5))
            .line_height(px(17.5))
            .on_click(cx.listener(|sheet, _, window, cx| {
                sheet.press(Control::Comments, window, cx);
            }))
            .child(tick(self.choice.comments, &palette))
            .child(
                div().v_flex().child("Include comments").child(
                    div()
                        .text_size(px(11.5))
                        .text_color(paint(palette.muted))
                        .child(match (self.remarks, self.answers) {
                            (0, 0) => "Nothing has been said about this deck yet.".to_string(),
                            (remarks, answers) => format!(
                                "The argument after each group: {remarks} remark{} and \
                                     {answers} answer{}.",
                                if remarks == 1 { "" } else { "s" },
                                if answers == 1 { "" } else { "s" },
                            ),
                        }),
                ),
            );

        // What the export has to say before it is made.
        let notices = fresh
            .as_ref()
            .filter(|laid| !laid.notices.is_empty())
            .map(|laid| {
                notice(warn, &palette)
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(paint(warn))
                            .child("Before you export"),
                    )
                    .children(laid.notices.iter().map(|said| div().child(sentence(said))))
            });
        let secret = match &self.scan {
            Scan::Found(hits) => Some(
                notice(bad, &palette)
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(paint(bad))
                            .child(sentence(&deck_cli::pack::looks_like(hits.len()))),
                    )
                    .children(hits.iter().map(|hit| {
                        div()
                            .font_family(mono.clone())
                            .text_size(px(11.))
                            .text_color(paint(palette.fg.mix(palette.band, 0.15)))
                            .child(hit.to_string())
                    }))
                    .child(div().child(format!(
                        "Take it out of the deck and export again. Or, for each one you \
                         have looked at and know is safe, let it through, and {}.",
                        deck_cli::pack::LET_THROUGH
                    )))
                    .children(hits.iter().enumerate().map(|(ix, hit)| {
                        let link = |id: (&'static str, usize), control: Control| {
                            ring(div().id(id), control)
                                .rounded(px(crate::radius::CAP))
                                .cursor_pointer()
                                .text_color(paint(palette.accent))
                                .hover(|this| this.text_color(paint(palette.fg)))
                                .on_click(cx.listener(move |sheet, _, window, cx| {
                                    sheet.press(control, window, cx);
                                }))
                        };
                        // Two clicks, the second saying it was looked at: one
                        // click near the button should never send a secret.
                        if self.confirming.as_deref() == Some(hit.fingerprint.as_str()) {
                            div()
                                .mt(px(3.))
                                .h_flex()
                                .flex_wrap()
                                .gap(px(12.))
                                .child(format!("{}:", hit.fingerprint))
                                .child(
                                    link(("x-allow-sure", ix), Control::AllowSure(ix))
                                        .child("Let it through — I checked it"),
                                )
                                .child(
                                    link(("x-allow-cancel", ix), Control::AllowCancel(ix))
                                        .child("Cancel"),
                                )
                                .into_any_element()
                        } else {
                            link(("x-allow", ix), Control::Allow(ix))
                                .mt(px(3.))
                                .child(format!("Let {} through", hit.fingerprint))
                                .into_any_element()
                        }
                    })),
            ),
            Scan::Refused(why) => Some(
                notice(bad, &palette)
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(paint(bad))
                            .child("This deck cannot be exported as it is."),
                    )
                    .child(div().whitespace_normal().child(why.clone())),
            ),
            Scan::Reading | Scan::Clear => None,
        };
        let failed = match &self.phase {
            Phase::Failed(why) => Some(
                notice(bad, &palette)
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(paint(bad))
                            .child("It was not exported."),
                    )
                    .child(div().child(sentence(why))),
            ),
            _ => None,
        };
        let trust = div()
            .h_flex()
            .gap(px(8.))
            .items_start()
            .text_size(px(11.5))
            .line_height(px(16.5))
            .text_color(paint(palette.muted))
            // No mark: deck's icons have no lock or shield, and a tick box here
            // read as one more setting beside the real ones. The text keeps
            // the column a mark would have had (14px and the gap), so it
            // lines up with the tick boxes' text.
            .child(div().flex_none().w(px(14.)))
            .child(div().flex_1().min_w_0().child(
                "Leaves the machine the way a .deck does: paths taken out, scanned for \
                 secrets, the deck itself inside.",
            ));
        // A button that is a button beside the real ones: edged, washed on
        // hover. Reveal and Open are small ones; Export again is a large one.
        let secondary = |id: &'static str| {
            div()
                .id(id)
                .flex_none()
                .cursor_pointer()
                .border_1()
                .border_color(paint(palette.edge))
                .text_color(paint(palette.fg))
                .hover(|this| this.bg(paint(hover_on_band(&palette))))
        };
        let go = match &self.phase {
            Phase::Saved(saved) => {
                let (reveal, open) = (saved.path.clone(), saved.path.clone());
                div()
                    .v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .h_flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(10.))
                            .text_size(px(12.5))
                            .text_color(paint(palette.fg.mix(palette.band, 0.1)))
                            .child(
                                div()
                                    .flex_none()
                                    .size(px(16.))
                                    .rounded_full()
                                    .bg(paint(good))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.))
                                    .text_color(paint(palette.band))
                                    .child("✓"),
                            )
                            .child(saved.line())
                            .child(
                                secondary("x-reveal")
                                    .px(px(10.))
                                    .py(px(4.))
                                    .rounded(px(crate::radius::CONTROL))
                                    .text_size(px(12.))
                                    .on_click(move |_, _, cx| cx.reveal_path(&reveal))
                                    .child("Reveal"),
                            )
                            .child(
                                secondary("x-open")
                                    .px(px(10.))
                                    .py(px(4.))
                                    .rounded(px(crate::radius::CONTROL))
                                    .text_size(px(12.))
                                    .on_click(move |_, _, cx| cx.open_with_system(&open))
                                    .child("Open"),
                            ),
                    )
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_size(px(11.))
                            .text_color(paint(palette.muted))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(model::tilde(
                                &saved.path,
                                deck_core::home::home().as_deref(),
                            )),
                    )
                    // Where it goes from here: out, or round again with the
                    // same choices. A saved sheet was a dead end that only
                    // escape got out of.
                    .child(
                        div()
                            .h_flex()
                            .gap(px(8.))
                            .child(
                                ring(div().id("x-done"), Control::Done)
                                    .flex_1()
                                    .h_flex()
                                    .justify_center()
                                    .items_center()
                                    .px(px(12.))
                                    .py(px(10.))
                                    .rounded(px(crate::radius::CONTROL))
                                    .border_1()
                                    .border_color(paint(palette.accent))
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .bg(paint(palette.accent))
                                    .text_color(paint(palette.on_accent))
                                    .cursor_pointer()
                                    .hover(|this| {
                                        let lit = palette.accent.mix(palette.fg, 0.16);
                                        this.bg(paint(lit)).border_color(paint(lit))
                                    })
                                    .on_click(cx.listener(|sheet, _, window, cx| {
                                        sheet.press(Control::Done, window, cx);
                                    }))
                                    .child("Done"),
                            )
                            .child(
                                ring(secondary("x-again"), Control::Again)
                                    .h_flex()
                                    .justify_center()
                                    .items_center()
                                    .px(px(14.))
                                    .py(px(10.))
                                    .rounded(px(crate::radius::CONTROL))
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .on_click(cx.listener(|sheet, _, window, cx| {
                                        sheet.press(Control::Again, window, cx);
                                    }))
                                    .child("Export again"),
                            ),
                    )
            }
            _ => div().v_flex().gap(px(8.)).child(
                ring(div().id("x-export"), Control::Export)
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(crate::radius::CONTROL))
                    .border_1()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .on_click(cx.listener(|sheet, _, window, cx| {
                        sheet.press(Control::Export, window, cx);
                    }))
                    // The same size ready or not: the edge is there either way.
                    .when(ready, |this| {
                        this.bg(paint(palette.accent))
                            .border_color(paint(palette.accent))
                            .text_color(paint(palette.on_accent))
                            .cursor_pointer()
                            .hover(|this| {
                                let lit = palette.accent.mix(palette.fg, 0.16);
                                this.bg(paint(lit)).border_color(paint(lit))
                            })
                    })
                    // Muted text on the edge colour was barely there in the
                    // light themes; this is a field with nothing to press.
                    .when(!ready, |this| {
                        this.bg(paint(palette.wash))
                            .border_color(paint(palette.edge))
                            .text_color(paint(palette.muted))
                    })
                    .child(match waiting {
                        Some(doing) => breathing("x-working", doing, &palette).into_any_element(),
                        None => div().child(says).into_any_element(),
                    }),
            ),
        };

        // Every row keeps its height and the column scrolls: in a short
        // window a flex column would otherwise squeeze the title to nothing
        // to make room for the button.
        let rows: Vec<AnyElement> = vec![
            div()
                .v_flex()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Export this deck"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(paint(palette.muted))
                        .child(format!(
                            "{} · {count} group{}",
                            self.source.title,
                            if count == 1 { "" } else { "s" }
                        )),
                )
                .into_any_element(),
            div()
                .v_flex()
                .gap(px(7.))
                .child(label("Format", &palette))
                .child(formats)
                .into_any_element(),
        ]
        .into_iter()
        .chain(pdf.then(|| {
            div()
                .v_flex()
                .gap(px(7.))
                .child(label("Layout", &palette))
                .child(layouts)
                .into_any_element()
        }))
        .chain([
            div()
                .v_flex()
                .gap(px(7.))
                .child(label("Theme", &palette))
                .child(theme)
                .into_any_element(),
            div()
                .v_flex()
                .gap(px(7.))
                .child(label("Groups", &palette))
                .child(groups)
                .into_any_element(),
            comments.into_any_element(),
        ])
        .collect();
        // What has to be read before pressing: never scrolled out of sight.
        let foot: Vec<AnyElement> = notices
            .map(IntoElement::into_any_element)
            .into_iter()
            .chain(secret.map(IntoElement::into_any_element))
            .chain(failed.map(IntoElement::into_any_element))
            .chain([trust.into_any_element(), go.into_any_element()])
            .collect();
        // The options scroll in a short window; the button and the line
        // under it stay where they can be pressed.
        let options = div()
            .w(px(360.))
            .flex_none()
            .v_flex()
            .border_r_1()
            .border_color(paint(palette.edge))
            .child(
                div()
                    .id("x-options")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .v_flex()
                            .gap(px(14.))
                            .px(px(20.))
                            .pt(px(18.))
                            .pb(px(14.))
                            .children(rows.into_iter().map(|row| div().flex_none().child(row))),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .v_flex()
                    .gap(px(12.))
                    .px(px(20.))
                    .pt(px(12.))
                    .pb(px(16.))
                    .border_t_1()
                    .border_color(paint(palette.edge))
                    .children(foot),
            );

        // The pages.
        let pages: Vec<Arc<Page>> = laid.as_ref().map(|l| l.pages.clone()).unwrap_or_default();
        let shown = pages.get(self.page).cloned();
        let page_label = laid
            .as_ref()
            .and_then(|l| l.labels.get(self.page).cloned())
            .unwrap_or_else(|| "laying it out".to_string());
        let unit = if pdf { "page" } else { "view" };
        // The small pages' lights follow the choice, a pointer at a time:
        // one for each page, kept as pages come and go.
        self.thumb_lights.resize_with(pages.len(), Thumb::new);
        let at = self.page;
        for (ix, light) in self.thumb_lights.iter_mut().enumerate() {
            light.chosen.set(ix == at);
        }
        let lights = self.thumb_lights.clone();
        let meta = match self.choice.shape() {
            None => "HTML · previewed as landscape pages",
            Some(export::Format::Portrait) => "portrait · A4",
            Some(export::Format::Landscape) => "landscape · A4",
        };
        let preview = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .v_flex()
            .gap(px(12.))
            .px(px(18.))
            .pt(px(16.))
            .pb(px(14.))
            .bg(paint(palette.ground))
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .gap(px(10.))
                    .font_family(mono.clone())
                    .text_size(px(11.5))
                    .text_color(paint(palette.muted))
                    .child(
                        div()
                            .h_flex()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child("Preview · ")
                            .child(
                                div()
                                    .text_color(paint(palette.fg.mix(palette.band, 0.1)))
                                    .child(page_label),
                            )
                            .when(!pages.is_empty(), |this| {
                                this.child(format!(
                                    " · {unit} {} of {}",
                                    self.page + 1,
                                    pages.len()
                                ))
                            }),
                    )
                    .child(div().flex_none().child(meta)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .children(shown.map(|page| {
                        let drawings = self.drawings.clone();
                        // A hair lighter than the window's own edge, so a
                        // black page still has a border on a black stage.
                        let page_edge = paint(palette.edge.mix(palette.muted, 0.3));
                        div()
                            .absolute()
                            .inset_0()
                            .when(stale, |this| this.opacity(0.45))
                            .child(preview::page(page, drawings, page_edge))
                    }))
                    // Nothing laid out yet: the shape of a page, breathing,
                    // and what it is waiting on; or why it could not be.
                    .when(laid.is_none(), |this| {
                        let (w, h) = match self.choice.previewed() {
                            export::Format::Portrait => (300., 424.),
                            export::Format::Landscape => (480., 340.),
                        };
                        this.child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .w(px(w))
                                        .h(px(h))
                                        .max_w_full()
                                        .max_h_full()
                                        .rounded(px(crate::radius::CAP))
                                        .border_1()
                                        .border_color(paint(palette.edge))
                                        .bg(paint(palette.band))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(match unlaid {
                                            Some(why) => notice(bad, &palette)
                                                .m(px(16.))
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(paint(bad))
                                                        .child(
                                                            "These pages could not be laid \
                                                             out, so there is nothing to \
                                                             export yet.",
                                                        ),
                                                )
                                                .child(div().child(sentence(&why)))
                                                .into_any_element(),
                                            None => breathing(
                                                "x-loading",
                                                waiting.unwrap_or("Laying out the pages…"),
                                                &palette,
                                            )
                                            .into_any_element(),
                                        }),
                                ),
                        )
                    })
                    .when(stale, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(8.))
                                .right(px(8.))
                                .px(px(8.))
                                .py(px(4.))
                                .rounded(px(crate::radius::CAP))
                                .bg(paint(palette.band))
                                .border_1()
                                .border_color(paint(palette.edge))
                                .child(breathing("x-updating", "Updating…", &palette)),
                        )
                    }),
            )
            .child(
                div()
                    .id("x-thumbs")
                    .track_scroll(&self.thumbs)
                    .h_flex()
                    .flex_none()
                    .gap(px(8.))
                    .overflow_x_scroll()
                    .pb(px(4.))
                    .children(pages.iter().enumerate().map(|(ix, page)| {
                        let height = 84.;
                        let width = height * page.width / page.height;
                        let light = lights.get(ix).copied().unwrap_or_else(Thumb::new);
                        div()
                            .id(("x-thumb", ix))
                            .flex_none()
                            .w(px(width + 2.))
                            .h(px(height + 2.))
                            .rounded(px(crate::radius::CAP))
                            .border_1()
                            .border_color(paint(palette.edge.mix(
                                palette.accent,
                                light.chosen.level().max(light.hovered.level() * 0.5),
                            )))
                            .opacity(light.opacity())
                            .cursor_pointer()
                            .on_hover(cx.listener(move |sheet, hovered: &bool, _, cx| {
                                if let Some(light) = sheet.thumb_lights.get_mut(ix)
                                    && light.hovered.set(*hovered)
                                {
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |sheet, _, _, cx| {
                                sheet.page = ix;
                                cx.notify();
                            }))
                            .child(preview::page(
                                page.clone(),
                                self.drawings.clone(),
                                transparent_black(),
                            ))
                    })),
            );
        // The command that makes the same file, to copy: the sheet is `deck
        // export`, and this says which one.
        let command = self
            .choice
            .command(&self.source.deck, window_look(), &self.config);
        let preview = preview
            .child(
                div()
                    .id("x-command")
                    .flex_none()
                    .h_flex()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(crate::radius::CONTROL))
                    .bg(paint(palette.wash))
                    .font_family(mono.clone())
                    .text_size(px(11.))
                    .text_color(paint(palette.muted))
                    .cursor_pointer()
                    .on_click(cx.listener({
                        let command = command.clone();
                        move |sheet, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(command.clone()));
                            sheet.copied = true;
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(command),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(paint(palette.accent))
                            .child(if self.copied { "copied" } else { "copy" }),
                    ),
            )
            .child(
                // The strip's keys, drawn as the strip draws them, each with
                // its word.
                div()
                    .flex_none()
                    .h_flex()
                    .justify_center()
                    .items_center()
                    .gap(px(14.))
                    .font_family(mono.clone())
                    .text_size(px(11.))
                    .text_color(paint(palette.muted))
                    .children(
                        [
                            (&["esc"][..], "close"),
                            (&["←", "→"][..], "page"),
                            (&["tab"][..], "move"),
                            (&["space"][..], "choose"),
                        ]
                        .map(|(keys, word)| {
                            div()
                                .h_flex()
                                .items_center()
                                .gap(px(5.))
                                .children(
                                    keys.iter()
                                        .map(|key| crate::keys::cap(key, &palette, mono.clone())),
                                )
                                .child(word)
                        }),
                    ),
            );

        div()
            .id("export-shade")
            .track_focus(&self.focus)
            .key_context("DeckExport")
            .on_action(cx.listener(|sheet, _: &CloseSheet, window, cx| {
                sheet.escape(window, cx);
            }))
            .on_action(cx.listener(|sheet, _: &NextPage, _, cx| sheet.turn_page(1, cx)))
            .on_action(cx.listener(|sheet, _: &PrevPage, _, cx| sheet.turn_page(-1, cx)))
            .on_action(cx.listener(|sheet, _: &NextControl, window, cx| {
                sheet.step_focus(1, window, cx);
            }))
            .on_action(cx.listener(|sheet, _: &PrevControl, window, cx| {
                sheet.step_focus(-1, window, cx);
            }))
            .on_action(cx.listener(|sheet, _: &Choose, window, cx| {
                // A space typed into the theme filter is a space.
                if sheet.typing(window, cx) {
                    cx.propagate();
                } else if let Some(control) = sheet.focused(window) {
                    sheet.press(control, window, cx);
                }
            }))
            .on_action(cx.listener(|_, _: &Hold, _, _| {}))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|sheet, _, window, cx| sheet.escape(window, cx)),
            )
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .v_flex()
            .items_center()
            .px(px(16.))
            .pt(px(34.))
            .pb(px(52.))
            .opacity(shown_sheet)
            .bg(paint(palette.bg).opacity(0.72))
            .child(
                div()
                    .id("export-sheet")
                    // Rises the last few points into place as it comes in,
                    // and sinks back as it goes.
                    .relative()
                    .top(px(8. * (1. - shown_sheet)))
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .max_w(px(1180.))
                    .h_flex()
                    // Both columns the sheet's height, so the options scroll
                    // inside it rather than being centred past its edges.
                    .items_stretch()
                    .rounded(px(crate::radius::CARD))
                    .border_1()
                    .border_color(paint(palette.edge))
                    .bg(paint(palette.band))
                    .text_color(paint(palette.fg))
                    .overflow_hidden()
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(options)
                    .child(preview),
            )
            .children(badge)
            .children(list)
    }
}
