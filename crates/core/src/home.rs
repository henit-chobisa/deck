//! Where a person's things live, on whichever machine this is.
//!
//! Three names for one idea, and they disagree on every platform:
//!
//! | | home | config | applications |
//! | --- | --- | --- | --- |
//! | macOS | `$HOME` | `~/Library/Application Support` | `/Applications` |
//! | Linux | `$HOME` | `$XDG_CONFIG_HOME` or `~/.config` | `/usr/share` |
//! | Windows | `%USERPROFILE%` | `%APPDATA%` | `%LOCALAPPDATA%\Programs` |
//!
//! Everything that reads an editor's settings needs all three, and spelling the
//! macOS one out in eight places is how deck came to run on exactly one kind of
//! machine. So they are answered here, once.
//!
//! No dependency for this. The crates that do it well carry a dozen platforms
//! deck will never see, and the whole of what deck needs is three environment
//! variables and a fallback.

use std::path::PathBuf;

/// The person's home directory.
///
/// `HOME` everywhere, and `USERPROFILE` on Windows, where `HOME` is set only if
/// somebody has been using a Unix shell.
#[must_use]
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// Where applications keep a person's settings.
///
/// Not the same place as `home`, and on two of the three platforms not under a
/// dot-directory at all. An editor's `settings.json` lives here.
#[must_use]
pub fn config() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| home().map(|home| home.join("AppData/Roaming")));
    }
    if cfg!(target_os = "macos") {
        return home().map(|home| home.join("Library/Application Support"));
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| home().map(|home| home.join(".config")))
}

/// The XDG-shaped config directory, on every platform.
///
/// Separate from [`config`] because several things deck reads use `~/.config`
/// *even on macOS* — Zed keeps its settings there, and the shared agents
/// directory lives there too. A program written on Linux and ported badly is
/// the usual reason, but it is where the files are.
///
/// On Windows there is no XDG-shaped directory to give, so this answers with
/// the roaming one and [`local_config`] answers with the other. Anything that
/// does not know which an editor chose should ask for both.
#[must_use]
pub fn dot_config() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        return config();
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| home().map(|home| home.join(".config")))
}

/// The config directory that stays on this machine.
///
/// Windows keeps two and they are not interchangeable: `%APPDATA%` follows a
/// person between the machines they sign in to, `%LOCALAPPDATA%` does not.
/// Editors disagree about which they want — nvim keeps its init in the local
/// one — and guessing wrong means reading somebody's settings from a directory
/// they have never written to. Everywhere else there is only one, and this and
/// [`dot_config`] give the same answer.
#[must_use]
pub fn local_config() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())
            .or_else(|| home().map(|home| home.join("AppData/Local")));
    }
    dot_config()
}

/// Where installed applications live, for the themes they ship with.
///
/// A list rather than one path: macOS has a system-wide and a per-user one, and
/// Linux has three depending on how the thing was installed.
#[must_use]
pub fn applications() -> Vec<PathBuf> {
    let mut places = Vec::new();

    if cfg!(target_os = "windows") {
        for var in ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"] {
            // An installer puts an editor either straight under the root or in
            // the `Programs` directory beneath it, so both are looked at. The
            // variable is read once: reading it twice let an empty one through
            // as a relative path, and every place here has to be somewhere.
            let Some(at) = std::env::var_os(var).map(PathBuf::from) else {
                continue;
            };
            if at.as_os_str().is_empty() {
                continue;
            }
            places.push(at.join("Programs"));
            places.push(at);
        }
    } else if cfg!(target_os = "macos") {
        places.push(PathBuf::from("/Applications"));
        if let Some(home) = home() {
            places.push(home.join("Applications"));
        }
    } else {
        places.push(PathBuf::from("/usr/share"));
        places.push(PathBuf::from("/usr/local/share"));
        places.push(PathBuf::from("/opt"));
        if let Some(home) = home() {
            places.push(home.join(".local/share"));
        }
    }
    places
}

/// Where deck keeps its own things: the config, and the window's shape.
#[must_use]
pub fn deck() -> Option<PathBuf> {
    home().map(|home| home.join(".deck"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_built_on_an_empty_variable() {
        // An empty `HOME` is worse than none: every path built on it would be
        // relative, and deck would write its config into whatever directory it
        // happened to be run from. Checked on the shape rather than by setting
        // the variable, because a test that changes the environment changes it
        // for every other test running beside it.
        for at in [home(), config(), dot_config(), local_config(), deck()]
            .into_iter()
            .flatten()
        {
            assert!(!at.as_os_str().is_empty());
        }
    }

    #[test]
    fn every_place_is_absolute_or_absent() {
        for at in applications() {
            assert!(at.is_absolute(), "{} is not somewhere", at.display());
        }
        for at in [config(), dot_config(), local_config(), deck()]
            .into_iter()
            .flatten()
        {
            assert!(at.is_absolute(), "{} is not somewhere", at.display());
        }
    }
}
