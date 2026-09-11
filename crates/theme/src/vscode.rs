//! Reading a palette out of VS Code.
//!
//! Two files and a search. `settings.json` names a theme by its *label*, which
//! is what the theme picker shows and not what any file is called; every
//! extension's `package.json` declares the labels it contributes and where the
//! theme JSON for each one lives. So finding a theme means reading every
//! manifest until one claims the name.
//!
//! # Comments in the settings
//!
//! VS Code's settings are JSONC — JSON with comments and trailing commas, which
//! `serde_json` will not read. They are stripped before parsing rather than
//! parsed properly: this needs one string out of the file, and a whole JSONC
//! parser is a dependency to answer a question that small.
//!
//! # Scopes
//!
//! A theme's syntax colours are TextMate scopes, and a scope is a dotted path
//! whose prefixes match: a rule for `string` colours `string.quoted.double`
//! unless something more specific claims it. So the mapping here is *longest
//! prefix wins*, which is the rule TextMate itself uses and the reason a theme
//! can say `entity.name.function` and `entity` and mean two different things.

use std::path::{Path, PathBuf};

use deck_core::home;
use deck_core::theme::{Imported, Rgb, Syntax, pick_accent};

/// One theme, as an extension's manifest declares it.
#[derive(Debug, serde::Deserialize)]
struct Declared {
    label: Option<String>,
    id: Option<String>,
    path: String,
}

#[derive(Debug, serde::Deserialize)]
struct Contributes {
    #[serde(default)]
    themes: Vec<Declared>,
}

#[derive(Debug, serde::Deserialize)]
struct Manifest {
    contributes: Option<Contributes>,
}

/// A theme file: the editor's own colours, and the code's.
#[derive(Debug, Default, serde::Deserialize)]
struct Theme {
    /// The editor's chrome, by key.
    ///
    /// Values arrive as `Value` rather than `String` because a real theme does
    /// not keep the promise its schema makes: one shipped key holds a *list* of
    /// colours, and a stricter type refused the whole file over a key deck does
    /// not read. Anything that is not a string is skipped when it is asked for.
    #[serde(default)]
    colors: std::collections::HashMap<String, serde_json::Value>,
    #[serde(default, rename = "tokenColors")]
    tokens: Vec<Token>,
    /// A theme may be a patch on another one, by relative path.
    include: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct Token {
    /// One scope or several; VS Code allows a string or a list.
    #[serde(default)]
    scope: Scope,
    settings: Settings,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(untagged)]
enum Scope {
    One(String),
    Many(Vec<String>),
    #[default]
    None,
}

#[derive(Debug, serde::Deserialize)]
struct Settings {
    foreground: Option<String>,
}

/// Read the reader's VS Code colours.
///
/// # Errors
///
/// When the settings cannot be read, name no theme, or name one that is not
/// installed.
pub fn read() -> anyhow::Result<Imported> {
    read_from(CODE, None, true)?
        .ok_or_else(|| anyhow::anyhow!("{} has no theme deck can read", CODE.name))
}

/// One of the editors built on VS Code.
///
/// They are the same program with a different name on the window, so they keep
/// their settings in the same format in a different directory and their
/// extensions in a different one again. Three names is the whole difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flavour {
    /// What it is called, for an error message the reader will recognise.
    pub name: &'static str,
    /// The directory it keeps its settings under.
    pub settings: &'static str,
    /// The directory under `$HOME` where its extensions live.
    pub extensions: &'static str,
    /// What its own installed directory is called on macOS, on Windows, and on
    /// everything else.
    ///
    /// Three names because no two platforms agree: a bundle, a display name,
    /// and a lowercase one. They are names rather than paths so that [`bundled`]
    /// can ask the platform where applications go instead of this deciding.
    pub mac: &'static str,
    /// As `mac`, for Windows.
    pub windows: &'static str,
    /// As `mac`, for Linux and the rest.
    pub unix: &'static str,
}

/// Plain VS Code.
pub const CODE: Flavour = Flavour {
    name: "VS Code",
    settings: "Code",
    extensions: ".vscode/extensions",
    mac: "Visual Studio Code.app",
    windows: "Microsoft VS Code",
    unix: "code",
};
/// Cursor.
pub const CURSOR: Flavour = Flavour {
    name: "Cursor",
    settings: "Cursor",
    extensions: ".cursor/extensions",
    mac: "Cursor.app",
    windows: "cursor",
    unix: "cursor",
};
/// Windsurf.
pub const WINDSURF: Flavour = Flavour {
    name: "Windsurf",
    settings: "Windsurf",
    extensions: ".windsurf/extensions",
    mac: "Windsurf.app",
    windows: "Windsurf",
    unix: "windsurf",
};

/// Where this flavour keeps the themes it ships with.
///
/// Every place the platform installs applications, times the three names the
/// directory goes by. This used to be one absolute `/Applications/…` string,
/// which meant a reader anywhere else found none of the themes their editor
/// came with — and those are the common case, not the rare one, because an
/// editor left on its own default writes no theme name down at all.
#[must_use]
pub fn bundled(flavour: Flavour) -> Vec<PathBuf> {
    home::applications()
        .into_iter()
        .flat_map(|at| {
            [
                at.join(flavour.mac)
                    .join("Contents/Resources/app/extensions"),
                at.join(flavour.windows).join("resources/app/extensions"),
                at.join(flavour.unix).join("resources/app/extensions"),
            ]
        })
        .collect()
}

/// Whether this flavour is on the machine at all.
///
/// Its settings, its extensions, or the application. Not settings alone: an
/// editor on its own defaults keeps none, and a fresh install would come back
/// as missing.
#[must_use]
pub fn here(flavour: Flavour, home: &Path) -> bool {
    settings_of(flavour, home).is_some()
        || home.join(flavour.extensions).exists()
        || bundled(flavour).iter().any(|at| at.exists())
}

/// Where this flavour keeps its settings, if it does.
#[must_use]
pub fn settings_of(flavour: Flavour, home: &Path) -> Option<PathBuf> {
    settings_places(flavour, home)
        .into_iter()
        .find(|path| path.is_file())
}

/// Every place this flavour's settings could be, best first.
fn settings_places(flavour: Flavour, home: &Path) -> Vec<PathBuf> {
    let under = |at: PathBuf| at.join(flavour.settings).join("User/settings.json");

    // What the platform says, first — it is the only answer that survives a
    // redirected `%APPDATA%`, which a roaming Windows profile has.
    let mut places: Vec<PathBuf> = home::config().map(under).into_iter().collect();

    // Then the three shapes spelled out under the home given. This is what
    // makes the function testable, and it is the answer on a machine whose
    // variables are not set. `AppData/Roaming` was the one missing before, so
    // Windows fell through to `.config` and found nothing.
    places.extend(
        ["Library/Application Support", "AppData/Roaming", ".config"]
            .into_iter()
            .map(|at| under(home.join(at))),
    );

    places
}

/// Read one flavour's colours, or a named theme of it.
///
/// `named` is for trying a theme on without changing the editor's own settings,
/// which is what `deck open --theme "vscode:Solarized Dark"` does.
///
/// # Errors
///
/// When the settings cannot be read, name no theme, or name one that is not
/// installed.
pub fn read_from(
    flavour: Flavour,
    named: Option<&str>,
    dark: bool,
) -> anyhow::Result<Option<Imported>> {
    let home = home::home().ok_or_else(|| anyhow::anyhow!("no home directory"))?;

    // What the editor is set to, or what it would show if it were opened.
    //
    // Settings that name no theme are the ordinary case, not a broken one: an
    // editor left on its own default writes nothing down, and a fresh Cursor
    // install has three lines in it, none of them a colour. Failing there would
    // mean deck could only borrow from somebody who had already gone looking
    // for a theme.
    let wanted = match named {
        Some(name) => Some(name.to_string()),
        None => settings_of(flavour, &home)
            .and_then(|settings| std::fs::read_to_string(settings).ok())
            .and_then(|text| chosen(&text)),
    };

    let path = match wanted {
        Some(wanted) => Some(
            find(&wanted, flavour, &home)
                .ok_or_else(|| anyhow::anyhow!("no theme called `{wanted}` is installed"))?,
        ),
        // Nothing set, so whatever it would show — and which of its two
        // defaults depends on the machine, not on deck.
        None => default_theme(flavour, &home, dark),
    };

    match path {
        Some(path) => from_theme(&load(&path)?).map(Some),
        None => Ok(None),
    }
}

/// Every theme this flavour could show, by name.
#[must_use]
pub fn installed(flavour: Flavour, home: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for (at, manifest) in manifests(flavour, home) {
        for theme in manifest.contributes.map(|c| c.themes).unwrap_or_default() {
            if let Some(name) = name_of(&theme, &at) {
                out.push(name);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The theme named in `settings`, if one is.
fn chosen(settings: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(&strip_comments(settings)).ok()?;
    parsed
        .get("workbench.colorTheme")?
        .as_str()
        .map(ToString::to_string)
}

/// `text` with its comments taken out.
///
/// Shared with the Zed reader, whose settings are the same dialect.
///
/// Line and block comments, and never inside a string — a Windows path in a
/// setting is full of `\\`, and a URL is full of `//`.
pub(crate) fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            '/' if chars.peek() == Some(&'/') => {
                for ch in chars.by_ref() {
                    if ch == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut star = false;
                for ch in chars.by_ref() {
                    if star && ch == '/' {
                        break;
                    }
                    star = ch == '*';
                }
            }
            _ => out.push(ch),
        }
    }
    // Trailing commas, which JSONC allows and JSON does not.
    let mut cleaned = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(at) = rest.find(',') {
        let (before, after) = rest.split_at(at + 1);
        cleaned.push_str(before);
        rest = after;
        if rest.trim_start().starts_with(['}', ']']) {
            cleaned.pop();
        }
    }
    cleaned.push_str(rest);
    cleaned
}

/// Where the theme called `label` lives, if it is installed.
///
/// Every extension declares its themes by label, so this reads manifests until
/// one claims the name. There are a few hundred at most and each is a small
/// file; a cache would be a second thing to invalidate.
fn find(label: &str, flavour: Flavour, home: &Path) -> Option<PathBuf> {
    for (at, manifest) in manifests(flavour, home) {
        for theme in manifest.contributes.map(|c| c.themes).unwrap_or_default() {
            let named = name_of(&theme, &at).as_deref() == Some(label)
                || theme.id.as_deref() == Some(label);
            if named {
                return Some(at.join(theme.path.trim_start_matches("./")));
            }
        }
    }
    None
}

/// The theme this flavour shows when nobody has chosen one.
///
/// By id rather than by name: an id is stable across versions and translations,
/// and a label is neither. Newest first — an editor that ships Dark Modern
/// defaults to it, and one old enough not to defaults to Dark+.
fn default_theme(flavour: Flavour, home: &Path, dark: bool) -> Option<PathBuf> {
    let ids: [&str; 3] = if dark {
        ["Default Dark Modern", "Default Dark+", "Visual Studio Dark"]
    } else {
        [
            "Default Light Modern",
            "Default Light+",
            "Visual Studio Light",
        ]
    };
    ids.into_iter().find_map(|id| find(id, flavour, home))
}

/// Every extension manifest this flavour can see, with the directory it is in.
fn manifests(flavour: Flavour, home: &Path) -> Vec<(PathBuf, Manifest)> {
    let mut places = vec![home.join(flavour.extensions)];
    places.extend(bundled(flavour));
    let mut out = Vec::new();

    for place in places {
        let Ok(entries) = std::fs::read_dir(&place) else {
            continue;
        };
        for entry in entries.flatten() {
            let at = entry.path();
            let Ok(text) = std::fs::read_to_string(at.join("package.json")) else {
                continue;
            };
            if let Ok(parsed) = serde_json::from_str::<Manifest>(&text) {
                out.push((at, parsed));
            }
        }
    }
    out
}

/// What a declared theme is actually called.
///
/// A label of `%themeLabel%` is a lookup, not a name: every theme VS Code ships
/// with is written that way so it can be translated, and the real name lives in
/// the extension's `package.nls.json`. Without following it, none of the
/// built-in themes — Monokai, Solarized, Quiet Light — can be found by the name
/// the theme picker shows.
fn name_of(theme: &Declared, at: &Path) -> Option<String> {
    let label = theme.label.clone().or_else(|| theme.id.clone())?;
    let Some(key) = label
        .strip_prefix('%')
        .and_then(|key| key.strip_suffix('%'))
    else {
        return Some(label);
    };

    let text = std::fs::read_to_string(at.join("package.nls.json")).ok()?;
    let names: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(&strip_comments(&text)).ok()?;
    names
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .or_else(|| theme.id.clone())
}

/// Read a theme file, following whatever it is built on.
fn load(path: &Path) -> anyhow::Result<Theme> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("cannot read {}: {err}", path.display()))?;
    let mut theme: Theme = serde_json::from_str(&strip_comments(&text))
        .map_err(|err| anyhow::anyhow!("{} is not a theme: {err}", path.display()))?;

    // A theme that patches another is the base plus its own, and its own wins.
    // One level: two is rare enough that a loop would be code answering a case
    // nobody has.
    if let Some(base) = theme.include.take()
        && let Some(under) = path.parent().map(|at| at.join(base))
        && let Ok(text) = std::fs::read_to_string(&under)
        && let Ok(base) = serde_json::from_str::<Theme>(&strip_comments(&text))
    {
        let mut colors = base.colors;
        colors.extend(theme.colors);
        theme.colors = colors;

        let mut tokens = base.tokens;
        tokens.append(&mut theme.tokens);
        theme.tokens = tokens;
    }
    Ok(theme)
}

/// Turn a theme into what deck imports.
fn from_theme(theme: &Theme) -> anyhow::Result<Imported> {
    let colour = |key: &str| {
        theme
            .colors
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(Rgb::from_hex)
    };

    let bg = colour("editor.background")
        .or_else(|| colour("editorPane.background"))
        .ok_or_else(|| anyhow::anyhow!("this theme sets no editor background"))?;

    // A theme need not say what it writes in. Several shipped ones set only a
    // background and let VS Code's own default supply the rest — Quiet Light is
    // one — so a missing foreground is a gap to fill rather than a theme to
    // refuse. The workbench's own text is the next best answer, and failing
    // that, whatever reads on the page.
    let fg = colour("editor.foreground")
        .or_else(|| colour("foreground"))
        .or_else(|| scope(theme, "source"))
        .unwrap_or_else(|| {
            if bg.is_dark() {
                Rgb::new(0xdd, 0xdd, 0xdd)
            } else {
                Rgb::new(0x33, 0x33, 0x33)
            }
        });

    let accent = pick_accent(
        bg,
        fg,
        &[
            colour("editorCursor.foreground"),
            colour("focusBorder"),
            colour("textLink.foreground"),
            colour("editor.findMatchBackground"),
            scope(theme, "keyword"),
            colour("editor.selectionBackground"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>(),
    );

    Ok(Imported {
        bg,
        fg,
        accent,
        comment: scope(theme, "comment"),
        // The gutter's own marks, which are ink. The diff *editor* colours in
        // this file are grounds with alpha on them, and deck makes its own.
        add: colour("editorGutter.addedBackground"),
        del: colour("editorGutter.deletedBackground"),
        syntax: Syntax {
            keyword: scope(theme, "keyword"),
            string: scope(theme, "string"),
            function: scope(theme, "entity.name.function"),
            type_: scope(theme, "entity.name.type"),
            literal: scope(theme, "constant.numeric"),
        },
    })
}

/// The colour a theme gives `wanted`, by TextMate's own rule.
///
/// Longest prefix wins: a rule for `entity.name.function` beats one for
/// `entity`, which is how a theme says two different things about names that
/// share a root. Ties go to the later rule, because a theme is read top to
/// bottom and the last word is the one it meant.
fn scope(theme: &Theme, wanted: &str) -> Option<Rgb> {
    let mut best: Option<(usize, Rgb)> = None;

    for token in &theme.tokens {
        let Some(colour) = token.settings.foreground.as_deref().and_then(Rgb::from_hex) else {
            continue;
        };
        let scopes: &[String] = match &token.scope {
            Scope::One(one) => std::slice::from_ref(one),
            Scope::Many(many) => many,
            Scope::None => &[],
        };

        for scope in scopes {
            // A theme may list several scopes in one string, comma separated.
            for scope in scope.split(',').map(str::trim) {
                if wanted == scope || wanted.starts_with(&format!("{scope}.")) {
                    let len = scope.len();
                    if best.is_none_or(|(had, _)| len >= had) {
                        best = Some((len, colour));
                    }
                }
            }
        }
    }
    best.map(|(_, colour)| colour)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_keeps_its_settings_somewhere_that_is_looked_at() {
        // The bug this pins: the list held the macOS shape and then `.config`,
        // so a Windows reader fell through both and deck decided their editor
        // had no settings at all. Checked on the list rather than on disk —
        // `settings_of` asks the real platform first, which is right and is
        // also not something a test can depend on.
        //
        // Compared as paths rather than as the strings they print to. Windows
        // renders a separator this test does not write, so the string form of
        // the same path differs by platform — and a test of where deck looks
        // has no business also testing how a path is spelled.
        let home = Path::new("/somebody");
        let looked_at = settings_places(CODE, home);

        for shape in [
            "AppData/Roaming/Code/User/settings.json",
            "Library/Application Support/Code/User/settings.json",
            ".config/Code/User/settings.json",
        ] {
            let wanted = home.join(shape);
            assert!(
                looked_at.contains(&wanted),
                "nothing looks at {}",
                wanted.display()
            );
        }
    }

    #[test]
    fn an_editor_is_looked_for_wherever_the_platform_installs_things() {
        // This was one `/Applications/…` string, so the themes an editor ships
        // with — the only ones a reader on defaults has — were unfindable off
        // macOS. Every application root, times the three names the directory
        // goes by.
        let places = bundled(CODE);
        assert_eq!(places.len(), deck_core::home::applications().len() * 3);

        // On the components rather than on the rendered string, for the same
        // reason: the separator between them is the platform's business.
        for name in ["Visual Studio Code.app", "Microsoft VS Code", "code"] {
            assert!(
                places
                    .iter()
                    .any(|at| at.components().any(|part| part.as_os_str() == name)),
                "no place is called {name}"
            );
        }
    }

    #[test]
    fn comments_come_out_and_strings_are_left_alone() {
        let settings = r#"{
            // the theme
            "workbench.colorTheme": "GitHub Dark Default", // and a trailing note
            "path": "C:\\dev // not a comment",
            "url": "https://example.com",
        }"#;
        assert_eq!(chosen(settings).as_deref(), Some("GitHub Dark Default"));
    }

    #[test]
    fn a_block_comment_comes_out_too() {
        let settings = r#"{ /* off for now
            "workbench.colorTheme": "Monokai", */
            "workbench.colorTheme": "Solarized Light" }"#;
        assert_eq!(chosen(settings).as_deref(), Some("Solarized Light"));
    }

    /// The shape a real theme has, cut down to what deck reads.
    fn theme() -> Theme {
        serde_json::from_str(
            r##"{
              "colors": {
                "editor.background": "#0d1117",
                "editor.foreground": "#e6edf3",
                "editorCursor.foreground": "#2f81f7",
                "editorGutter.addedBackground": "#3fb950",
                "editorGutter.deletedBackground": "#f85149"
              },
              "tokenColors": [
                { "scope": ["comment", "string.comment"], "settings": { "foreground": "#8b949e" } },
                { "scope": "entity", "settings": { "foreground": "#d2a8ff" } },
                { "scope": "entity.name.function", "settings": { "foreground": "#d2a8ff" } },
                { "scope": ["keyword", "storage"], "settings": { "foreground": "#ff7b72" } },
                { "scope": "string", "settings": { "foreground": "#a5d6ff" } }
              ]
            }"##,
        )
        .expect("this is the shape a theme has")
    }

    #[test]
    fn a_real_theme_reads() {
        let got = from_theme(&theme()).expect("a background and a foreground are set");

        assert_eq!(got.bg, Rgb::from_hex("#0d1117").unwrap());
        assert_eq!(got.accent, Rgb::from_hex("#2f81f7").unwrap());
        assert_eq!(got.comment, Rgb::from_hex("#8b949e"));
        assert_eq!(got.syntax.keyword, Rgb::from_hex("#ff7b72"));
        assert_eq!(got.add, Rgb::from_hex("#3fb950"));
    }

    #[test]
    fn a_theme_that_names_no_foreground_still_reads() {
        // Quiet Light, which VS Code ships, sets a background and lets the
        // editor's own default supply the text. A gap to fill, not a theme to
        // refuse.
        let quiet: Theme =
            serde_json::from_str(r##"{ "colors": { "editor.background": "#f5f5f5" } }"##)
                .expect("this parses");

        let got = from_theme(&quiet).expect("a background is enough");
        assert!(
            got.fg.is_dark(),
            "a light page has to be written on in something dark"
        );
    }

    #[test]
    fn an_editor_with_no_theme_set_is_not_an_error() {
        // The ordinary case, not a broken one: an editor left on its own
        // default writes nothing down. A fresh Cursor install has three lines
        // in its settings and none of them is a colour.
        assert_eq!(chosen(r#"{ "window.commandCenter": true }"#), None);
    }

    #[test]
    fn a_scope_matches_by_its_prefix() {
        // `string` colours `string.quoted.double` — which is what the theme
        // means by writing one rule for all of them.
        assert_eq!(
            scope(&theme(), "string.quoted.double"),
            Rgb::from_hex("#a5d6ff")
        );
    }

    #[test]
    fn the_longer_rule_wins() {
        // `entity` and `entity.name.function` are both here, and a theme that
        // wrote both meant the second one for functions.
        assert_eq!(
            scope(&theme(), "entity.name.function"),
            Rgb::from_hex("#d2a8ff")
        );
        assert_eq!(scope(&theme(), "entity.other"), Rgb::from_hex("#d2a8ff"));
    }

    #[test]
    fn a_scope_nobody_claims_is_nothing() {
        assert_eq!(scope(&theme(), "markup.heading"), None);
    }
}
