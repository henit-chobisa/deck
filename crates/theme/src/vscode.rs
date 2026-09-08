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
    /// And the application bundle, for the themes it ships with.
    pub bundled: &'static str,
}

/// Plain VS Code.
pub const CODE: Flavour = Flavour {
    name: "VS Code",
    settings: "Code",
    extensions: ".vscode/extensions",
    bundled: "/Applications/Visual Studio Code.app/Contents/Resources/app/extensions",
};
/// Cursor.
pub const CURSOR: Flavour = Flavour {
    name: "Cursor",
    settings: "Cursor",
    extensions: ".cursor/extensions",
    bundled: "/Applications/Cursor.app/Contents/Resources/app/extensions",
};
/// Windsurf.
pub const WINDSURF: Flavour = Flavour {
    name: "Windsurf",
    settings: "Windsurf",
    extensions: ".windsurf/extensions",
    bundled: "/Applications/Windsurf.app/Contents/Resources/app/extensions",
};

/// Whether this flavour is on the machine at all.
///
/// Its settings, its extensions, or the application. Not settings alone: an
/// editor on its own defaults keeps none, and a fresh install would come back
/// as missing.
#[must_use]
pub fn here(flavour: Flavour, home: &Path) -> bool {
    settings_of(flavour, home).is_some()
        || home.join(flavour.extensions).exists()
        || Path::new(flavour.bundled).exists()
}

/// Where this flavour keeps its settings, if it does.
#[must_use]
pub fn settings_of(flavour: Flavour, home: &Path) -> Option<PathBuf> {
    // Where macOS keeps it, then where everyone else does.
    [
        home.join(format!(
            "Library/Application Support/{}/User/settings.json",
            flavour.settings
        )),
        home.join(format!(".config/{}/User/settings.json", flavour.settings)),
    ]
    .into_iter()
    .find(|path| path.is_file())
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
    let home = PathBuf::from(
        std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("no home directory"))?,
    );

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
    let places = [
        home.join(flavour.extensions),
        PathBuf::from(flavour.bundled),
    ];
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
