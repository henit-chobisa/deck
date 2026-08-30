//! What the reader has said they want, in one file.
//!
//! `~/.deck/config.toml` is the only source of truth. There is no second place
//! a preference can live and no question of which one wins — a settings panel,
//! when there is one, will be a typed editor for this file rather than a
//! parallel store that has to be reconciled with it.
//!
//! Everything here has a default that is the design's own answer, so an empty
//! file and no file are the same thing, and a file that says one word changes
//! one thing.
//!
//! The shape lives in this crate and the reading does not. `deck-core` has no
//! opinion about where a home directory is.

use serde::{Deserialize, Serialize};

use crate::layout::Layout;
use crate::theme::{Mode, Overrides, Paper};

/// The whole of `~/.deck/config.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// How panes are arranged. See [`Layout`].
    pub layout: Layout,
    /// What the deck is painted in.
    pub theme: Theme,
}

/// The `[theme]` section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    /// Which editor to borrow colours from, if any.
    ///
    /// A name rather than an enum in this crate: `deck-core` does not know what
    /// an editor is, and reading one is somebody else's job. What it knows is
    /// that the reader wrote a word here.
    pub editor: Option<String>,
    /// Which neutrals to paint on, when nothing is imported.
    pub paper: Paper,
    /// Light, dark, or whatever the machine says.
    pub mode: Mode,
    /// Colours written by hand. Anything here wins.
    pub colors: Overrides,
}
