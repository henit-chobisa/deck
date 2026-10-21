//! What the export sheet has been asked, as plain values: no window, no
//! GPUI, so every rule it keeps can be read by a test.
//!
//! The sheet is a front for `deck export`, and nothing more. Each control is
//! one of the command's flags — the format, `--landscape`, `--theme`,
//! `--groups`, `--no-comments` — and [`Choice::options`] is what the command
//! would hand the exporter for the same flags, so a file made from the
//! window is the file the command would make. [`Choice::args`] writes the
//! command line out, and a test reads it back through the command's own
//! parser to hold the two together.
//!
//! What it does not share with the command is where the file goes. The
//! command names a path; the window never does. It asks, with the system's
//! own save dialog, and [`save`] writes only to the path that dialog gave
//! back. There is no folder exports go to: only the last one somebody chose
//! ([`start_folder`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use deck_render::export;

/// What to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// A PDF, portrait or landscape.
    Pdf,
    /// One HTML file, which is the window a group to a screen.
    Html,
}

/// Which way up a PDF's pages are. Only a PDF has one: the HTML is the
/// window already.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// A cover, then each group on a page or two of its own.
    Portrait,
    /// Each page the window, on its side.
    Landscape,
}

/// Which colours the export is painted in: what `deck export --theme` takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Theme {
    /// What the window is painted in, taken from the window as it stands:
    /// the editor's theme it borrowed, by name when it knows it, a theme
    /// `deck open --theme` tried on, or deck's own when it borrowed none.
    /// Nearest to `--theme editor`, the command's default, but not always the
    /// same: the command reads the config afresh and has no window to have
    /// been told another theme, nor the platform's light or dark that the
    /// window asks once it is up.
    Window,
    /// Deck's own dark. `--theme dark`.
    Dark,
    /// Deck's own light, for printing. `--theme light`.
    Light,
    /// One of an editor's themes, `--theme vscode:Nord`; or, for Neovim,
    /// which has no names, the scheme it is in, `--theme nvim`.
    Editor {
        /// Whose.
        editor: deck_theme::Editor,
        /// Which, by the name the editor gives it.
        name: Option<Arc<str>>,
    },
}

/// The editor theme the window borrowed its colours from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Borrowed {
    /// The editor.
    pub editor: deck_theme::Editor,
    /// The theme, when the reader named one.
    pub name: Option<String>,
}

impl Theme {
    /// What `--theme` takes for it. The window's own is named as the theme
    /// it borrowed, when it borrowed one, so the command paints what the
    /// window shows rather than whatever the config says today.
    #[must_use]
    pub fn flag(&self, window: Option<&Borrowed>) -> String {
        match self {
            Self::Window => window.map_or_else(
                || "editor".to_string(),
                |window| editor_flag(window.editor, window.name.as_deref()),
            ),
            Self::Dark => "dark".to_string(),
            Self::Light => "light".to_string(),
            Self::Editor { editor, name } => editor_flag(*editor, name.as_deref()),
        }
    }
}

/// What the theme field and the list call `theme`: the window's own by the
/// theme it borrowed, `As the window · koala-hyderabad`, or by deck's own
/// when it borrowed none.
#[must_use]
pub fn label(theme: &Theme, window: Option<&Borrowed>, window_dark: bool) -> String {
    let own = |dark: bool| if dark { "Deck dark" } else { "Deck light" };
    match theme {
        Theme::Window => format!(
            "As the window · {}",
            window.map_or_else(
                || own(window_dark).to_string(),
                |window| window
                    .name
                    .clone()
                    .unwrap_or_else(|| window.editor.name().to_string()),
            )
        ),
        Theme::Dark => own(true).to_string(),
        Theme::Light => own(false).to_string(),
        Theme::Editor {
            editor,
            name: Some(name),
        } => format!("{name} · {}", editor.name()),
        Theme::Editor { editor, name: None } => format!("{} · the scheme it is in", editor.name()),
    }
}

/// `vscode:Nord`, or `nvim` for a theme with no name.
fn editor_flag(editor: deck_theme::Editor, name: Option<&str>) -> String {
    match name {
        Some(name) => format!("{}:{name}", editor.key()),
        None => editor.key().to_string(),
    }
}

/// `word` as a shell reads it back: as it is when nothing in it means
/// anything to a shell, and in single quotes otherwise.
#[must_use]
pub fn quoted(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if plain {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// Everything the sheet has been asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// PDF or HTML.
    pub format: Format,
    /// For a PDF, which way up.
    pub layout: Layout,
    /// The colours.
    pub theme: Theme,
    /// Each group, ticked or not, in the deck's order.
    pub groups: Vec<bool>,
    /// Whether what was said goes with it.
    pub comments: bool,
}

impl Choice {
    /// The sheet as it opens on a deck of `count` groups: the command's own
    /// defaults, except that the window exports a PDF first, since a PDF is
    /// what somebody without deck can open anywhere.
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            format: Format::Pdf,
            layout: Layout::Portrait,
            theme: Theme::Window,
            groups: vec![true; count],
            comments: true,
        }
    }

    /// How many groups are ticked.
    #[must_use]
    pub fn chosen(&self) -> usize {
        self.groups.iter().filter(|on| **on).count()
    }

    /// The groups ticked, by number from one, as `--groups` takes them:
    /// empty when every one is, which is what the command means by leaving
    /// the flag out.
    #[must_use]
    pub fn numbers(&self) -> Vec<u32> {
        if self.groups.iter().all(|on| *on) {
            return Vec::new();
        }
        self.groups
            .iter()
            .zip(1u32..)
            .filter(|(on, _)| **on)
            .map(|(_, n)| n)
            .collect()
    }

    /// The shape of the pages the file is written in: `None` for the HTML
    /// file, which has no pages.
    #[must_use]
    pub fn shape(&self) -> Option<export::Format> {
        match (self.format, self.layout) {
            (Format::Html, _) => None,
            (Format::Pdf, Layout::Portrait) => Some(export::Format::Portrait),
            (Format::Pdf, Layout::Landscape) => Some(export::Format::Landscape),
        }
    }

    /// The shape the preview lays out. The HTML file is the window a group
    /// to a screen, and the landscape page is that window on paper, laid
    /// out at the window's own measurements; so the HTML is previewed as
    /// the landscape pages.
    #[must_use]
    pub fn previewed(&self) -> export::Format {
        self.shape().unwrap_or(export::Format::Landscape)
    }

    /// What the exporter is handed, for a palette that is `dark` or not:
    /// what `deck export` hands it for [`Self::args`].
    #[must_use]
    pub fn options(&self, dark: bool) -> export::Options {
        export::Options {
            groups: self.numbers(),
            comments: self.comments,
            dark,
        }
    }

    /// This choice as the `deck export` command line it stands for, after
    /// the deck: `--format pdf --landscape --theme light --groups 1,3`,
    /// with the window's own theme named as `window` says. What the tests
    /// hold [`Self::options`] to.
    #[must_use]
    pub fn args(&self, window: Option<&Borrowed>) -> Vec<String> {
        let mut args = vec![
            "--format".to_string(),
            match self.format {
                Format::Pdf => "pdf",
                Format::Html => "html",
            }
            .to_string(),
        ];
        if self.shape() == Some(export::Format::Landscape) {
            args.push("--landscape".to_string());
        }
        args.push("--theme".to_string());
        args.push(self.theme.flag(window));
        let numbers = self.numbers();
        if !numbers.is_empty() {
            args.push("--groups".to_string());
            args.push(
                numbers
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        if !self.comments {
            args.push("--no-comments".to_string());
        }
        args
    }

    /// The whole command, for `deck` at `deck`, quoted for a shell to read
    /// back: `deck export d.deck --format pdf --theme 'vscode:Nord'`. An
    /// editor's theme is always quoted, since its name is the editor's to
    /// choose.
    #[must_use]
    pub fn command(&self, deck: &Path, window: Option<&Borrowed>) -> String {
        let mut line = vec![
            "deck".to_string(),
            "export".to_string(),
            quoted(&deck.to_string_lossy()),
        ];
        let mut theme_next = false;
        for arg in self.args(window) {
            let editor_theme = theme_next && arg.contains(':');
            theme_next = arg == "--theme";
            line.push(if editor_theme {
                format!("'{}'", arg.replace('\'', "'\\''"))
            } else {
                quoted(&arg)
            });
        }
        line.join(" ")
    }

    /// The extension the file gets.
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self.format {
            Format::Pdf => "pdf",
            Format::Html => "html",
        }
    }
}

/// The name the save dialog offers: the `.deck` file's name, which is the
/// day it was packed and its title, `2026-10-21-why-the-batch-stalls`, with
/// the export's extension. What `deck export` names it when it is not told.
#[must_use]
pub fn file_name(packed: &str, choice: &Choice) -> String {
    format!(
        "{}.{}",
        packed.trim_end_matches(".deck"),
        choice.extension()
    )
}

/// Where the save dialog opens: the folder the last export was saved in,
/// while it is still there; the first time, the project the deck was
/// written against, or the folder deck was started in; and failing those,
/// home.
///
/// Never a folder of deck's own. A file the reader has to go and find in
/// `~/decks` is a file deck put somewhere for them, and the rule is that
/// where an export goes is theirs to say.
#[must_use]
pub fn start_folder(
    remembered: Option<PathBuf>,
    project: Option<&Path>,
    home: Option<PathBuf>,
) -> PathBuf {
    remembered
        .filter(|dir| dir.is_dir())
        .or_else(|| project.filter(|dir| dir.is_dir()).map(Path::to_path_buf))
        .or(home)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// What the export button says, and whether it can be pressed.
///
/// `pages` is how many the PDF will have, once the preview knows; `blocked`
/// is a secret nobody has let through, which leaves the words as they are
/// and the button off, so the reader can see what they would have sent.
#[must_use]
pub fn button(choice: &Choice, pages: Option<usize>, blocked: bool) -> (String, bool) {
    let chosen = choice.chosen();
    if chosen == 0 {
        return ("Choose a group to export".to_string(), false);
    }
    let mut said = match (choice.format, pages) {
        (Format::Pdf, Some(1)) => "Export PDF · 1 page".to_string(),
        (Format::Pdf, Some(n)) => format!("Export PDF · {n} pages"),
        (Format::Pdf, None) => "Export PDF".to_string(),
        (Format::Html, _) => "Export HTML · 1 file".to_string(),
    };
    if chosen < choice.groups.len() {
        said.push_str(&format!(" · {chosen} of {} groups", choice.groups.len()));
    }
    said.push('…');
    (said, !blocked && pages.is_some())
}

/// A size as the Finder says it: `412 KB`, `1.2 MB`.
#[must_use]
pub fn size(bytes: usize) -> String {
    #[allow(clippy::cast_precision_loss)]
    let bytes = bytes as f64;
    if bytes < 1000. {
        format!("{bytes} bytes")
    } else if bytes < 1_000_000. {
        format!("{:.0} KB", bytes / 1000.)
    } else {
        format!("{:.1} MB", bytes / 1_000_000.)
    }
}

/// A file the sheet wrote, for the line that says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    /// Where it is.
    pub path: PathBuf,
    /// How large.
    pub bytes: usize,
    /// How many pages, for a PDF.
    pub pages: Option<usize>,
}

impl Saved {
    /// `Saved · 7 pages · 412 KB`, or `Saved · 1 file · 163 KB`.
    #[must_use]
    pub fn line(&self) -> String {
        let what = match self.pages {
            Some(1) => "1 page".to_string(),
            Some(n) => format!("{n} pages"),
            None => "1 file".to_string(),
        };
        format!("Saved · {what} · {}", size(self.bytes))
    }
}

/// `bytes` written at `path`, which the save dialog gave back.
///
/// Over a file already there: the dialog has asked the reader whether to
/// replace it, and they said yes. The folder it is in is remembered by the
/// caller, for the next dialog to open in.
///
/// # Errors
///
/// When the file cannot be written.
pub fn save(path: &Path, bytes: &[u8], pages: Option<usize>) -> anyhow::Result<Saved> {
    deck_cli::pack::write_new(path, bytes, true)?;
    Ok(Saved {
        path: path.to_path_buf(),
        bytes: bytes.len(),
        pages,
    })
}

/// What a page is, for the line over the preview: `cover`, `2 · Where a
/// failed write comes from`, `2 · continued, 2 of 2`, `2 · the argument`.
#[must_use]
pub fn page_label(kind: export::Kind, title: impl Fn(u32) -> Option<String>) -> String {
    match kind {
        export::Kind::Cover => "cover".to_string(),
        export::Kind::Group { n, part: 1, .. } => {
            title(n).map_or_else(|| format!("group {n}"), |title| format!("{n} · {title}"))
        }
        export::Kind::Group { n, part, of } => format!("{n} · continued, {part} of {of}"),
        export::Kind::Argument { n, part: 1, .. } => format!("{n} · the argument"),
        export::Kind::Argument { n, part, of } => {
            format!("{n} · the argument, {part} of {of}")
        }
        export::Kind::Said => "what was said about the deck".to_string(),
    }
}

/// `path` as a person writes it: under `home`, from `~`.
#[must_use]
pub fn tilde(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}
