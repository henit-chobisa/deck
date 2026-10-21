//! The paged export: its layout, and the PDF written from it.

use deck_core::bundle::Bundle;
use deck_core::document::Document;
use deck_core::theme::Syntax;

use super::*;
use crate::export::{Face, Format, Item, Kind, Page, Writing, deck_pdf, layout, shown};

/// The fixture as the document the pages are laid out from.
fn document_of(bundle: &Bundle, options: &Options) -> Document {
    let document =
        crate::document::build(bundle, &palette(), true, Syntax::default()).expect("builds");
    shown(&document, options)
}

fn portrait() -> Vec<Page> {
    layout(
        &document_of(&bundle(), &Options::default()),
        Format::Portrait,
    )
}

fn writing() -> Writing {
    Writing {
        name: "2026-10-20-why-the-batch-counter-stalls-at-63.deck".to_string(),
        grid: false,
    }
}

fn export_pdf(options: &Options, format: Format) -> (Vec<u8>, Vec<String>) {
    deck_pdf(
        &bundle(),
        &palette(),
        Syntax::default(),
        options,
        format,
        &writing(),
    )
    .expect("exports")
}

/// Every word on a page, in the order it is drawn.
fn words(page: &Page) -> String {
    page.items
        .iter()
        .filter_map(|item| match item {
            Item::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The layout of the fixture, every box and line of it.
///
/// Run with `UPDATE_GOLDEN=1` when a change is meant to move it, and read
/// the diff: it is every page the PDF draws.
#[test]
fn the_fixture_deck_is_laid_out_as_it_was() {
    let mut pages = portrait();
    pages.extend(layout(
        &document_of(&bundle(), &Options::default()),
        Format::Landscape,
    ));
    // The drawings are the SVG writer's, golden-tested there.
    for page in &mut pages {
        for item in &mut page.items {
            if let Item::Svg { svg, .. } = item {
                *svg = format!("<svg: {} bytes>", svg.len());
            }
        }
    }
    let now = serde_json::to_string_pretty(&pages).expect("serialises") + "\n";
    let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/layout.json");
    if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
        std::fs::write(&at, &now).expect("writable");
    }
    let was = std::fs::read_to_string(&at)
        .expect("the golden file exists; run with UPDATE_GOLDEN=1")
        .replace("\r\n", "\n");
    assert_eq!(
        now, was,
        "the layout moved; if that was meant, run with UPDATE_GOLDEN=1"
    );
}

/// Every box on a portrait page sits on the 4-point grid.
///
/// A box's four edges, a rule's ends and height, a diagram's room and a
/// link's area are checked. Text is not: a line's baseline is placed inside
/// its row from the face's own ascent and descent, as the window centres a
/// line in its height, and it is the row that is on the grid. A numeral's
/// disc and a remark's dot are centred on their row the same way, and a
/// chip's ground is the text's, so it goes with the text.
#[test]
fn every_portrait_box_is_on_the_four_point_grid() {
    let on = |v: f32| (v / 4. - (v / 4.).round()).abs() < 1e-3;
    let long = deck_of(&[&long_group()], None);
    let mut pages = portrait();
    pages.extend(layout(
        &document_of(&long, &Options::default()),
        Format::Portrait,
    ));
    for (ix, page) in pages.iter().enumerate() {
        for item in &page.items {
            let edges: Vec<f32> = match item {
                Item::Fill { rect, .. }
                | Item::Frame { rect, .. }
                | Item::Svg { rect, .. }
                | Item::Dots { rect, .. }
                | Item::Url { rect, .. }
                | Item::Link { rect, .. } => {
                    vec![rect.x, rect.y, rect.x + rect.w, rect.y + rect.h]
                }
                Item::Rule { x, y, w, .. } => vec![*x, *y, x + w],
                Item::Text { .. }
                | Item::Badge { .. }
                | Item::Dot { .. }
                | Item::Chip { .. }
                | Item::Ground { .. } => Vec::new(),
            };
            for edge in edges {
                assert!(on(edge), "page {ix}: {edge} is off the grid in {item:?}");
            }
        }
    }
}

/// A group long enough for three pages: many panes of plain code.
fn long_group() -> String {
    // Sixteen panes of three lines, in the two stretches the fixture's
    // snapshot keeps.
    let refs: Vec<String> = (0..16)
        .map(|ix| {
            let first = if ix < 10 { 1 + ix * 3 } else { 40 + (ix - 10) * 3 };
            format!(
                r#"{{"id": "g1r{ix}", "file": "src/batch.ts", "range": [{first}, {}], "name": "part{ix}"}}"#,
                first + 2
            )
        })
        .collect();
    format!(
        r#"{{"id": "g1", "ord": 1, "title": "A long walk through the file", "say": "{}[point 3]", "refs": [{}]}}"#,
        "Every part of the file, one pane at a time. ".repeat(12),
        refs.join(",")
    )
}

#[test]
fn no_page_holds_two_groups() {
    let pages = portrait();
    let document = document_of(&bundle(), &Options::default());
    for page in &pages {
        let text = words(page);
        let Kind::Group { n, .. } = page.kind else {
            continue;
        };
        for group in &document.groups {
            if group.n != n {
                // Another group's title appears only in the foot's map and
                // the hand-off, never as a band.
                let bands = page.items.iter().filter(|item| {
                    matches!(item, Item::Text { text, size, .. } if *size >= 18. && text == &group.title)
                });
                assert_eq!(bands.count(), 0, "group {} on a page of group {n}", group.n);
            }
        }
        assert!(text.contains("Powered by Deck"));
    }
    // Each group starts a page of its own, after the cover.
    let starts: Vec<u32> = pages
        .iter()
        .filter_map(|page| match page.kind {
            Kind::Group { n, part: 1, .. } => Some(n),
            _ => None,
        })
        .collect();
    assert_eq!(starts, [1, 2, 3]);
    assert_eq!(pages[0].kind, Kind::Cover);
}

/// A group that does not fit on two pages, even trimmed, runs on with a
/// heading on each page, the export warns, and nothing is dropped.
#[test]
fn a_group_past_two_pages_is_continued_and_said() {
    let long = deck_of(&[&long_group()], None);
    let document = document_of(&long, &Options::default());
    let pages = layout(&document, Format::Portrait);
    let parts: Vec<(u32, u32)> = pages
        .iter()
        .filter_map(|page| match page.kind {
            Kind::Group { part, of, .. } => Some((part, of)),
            _ => None,
        })
        .collect();
    let of = parts[0].1;
    assert!(of > 2, "{parts:?}");
    for page in pages
        .iter()
        .filter(|p| matches!(p.kind, Kind::Group { part, .. } if part > 1))
    {
        let Kind::Group { part, .. } = page.kind else {
            unreachable!()
        };
        assert!(words(page).contains(&format!("Group 1, continued ({part} of {of})")));
    }
    assert_eq!(
        pages.iter().map(|page| page.warnings.len()).sum::<usize>(),
        1
    );
    // Every pane is there, and every line it lights.
    let all: String = pages.iter().map(words).collect::<Vec<_>>().join(" ");
    for ix in 0..16 {
        assert!(all.contains(&format!("part{ix}")), "part{ix}");
    }
    let (_, warnings) = deck_pdf(
        &long,
        &palette(),
        Syntax::default(),
        &Options::default(),
        Format::Portrait,
        &writing(),
    )
    .expect("exports");
    assert_eq!(warnings.len(), 1);
}

/// The footer is one line however long the repo's name: the repo is cut in
/// the middle and the title shortened, and nothing runs past the column or
/// into its neighbour.
#[test]
fn the_footer_never_wraps() {
    let mut document = document_of(&bundle(), &Options::default());
    document.repo = Some(deck_core::bundle::Repo {
        remote: Some(format!(
            "https://github.com/a-very-long-organisation-name/{}",
            "and-an-even-longer-repository-name-".repeat(4)
        )),
        commit: Some("a1b2c3d4e5".to_string()),
    });
    document.groups[0].title =
        "A title long enough that it could never fit beside the rest of the footer".to_string();
    for page in layout(&document, Format::Portrait) {
        let footer: Vec<(f32, f32, f32)> = page
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Text {
                    x,
                    baseline,
                    text,
                    face,
                    size,
                    ..
                } if *baseline > 800. => Some((
                    *x,
                    *x + crate::export::layout::measure(text, *face, *size),
                    *baseline,
                )),
                _ => None,
            })
            .collect();
        assert_eq!(footer.len(), 3);
        assert!(
            footer
                .iter()
                .all(|(_, _, b)| (b - footer[0].2).abs() < 0.01)
        );
        assert!(footer.iter().all(|(l, r, _)| *l >= 48. && *r <= 548.01));
        let mut spans: Vec<(f32, f32)> = footer.iter().map(|(l, r, _)| (*l, *r)).collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in spans.windows(2) {
            assert!(pair[0].1 <= pair[1].0, "{spans:?}");
        }
    }
}

/// The embedded `.deck`, found in the PDF: stored uncompressed, it is the
/// zip from its first local header to the end of its stream.
fn attached(pdf: &[u8]) -> Bundle {
    let start = pdf
        .windows(4)
        .position(|w| w == b"PK\x03\x04")
        .expect("a zip in the PDF");
    let end = start
        + pdf[start..]
            .windows(9)
            .position(|w| w == b"endstream")
            .expect("the stream ends");
    let mut zip = &pdf[start..end];
    while zip.last().is_some_and(|b| b.is_ascii_whitespace()) {
        zip = &zip[..zip.len() - 1];
    }
    Bundle::open(zip).expect("the attached deck opens").0
}

/// The same deck makes the same bytes, and they are a PDF a reader can
/// find its way through: a header, a cross-reference table every object's
/// offset in which lands on that object, and a trailer.
#[test]
fn the_pdf_is_the_same_every_time_and_well_formed() {
    let (one, _) = export_pdf(&Options::default(), Format::Portrait);
    let (two, _) = export_pdf(&Options::default(), Format::Portrait);
    assert_eq!(one, two);
    assert!(one.starts_with(b"%PDF-"));
    let at = one
        .windows(9)
        .rposition(|w| w == b"startxref")
        .expect("startxref");
    let offset: usize = String::from_utf8_lossy(&one[at + 9..])
        .split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .expect("an offset");
    assert!(one[offset..].starts_with(b"xref"), "a classic xref table");
    let table = String::from_utf8_lossy(&one[offset..at]).into_owned();
    let mut lines = table.lines().skip(1);
    let header: Vec<usize> = lines
        .next()
        .unwrap()
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    let (first, count) = (header[0], header[1]);
    for n in first..first + count {
        let entry = lines.next().unwrap();
        if entry.trim_end().ends_with('f') {
            continue;
        }
        let at: usize = entry[..10].parse().unwrap();
        assert!(
            one[at..].starts_with(format!("{n} 0 obj").as_bytes()),
            "object {n} at {at}"
        );
    }
    let text = String::from_utf8_lossy(&one);
    assert!(text.trim_end().ends_with("%%EOF"));
    // And the date is the deck's, not today's.
    assert!(text.contains("D:20261020093000"));
}

#[test]
fn the_attached_deck_round_trips() {
    for format in [Format::Portrait, Format::Landscape] {
        let (pdf, _) = export_pdf(&Options::default(), format);
        assert_eq!(attached(&pdf), bundle());
    }
}

#[test]
fn the_attached_deck_keeps_to_what_was_asked() {
    let (pdf, _) = export_pdf(
        &Options {
            comments: false,
            ..Options::default()
        },
        Format::Portrait,
    );
    let carried = attached(&pdf);
    assert!(!carried.files.contains_key("review.json"));
    assert!(!String::from_utf8_lossy(&pdf).contains("finally"));

    let (pdf, _) = export_pdf(
        &Options {
            groups: vec![2],
            ..Options::default()
        },
        Format::Portrait,
    );
    let carried = attached(&pdf);
    let groups = carried.groups().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, "g2");
    let pages = layout(
        &document_of(
            &bundle(),
            &Options {
                groups: vec![2],
                ..Options::default()
            },
        ),
        Format::Portrait,
    );
    let all: String = pages.iter().map(words).collect::<Vec<_>>().join(" ");
    assert!(!all.contains("Nothing else moves") && !all.contains("The error path"));
}

/// A page pane has no PDF form: it is a card saying where it can be used.
#[test]
fn a_page_pane_is_a_card() {
    for format in [Format::Portrait, Format::Landscape] {
        let pages = layout(&document_of(&bundle(), &Options::default()), format);
        let all: String = pages.iter().map(words).collect::<Vec<_>>().join(" ");
        assert!(all.contains("Interactive page: open the HTML export to use it"));
        assert!(all.contains("shown"));
        assert!(!all.contains("Hello"));
        assert!(
            pages
                .iter()
                .flat_map(|p| &p.items)
                .any(|item| matches!(item, Item::Frame { .. }))
        );
    }
}

/// Landscape: a group to a page, then its argument on a page of its own,
/// and the strip on every one.
#[test]
fn landscape_gives_an_argument_its_own_page() {
    let pages = layout(
        &document_of(&bundle(), &Options::default()),
        Format::Landscape,
    );
    let kinds: Vec<Kind> = pages.iter().map(|page| page.kind).collect();
    assert_eq!(
        kinds,
        [
            Kind::Group {
                n: 1,
                part: 1,
                of: 1
            },
            Kind::Argument {
                n: 1,
                part: 1,
                of: 1
            },
            Kind::Group {
                n: 2,
                part: 1,
                of: 1
            },
            Kind::Group {
                n: 3,
                part: 1,
                of: 1
            },
            Kind::Argument {
                n: 3,
                part: 1,
                of: 1
            },
            Kind::Said,
        ]
    );
    for (ix, page) in pages.iter().enumerate() {
        assert!((page.scale - 0.75).abs() < f32::EPSILON);
        let text = words(page);
        assert!(text.contains("Powered by Deck"));
        assert!(text.contains(&format!("page {} of {}", ix + 1, pages.len())));
    }
    // The argument is not in the side column of the group's page.
    assert!(!words(&pages[0]).contains("retry branch"));
    assert!(words(&pages[1]).contains("retry branch"));
}

/// A code line too long for its pane wraps, hung under itself with a mark,
/// and none of it is cut.
#[test]
fn a_long_line_wraps_and_is_never_clipped() {
    let long = "x".repeat(150);
    let group = format!(
        r#"{{"id": "g1", "ord": 1, "say": "Long.[point 2]", "refs": [{{"id": "g1r1", "file": "src/batch.ts", "range": [2, 2],
            "values": [{{"line": 2, "at": "nowhere", "is": "{long}"}}]}}]}}"#
    );
    let pages = layout(
        &document_of(&deck_of(&[&group], None), &Options::default()),
        Format::Portrait,
    );
    let mono: Vec<&str> = pages
        .iter()
        .flat_map(|p| &p.items)
        .filter_map(|item| match item {
            Item::Text {
                text,
                face: Face::Mono,
                ..
            } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    // The value's rows, which are all `x`; the ledger in the foot shows it
    // again, cut short with an ellipsis, and is not these.
    let rows: usize = mono
        .iter()
        .filter(|text| text.chars().all(|ch| ch == 'x'))
        .map(|text| text.len())
        .sum();
    assert_eq!(rows, 150, "{mono:?}");
    let marks = mono.iter().filter(|text| **text == "↪" || **text == ">");
    assert!(marks.count() >= 1);
}

/// The PDF's links are real annotations: deck's site and the code's lines
/// as URIs, the contents and the hand-offs as jumps to a page.
#[test]
fn the_pdf_carries_its_links() {
    for format in [Format::Portrait, Format::Landscape] {
        let (pdf, _) = export_pdf(&Options::default(), format);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Subtype/Link"));
        assert!(text.contains(&format!("/URI({})", crate::export::links::DECK)));
        assert!(
            text.contains(
                "/URI(https://github.com/acme/ingest/blob/a1b2c3d4e5/src/batch.ts#L6-L13)"
            )
        );
    }
    let pages = portrait();
    // The cover's contents and every hand-off jump to the group's page.
    let jumps = pages
        .iter()
        .flat_map(|page| &page.items)
        .filter(|item| matches!(item, Item::Link { .. }))
        .count();
    assert!(jumps >= 3 + 2, "{jumps}");
    let urls = pages
        .iter()
        .flat_map(|page| &page.items)
        .filter(|item| matches!(item, Item::Url { url, .. } if url == crate::export::links::DECK))
        .count();
    assert_eq!(urls, pages.len());
}
