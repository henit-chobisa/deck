//! The theme field's list: deck's own three, then every theme of every
//! editor on this machine, each with its page, text and accent, to export in.
//!
//! Only editors that are installed are listed, and only the themes they
//! have: nothing is made up, and no theme is given a light or dark partner
//! it does not have. The names are found off the window's thread when the
//! sheet opens and appear at once; each theme's colours are read after,
//! one at a time, and fill in as they land. A theme still being read cannot
//! be chosen, since there is nothing yet to paint it in.
//!
//! Neovim is one row, the scheme it is in, and is read only when it is
//! chosen: reading it means starting Neovim with the reader's whole config,
//! which is too much to do every time the sheet opens, and it is given a
//! time limit, since a config can hang.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use deck_core::theme::Imported;
use deck_theme::Editor;
use gpui_kit::component::input::{Input, InputEvent, InputState};

use super::*;

/// How long Neovim is given to say what its scheme is.
const NVIM_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Editors in the order the list shows them, after deck's own.
const ORDER: [Editor; 5] = [
    Editor::Vscode,
    Editor::Cursor,
    Editor::Windsurf,
    Editor::Zed,
    Editor::Nvim,
];

/// A theme's colours as far as they are known.
#[derive(Debug, Clone)]
pub(super) enum Swatch {
    /// Being read.
    Reading,
    /// Read.
    Read(Imported),
    /// Not read yet, and not until it is chosen: Neovim's.
    OnClick,
    /// It would not read, and this is why.
    Failed(String),
}

/// One theme in the list of an editor's.
#[derive(Debug, Clone)]
pub(super) struct Row {
    pub editor: Editor,
    /// Its name; `None` for Neovim's one row.
    pub name: Option<Arc<str>>,
    pub swatch: Swatch,
}

impl Row {
    /// The theme the export takes, once it is chosen.
    pub fn theme(&self) -> Theme {
        Theme::Editor {
            editor: self.editor,
            name: self.name.clone(),
        }
    }

    /// What the list calls it.
    pub fn title(&self) -> String {
        self.name
            .as_deref()
            .map_or_else(|| "The scheme it is in".to_string(), str::to_string)
    }
}

/// The themes `installed` editors have, as `themes` lists them, in the
/// order the list shows them: Neovim as one row, read on click, and an
/// editor with nothing to list left out.
pub(super) fn listed(
    installed: impl Fn(Editor) -> bool,
    mut themes: impl FnMut(Editor) -> Vec<String>,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for editor in ORDER.into_iter().filter(|editor| installed(*editor)) {
        if editor == Editor::Nvim {
            rows.push(Row {
                editor,
                name: None,
                swatch: Swatch::OnClick,
            });
            continue;
        }
        rows.extend(themes(editor).into_iter().map(|name| Row {
            editor,
            name: Some(name.into()),
            swatch: Swatch::Reading,
        }));
    }
    rows
}

/// What has been read, by editor and theme: the colours, or why not.
type Read = HashMap<(Editor, Option<String>), Result<Imported, String>>;

/// What has been read this session, so a sheet opened again paints its
/// swatches at once.
fn cache() -> &'static Mutex<Read> {
    static READ: OnceLock<Mutex<Read>> = OnceLock::new();
    READ.get_or_init(Mutex::default)
}

/// `editor`'s theme `name`, read once a session by `reading`: its colours,
/// or why not.
///
/// A theme that would not read is remembered too — a file that did not parse
/// will not parse the second time either — except Neovim's. Its failure is
/// usually a config that took too long, and that is worth asking again.
fn read(
    editor: Editor,
    name: Option<&str>,
    reading: impl FnOnce() -> anyhow::Result<Option<Imported>>,
) -> Result<Imported, String> {
    let key = (editor, name.map(str::to_string));
    if let Some(known) = cache().lock().ok().and_then(|read| read.get(&key).cloned()) {
        return known;
    }
    let got = match reading() {
        Ok(Some(imported)) => Ok(imported),
        Ok(None) => Err("it has no colours to read".to_string()),
        Err(err) => Err(format!("{err:#}")),
    };
    if (got.is_ok() || editor != Editor::Nvim)
        && let Ok(mut read) = cache().lock()
    {
        read.insert(key, got.clone());
    }
    got
}

/// What the theme list is doing, while it is open.
pub(super) struct Picking {
    /// The filter box over the list.
    pub filter: Entity<InputState>,
    _typed: Subscription,
}

/// A row of the open list: one of deck's own, or an editor's.
#[derive(Debug, Clone)]
enum Entry {
    Own(Theme, &'static str),
    Theirs(usize),
}

impl ExportSheet {
    /// Find the editors' themes, off the window's thread, then read each
    /// one's colours in turn.
    pub(super) fn discover(&mut self, cx: &mut Context<Self>) {
        let dark = self.source.machine_dark;
        self.discovering = cx.spawn(async move |this, cx| {
            // Each editor's themes are found with where they live, once, and
            // read from there: looking each one up again by name would scan
            // every extension per theme, which with a few hundred of each is
            // most of the time the list takes to fill in.
            let (rows, mut shelf) = cx
                .background_executor()
                .spawn(async move {
                    let mut shelf: HashMap<(Editor, String), deck_theme::Listed> = HashMap::new();
                    let rows = listed(Editor::installed, |editor| {
                        let found = deck_theme::listed(editor);
                        let names = found.iter().map(|theme| theme.name.clone()).collect();
                        shelf.extend(
                            found
                                .into_iter()
                                .map(|theme| ((theme.editor, theme.name.clone()), theme)),
                        );
                        names
                    });
                    (rows, shelf)
                })
                .await;
            let reading: Vec<_> = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| matches!(row.swatch, Swatch::Reading))
                .map(|(ix, row)| {
                    let listed = row
                        .name
                        .as_deref()
                        .and_then(|name| shelf.remove(&(row.editor, name.to_string())));
                    (ix, row.editor, row.name.clone(), listed)
                })
                .collect();
            if this
                .update(cx, |sheet, cx| {
                    sheet.editors = rows;
                    cx.notify();
                })
                .is_err()
            {
                return;
            }
            for (ix, editor, name, listed) in reading {
                let got = cx
                    .background_executor()
                    .spawn(async move {
                        read(editor, name.as_deref(), || match &listed {
                            Some(listed) => listed.read(dark),
                            None => deck_theme::read(editor, name.as_deref(), dark),
                        })
                    })
                    .await;
                if this
                    .update(cx, |sheet, cx| {
                        sheet.set_swatch(ix, got);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
    }

    fn set_swatch(&mut self, ix: usize, got: Result<Imported, String>) {
        if let Some(row) = self.editors.get_mut(ix) {
            row.swatch = match got {
                Ok(imported) => Swatch::Read(imported),
                Err(why) => Swatch::Failed(why),
            };
        }
    }

    /// The colours of `editor`'s theme `name`, when they have been read:
    /// what the sheet paints an editor's theme in, so painting never waits
    /// on a disk or a process.
    pub(super) fn imported(&self, editor: Editor, name: Option<&str>) -> Option<Imported> {
        let read = self.editors.iter().find_map(|row| match &row.swatch {
            Swatch::Read(imported) if row.editor == editor && row.name.as_deref() == name => {
                Some(*imported)
            }
            _ => None,
        });
        read.or_else(|| {
            // The window's own, which it read before the sheet was opened.
            window_theme()
                .filter(|window| window.editor == editor && window.name.as_deref() == name)
                .and(crate::palette::asked().imported)
        })
    }

    /// Open the list, with the focus in its filter.
    pub(super) fn open_themes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Find a theme"));
        let typed = cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let focus = filter.read(cx).focus_handle(cx);
        self.stops.insert(Control::Filter, focus.clone());
        // After the click that opened it has finished handing out the focus.
        cx.defer_in(window, move |_, window, cx| focus.focus(window, cx));
        self.picking = Some(Picking {
            filter,
            _typed: typed,
        });
        cx.notify();
    }

    /// Put the list away, with the focus back on the field that opened it.
    pub(super) fn close_themes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picking = None;
        self.stops.remove(&Control::Filter);
        self.stop(Control::Theme, cx).focus(window, cx);
        cx.notify();
    }

    /// Whether the filter box has the keyboard: its letters, and space, are
    /// for typing then.
    pub(super) fn typing(&self, window: &Window, cx: &App) -> bool {
        self.picking
            .as_ref()
            .is_some_and(|picking| picking.filter.read(cx).focus_handle(cx).is_focused(window))
    }

    /// The rows the filter lets through, in the list's order.
    fn entries(&self, cx: &App) -> Vec<Entry> {
        let typed = self
            .picking
            .as_ref()
            .map(|picking| picking.filter.read(cx).value().to_lowercase())
            .unwrap_or_default();
        let matches = |text: &str, editor: &str| {
            typed.is_empty()
                || text.to_lowercase().contains(&typed)
                || editor.to_lowercase().contains(&typed)
        };
        let mut out: Vec<Entry> = [
            (Theme::Window, ""),
            (Theme::Dark, ""),
            (Theme::Light, "for printing"),
        ]
        .into_iter()
        .filter(|(theme, _)| matches(&self.label_of(theme), "deck"))
        .map(|(theme, note)| Entry::Own(theme, note))
        .collect();
        out.extend(
            self.editors
                .iter()
                .enumerate()
                .filter(|(_, row)| matches(&row.title(), row.editor.name()))
                .map(|(ix, _)| Entry::Theirs(ix)),
        );
        out
    }

    /// The list's controls, for tab to move through while it is open: the
    /// filter, then every row it shows.
    pub(super) fn theme_order(&self, cx: &App) -> Vec<Control> {
        let mut order = vec![Control::Filter];
        order.extend(self.entries(cx).iter().map(|entry| match entry {
            Entry::Own(Theme::Window, _) => Control::Own(0),
            Entry::Own(Theme::Dark, _) => Control::Own(1),
            Entry::Own(_, _) => Control::Own(2),
            Entry::Theirs(ix) => Control::Row(*ix),
        }));
        order
    }

    /// A row clicked, or pressed with space: chosen, when it can be.
    pub(super) fn press_theme(
        &mut self,
        control: Control,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let theme = match control {
            Control::Own(0) => Theme::Window,
            Control::Own(1) => Theme::Dark,
            Control::Own(_) => Theme::Light,
            Control::Row(ix) => {
                let Some(row) = self.editors.get(ix) else {
                    return;
                };
                match row.swatch {
                    Swatch::Read(_) => row.theme(),
                    // Neovim that failed is asked again: what failed is
                    // usually a config that was slow this once, and the
                    // reader now stops one that runs over, so another try
                    // costs a few seconds and nothing left running.
                    Swatch::OnClick | Swatch::Failed(_) if row.editor == Editor::Nvim => {
                        self.read_nvim(ix, cx);
                        return;
                    }
                    Swatch::OnClick | Swatch::Reading | Swatch::Failed(_) => return,
                }
            }
            _ => return,
        };
        self.pick(theme, window, cx);
    }

    fn pick(&mut self, theme: Theme, window: &mut Window, cx: &mut Context<Self>) {
        self.choice.theme = theme;
        self.close_themes(window, cx);
        self.changed(false, cx);
    }

    /// Ask Neovim for its scheme, giving it [`NVIM_LIMIT`]; chosen when it
    /// answers, and said honestly when it does not.
    ///
    /// The limit is kept by the reader itself, which stops an nvim that runs
    /// over and waits for it to go. Racing a timer here instead used to give
    /// up on the answer and leave the nvim running, and the thread waiting on
    /// it, for as long as the config cared to hang.
    fn read_nvim(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.editors.get_mut(ix) {
            row.swatch = Swatch::Reading;
        }
        cx.notify();
        self.asking_nvim = cx.spawn(async move |this, cx| {
            let got = cx
                .background_executor()
                .spawn(async move {
                    read(Editor::Nvim, None, || {
                        deck_theme::nvim::read_within(NVIM_LIMIT).map(Some)
                    })
                })
                .await;
            this.update_in(cx, |sheet, window, cx| {
                let read = got.is_ok();
                sheet.set_swatch(ix, got);
                if read && sheet.picking.is_some() {
                    sheet.pick(
                        Theme::Editor {
                            editor: Editor::Nvim,
                            name: None,
                        },
                        window,
                        cx,
                    );
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// The open list, hung under the theme field.
    pub(super) fn theme_list(
        &self,
        stops: &HashMap<Control, FocusHandle>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let picking = self.picking.as_ref()?;
        let palette = self.palette;
        let field = self.field;
        let mono = cx.theme().mono_font_family.clone();
        let entries = self.entries(cx);
        let chosen = &self.choice.theme;
        let swatches = |colours: Option<[Rgb; 3]>| {
            div()
                .flex_none()
                .h_flex()
                .gap(px(3.))
                .children((0..3).map(move |ix| {
                    let tile = div()
                        .size(px(12.))
                        .rounded(px(3.))
                        .border_1()
                        .border_color(paint(palette.edge));
                    match colours {
                        Some(colours) => tile.bg(paint(colours[ix])),
                        None => tile.bg(paint(palette.wash)),
                    }
                }))
        };
        let row = |id: (&'static str, usize),
                   control: Control,
                   colours: Option<[Rgb; 3]>,
                   title: String,
                   note: String,
                   on: bool,
                   ready: bool| {
            let base = div()
                .id(id)
                .h_flex()
                .items_center()
                .gap(px(10.))
                .px(px(12.))
                .py(px(6.))
                .rounded(px(5.))
                .text_size(px(12.5))
                .when(ready, |this| {
                    this.cursor_pointer()
                        .hover(|this| this.bg(paint(palette.wash)))
                })
                .when(!ready, |this| this.text_color(paint(palette.muted)))
                .on_click(cx.listener(move |sheet, _, window, cx| {
                    sheet.press_theme(control, window, cx);
                }));
            let base = match stops.get(&control) {
                Some(stop) => ringed(base, stop, &palette),
                None => base,
            };
            base.child(swatches(colours))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(title),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(paint(if on { palette.accent } else { palette.muted }))
                        .child(if on { "✓".to_string() } else { note }),
                )
        };
        let heading = |title: &'static str| {
            div()
                .px(px(12.))
                .pt(px(8.))
                .pb(px(2.))
                .text_size(px(10.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(paint(palette.muted))
                .child(title.to_uppercase())
        };
        let mut list: Vec<AnyElement> = Vec::new();
        let mut last: Option<&'static str> = None;
        for entry in &entries {
            let group = match entry {
                Entry::Own(..) => "Deck",
                Entry::Theirs(ix) => self.editors[*ix].editor.name(),
            };
            if last != Some(group) {
                list.push(heading(group).into_any_element());
                last = Some(group);
            }
            list.push(match entry {
                Entry::Own(theme, note) => {
                    let ix = match theme {
                        Theme::Window => 0,
                        Theme::Dark => 1,
                        _ => 2,
                    };
                    let p = self.painted(theme).palette;
                    row(
                        ("x-own", ix),
                        Control::Own(ix),
                        Some([p.bg, p.fg, p.accent]),
                        self.label_of(theme),
                        (*note).to_string(),
                        chosen == theme,
                        true,
                    )
                    .into_any_element()
                }
                Entry::Theirs(ix) => {
                    let them = &self.editors[*ix];
                    let (colours, note, ready) = match &them.swatch {
                        Swatch::Read(imported) => (
                            Some([imported.bg, imported.fg, imported.accent]),
                            String::new(),
                            true,
                        ),
                        Swatch::Reading => (None, "reading…".to_string(), false),
                        Swatch::OnClick => (None, "read on click".to_string(), true),
                        // Neovim's can be asked again, so it stays a row to click.
                        Swatch::Failed(_) if them.editor == Editor::Nvim => {
                            (None, "try again".to_string(), true)
                        }
                        Swatch::Failed(_) => (None, "could not be read".to_string(), false),
                    };
                    let row = row(
                        ("x-row", *ix),
                        Control::Row(*ix),
                        colours,
                        them.title(),
                        note,
                        *chosen == them.theme(),
                        ready,
                    );
                    // Neovim being asked: the dot that breathes, as everything
                    // the sheet waits on has.
                    if them.editor == Editor::Nvim && matches!(them.swatch, Swatch::Reading) {
                        row.child(breathing("x-nvim", "", &palette))
                            .into_any_element()
                    } else {
                        row.into_any_element()
                    }
                }
            });
            if let Entry::Theirs(ix) = entry
                && let Swatch::Failed(why) = &self.editors[*ix].swatch
                && self.editors[*ix].editor == Editor::Nvim
            {
                list.push(
                    div()
                        .px(px(12.))
                        .pb(px(6.))
                        .text_size(px(11.))
                        .text_color(paint(palette.del))
                        .child(sentence(why))
                        .into_any_element(),
                );
            }
        }
        if entries.is_empty() {
            list.push(
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .text_size(px(12.))
                    .text_color(paint(palette.muted))
                    .child("No theme by that name.")
                    .into_any_element(),
            );
        }
        let panel = div()
            .id("x-themes")
            .occlude()
            // As wide as the field it drops from, so it stays in the options column.
            .w(field.size.width)
            .v_flex()
            .rounded(px(10.))
            .border_1()
            .border_color(paint(palette.edge))
            .bg(paint(palette.band))
            .text_color(paint(palette.fg))
            .shadow_lg()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(
                cx.listener(move |sheet, event: &MouseDownEvent, window, cx| {
                    // The field opens and closes the list itself.
                    if !field.contains(&event.position) {
                        sheet.close_themes(window, cx);
                    }
                }),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(paint(palette.edge))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&picking.filter).appearance(false)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(mono)
                            .text_size(px(10.5))
                            .text_color(paint(palette.muted))
                            .child("esc closes this first"),
                    ),
            )
            .child(
                div()
                    .id("x-theme-list")
                    .max_h(px(330.))
                    .overflow_y_scroll()
                    .py(px(4.))
                    .children(list),
            );
        Some(
            deferred(
                anchored()
                    .position(point(field.left(), field.bottom() + px(4.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(panel),
            )
            .into_any_element(),
        )
    }
}
