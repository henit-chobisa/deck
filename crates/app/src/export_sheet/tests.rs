//! The sheet's rules, without a window: what it hands the exporter, what it
//! previews, what it says before an export, what stops one, and where a file
//! can be written.

// Spelled out rather than `#[test]`: this crate glob-imports GPUI, which
// exports a `test` attribute of its own.
use core::prelude::v1::test;
use std::path::{Path, PathBuf};

use deck_render::export::Kind;

use super::*;

/// A `.deck` file of `says`, a group each, written by hand into `dir`, with a
/// review that says something about the first group.
fn hand_built(dir: &Path, says: &[&str]) -> PathBuf {
    let mut files = std::collections::BTreeMap::new();
    files.insert(
        "deck/deck.json".to_string(),
        br#"{"v":1,"id":"d-1","title":"Why the batch counter stalls at 63"}"#.to_vec(),
    );
    for (ix, say) in says.iter().enumerate() {
        let group = serde_json::json!({
            "id": format!("g{}", ix + 1),
            "ord": ix + 1,
            "title": format!("Group {}", ix + 1),
            "say": say,
        });
        files.insert(
            format!("deck/g{}.json", ix + 1),
            group.to_string().into_bytes(),
        );
    }
    files.insert(
        "review.json".to_string(),
        br#"{"v":1,"deck":"d-1","comments":[],"transcript":[
            {"at_ms":1000,"what":"wrote","group":"g1","text":"Why twice?"},
            {"at_ms":2000,"what":"said","group":"g1","text":"The catch and the tail."}]}"#
            .to_vec(),
    );
    let bundle = deck_core::bundle::Bundle {
        manifest: deck_core::bundle::Manifest {
            container: deck_core::bundle::CONTAINER,
            protocol: 1,
            created: "2026-10-21T09:30:00Z".to_string(),
            generator: "deck test".to_string(),
            repo: None,
            entries: std::collections::BTreeMap::new(),
            allowed: Vec::new(),
        },
        files,
    };
    let path = dir.join("hand.deck");
    std::fs::write(&path, bundle.pack().expect("packs")).expect("writes");
    path
}

/// Deck's own dark, which needs no window to work out.
fn painted() -> Painted {
    Painted {
        palette: crate::palette::current_on(true, deck_core::theme::Paper::default()),
        dark: true,
        syntax: deck_core::theme::Syntax::default(),
    }
}

fn titles(n: usize) -> Vec<String> {
    (1..=n).map(|n| format!("Group {n}")).collect()
}

/// A group whose narration runs past a landscape band, and one with words
/// the embedded faces cannot set: the two kinds of notice.
fn noticed(dir: &Path) -> PathBuf {
    let long = "A retry lands here as well, and takes the counter down again. ".repeat(40);
    hand_built(
        dir,
        &[
            &long,
            "The log line reads 日本語のログ when it fails.",
            "Count once.",
        ],
    )
}

/// The sheet's notices are `deck export`'s, word for word, from the same
/// pages, for every shape and for a subset of groups.
#[test]
fn the_notices_are_the_commands_notices() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let deck = noticed(dir.path());
    let packing = crate::cli::export_packing(&deck, &[], false, None).expect("opens");
    for (layout, groups) in [
        (Layout::Portrait, vec![true, true, true]),
        (Layout::Landscape, vec![true, true, true]),
        (Layout::Landscape, vec![false, true, true]),
    ] {
        let choice = Choice {
            layout,
            groups,
            ..Choice::new(3)
        };
        let (_, _, sheet) = lay(&packing.bundle, painted(), &choice, &titles(3)).expect("lays out");
        let (_, command) = crate::cli::written(
            &packing.bundle,
            &packing.name,
            &painted().palette,
            painted().syntax,
            &choice.options(true),
            choice.shape(),
        )
        .expect("writes");
        assert_eq!(sheet, command, "{choice:?}");
        // And there is something to say: a borrowed face for group two.
        assert!(
            sheet.iter().any(|said| said.starts_with("group 2:")),
            "{sheet:?}"
        );
    }
    // The landscape band is the one that runs over, and says so.
    let landscape = Choice {
        layout: Layout::Landscape,
        ..Choice::new(3)
    };
    let (_, _, said) = lay(&packing.bundle, painted(), &landscape, &titles(3)).expect("lays out");
    assert!(
        said.iter().any(|said| said.contains("narration is longer")),
        "{said:?}"
    );
    // The HTML has nothing to say, as the command prints nothing for it.
    let html = Choice {
        format: Format::Html,
        ..Choice::new(3)
    };
    let (pages, _, said) = lay(&packing.bundle, painted(), &html, &titles(3)).expect("lays out");
    assert!(said.is_empty());
    assert!(
        pages
            .iter()
            .all(|page| matches!(page.kind, Kind::Group { .. })),
        "a group to a screen"
    );
}

/// A group unticked leaves the pages, the contents, the counts and the
/// button's words, and the rest keep their numbers.
#[test]
fn a_group_left_out_leaves_the_preview_and_the_counts() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let deck = hand_built(dir.path(), &["One.", "Two.", "Three."]);
    let packing = crate::cli::export_packing(&deck, &[], false, None).expect("opens");
    let all = Choice::new(3);
    let (every, labels, _) = lay(&packing.bundle, painted(), &all, &titles(3)).expect("lays out");
    let some = Choice {
        groups: vec![true, false, true],
        ..Choice::new(3)
    };
    let (fewer, _, _) = lay(&packing.bundle, painted(), &some, &titles(3)).expect("lays out");

    let groups = |pages: &[deck_render::export::Page]| -> Vec<u32> {
        let mut seen: Vec<u32> = pages
            .iter()
            .filter_map(|page| match page.kind {
                Kind::Group { n, .. } => Some(n),
                _ => None,
            })
            .collect();
        seen.dedup();
        seen
    };
    assert_eq!(groups(&every), [1, 2, 3]);
    assert_eq!(
        groups(&fewer),
        [1, 3],
        "group 2 is on no page, and 3 is still 3"
    );
    let two = every
        .iter()
        .filter(|page| {
            matches!(
                page.kind,
                Kind::Group { n: 2, .. } | Kind::Argument { n: 2, .. }
            )
        })
        .count();
    assert_eq!(fewer.len(), every.len() - two);
    assert_eq!(labels[0], "cover");
    assert!(labels.contains(&"2 · Group 2".to_string()), "{labels:?}");

    assert_eq!(some.numbers(), [1, 3]);
    assert!(
        all.numbers().is_empty(),
        "every group is no --groups at all"
    );
    let (said, on) = model::button(&some, Some(fewer.len()), false);
    assert_eq!(
        said,
        format!("Export PDF · {} pages · 2 of 3 groups…", fewer.len())
    );
    assert!(on);
    let (said, on) = model::button(&all, Some(every.len()), false);
    assert_eq!(said, format!("Export PDF · {} pages…", every.len()));
    assert!(on);
    let html = Choice {
        format: Format::Html,
        ..some.clone()
    };
    assert_eq!(
        model::button(&html, Some(2), false).0,
        "Export HTML · 1 file · 2 of 3 groups…"
    );
    let none = Choice {
        groups: vec![false; 3],
        ..Choice::new(3)
    };
    assert_eq!(
        model::button(&none, Some(0), false),
        ("Choose a group to export".to_string(), false)
    );
}

/// A secret stops the export in the command's words, a subset that does
/// not carry it does not, and letting it through as `--allow` would starts
/// it again and puts it in the manifest.
#[test]
fn a_secret_stops_the_export_until_it_is_let_through() {
    let dir = tempfile::tempdir().expect("a temp dir");
    // Put together here, so no whole token is ever in the source.
    let token = format!("ghp_{}", "a1B2".repeat(9));
    let deck = hand_built(
        dir.path(),
        &[
            "Nothing here.",
            &format!("The header held {token}."),
            "Nor here.",
        ],
    );
    let read = || crate::cli::export_packing(&deck, &[], false, None).expect("opens");

    let scan = scan_of(crate::cli::carrying(read(), &[], true));
    let Scan::Found(hits) = &scan else {
        panic!("a hit: {scan:?}");
    };
    assert!(blocks(&scan));
    let choice = Choice::new(3);
    assert!(
        !model::button(&choice, Some(4), blocks(&scan)).1,
        "Export is off"
    );
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].entry, "deck/g2.json");
    assert!(hits[0].to_string().contains("ghp_****"), "redacted");
    assert_eq!(
        deck_cli::pack::looks_like(hits.len()),
        "one thing looks like a secret"
    );
    let refusal = read().refusal().expect("the command refuses it");
    assert!(
        refusal.contains(&deck_cli::pack::looks_like(1)),
        "{refusal}"
    );
    assert!(refusal.contains(&hits[0].to_string()), "{refusal}");
    assert!(refusal.contains(deck_cli::pack::LET_THROUGH), "{refusal}");

    // Groups one and three do not carry it.
    let without = scan_of(crate::cli::carrying(read(), &[1, 3], true));
    assert!(!blocks(&without), "{without:?}");

    // Let through, as `--allow` would: clear, and recorded.
    let fingerprint = hits[0].fingerprint.clone();
    let allowed =
        crate::cli::export_packing(&deck, std::slice::from_ref(&fingerprint), false, None)
            .expect("opens");
    assert!(allowed.bundle.manifest.allowed.contains(&fingerprint));
    let scan = scan_of(crate::cli::carrying(allowed, &[], true));
    assert!(!blocks(&scan), "{scan:?}");
    assert!(
        model::button(&choice, Some(4), blocks(&scan)).1,
        "Export is on again"
    );
}

/// Comments the reader wrote in the window and has not sent go with the
/// sheet's packing: they are counted where the sheet says how much was said,
/// scanned, and carried in place of the review the file holds.
#[test]
fn unsent_comments_are_counted_scanned_and_carried() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let token = format!("ghp_{}", "a1B2".repeat(9));
    let deck = hand_built(dir.path(), &["Nothing here.", "Nor here."]);
    let unsent: deck_core::Review = serde_json::from_value(serde_json::json!({
        "v": 1, "deck": "d-1",
        "comments": [
            { "group": "g2", "quote": "", "text": "Why here?" },
            { "group": "g2", "quote": "", "text": format!("It needs {token}.") },
        ],
    }))
    .expect("a review");

    let packing = crate::cli::export_packing(&deck, &[], false, Some(unsent)).expect("opens");
    let document = export::paged(
        &packing.bundle,
        &painted().palette,
        painted().syntax,
        &export::Options::default(),
    )
    .expect("pages");
    let remarks: Vec<(u32, usize)> = document
        .contents
        .iter()
        .map(|entry| (entry.n, entry.remarks))
        .collect();
    assert_eq!(remarks, [(1, 0), (2, 2)], "the unsent two, on group two");

    let scan = scan_of(crate::cli::carrying(packing.clone(), &[], true));
    let Scan::Found(hits) = &scan else {
        panic!("a hit: {scan:?}");
    };
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].entry, "review.json");
    // Left out with the comments, it stops nothing.
    let without = scan_of(crate::cli::carrying(packing, &[], false));
    assert!(!blocks(&without), "{without:?}");
}

/// The dialog offers the name `deck export` would give the file, and opens
/// where the last export was saved, or else the project, or else home.
#[test]
fn the_dialog_is_named_and_placed_as_the_rule_says() {
    let pdf = Choice::new(2);
    let html = Choice {
        format: Format::Html,
        ..Choice::new(2)
    };
    let packed =
        deck_cli::pack::default_name("Why the batch counter stalls at 63", "2026-10-21T09:30:00Z");
    assert_eq!(
        model::file_name(&packed, &pdf),
        "2026-10-21-why-the-batch-counter-stalls-at-63.pdf"
    );
    assert_eq!(
        model::file_name(&packed, &html),
        "2026-10-21-why-the-batch-counter-stalls-at-63.html"
    );

    let dir = tempfile::tempdir().expect("a temp dir");
    let last = dir.path().join("last");
    let project = dir.path().join("project");
    let home = dir.path().join("home");
    for at in [&last, &project, &home] {
        std::fs::create_dir(at).expect("a folder");
    }
    let start = |last: Option<&Path>, project: Option<&Path>| {
        model::start_folder(last.map(Path::to_path_buf), project, Some(home.clone()))
    };
    assert_eq!(start(Some(&last), Some(&project)), last);
    assert_eq!(start(None, Some(&project)), project);
    assert_eq!(start(None, None), home);
    // A remembered folder that has gone since is no answer.
    let gone = dir.path().join("gone");
    assert_eq!(start(Some(&gone), Some(&project)), project);
    assert_eq!(start(Some(&gone), Some(&gone)), home);
}

/// The one writer takes the path the save dialog answered, and writes there
/// and nowhere else.
#[test]
fn a_file_is_written_only_where_the_dialog_said() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let chosen = dir.path().join("somewhere").join("theirs.pdf");
    std::fs::create_dir(chosen.parent().unwrap()).expect("a folder");
    let saved = model::save(&chosen, b"%PDF-1.7 bytes", Some(3)).expect("saved");
    assert_eq!(saved.path, chosen);
    assert_eq!(std::fs::read(&chosen).unwrap(), b"%PDF-1.7 bytes");
    let everything: Vec<PathBuf> = walk(dir.path());
    assert_eq!(
        everything,
        std::slice::from_ref(&chosen),
        "and nothing else anywhere"
    );
    assert_eq!(saved.line(), "Saved · 3 pages · 14 bytes");
    // Over a file already there: the dialog asked about replacing it.
    model::save(&chosen, b"again", None).expect("replaced");
    assert_eq!(std::fs::read(&chosen).unwrap(), b"again");
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// While the sheet is up the only keys are its own, and none of them is a
/// letter or makes a file by itself: escape, the arrows, tab to move, space
/// to press what has the ring, and return held so it presses nothing.
#[test]
fn the_sheet_has_no_key_that_exports() {
    let keys: Vec<String> = bindings()
        .iter()
        .map(|binding| {
            assert_eq!(
                binding.predicate().map(|it| it.to_string()).as_deref(),
                Some("DeckExport")
            );
            binding
                .keystrokes()
                .iter()
                .map(|stroke| stroke.unparse())
                .collect()
        })
        .collect();
    assert_eq!(
        keys,
        [
            "escape",
            "left",
            "right",
            "tab",
            "shift-tab",
            "space",
            "enter"
        ]
    );
    let actions: Vec<&str> = bindings()
        .iter()
        .filter(|binding| binding.keystrokes()[0].unparse() == "enter")
        .map(|binding| binding.action().name())
        .collect();
    assert_eq!(actions, ["deck_export::Hold"], "return does nothing");
}

#[test]
fn a_size_is_said_as_the_finder_says_it() {
    assert_eq!(model::size(412_345), "412 KB");
    assert_eq!(model::size(1_234_567), "1.2 MB");
    assert_eq!(model::size(900), "900 bytes");
}

#[test]
fn a_notice_reads_as_a_sentence() {
    assert_eq!(sentence("group 2: it runs over"), "Group 2: it runs over.");
    assert_eq!(sentence("done."), "Done.");
}

/// Space, then space again, on a secret's *let through* lands on *cancel*:
/// the first press hands the ring to changing your mind, never to the second
/// step that lets the secret through, and *cancel* hands it back.
#[test]
fn space_twice_lets_no_secret_through() {
    assert_eq!(ring_after(Control::Allow(2)), Some(Control::AllowCancel(2)));
    assert_eq!(ring_after(Control::AllowCancel(2)), Some(Control::Allow(2)));
    for control in [Control::AllowSure(2), Control::Export] {
        assert_eq!(ring_after(control), None, "{control:?} keeps the ring");
    }
}

/// The default names the theme the window really borrowed, and deck's own
/// when it borrowed none.
#[test]
fn as_the_window_names_the_real_theme() {
    let borrowed = Borrowed {
        editor: deck_theme::Editor::Nvim,
        name: Some("koala-hyderabad".to_string()),
    };
    assert_eq!(Choice::new(1).theme, Theme::Window, "the default");
    assert_eq!(
        model::label(&Theme::Window, Some(&borrowed), true),
        "As the window · koala-hyderabad"
    );
    let unnamed = Borrowed {
        editor: deck_theme::Editor::Zed,
        name: None,
    };
    assert_eq!(
        model::label(&Theme::Window, Some(&unnamed), true),
        "As the window · Zed"
    );
    assert_eq!(
        model::label(&Theme::Window, None, false),
        "As the window · Deck light"
    );
    assert_eq!(model::label(&Theme::Light, None, true), "Deck light");
}

/// Only installed editors are listed, in the list's order, with only the
/// themes they have; Neovim is one row that waits to be clicked, so opening
/// the sheet never starts it.
#[test]
fn the_list_holds_what_is_installed_and_nothing_else() {
    use deck_theme::Editor;
    let installed = |editor: Editor| matches!(editor, Editor::Zed | Editor::Vscode | Editor::Nvim);
    let asked = std::cell::RefCell::new(Vec::new());
    let rows = themes::listed(installed, |editor| {
        asked.borrow_mut().push(editor);
        match editor {
            Editor::Vscode => vec!["Nord".to_string(), "One Dark Pro".to_string()],
            Editor::Zed => vec!["One Dark".to_string(), "One Light".to_string()],
            _ => Vec::new(),
        }
    });
    let seen: Vec<(Editor, Option<&str>)> = rows
        .iter()
        .map(|row| (row.editor, row.name.as_deref()))
        .collect();
    assert_eq!(
        seen,
        [
            (Editor::Vscode, Some("Nord")),
            (Editor::Vscode, Some("One Dark Pro")),
            (Editor::Zed, Some("One Dark")),
            (Editor::Zed, Some("One Light")),
            (Editor::Nvim, None),
        ]
    );
    assert_eq!(
        *asked.borrow(),
        [Editor::Vscode, Editor::Zed],
        "Neovim is never asked for a list"
    );
    assert!(matches!(rows[4].swatch, themes::Swatch::OnClick));
    assert_eq!(rows[4].title(), "The scheme it is in");
    assert!(
        rows[..4]
            .iter()
            .all(|row| matches!(row.swatch, themes::Swatch::Reading)),
        "and nothing can be picked before it is read"
    );
}
