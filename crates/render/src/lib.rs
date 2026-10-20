//! How deck colours code, without a window.
//!
//! The window paints code through gpui-component's tree-sitter highlighter, and
//! every export (#170) has to colour the same code the same way: a PDF whose
//! keywords are a shade off the window's is a PDF that looks like it came from
//! somewhere else. Two highlighters would drift on their own — the queries that
//! decide what a keyword is change with every release of the library — so there
//! is one, and it is here. The window asks [`styled`] for its runs; an export
//! asks [`Code::spans`], which runs the very same call and reads the colours
//! back out exactly.
//!
//! The highlighter needs no window, no `App` and no GPU when it runs: it is a
//! tree-sitter parse and a table lookup. That is what lets this crate exist.
//! It still links gpui, whose types the highlighter is written in, so it builds
//! where the window builds and not in the jobs that leave the window out.

#![forbid(unsafe_code)]

pub mod document;
pub mod svg;
pub mod text;

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use deck_core::theme::{Palette, Rgb, Syntax};
use gpui_kit::component::Rope;
use gpui_kit::component::highlighter::{HighlightTheme, SyntaxHighlighter};
use gpui_kit::{FontStyle, FontWeight, HighlightStyle, Hsla};

/// The highlighter's name for the language in `file`.
///
/// A file extension is not a language name — the highlighter wants `rust`, and
/// the file is called `.rs`.
#[must_use]
pub fn language_of(file: &Path) -> &'static str {
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        Some("ts" | "mts" | "cts") => "typescript",
        Some("tsx") => "tsx",
        Some("js" | "mjs" | "cjs") => "javascript",
        // Not "jsx", which is nobody's grammar. The highlighter answers for a
        // name it does not know with `Plain` rather than an error, so every
        // `.jsx` file came up as unpainted text and nothing anywhere said why.
        // Tsx is the grammar that parses JSX tags; plain javascript does not.
        Some("jsx") => "tsx",
        Some("py") => "python",
        Some("go") => "go",
        Some("rb") => "ruby",
        Some("lua") => "lua",
        Some("md" | "markdown") => "markdown",
        Some("yml" | "yaml") => "yaml",
        Some("sh" | "bash" | "zsh") => "bash",
        Some("json") => "json",
        Some("toml") => "toml",
        Some("html") => "html",
        Some("css") => "css",
        Some("c" | "h") => "c",
        Some("cpp" | "cc" | "hpp") => "cpp",
        Some("java") => "java",
        _ => "text",
    }
}

/// One kind of token, and how it is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capture {
    /// The highlighter's name for it: `keyword`, `function.method`, `comment`.
    pub name: &'static str,
    /// Its colour.
    pub colour: Rgb,
    /// Whether it is set in italics.
    pub italic: bool,
}

/// Every kind of token deck paints, and how.
///
/// Syntax stays quiet. A review surface is read for its *shape* — what is lit,
/// what is washed back — and a rainbow underneath competes with the one thing
/// the deck is pointing at. The editor's own colours are taken where it had
/// them and deck's quiet set where it did not: mixed rather than all-or-nothing,
/// because a theme that names four of the five should give four, not none.
#[must_use]
pub fn captures(palette: &Palette, dark: bool, imported: Syntax) -> Vec<Capture> {
    let quiet = if dark {
        ("#d3869b", "#b8bb26", "#fabd2f", "#d3869b", "#8ec07c")
    } else {
        ("#8f3f71", "#79740e", "#b57614", "#8f3f71", "#427b58")
    };
    let taken = |had: Option<Rgb>, quiet: &str| {
        had.unwrap_or_else(|| Rgb::from_hex(quiet).unwrap_or(palette.fg))
    };
    let keyword = taken(imported.keyword, quiet.0);
    let string = taken(imported.string, quiet.1);
    let function = taken(imported.function, quiet.2);
    let literal = taken(imported.literal, quiet.3);
    let type_ = taken(imported.type_, quiet.4);
    let comment = palette.fg.mix(palette.wash, 0.42);

    let one = |name, colour| Capture {
        name,
        colour,
        italic: false,
    };
    let leaning = |name, colour| Capture {
        name,
        colour,
        italic: true,
    };
    vec![
        leaning("comment", comment),
        leaning("comment.doc", comment),
        one("keyword", keyword),
        one("string", string),
        one("string.special", string),
        one("function", function),
        one("function.method", function),
        one("constructor", function),
        one("type", type_),
        one("enum", type_),
        one("number", literal),
        one("boolean", literal),
        one("constant", literal),
        one("attribute", palette.muted),
        one("operator", palette.muted),
        one("punctuation", palette.muted),
        one("variable", palette.fg),
        one("property", palette.fg),
    ]
}

/// The theme the window's code panes are painted with.
///
/// Assembled as JSON and parsed back rather than constructed field by field.
/// `SyntaxColors` has some forty of them, most irrelevant here, and this is the
/// same shape a real editor theme arrives in.
#[must_use]
pub fn theme(palette: &Palette, dark: bool, imported: Syntax) -> Arc<HighlightTheme> {
    let syntax: Vec<String> = captures(palette, dark, imported)
        .iter()
        .map(|capture| {
            let style = if capture.italic {
                r#", "font_style": "italic""#
            } else {
                ""
            };
            format!(
                r#""{}": {{ "color": "{}"{style} }}"#,
                capture.name,
                capture.colour.to_hex()
            )
        })
        .collect();
    let json = format!(
        r#"{{
          "name": "deck",
          "appearance": "{mode}",
          "style": {{
            "editor.background": "{wash}",
            "editor.foreground": "{fg}",
            "editor.gutter.background": "{wash}",
            "editor.line_number": "{gutter}",
            "editor.active_line_number": "{muted}",
            "editor.active_line.background": "{wash}",
            "syntax": {{ {syntax} }}
          }}
        }}"#,
        mode = if dark { "dark" } else { "light" },
        wash = palette.wash.to_hex(),
        fg = palette.fg.to_hex(),
        // The gutter recedes further than any label: line numbers are there to
        // be referred to, not read.
        gutter = palette.fg.mix(palette.wash, 0.62).to_hex(),
        muted = palette.muted.to_hex(),
        syntax = syntax.join(", "),
    );

    serde_json::from_str(&json).map_or_else(
        // A palette cannot make this JSON invalid, so a failure here is a typo
        // in the literal above — and a deck rendered in the library's default
        // colours is better than one that does not open.
        |_| {
            if dark {
                HighlightTheme::default_dark()
            } else {
                HighlightTheme::default_light()
            }
        },
        Arc::new,
    )
}

/// The runs a code pane paints `source` with: byte ranges and their styles.
///
/// The window's own call. Every export goes through it too, by way of
/// [`Code::spans`], which is what keeps the two from ever disagreeing.
#[must_use]
pub fn styled(
    source: &str,
    file: &Path,
    theme: &HighlightTheme,
) -> Vec<(Range<usize>, HighlightStyle)> {
    styled_as(source, language_of(file), theme)
}

/// [`styled`], for code whose language is known by name rather than by a
/// file: a fenced block in Markdown says `rust`, and has no extension.
#[must_use]
pub fn styled_as(
    source: &str,
    language: &str,
    theme: &HighlightTheme,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut highlighter = SyntaxHighlighter::new(language);
    highlighter.update(None, &Rope::from(source), None);
    highlighter.styles(&(0..source.len()), theme)
}

/// One run of highlighted code, in plain colours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Byte offsets into the source.
    pub range: Range<usize>,
    /// Its colour, or `None` for the text's own.
    pub colour: Option<Rgb>,
    /// Set in italics.
    pub italic: bool,
    /// Set in bold.
    pub bold: bool,
}

/// Code highlighting for something that is not the window: an export, a
/// preview, a test.
pub struct Code {
    theme: Arc<HighlightTheme>,
    /// What a run with no colour of its own is written in.
    text: Rgb,
    /// Each capture's colour as the theme holds it, beside the colour it came
    /// from, so a run's colour is read back exactly rather than converted.
    colours: Vec<(Hsla, Rgb)>,
}

impl Code {
    /// Highlighting in this palette, as the window would paint it.
    #[must_use]
    pub fn new(palette: &Palette, dark: bool, imported: Syntax) -> Self {
        let theme = theme(palette, dark, imported);
        let colours = captures(palette, dark, imported)
            .into_iter()
            .filter_map(|capture| {
                let held = theme.style(capture.name)?.color?;
                Some((held, capture.colour))
            })
            .collect();
        Self {
            theme,
            text: palette.fg,
            colours,
        }
    }

    /// The theme it highlights with — the one the window installs.
    #[must_use]
    pub fn theme(&self) -> &Arc<HighlightTheme> {
        &self.theme
    }

    /// The colour of text no token claims: what a [`Span`] with no colour
    /// is written in.
    #[must_use]
    pub fn text(&self) -> Rgb {
        self.text
    }

    /// `source` as runs of plain colour, exactly as the window paints it.
    #[must_use]
    pub fn spans(&self, source: &str, file: &Path) -> Vec<Span> {
        self.spans_as(source, language_of(file))
    }

    /// [`Self::spans`], for code whose language is known by name.
    #[must_use]
    pub fn spans_as(&self, source: &str, language: &str) -> Vec<Span> {
        styled_as(source, language, &self.theme)
            .into_iter()
            .map(|(range, style)| Span {
                range,
                colour: style.color.and_then(|held| {
                    self.colours
                        .iter()
                        .find(|(theirs, _)| *theirs == held)
                        .map(|(_, ours)| *ours)
                }),
                italic: style.font_style == Some(FontStyle::Italic),
                bold: style
                    .font_weight
                    .is_some_and(|weight| weight >= FontWeight::BOLD),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::highlighter::Language;

    #[test]
    fn every_language_we_name_is_one_the_highlighter_knows() {
        // The failure this catches is silent by construction: an unknown name
        // comes back as `Plain`, so the file renders, reads fine, and is simply
        // never painted. `.jsx` did that from the first release to this test.

        for file in [
            "a.rs",
            "a.ts",
            "a.mts",
            "a.cts",
            "a.tsx",
            "a.js",
            "a.mjs",
            "a.cjs",
            "a.jsx",
            "a.py",
            "a.go",
            "a.rb",
            "a.lua",
            "a.md",
            "a.markdown",
            "a.yml",
            "a.yaml",
            "a.sh",
            "a.bash",
            "a.zsh",
            "a.json",
            "a.toml",
            "a.html",
            "a.css",
            "a.c",
            "a.h",
            "a.cpp",
            "a.cc",
            "a.hpp",
            "a.java",
        ] {
            let named = language_of(std::path::Path::new(file));
            assert_ne!(
                Language::from_str(named),
                Language::Plain,
                "{file} is called `{named}`, which the highlighter does not know"
            );
        }

        // A name the highlighter knows is not enough: `.jsx` called
        // `javascript` passes the check above and paints no tag.
        assert_eq!(language_of(std::path::Path::new("a.jsx")), "tsx");

        // And the fallback still is what it says it is.
        assert_eq!(language_of(std::path::Path::new("a.unknown")), "text");
    }

    use deck_core::theme::{Imported, derive};

    fn imported(dark: bool, syntax: Syntax) -> Imported {
        let hex = |s: &str| Rgb::from_hex(s).expect("a colour");
        Imported {
            bg: hex(if dark { "#1e1f22" } else { "#fafafa" }),
            fg: hex(if dark { "#d4d4d4" } else { "#2b2723" }),
            accent: hex(if dark { "#fe8019" } else { "#af3a03" }),
            comment: None,
            add: None,
            del: None,
            syntax,
        }
    }

    // The last three are where a library bump moves tokens first: tags, and one
    // language inside another — a script and a style in a page, a fenced block
    // in Markdown.
    const FIXTURES: [&str; 20] = [
        "a.rs", "a.ts", "a.py", "a.json", "a.md", "a.go", "a.tsx", "a.html", "b.md", "a.jsx",
        "a.js", "a.rb", "a.lua", "a.yaml", "a.sh", "a.toml", "a.css", "a.c", "a.cpp", "a.java",
    ];

    fn fixture(name: &str) -> String {
        let at = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        std::fs::read_to_string(at).expect("the fixture exists")
    }

    #[test]
    fn every_coloured_run_is_read_back_as_its_own_colour() {
        // The window's runs carry gpui colours. An export needs the colour the
        // table said, exactly, and a conversion back from gpui's would be a
        // shade off. So every coloured run must come back as one of the
        // table's own colours, and none may come back unknown.
        let syntax = Syntax {
            keyword: Rgb::from_hex("#cc3333"),
            ..Syntax::default()
        };
        for dark in [false, true] {
            let palette = derive(imported(dark, syntax));
            let code = Code::new(&palette, dark, syntax);
            let table: Vec<Rgb> = captures(&palette, dark, syntax)
                .iter()
                .map(|c| c.colour)
                .collect();
            for name in FIXTURES {
                let source = fixture(name);
                let styled = styled(&source, Path::new(name), code.theme());
                let spans = code.spans(&source, Path::new(name));
                assert_eq!(styled.len(), spans.len(), "{name}: one span per run");
                for ((range, style), span) in styled.iter().zip(&spans) {
                    assert_eq!(range, &span.range);
                    assert_eq!(
                        style.color.is_some(),
                        span.colour.is_some(),
                        "{name} {range:?}: a colour lost"
                    );
                    if let Some(colour) = span.colour {
                        assert!(
                            table.contains(&colour),
                            "{name}: {colour:?} is not the table's"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn code_is_coloured_as_it_was() {
        // What every export and the window paint code with, held still: any
        // change shows here as exactly which tokens moved. `UPDATE_GOLDEN=1`
        // writes it again, on purpose.
        let mut now = String::new();
        let quiet = Syntax::default();
        let editor = Syntax {
            keyword: Rgb::from_hex("#cc3333"),
            string: Rgb::from_hex("#33aa55"),
            ..Syntax::default()
        };
        for (theme, dark, syntax) in [
            ("light", false, quiet),
            ("dark", true, quiet),
            ("dark, two imported", true, editor),
        ] {
            let palette = derive(imported(dark, syntax));
            let code = Code::new(&palette, dark, syntax);
            for name in FIXTURES {
                let source = fixture(name);
                now += &format!("# {name}, {theme}\n");
                for span in code.spans(&source, Path::new(name)) {
                    let colour = span.colour.map_or_else(|| "text".to_string(), Rgb::to_hex);
                    let style = match (span.italic, span.bold) {
                        (true, true) => " italic bold",
                        (true, false) => " italic",
                        (false, true) => " bold",
                        (false, false) => "",
                    };
                    now += &format!(
                        "{:?} {colour}{style} {:?}\n",
                        span.range,
                        &source[span.range.clone()]
                    );
                }
            }
        }
        let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/spans.txt");
        if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
            std::fs::create_dir_all(at.parent().expect("a parent")).expect("writable");
            std::fs::write(&at, &now).expect("writable");
        }
        let was =
            std::fs::read_to_string(&at).expect("the golden file exists; run with UPDATE_GOLDEN=1");
        // Line endings are not colours: a checkout may have made them CRLF.
        let was = was.replace("\r\n", "\n");
        if now != was {
            // The first lines that differ, rather than two whole files.
            let (now, was): (Vec<&str>, Vec<&str>) = (now.lines().collect(), was.lines().collect());
            let at = now
                .iter()
                .zip(&was)
                .position(|(a, b)| a != b)
                .unwrap_or(now.len().min(was.len()));
            let near =
                |lines: &[&str]| lines[at.saturating_sub(3)..(at + 4).min(lines.len())].join("\n");
            panic!(
                "code is coloured differently from line {}; if that was meant, run with \
                 UPDATE_GOLDEN=1\n--- was\n{}\n--- now\n{}",
                at + 1,
                near(&was),
                near(&now)
            );
        }
    }
}
