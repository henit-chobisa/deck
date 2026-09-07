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
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".deck").join("config.toml"))
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
