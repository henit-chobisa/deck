//! The HTML export, against a small deck written the way a real one is.

use std::collections::BTreeMap;

use deck_core::bundle::{Bundle, CONTAINER, Manifest, Repo, sha256, snap};
use deck_core::line::LineRange;
use deck_core::theme::{Imported, Palette, Rgb, Syntax, derive};

use super::*;

/// The file the deck points into: the loop at the top, and forty lines
/// further down the close only the third group points at, far enough that
/// no span kept around the loop reaches it.
fn batch_ts() -> String {
    let mut out = String::from(
        "\
import { write } from './store'

export async function flush(rows: Row[]) {
  let pending = rows.length
  for (const row of rows) {
    try {
      await write(row)
    } catch (err) {
      log.error(err)
      pending -= 1
    }
    // every row, failed or not, reaches this
    pending -= 1
  }
  if (pending === 0) {
    settle(rows)
  }
}
",
    );
    for n in 19..=58 {
        out.push_str(&format!("// spacer {n}\n"));
    }
    out.push_str("export function close(batch: Batch) {\n  finish(batch)\n}\n");
    out
}

/// A second file, which only the third group points into.
const STORE: &str =
    "export async function write(row: Row) {\n  await db.put(row.id, row.payload)\n}\n";

fn palette() -> Palette {
    derive(Imported::plain(
        Rgb::from_hex("#282828").unwrap(),
        Rgb::from_hex("#ebdbb2").unwrap(),
        Rgb::from_hex("#fe8019").unwrap(),
    ))
}

/// A deck of `groups`, with `review` if there is one, pointing into
/// `src/batch.ts` and `src/store.ts`, packed and opened again so it is what
/// a reader would open.
fn deck_of(groups: &[&str], review: Option<&str>) -> Bundle {
    let batch = batch_ts();
    let (kept, text) = snap::keep(
        &batch,
        &[LineRange::new(5, 13), LineRange::new(59, 61)],
        snap::CONTEXT,
    );
    let (store_kept, store_text) = snap::keep(STORE, &[LineRange::new(2, 2)], snap::CONTEXT);
    let index = snap::Index {
        files: vec![
            snap::File {
                path: "src/batch.ts".to_string(),
                entry: "snap/1.txt".to_string(),
                sha256: sha256(batch.as_bytes()),
                commit: Some("a1b2c3d4e5".to_string()),
                state: snap::State::Clean,
                kept,
                lines: None,
            },
            snap::File {
                path: "src/store.ts".to_string(),
                entry: "snap/2.txt".to_string(),
                sha256: sha256(STORE.as_bytes()),
                commit: Some("a1b2c3d4e5".to_string()),
                state: snap::State::Clean,
                kept: store_kept,
                lines: None,
            },
        ],
    };
    let mut files = BTreeMap::new();
    files.insert(
        "deck/deck.json".to_string(),
        br#"{"v":1,"id":"d-1","title":"Why the batch counter stalls at 63"}"#.to_vec(),
    );
    for (ix, group) in groups.iter().enumerate() {
        files.insert(format!("deck/g{}.json", ix + 1), group.as_bytes().to_vec());
    }
    files.insert(
        "snap/index.json".to_string(),
        serde_json::to_vec(&index).unwrap(),
    );
    files.insert("snap/1.txt".to_string(), text.into_bytes());
    files.insert("snap/2.txt".to_string(), store_text.into_bytes());
    if let Some(review) = review {
        files.insert("review.json".to_string(), review.as_bytes().to_vec());
    }
    let bundle = Bundle {
        manifest: Manifest {
            container: CONTAINER,
            protocol: 1,
            created: "2026-10-20T09:30:00Z".to_string(),
            generator: "deck test".to_string(),
            repo: Some(Repo {
                remote: Some("https://github.com/acme/ingest.git".to_string()),
                commit: Some("a1b2c3d4e5".to_string()),
            }),
            entries: BTreeMap::new(),
            allowed: Vec::new(),
        },
        files,
    };
    Bundle::open(&bundle.pack().expect("packs"))
        .expect("opens")
        .0
}

/// Three groups: one with code, a picture and a page, and a title that
/// tries to run a script; one with a proposed change; one quiet. A review
/// with a remark on a line, an answer, and a word for the whole deck.
fn bundle() -> Bundle {
    let g1 = r#"{
        "id": "g1", "ord": 1,
        "title": "The error path <script>alert(1)</script> counts twice",
        "confidence": {"level": "medium", "why": "read both paths & did not run it"},
        "say": "A write that fails lands in the catch in [batch] and takes the counter down.[point 10] It goes on to the tail and takes it down again.[point 13]\n\nSo every failed write costs **two**. [path] follows one row, and `pending` is the count. The [shown] page is the picture.",
        "refs": [
            {"id": "g1r1", "file": "src/batch.ts", "range": [6, 13], "name": "batch", "note": "the loop <b>body</b>", "values": [
                {"line": 7, "at": "row", "is": "Row { id: \"r63\", … }", "keys": [{"key": "id", "is": "\"r63\"", "lit": true}, {"more": 1}]},
                {"line": 10, "at": "pending", "is": "63", "lit": true},
                {"line": 13, "at": "missing", "is": "62"}
            ]},
            {"id": "g1d1", "name": "path", "diagram": {"nodes": [{"id": "check", "label": "write ok?", "role": "decision"}, {"id": "tail", "label": "tail"}], "edges": [{"from": "check", "to": "tail"}]}},
            {"id": "g1p1", "name": "shown", "page": "<h1 id=\"hi\">Hello</h1><script>parent.document.body.remove()</script><img src=\"https://example.com/x.png\" srcset=\"//cdn.example.com/y.png 2x\">"}
        ]
    }"#;
    let g2 = r#"{
        "id": "g2", "ord": 2,
        "title": "Count once",
        "say": "Count once, where the write settles.[point +10]",
        "refs": [{"id": "g2r1", "file": "src/batch.ts", "range": [10, 10], "name": "fix",
                  "after": "      // counted at the tail\n"}]
    }"#;
    let g3 = r#"{"id": "g3", "ord": 3, "title": "Nothing else moves", "say": "The close stays as it is.[point 60]",
        "refs": [{"id": "g3r1", "file": "src/batch.ts", "range": [59, 61]},
                 {"id": "g3r2", "file": "src/store.ts", "range": [2, 2], "name": "store"}]}"#;
    let review = r#"{
        "v": 1, "deck": "d-1",
        "comments": [
            {"group": "g1", "ref": "g1r1", "range": [10, 10], "source": "extmark", "kind": "question",
             "quote": "pending -= 1", "text": "Why not decrement once, in a <finally>?"},
            {"group": "g3", "ref": "g3r1", "file": "src/batch.ts", "range": [60, 60], "source": "extmark", "kind": "nit",
             "quote": "finish(batch)", "text": "Close it where it opens."}
        ],
        "transcript": [
            {"at_ms": 5000, "what": "wrote", "group": "g1", "ref": "g1r1", "text": "Why not decrement once, in a <finally>?"},
            {"at_ms": 9000, "what": "said", "group": "g1", "text": "A finally also runs on the retry branch."},
            {"at_ms": 12000, "what": "said", "text": "That is everything I had."}
        ]
    }"#;
    deck_of(&[g1, g2, g3], Some(review))
}

fn export(options: &Options) -> String {
    deck(&bundle(), &palette(), Syntax::default(), options).expect("exports")
}

/// The `.deck` a page carries, taken back out of it.
fn carried(html: &str) -> Bundle {
    let start = html
        .find("<script type=\"application/x-deck\" id=\"deck\">")
        .expect("the deck is embedded")
        + "<script type=\"application/x-deck\" id=\"deck\">".len();
    let end = start + html[start..].find("</script>").expect("closed");
    Bundle::open(&unbase64(&html[start..end]))
        .expect("the embedded deck opens")
        .0
}

fn unbase64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("not base64: {c}"),
    };
    let clean: Vec<u8> = text.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::new();
    for chunk in clean.chunks(4) {
        let mut word = 0u32;
        for (ix, &c) in chunk.iter().enumerate() {
            word |= u32::from(value(c)) << (18 - 6 * ix);
        }
        for ix in 0..chunk.len() - 1 {
            out.push(word.to_be_bytes()[1 + ix]);
        }
    }
    out
}

/// The page without what it carries: the faces and the deck as `…`, so
/// what is left is the markup a person can read in a diff.
fn readable(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(at) = rest.find("base64,") {
        out.push_str(&rest[..at + "base64,".len()]);
        out.push('…');
        rest = &rest[at..];
        rest = &rest[rest.find(')').expect("a face ends")..];
    }
    out.push_str(rest);
    let open = "<script type=\"application/x-deck\" id=\"deck\">";
    let start = out.find(open).expect("embedded") + open.len();
    let end = start + out[start..].find("</script>").expect("closed");
    out.replace_range(start..end, "…");
    out
}

#[test]
fn every_word_from_the_deck_is_escaped() {
    let html = export(&Options::default());
    assert!(!html.contains("<script>alert(1)"));
    assert!(html.contains("The error path &lt;script&gt;alert(1)&lt;/script&gt; counts twice"));
    assert!(html.contains("the loop &lt;b&gt;body&lt;/b&gt;"));
    assert!(html.contains("read both paths &amp; did not run it"));
    assert!(html.contains("in a &lt;finally&gt;?"));
    // The only scripts in the file are the deck it carries; the page's own
    // script is inside an attribute, escaped, in a frame that runs nothing.
    assert_eq!(html.matches("<script").count(), 1);
    assert!(html.contains("&lt;script&gt;parent.document.body.remove()"));
}

#[test]
fn a_page_pane_is_sandboxed_with_no_permissions() {
    let html = export(&Options::default());
    let frame = &html[html.find("<iframe").expect("a frame")..];
    let frame = &frame[..frame.find('>').unwrap()];
    assert!(frame.contains("sandbox=\"\""), "{frame}");
    assert!(!frame.contains("allow-"));
    assert!(html.contains("Content-Security-Policy\" content=\"default-src 'none'"));
}

/// Entities in a `srcdoc` attribute, decoded back to the markup the frame
/// is given.
fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// The page with every frame's `srcdoc` decoded in place, so a URL in a page
/// pane is looked at as the markup it is rather than hidden in entities.
fn decoded(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(at) = rest.find("srcdoc=\"") {
        let start = at + "srcdoc=\"".len();
        let end = start + rest[start..].find('"').expect("closed");
        out.push_str(&rest[..start]);
        out.push_str(&unescape(&rest[start..end]));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The policy refuses every load; this is the second line, and looks for
/// anything a browser could be asked to fetch, frames included.
#[test]
fn nothing_is_asked_for_from_the_network() {
    let html = decoded(&readable(&export(&Options::default())));
    // The fixture's page pane asks for two images on purpose: they are the
    // only URLs in the deck, inside a frame that is sandboxed and under the
    // policy, and finding them proves the test would.
    let planted = "<img src=\"https://example.com/x.png\" srcset=\"//cdn.example.com/y.png 2x\">";
    assert!(html.contains(planted));
    let html = html.replace(planted, "");
    // Links the reader can press are navigations, not loads: the policy
    // does not stop them and nothing is fetched until somebody clicks. Each
    // is an anchor opening a new tab, and they go only to deck's site and to
    // the fixture's lines on GitHub; with them taken out, nothing else may
    // name the network.
    let mut html = html;
    let anchor = " href=\"https://";
    while let Some(at) = html.find(anchor) {
        let tag_start = html[..at].rfind('<').expect("in a tag");
        let tag_end = at + html[at..].find('>').expect("closed");
        let tag = &html[tag_start..=tag_end];
        assert!(tag.starts_with("<a "), "{tag}");
        assert!(
            tag.contains("rel=\"noopener") && tag.contains("target=\"_blank\""),
            "{tag}"
        );
        let url_start = at + " href=\"".len();
        let url_end = url_start + html[url_start..].find('"').expect("quoted");
        let url = &html[url_start..url_end];
        assert!(
            url == crate::export::links::DECK
                || url.starts_with("https://github.com/acme/ingest/blob/a1b2c3d4e5/src/"),
            "{url}"
        );
        html.replace_range(at..url_end + 1, "");
    }
    let svg = "xmlns=\"http://www.w3.org/2000/svg\"";
    let lower = html.to_ascii_lowercase();
    for scheme in ["http:", "https:"] {
        for (at, _) in lower.match_indices(scheme) {
            let before = &lower[at.saturating_sub("xmlns=\"".len())..];
            assert!(
                before.starts_with(svg),
                "a URL at {at}: {}",
                &html[at..(at + 60).min(html.len())]
            );
        }
    }
    for asked in [
        "=\"//",
        "='//",
        "url(//",
        "srcset",
        "xlink:href",
        "url(",
        "@import",
        "<link",
    ] {
        // `url(` is in the faces, as data, and only there.
        let found = lower
            .match_indices(asked)
            .filter(|(at, _)| !lower[*at..].starts_with("url(data:"))
            .count();
        assert_eq!(found, 0, "{asked}");
    }
}

/// "Powered by Deck" goes to deck's site, and a clean file's header to its
/// lines at the packed commit; a file the deck does not hold goes nowhere.
#[test]
fn the_brand_and_the_code_are_links() {
    let html = export(&Options::default());
    let brand = format!("<a class=\"brand\" href=\"{}\"", crate::export::links::DECK);
    assert_eq!(
        html.matches(&brand).count(),
        html.matches("<footer").count()
    );
    assert!(
        html.contains(
            "href=\"https://github.com/acme/ingest/blob/a1b2c3d4e5/src/batch.ts#L6-L13\""
        )
    );
}

#[test]
fn the_deck_it_carries_opens_and_is_the_deck() {
    let html = export(&Options::default());
    let carried = carried(&html);
    assert_eq!(carried, bundle());
    assert!(carried.review().unwrap().is_some());
}

#[test]
fn without_comments_nothing_said_is_on_the_page_or_in_the_deck() {
    let with = export(&Options::default());
    assert!(with.contains("id=\"g1-argument\""));
    assert!(with.contains("id=\"said\""));
    let html = export(&Options {
        comments: false,
        ..Options::default()
    });
    assert!(carried(&html).review().unwrap().is_none());
    assert!(!carried(&html).files.contains_key("review.json"));
    assert!(!html.contains("-argument"));
    assert!(!html.contains("id=\"said\""));
    assert!(!html.contains("in a &lt;finally&gt;?"));
    assert!(!html.contains("retry branch"));
}

#[test]
fn an_argument_follows_its_group_and_carries_its_line() {
    let html = export(&Options::default());
    let group = html.find("id=\"g1\"").unwrap();
    let argument = html.find("id=\"g1-argument\"").unwrap();
    let next = html.find("id=\"g2\"").unwrap();
    assert!(group < argument && argument < next);
    let stop = &html[argument..next];
    // The remark's line, with its number, quoted under it.
    let quoted = &stop[stop.find("<div class=\"q\">").expect("a quote")..];
    assert!(quoted.contains(">10</span><span class=\"t\">"));
    assert!(stop.contains("batch, line 10"));
    // The side column is the contents, not the argument.
    let side = &html[group..argument];
    let side = &side[side.find("<nav class=\"side\"").unwrap()..];
    assert!(!side.contains("retry branch"));
    // Next from the group is its argument.
    assert!(html[group..argument].contains("href=\"#g1-argument\" aria-label=\"Next\""));
}

#[test]
fn only_the_groups_asked_for_are_shown() {
    let html = export(&Options {
        groups: vec![2, 3],
        ..Options::default()
    });
    assert!(!html.contains("id=\"g1\""));
    assert!(!html.contains("id=\"g1-argument\""));
    assert!(html.contains("id=\"g2\"") && html.contains("id=\"g3\""));
    assert!(html.contains("group 2/3"));
    assert!(html.contains("page 1 of 3"));
    assert_eq!(groups("1,3-5").unwrap(), [1, 3, 4, 5]);
    assert_eq!(groups("3, 1,1").unwrap(), [1, 3]);
    for wrong in ["0", "a", "5-3", "1-", "1-99999999"] {
        assert!(groups(wrong).is_err(), "{wrong}");
    }
}

/// Choosing groups is choosing what to share: the `.deck` a page of group
/// two carries holds group two and nothing anybody said or showed about the
/// others.
#[test]
fn a_deck_of_some_groups_carries_nothing_of_the_others() {
    let html = export(&Options {
        groups: vec![2],
        ..Options::default()
    });
    let carried = carried(&html);
    let names: Vec<&str> = carried.files.keys().map(String::as_str).collect();
    // Renumbered from one: the name says nothing about the groups before it.
    assert!(names.contains(&"deck/g1.json"), "{names:?}");
    assert!(!names.contains(&"deck/g2.json") && !names.contains(&"deck/g3.json"));
    let groups = carried.groups().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, "g2");
    // Only the file group two points into, and only the lines it needs:
    // its range with the usual context, kept again rather than the span the
    // deck was packed with, which ran on into the third group's code.
    let snapshots = carried.snapshots().unwrap();
    let paths: Vec<&str> = snapshots.iter().map(|s| s.file.path.as_str()).collect();
    assert_eq!(paths, ["src/batch.ts"]);
    let snapshot = &snapshots[0];
    assert_eq!(snapshot.file.entry, "snap/1.txt");
    assert_eq!(snapshot.file.kept, [LineRange::new(1, 10 + snap::CONTEXT)]);
    assert!(!snapshot.text.contains("finish(batch)"));
    // No whole-file hash for a file it holds a piece of.
    assert!(snapshot.file.sha256.is_empty());
    assert!(!String::from_utf8_lossy(&carried.files["snap/index.json"]).contains("sha256"));
    let review = carried.review().unwrap().expect("group two's review");
    assert!(review.comments.is_empty() && review.transcript.is_empty());
    let everything: String = carried
        .files
        .values()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .collect();
    for trace in [
        "The error path",
        "takes the counter down",
        "Nothing else moves",
        "Close it where it opens",
        "finish(batch)",
        "spacer 40",
        "src/store.ts",
        "db.put",
        "finally",
        "retry branch",
        "everything I had",
    ] {
        assert!(!everything.contains(trace), "{trace}");
    }
    // Nor on the page, where the whole-deck remark has no group to follow.
    assert!(!html.contains("everything I had"));
    assert!(html.contains("group 2/3"));
}

/// A deck of groups two and three opens to groups two and three: renumbered
/// from one in their order, as a window reads a numbered deck — from `ord`
/// 1, up to the first gap.
#[test]
fn a_deck_of_some_groups_opens_to_those_groups_in_order() {
    let html = export(&Options {
        groups: vec![2, 3],
        ..Options::default()
    });
    let mut groups = carried(&html).groups().unwrap();
    groups.sort_by_key(|group| group.ord.unwrap_or(u32::MAX));
    // The window's rule (`load::readable`): every group numbered, and read
    // while each is numbered one more than the last.
    let readable = groups
        .iter()
        .enumerate()
        .take_while(|(ix, group)| group.ord == u32::try_from(*ix + 1).ok())
        .count();
    assert_eq!(readable, 2);
    let ids: Vec<&str> = groups.iter().map(|group| group.id.as_str()).collect();
    assert_eq!(ids, ["g2", "g3"]);
    let titles: Vec<Option<&str>> = groups.iter().map(|g| g.title.as_deref()).collect();
    assert_eq!(titles, [Some("Count once"), Some("Nothing else moves")]);
}

#[test]
fn the_code_shows_its_numerals_values_and_changes() {
    let html = export(&Options::default());
    // A value sits right after its name, as the window draws it, and one
    // whose name is not on the line sits at its end.
    let batch = &html[html.find("id=\"pane-g1r1\"").unwrap()..];
    let line = |no: u32| {
        let at = batch
            .find(&format!(">{no}</span><span class=\"t\">"))
            .unwrap_or_else(|| panic!("line {no}"));
        text_of(&batch[at + 1..at + batch[at..].find("</div>").unwrap()])
    };
    assert_eq!(line(10).trim(), "10      pending63 -= 1".trim());
    assert_eq!(line(13).trim(), "13    pending -= 162".trim());
    // A value with keys has a row of its own under its line.
    assert!(batch.contains("row preview"));
    assert!(text_of(batch).contains("↳ row ▸ Row { id: \"r63\", … }"));
    // The proposed line arrives under the one it replaces.
    let fix = &html[html.find("id=\"pane-g2r1\"").unwrap()..];
    let fix = &fix[..fix.find("</section>").unwrap()];
    let going = fix.find("row going").expect("a going row");
    let arriving = fix.find("row arriving").expect("an arriving row");
    assert!(going < arriving);
    assert!(fix.contains(">−</span>") && fix.contains(">+</span>"));
    // The strip.
    assert!(html.contains("Powered by Deck"));
    assert!(html.contains("acme/ingest@a1b2c3d"));
    assert!(html.contains("20 Oct 2026"));
    assert!(!html.contains("09:30"));
}

/// The words in a stretch of markup, tags taken out and entities read.
fn text_of(html: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for ch in html.chars() {
        match ch {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(ch),
            _ => {}
        }
    }
    unescape(&out)
}

/// A point can start well above its range, and the lines it lights are the
/// point: neither its numeral nor its lit rows are context to be trimmed.
#[test]
fn a_point_that_starts_far_above_its_range_keeps_its_numeral() {
    let group = r#"{"id": "g1", "say": "From the loop down.[point 5-10]",
        "refs": [{"id": "g1r1", "file": "src/batch.ts", "range": [9, 10]}]}"#;
    let html = deck(
        &deck_of(&[group], None),
        &palette(),
        Syntax::default(),
        &Options::default(),
    )
    .expect("exports");
    let pane = &html[html.find("id=\"pane-g1r1\"").unwrap()..];
    let pane = &pane[..pane.find("</section>").unwrap()];
    // Line 5 is four out from the range, past the context a page shows.
    let five = pane
        .find(">5</span><span class=\"t\">")
        .expect("line 5 is shown");
    let row = &pane[pane[..five].rfind("<div class=\"row").unwrap()..five];
    assert!(row.contains("lit") && row.contains(">1</span>"), "{row}");
    for no in 5..=8 {
        assert!(
            pane.contains(&format!(">{no}</span><span class=\"t\">")),
            "{no}"
        );
    }
    // And the context past the point still goes.
    assert!(!pane.contains(">4</span><span class=\"t\">"));
}

/// A remark on a change already made is about the lines in the file, which
/// are the ones that arrived; on anything else, about what is there now.
#[test]
fn a_remark_quotes_the_half_of_a_change_that_is_in_the_file() {
    let group = r#"{"id": "g1", "say": "Counted once now.[point 10]",
        "refs": [{"id": "g1r1", "file": "src/batch.ts", "range": [10, 10],
                  "before": "      pending -= 2 // went\n"}]}"#;
    let review = r#"{"v": 1, "deck": "d-1", "comments": [
        {"group": "g1", "ref": "g1r1", "range": [10, 10], "kind": "question", "text": "Why one?"}]}"#;
    let html = deck(
        &deck_of(&[group], Some(review)),
        &palette(),
        Syntax::default(),
        &Options::default(),
    )
    .expect("exports");
    let argument = &html[html.find("id=\"g1-argument\"").unwrap()..];
    let quoted = &argument[argument.find("<div class=\"q\">").expect("a quote")..];
    let quoted = text_of(&quoted[..quoted.find("</article>").unwrap()]);
    assert!(quoted.contains("pending -= 1"), "{quoted}");
    assert!(!quoted.contains("went"), "{quoted}");
}

#[test]
fn the_faces_are_cut_down() {
    let html = export(&Options::default());
    assert_eq!(html.matches("@font-face").count(), 5);
    // Five faces whole are over a megabyte before base64.
    assert!(html.len() < 400_000, "{}", html.len());
}

/// The whole page, faces and deck aside.
///
/// Run with `UPDATE_GOLDEN=1` when a change is meant to move it, and read the
/// diff. Nothing in it depends on the clock: the date is the manifest's.
#[test]
fn the_fixture_deck_is_the_page_it_was() {
    let now = readable(&export(&Options::default()));
    let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/export.html");
    if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, &now).expect("writable");
    }
    let was = std::fs::read_to_string(&at)
        .expect("the golden file exists; run with UPDATE_GOLDEN=1")
        .replace("\r\n", "\n");
    assert_eq!(
        now, was,
        "the page moved; if that was meant, run with UPDATE_GOLDEN=1"
    );
}

mod pdf;
