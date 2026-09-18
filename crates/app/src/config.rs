//! Reading `~/.deck/config.toml`.
//!
//! The shape lives in `deck-core`, which has no opinion about where a home
//! directory is. This finds the file, reads it, and says so when it will not
//! parse.
//!
//! # Why a bad config is loud
//!
//! Everything else deck reads is written by a machine, and the forgiving thing
//! to do with a name a machine got wrong is to fall back to a default. This is
//! written by a person, and the forgiving thing is the opposite: a config that
//! silently ignored `acent = "#af3a03"` would leave them wondering all
//! afternoon why the deck looks the same, and the answer is a missing letter.

use std::path::PathBuf;

use deck_core::config::Config;

/// Where the config lives.
#[must_use]
pub fn path() -> Option<PathBuf> {
    Some(deck_core::home::deck()?.join("config.toml"))
}

/// Read the config, and say what was wrong with it.
///
/// No file is not a problem: every field has a default that is the design's own
/// answer, so an empty config and no config are the same thing.
///
/// # Errors
///
/// When the file exists and will not parse.
pub fn read() -> anyhow::Result<Config> {
    let Some(path) = path() else {
        return Ok(Config::default());
    };
    read_from(&path)
}

/// [`read`], against a named file.
fn read_from(path: &std::path::Path) -> anyhow::Result<Config> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(err) => return Err(anyhow::anyhow!("{}: {err}", path.display())),
    };

    toml::from_str(&text).map_err(|err| anyhow::anyhow!("{}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this crate glob-imports GPUI
    // elsewhere, which exports a `test` attribute of its own.
    use core::prelude::v1::test;

    #[test]
    fn a_config_written_for_the_old_engines_still_parses() {
        // `[speech]` refuses fields it does not know, which is how a typo in a
        // voice name gets caught. The engines it used to choose between are
        // gone, so those two fields are read and dropped — because failing the
        // file would fail the whole config, and take the reader's colours down
        // with a setting that no longer does anything.
        let old = r#"
            [speech]
            aloud   = true
            engine  = "command"
            command = "kokoro-cli --voice af_heart -"
            voice   = "en-US-Chirp3-HD-Charon"
        "#;
        let config: deck_core::config::Config =
            toml::from_str(old).expect("an old config still reads");
        assert_eq!(
            config.speech.voice.as_deref(),
            Some("en-US-Chirp3-HD-Charon")
        );

        // And it leaves on the next write rather than lingering in the file.
        let back = toml::to_string(&config).expect("and writes back");
        assert!(!back.contains("engine"), "{back}");
        assert!(!back.contains("kokoro"), "{back}");
    }

    use super::*;

    fn scratch(name: &str, text: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("deck-config-{name}.toml"));
        std::fs::write(&path, text).expect("the scratch file is writable");
        path
    }

    #[test]
    fn no_file_is_the_designs_own_answer() {
        let missing = std::env::temp_dir().join("deck-config-never-written.toml");
        let _ = std::fs::remove_file(&missing);

        let config = read_from(&missing).expect("a missing config is not a failure");
        assert_eq!(config.theme.paper, deck_core::theme::Paper::Grey);
    }

    #[test]
    fn a_config_that_says_one_thing_changes_one_thing() {
        let path = scratch(
            "one-word",
            "[theme]\npaper = \"warm\"\n\n[layout]\narrange = \"columns\"\n",
        );
        let config = read_from(&path).expect("this config parses");

        assert_eq!(config.theme.paper, deck_core::theme::Paper::Warm);
        assert_eq!(config.layout.arrange, deck_core::layout::Arrange::Columns);
        assert_eq!(config.layout.max_columns, 3, "the rest is untouched");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_misspelled_key_is_said_out_loud() {
        // The one place in deck where being forgiving is wrong: this file is
        // written by a person, and an override that quietly does nothing is a
        // worse afternoon than one that refuses.
        let path = scratch("typo", "[theme.colors]\nacent = \"#af3a03\"\n");
        let err = read_from(&path).expect_err("a key nobody recognises is an error");

        assert!(err.to_string().contains("acent"), "and it names the key");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_colour_that_is_not_one_is_said_out_loud_too() {
        let path = scratch("colour", "[theme.colors]\naccent = \"nearly red\"\n");
        assert!(read_from(&path).is_err());

        let _ = std::fs::remove_file(&path);
    }
}
