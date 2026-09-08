//! Reading a palette out of Zed.
//!
//! The cheapest of the three, and the reason is one decision Zed made: its
//! theme files name syntax colours with **tree-sitter capture names** — the
//! same `keyword`, `string`, `function` that deck's own highlighter reports. So
//! there is no mapping. VS Code needs one because TextMate scopes are a
//! different vocabulary; Neovim needs a subprocess because it has no theme file
//! at all.
//!
//! A theme file holds several themes — a light and a dark, usually — and
//! `settings.json` names one by its display name.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use deck_core::theme::{Imported, Rgb, Syntax, pick_accent};

/// A theme file: a family of themes under one name.
#[derive(Debug, serde::Deserialize)]
struct Family {
    #[serde(default)]
    themes: Vec<Theme>,
}

#[derive(Debug, serde::Deserialize)]
struct Theme {
    name: String,
    #[serde(default)]
    style: Style,
}

#[derive(Debug, Default, serde::Deserialize)]
struct Style {
    #[serde(flatten)]
    colors: HashMap<String, serde_json::Value>,
    #[serde(default)]
    syntax: HashMap<String, Highlight>,
}

#[derive(Debug, serde::Deserialize)]
struct Highlight {
    color: Option<String>,
}

/// Read the reader's Zed colours.
///
/// `None` when there is nothing on disk to read. Zed ships its defaults —
/// One Dark and One Light — compiled into the binary rather than as theme
/// files, so an installation nobody has themed has nothing to borrow. Deck uses
/// its own palette then, in whatever mode the machine is in, which is the
/// closest thing to the truth available.
///
/// # Errors
///
/// When the settings will not parse, or name a theme that is not installed.
pub fn read(named: Option<&str>, dark: bool) -> anyhow::Result<Option<Imported>> {
    let home = PathBuf::from(
        std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("no home directory"))?,
    );

    let wanted = match named {
        Some(name) => Some(name.to_string()),
        None => std::fs::read_to_string(home.join(".config/zed/settings.json"))
            .ok()
            .and_then(|text| chosen(&text))
            // What it would show if it were opened. Named on the chance the
            // reader has One Dark installed as an extension, which many do —
            // and skipped without complaint when they have not.
            .or_else(|| {
                Some(if dark { "One Dark" } else { "One Light" }.to_string())
                    .filter(|name| find(name, &home).is_some())
            }),
    };

    let Some(wanted) = wanted else {
        // Nothing set, and its defaults are not on disk. See `ONE`.
        return Ok(Some(one(dark)));
    };

    // A named theme has to exist; a *default* one falls back on the built-in of
    // the same appearance, because that is what Zed itself would show.
    match find(&wanted, &home) {
        Some(theme) => from_theme(&theme).map(Some),
        None if named.is_some() => Err(anyhow::anyhow!("no theme called `{wanted}` is installed")),
        None => Ok(Some(one(dark))),
    }
}

/// The theme named in `settings`.
///
/// Zed's `theme` is either a name or an object with a `light` and a `dark` and
/// a `mode`. Both are read; the object is asked for the mode it says it is in.
fn chosen(settings: &str) -> Option<String> {
    let parsed: serde_json::Value =
        serde_json::from_str(&crate::vscode::strip_comments(settings)).ok()?;
    let theme = parsed.get("theme")?;

    if let Some(name) = theme.as_str() {
        return Some(name.to_string());
    }
    let mode = theme.get("mode").and_then(serde_json::Value::as_str);
    theme
        .get(mode.unwrap_or("dark"))
        .or_else(|| theme.get("dark"))
        .or_else(|| theme.get("light"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

/// The theme called `name`, wherever it is installed.
fn find(name: &str, home: &Path) -> Option<Theme> {
    let places = [
        home.join(".config/zed/themes"),
        home.join("Library/Application Support/Zed/extensions/installed"),
        PathBuf::from("/Applications/Zed.app/Contents/Resources/themes"),
    ];

    for place in places {
        for file in files_under(&place) {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let Ok(family) = serde_json::from_str::<Family>(&text) else {
                continue;
            };
            if let Some(theme) = family.themes.into_iter().find(|theme| theme.name == name) {
                return Some(theme);
            }
        }
    }
    None
}

/// Every JSON file under `at`, one directory deep.
///
/// An extension keeps its themes in a `themes/` of its own, so the search has
/// to look inside a directory as well as at it.
fn files_under(at: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(at) else {
        return out;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|end| end == "json") {
            out.push(path);
        } else if path.is_dir()
            && let Ok(inner) = std::fs::read_dir(path.join("themes"))
        {
            out.extend(
                inner
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().is_some_and(|end| end == "json")),
            );
        }
    }
    out
}

/// Turn a Zed theme into what deck imports.
fn from_theme(theme: &Theme) -> anyhow::Result<Imported> {
    let colour = |key: &str| {
        theme
            .style
            .colors
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(Rgb::from_hex)
    };
    let capture = |name: &str| {
        theme
            .style
            .syntax
            .get(name)
            .and_then(|hl| hl.color.as_deref())
            .and_then(Rgb::from_hex)
    };

    let bg = colour("editor.background")
        .ok_or_else(|| anyhow::anyhow!("this theme sets no editor background"))?;
    let fg = colour("editor.foreground")
        .ok_or_else(|| anyhow::anyhow!("this theme sets no editor foreground"))?;

    Ok(Imported {
        bg,
        fg,
        accent: pick_accent(
            bg,
            fg,
            &[
                colour("players.0.cursor"),
                colour("border.focused"),
                colour("link_text.hover"),
                capture("keyword"),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
        ),
        // Named the same as deck's own highlighter reports them, which is the
        // whole reason this file is short.
        comment: capture("comment"),
        add: colour("created").or_else(|| colour("version_control.added")),
        del: colour("deleted").or_else(|| colour("version_control.deleted")),
        syntax: Syntax {
            keyword: capture("keyword"),
            string: capture("string"),
            function: capture("function"),
            type_: capture("type"),
            literal: capture("number").or_else(|| capture("constant")),
        },
    })
}

/// Zed's own default, which is not a file.
///
/// One Dark and One Light are compiled into the Zed binary as embedded assets
/// rather than shipped as theme files, so an installation nobody has themed has
/// nothing on disk to read — and that is most installations.
///
/// These values were taken out of an installed Zed, from the JSON it carries at
/// `themes/one/one.json`. Not remembered, and not approximated: a palette
/// claiming to be somebody's theme has to *be* it. They will drift when Zed
/// retunes One, which is the cost of Zed keeping them where nothing else can
/// read them.
fn one(dark: bool) -> Imported {
    let hex = |s: &str| Rgb::from_hex(s).unwrap_or(Rgb::new(0, 0, 0));

    if dark {
        Imported {
            bg: hex("#282c33"),
            fg: hex("#acb2be"),
            accent: hex("#74ade8"),
            comment: Some(hex("#5d636f")),
            add: Some(hex("#a1c181")),
            del: Some(hex("#d07277")),
            syntax: Syntax {
                keyword: Some(hex("#b477cf")),
                string: Some(hex("#a1c181")),
                function: Some(hex("#73ade9")),
                type_: Some(hex("#6eb4bf")),
                literal: Some(hex("#bf956a")),
            },
        }
    } else {
        Imported {
            bg: hex("#fafafa"),
            fg: hex("#242529"),
            accent: hex("#5c78e2"),
            comment: Some(hex("#a2a3a7")),
            add: Some(hex("#669f59")),
            del: Some(hex("#d36151")),
            syntax: Syntax {
                keyword: Some(hex("#a449ab")),
                string: Some(hex("#649f57")),
                function: Some(hex("#5b79e3")),
                type_: Some(hex("#3882b7")),
                literal: Some(hex("#ad6e25")),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_named_outright() {
        assert_eq!(
            chosen(r#"{ "theme": "One Dark" }"#).as_deref(),
            Some("One Dark")
        );
    }

    #[test]
    fn a_theme_that_follows_the_system() {
        // Zed's own shape for "light in the day, dark at night". The mode says
        // which one is in use, and that is the one deck should look like.
        let settings = r#"{
            "theme": { "mode": "light", "light": "One Light", "dark": "One Dark" }
        }"#;
        assert_eq!(chosen(settings).as_deref(), Some("One Light"));
    }

    #[test]
    fn zeds_own_default_is_whichever_way_the_machine_is() {
        assert!(one(true).bg.is_dark());
        assert!(!one(false).bg.is_dark());
        assert!(
            one(true).syntax.keyword.is_some(),
            "and it is a whole theme"
        );
    }

    #[test]
    fn a_theme_file_reads() {
        let family: Family = serde_json::from_str(
            r##"{
              "name": "One",
              "themes": [{
                "name": "One Dark",
                "style": {
                  "editor.background": "#282c34",
                  "editor.foreground": "#dcdfe4",
                  "border.focused": "#61afef",
                  "created": "#98c379",
                  "syntax": {
                    "keyword": { "color": "#c678dd" },
                    "string": { "color": "#98c379" },
                    "comment": { "color": "#5c6370" }
                  }
                }
              }]
            }"##,
        )
        .expect("this is the shape a Zed theme has");

        let got = from_theme(&family.themes[0]).expect("a background and foreground are set");
        assert_eq!(got.bg, Rgb::from_hex("#282c34").unwrap());
        assert_eq!(got.accent, Rgb::from_hex("#61afef").unwrap());
        assert_eq!(got.syntax.keyword, Rgb::from_hex("#c678dd"));
        assert_eq!(got.comment, Rgb::from_hex("#5c6370"));
        assert_eq!(got.add, Rgb::from_hex("#98c379"));
    }
}
