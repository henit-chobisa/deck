//! The window's ink, for a document.
//!
//! [`deck_core::document`] lays a deck out without knowing how code is
//! coloured or a picture drawn, because both live here, beside the window.
//! [`Painter`] is the one place they meet: the window's own highlighter and
//! the SVG writer the window's pictures agree with, so a document's code is
//! the colour the reader saw and its diagrams are the shapes they saw.

use std::path::Path;

use deck_core::bundle::{Bundle, Unreadable};
use deck_core::diagram::Diagram;
use deck_core::document::{self, Document, Paint, Painted};
use deck_core::theme::{Palette, Syntax};

use crate::Code;
use crate::svg::{self, Fonts};

/// Code coloured and pictures drawn as the window does it, in one palette.
pub struct Painter {
    code: Code,
    palette: Palette,
}

impl Painter {
    /// A painter for `palette`, light or dark, with the syntax colours the
    /// editor theme gave.
    #[must_use]
    pub fn new(palette: &Palette, dark: bool, imported: Syntax) -> Self {
        Self {
            code: Code::new(palette, dark, imported),
            palette: *palette,
        }
    }
}

impl Paint for Painter {
    fn spans(&self, source: &str, file: &str) -> Vec<Painted> {
        self.code
            .spans(source, Path::new(file))
            .into_iter()
            .map(|span| Painted {
                range: span.range,
                colour: span.colour,
                italic: span.italic,
            })
            .collect()
    }

    fn diagram(&self, diagram: &Diagram) -> String {
        // The face is named, not embedded: a document loads it once for
        // every picture in it.
        svg::diagram(diagram, &self.palette, Fonts::Named)
    }
}

/// `bundle` as a document, painted as the window paints.
///
/// # Errors
///
/// When a part of the bundle does not parse.
pub fn build(
    bundle: &Bundle,
    palette: &Palette,
    dark: bool,
    imported: Syntax,
) -> Result<Document, Unreadable> {
    document::build(bundle, palette, &Painter::new(palette, dark, imported))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use deck_core::bundle::{CONTAINER, Manifest, sha256, snap};
    use deck_core::document::{Line, Pane};
    use deck_core::line::LineRange;
    use deck_core::theme::{Imported, LEGIBLE, Rgb, derive};

    use super::*;

    #[test]
    fn a_deck_is_painted_as_the_window_paints_it() {
        let source = "fn main() {\n    // the one that matters\n    let total = 63;\n    println!(\"{total}\");\n}\n";
        let (kept, text) = snap::keep(source, &[LineRange::new(2, 3)], snap::CONTEXT);
        let index = snap::Index {
            files: vec![snap::File {
                path: "src/main.rs".to_string(),
                entry: "snap/1.txt".to_string(),
                sha256: sha256(source.as_bytes()),
                commit: None,
                state: snap::State::Untracked,
                kept,
            }],
        };
        let group = r#"{"id":"g1","say":"The total is wrong.[point 3]","refs":[
            {"id":"g1r1","file":"src/main.rs","range":[2,3]},
            {"id":"g1d1","diagram":{"nodes":[{"id":"a","label":"start"}]}}]}"#;
        let mut files = BTreeMap::new();
        files.insert(
            "deck/deck.json".to_string(),
            br#"{"v":1,"id":"d","title":"t"}"#.to_vec(),
        );
        files.insert("deck/g1.json".to_string(), group.as_bytes().to_vec());
        files.insert(
            "snap/index.json".to_string(),
            serde_json::to_vec(&index).unwrap(),
        );
        files.insert("snap/1.txt".to_string(), text.into_bytes());
        let bundle = Bundle {
            manifest: Manifest {
                container: CONTAINER,
                protocol: 1,
                created: "2026-10-20T09:30:00Z".to_string(),
                generator: "deck test".to_string(),
                repo: None,
                entries: BTreeMap::new(),
                allowed: Vec::new(),
            },
            files,
        };
        let palette = derive(Imported::plain(
            Rgb::from_hex("#282828").unwrap(),
            Rgb::from_hex("#ebdbb2").unwrap(),
            Rgb::from_hex("#fe8019").unwrap(),
        ));
        let doc = build(&bundle, &palette, true, Syntax::default()).expect("builds");
        let Pane::Code(code) = &doc.groups[0].panes[0] else {
            panic!("a code pane first")
        };
        let rows: Vec<_> = code
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Code(row) => Some(row),
                Line::Gap { .. } => None,
            })
            .collect();
        // Real highlighting: the comment leans, the keyword is not the text's
        // own colour, and every run reads on its ground.
        let comment = rows.iter().find(|row| row.no == 2).unwrap();
        assert!(comment.spans.iter().any(|span| span.italic));
        let keyword = rows
            .iter()
            .find(|row| row.no == 3)
            .and_then(|row| row.spans.iter().find(|span| span.text == "let"))
            .expect("let is its own run");
        assert_ne!(keyword.ink, palette.fg);
        for row in &rows {
            for span in &row.spans {
                assert!(span.ink.contrast(row.ground) >= LEGIBLE);
            }
        }
        assert_eq!(
            rows.iter().find(|row| row.no == 3).unwrap().numeral,
            Some(1)
        );
        let Pane::Diagram(picture) = &doc.groups[0].panes[1] else {
            panic!("the picture second")
        };
        assert!(picture.svg.starts_with("<svg"));
    }
}
