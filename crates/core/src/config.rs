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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Arrange;
    use crate::theme::Rgb;

    /// Parsing through JSON rather than TOML: the shape is the same to serde,
    /// and this crate has no business depending on a file format it never
    /// reads.
    fn read(json: &str) -> Config {
        serde_json::from_str(json).expect("this config parses")
    }

    #[test]
    fn an_empty_file_and_no_file_are_the_same_thing() {
        let empty = read("{}");
        assert_eq!(empty.layout.arrange, Arrange::Stacked);
        assert_eq!(empty.theme.paper, Paper::Grey);
        assert_eq!(empty.theme.mode, Mode::Auto);
        assert!(empty.theme.colors.accent.is_none());
    }

    #[test]
    fn an_editor_is_named_by_the_reader_and_read_by_somebody_else() {
        // The word is the config's business; what to do with it is not. This
        // crate has no idea what a `nvim` is.
        let config = read(r#"{ "theme": { "editor": "nvim" } }"#);
        assert_eq!(config.theme.editor.as_deref(), Some("nvim"));
        assert!(read("{}").theme.editor.is_none());
    }

    #[test]
    fn saying_one_word_changes_one_thing() {
        let config = read(r#"{ "theme": { "paper": "warm" } }"#);
        assert_eq!(config.theme.paper, Paper::Warm);
        assert_eq!(
            config.layout.max_columns,
            Layout::default().max_columns,
            "and leaves everything else where the design put it"
        );
    }

    #[test]
    fn a_colour_is_written_the_way_a_person_writes_one() {
        let config = read(r##"{ "theme": { "colors": { "accent": "#00ff7f" } } }"##);
        assert_eq!(config.theme.colors.accent, Some(Rgb::new(0, 255, 127)));
    }

    #[test]
    fn a_key_nobody_recognises_is_said_out_loud() {
        // Not ignored. An override that does nothing is worse than one that
        // refuses: the reader spends the afternoon wondering why the deck looks
        // the same, and the answer is a missing letter.
        assert!(
            serde_json::from_str::<Config>(r##"{ "theme": { "colors": { "acent": "#000" } } }"##)
                .is_err()
        );
        assert!(serde_json::from_str::<Config>(r#"{ "layuot": {} }"#).is_err());
    }

    #[test]
    fn an_override_wins_over_what_was_derived() {
        let config = read(r##"{ "theme": { "colors": { "accent": "#00ff7f" } } }"##);
        let mut palette = crate::theme::derive(crate::theme::Imported::plain(
            Rgb::new(0xdf, 0xdf, 0xdf),
            Rgb::new(0x3c, 0x38, 0x36),
            Rgb::new(0xaf, 0x3a, 0x03),
        ));

        config.theme.colors.apply(&mut palette);
        assert_eq!(palette.accent, Rgb::new(0, 255, 127));
        assert_eq!(
            palette.bg,
            Rgb::new(0xdf, 0xdf, 0xdf),
            "and nothing else moved"
        );
    }
}
