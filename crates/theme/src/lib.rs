//! Reading a palette out of the editor you already use.
//!
//! Deck is a window that appears beside your work, and a window that appears
//! beside your work in somebody else's colours is a second application. So it
//! borrows yours.
//!
//! # What is taken, and what is not
//!
//! Three colours are always taken — the page, the text, and one accent — and a
//! few more when the editor has them: what it writes comments in, what it marks
//! a changed line with, and five syntax colours. Everything else deck computes
//! itself, by the rules in [`deck_core::theme`].
//!
//! **Chrome is never imported.** A theme designs its own status line, its own
//! tab bar, its own popup, and those are answers to questions deck is not
//! asking. Importing them is how you end up with a black slab across a light
//! page — which is exactly what happened the first time this was tried, in the
//! Neovim plugin, and is why the boundary is drawn here.
//!
//! # What each editor costs
//!
//! Not the same, and the difference is in how the theme is stored.
//!
//! - **Neovim** has no theme file at all: a colour scheme is a script, and the
//!   only way to know what it produced is to run it. So deck runs one, headless,
//!   and asks the result. See [`nvim`].
//! - **Zed** keeps themes as JSON whose keys are already tree-sitter capture
//!   names, so a syntax colour needs no translation. See [`zed`].
//! - **VS Code** keeps them as JSON too, but its syntax colours are TextMate
//!   scopes, which have to be mapped. See [`vscode`].

use deck_core::theme::Imported;

pub mod nvim;
pub mod vscode;
pub mod zed;

/// An editor deck can borrow a palette from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Editor {
    /// Deck's own palette. Nothing is read.
    #[default]
    None,
    /// See [`nvim`].
    Nvim,
    /// See [`zed`].
    Zed,
    /// See [`vscode`].
    Vscode,
    /// Cursor: VS Code with a different name on the window.
    Cursor,
    /// Windsurf, likewise.
    Windsurf,
}

impl From<String> for Editor {
    fn from(name: String) -> Self {
        match name.as_str() {
            "nvim" | "neovim" => Self::Nvim,
            "zed" => Self::Zed,
            "vscode" | "code" => Self::Vscode,
            "cursor" => Self::Cursor,
            "windsurf" => Self::Windsurf,
            _ => Self::None,
        }
    }
}

impl Editor {
    /// The word the reader writes for this one.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Nvim => "nvim",
            Self::Zed => "zed",
            Self::Vscode => "vscode",
            Self::Cursor => "cursor",
            Self::Windsurf => "windsurf",
        }
    }

    /// What it is called, for something a person reads.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "deck's own palette",
            Self::Nvim => "Neovim",
            Self::Zed => "Zed",
            Self::Vscode => "VS Code",
            Self::Cursor => "Cursor",
            Self::Windsurf => "Windsurf",
        }
    }

    /// Every editor deck knows how to read.
    #[must_use]
    pub fn all() -> [Self; 5] {
        [
            Self::Nvim,
            Self::Vscode,
            Self::Cursor,
            Self::Windsurf,
            Self::Zed,
        ]
    }

    /// Whether this editor is on the machine at all.
    ///
    /// Asked so the reader is offered what they have rather than a list of
    /// things to install. What counts as *installed* is having somewhere the
    /// colours could be read from — a config, or the application itself.
    #[must_use]
    pub fn installed(self) -> bool {
        let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
            return false;
        };
        // Somewhere it keeps its things, or the application itself. Not its
        // *settings*: an editor on its own defaults writes none, and asking for
        // a settings file was how a Zed that is plainly installed came back as
        // missing.
        let here = |paths: &[std::path::PathBuf]| paths.iter().any(|path| path.exists());

        match self {
            Self::None => true,
            Self::Nvim => nvim::init().is_some(),
            Self::Zed => here(&[
                home.join(".config/zed"),
                home.join("Library/Application Support/Zed"),
                std::path::PathBuf::from("/Applications/Zed.app"),
            ]),
            Self::Vscode => vscode::here(vscode::CODE, &home),
            Self::Cursor => vscode::here(vscode::CURSOR, &home),
            Self::Windsurf => vscode::here(vscode::WINDSURF, &home),
        }
    }
}

/// Read `editor`'s colours, or a named theme of it.
///
/// `None` means there was nothing to borrow, and deck should use its own
/// palette in whatever mode the machine is in. That is not a failure: an editor
/// on its built-in theme has nothing on disk to read, and Zed's defaults live
/// inside the binary.
///
/// An error means the reader asked for something specific and it was not there
/// — a theme named in settings that is no longer installed, or a settings file
/// that will not parse. Worth saying out loud rather than falling back in
/// silence, because a deck quietly in the wrong colours looks like deck being
/// wrong rather than the import having failed.
///
/// `dark` is what the machine itself is set to, which decides an editor's own
/// default when it has one for each.
///
/// `named` tries a theme on without changing the editor's settings. Neovim has
/// no such thing to name: a colour scheme is a script, and the only theme it can
/// report is the one it is in.
///
/// # Errors
///
/// When a named theme cannot be found, or an editor's settings cannot be read.
pub fn read(editor: Editor, named: Option<&str>, dark: bool) -> anyhow::Result<Option<Imported>> {
    match editor {
        Editor::None => Ok(None),
        Editor::Nvim => nvim::read().map(Some),
        Editor::Zed => zed::read(named, dark),
        Editor::Vscode => vscode::read_from(vscode::CODE, named, dark),
        Editor::Cursor => vscode::read_from(vscode::CURSOR, named, dark),
        Editor::Windsurf => vscode::read_from(vscode::WINDSURF, named, dark),
    }
}

/// Every theme `editor` could show, by name.
///
/// Empty for Neovim, which has no list to give: a colour scheme is a script and
/// the ones installed are whatever the reader's config happens to require.
#[must_use]
pub fn themes(editor: Editor) -> Vec<String> {
    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        return Vec::new();
    };
    match editor {
        Editor::Vscode => vscode::installed(vscode::CODE, &home),
        Editor::Cursor => vscode::installed(vscode::CURSOR, &home),
        Editor::Windsurf => vscode::installed(vscode::WINDSURF, &home),
        Editor::Zed | Editor::Nvim | Editor::None => Vec::new(),
    }
}
