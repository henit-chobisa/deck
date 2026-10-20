//! Opening `.deck` files into the cache, and telling which repo the reader is
//! in, against real folders and real repos.
//!
//! Every repo is a fresh `git init` in a temporary directory, run as somebody
//! with no settings of their own. A machine with no git skips the tests that
//! need one rather than failing them.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use deck_core::LineRange;
use deck_core::bundle::{CONTAINER, snap};

use super::*;

/// A small packed deck: one group, one file frozen around lines 30–31.
fn packed(repo: Option<Repo>) -> Vec<u8> {
    let source: String = (1..=60).map(|n| format!("line {n}\n")).collect();
    let (kept, text) = snap::keep(&source, &[LineRange::new(30, 31)], 5);
    let index = snap::Index {
        files: vec![snap::File {
            path: "src/batch.ts".to_string(),
            entry: "snap/1.txt".to_string(),
            sha256: bundle::sha256(source.as_bytes()),
            commit: None,
            state: snap::State::Untracked,
            kept,
            lines: Some(60),
        }],
    };
    let mut files = BTreeMap::new();
    files.insert(
        "deck/deck.json".to_string(),
        br#"{"v":1,"id":"d-1","title":"Why the batch stalls"}"#.to_vec(),
    );
    files.insert(
        "deck/g1.json".to_string(),
        br#"{"id":"g1","ord":1,"say":"look","refs":[{"id":"g1r1","file":"src/batch.ts","range":[30,31]}]}"#
            .to_vec(),
    );
    files.insert("deck/done".to_string(), Vec::new());
    files.insert(
        "snap/index.json".to_string(),
        serde_json::to_vec(&index).expect("serialises"),
    );
    files.insert("snap/1.txt".to_string(), text.into_bytes());
    Bundle {
        manifest: Manifest {
            container: CONTAINER,
            protocol: 1,
            created: "2026-10-20T09:30:00Z".to_string(),
            generator: "deck test".to_string(),
            repo,
            entries: BTreeMap::new(),
            allowed: Vec::new(),
        },
        files,
    }
    .pack()
    .expect("packs")
}

/// A packed deck written to `dir` as `2026-10-20-batch.deck`.
fn file_in(dir: &Path) -> PathBuf {
    let file = dir.join("2026-10-20-batch.deck");
    std::fs::write(&file, packed(None)).expect("written");
    file
}

#[test]
fn a_file_opens_into_a_folder_named_by_its_hash() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let file = file_in(dir.path());
    let cache = dir.path().join("opened");

    let opened = open(&file, &cache).expect("opens");

    let sha = bundle::sha256(&std::fs::read(&file).expect("read"));
    assert_eq!(opened.dir, cache.join(&sha));
    assert_eq!(opened.deck, cache.join(&sha).join("deck"));
    assert!(opened.deck.join("deck.json").is_file());
    assert!(opened.deck.join("g1.json").is_file());
    assert!(opened.deck.join("done").is_file());
    assert!(!opened.reused, "nothing was there to reuse");
    // The window finds its way back to the file from the folder alone.
    assert_eq!(origin(&opened.deck), Some(opened.origin.clone()));
    assert_eq!(snapshots(&opened.deck).len(), 1);
    assert_eq!(snapshots(&opened.deck)[0].line(30), Some("line 30"));
}

#[test]
fn opening_it_again_reuses_what_the_first_time_wrote() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let file = file_in(dir.path());
    let cache = dir.path().join("opened");

    let first = open(&file, &cache).expect("opens");
    // A reader's own notes beside the deck are theirs, and do not make the
    // copy suspect.
    std::fs::write(first.deck.join("draft.json"), "{}").expect("a draft");
    let second = open(&file, &cache).expect("opens again");

    assert!(second.reused);
    assert_eq!(first.dir, second.dir);
    assert!(
        second.deck.join("draft.json").is_file(),
        "the draft is kept"
    );
}

#[test]
fn every_file_the_deck_holds_is_read_only() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let file = file_in(dir.path());
    let opened = open(&file, &dir.path().join("opened")).expect("opens");

    for name in [
        "deck/deck.json",
        "deck/g1.json",
        "deck/done",
        "snap/index.json",
        "snap/1.txt",
        "manifest.json",
    ] {
        let meta = std::fs::metadata(opened.dir.join(name)).expect("there");
        assert!(meta.permissions().readonly(), "{name} is read-only");
    }
    assert!(
        std::fs::write(opened.deck.join("g1.json"), "{}").is_err(),
        "a group cannot be written over"
    );
}

#[test]
fn a_cache_somebody_edited_is_written_again() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let file = file_in(dir.path());
    let cache = dir.path().join("opened");
    let first = open(&file, &cache).expect("opens");

    let group = first.deck.join("g1.json");
    let mut permissions = std::fs::metadata(&group).expect("there").permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&group, permissions).expect("unlocked");
    std::fs::write(&group, br#"{"id":"g1","say":"something else"}"#).expect("edited");
    // And a group the author never wrote, dropped in beside it.
    std::fs::write(first.deck.join("g2.json"), br#"{"id":"g2","say":"no"}"#).expect("added");

    let second = open(&file, &cache).expect("opens again");

    assert!(!second.reused, "the edited copy is not trusted");
    assert!(
        std::fs::read_to_string(second.deck.join("g1.json"))
            .expect("there")
            .contains("look"),
        "the group is the author's again"
    );
    assert!(!second.deck.join("g2.json").exists());
}

#[test]
fn a_tampered_file_is_refused() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let file = dir.path().join("x.deck");
    let mut bytes = packed(None);
    // One byte of the group changed, its checksum and the manifest's hash
    // left as they were.
    let at = bytes
        .windows(4)
        .position(|window| window == b"look")
        .expect("the group's words are in the file");
    bytes[at] = b'b';
    std::fs::write(&file, bytes).expect("written");
    let cache = dir.path().join("opened");

    let err = open(&file, &cache).expect_err("refused");

    assert!(err.to_string().contains("cannot be opened"), "{err}");
    assert!(
        std::fs::read_dir(&cache).map_or(true, |mut entries| entries.next().is_none()),
        "nothing was written"
    );
}

#[test]
fn the_review_goes_beside_the_file() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let file = file_in(dir.path());
    let opened = open(&file, &dir.path().join("opened")).expect("opens");

    let beside = std::path::absolute(dir.path().join("2026-10-20-batch.review")).expect("abs");
    assert_eq!(opened.origin.review, beside);
    // And everything that only has the deck folder finds the same place.
    assert_eq!(crate::review_path(&opened.deck), beside);
}

#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_written_sends_the_review_to_the_cache() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().expect("a temp dir");
    let shelf = dir.path().join("shelf");
    std::fs::create_dir(&shelf).expect("a folder");
    let file = file_in(&shelf);
    std::fs::set_permissions(&shelf, std::fs::Permissions::from_mode(0o555)).expect("locked");
    let cache = dir.path().join("opened");

    let opened = open(&file, &cache);
    // Unlocked before anything is asserted, so a failure still cleans up.
    std::fs::set_permissions(&shelf, std::fs::Permissions::from_mode(0o755)).expect("unlocked");
    let opened = opened.expect("opens");

    // Root writes anywhere, and then the folder is writable after all.
    if writable(&shelf) && opened.origin.review.starts_with(&shelf) {
        return;
    }
    assert_eq!(
        opened.origin.review,
        opened.dir.join("2026-10-20-batch.review")
    );
}

#[test]
fn an_ordinary_deck_folder_has_no_origin() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = dir.path().join("d-1.deck");
    std::fs::create_dir(&root).expect("a deck folder");
    assert_eq!(origin(&root), None);
    assert_eq!(crate::review_path(&root), dir.path().join("d-1.review"));
}

#[test]
fn a_commit_is_shown_short() {
    assert_eq!(short("a1b2c3d4e5f6"), "a1b2c3d");
    assert_eq!(short("a1b"), "a1b");
}

fn has_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Run git in `dir` as somebody with no settings of their own, and say what
/// it printed: no global or system config, none of the caller's `GIT_*`
/// variables, and no looking above the temporary directory for some other
/// repo.
fn run_git(outer: &Path, dir: &Path, args: &[&str]) -> String {
    let config = outer.join("empty.gitconfig");
    std::fs::write(&config, "").expect("an empty config");
    let mut command = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    let out = command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &config)
        .env(
            "GIT_CEILING_DIRECTORIES",
            std::fs::canonicalize(outer).expect("the temp dir resolves"),
        )
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=deck",
            "-c",
            "user.email=deck@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A repo under `outer` with one commit and `origin` set to `remote`, and the
/// commit it is at.
fn repo(outer: &Path, remote: &str) -> Option<(PathBuf, String)> {
    if !has_git() {
        eprintln!("no git on this machine: skipped");
        return None;
    }
    let dir = outer.join("demo");
    std::fs::create_dir_all(dir.join("src")).expect("a folder");
    std::fs::write(dir.join("src/batch.ts"), "one\n").expect("a file");
    run_git(outer, &dir, &["init", "-q"]);
    run_git(outer, &dir, &["add", "-A"]);
    run_git(outer, &dir, &["commit", "-qm", "first"]);
    run_git(outer, &dir, &["remote", "add", "origin", remote]);
    let head = run_git(outer, &dir, &["rev-parse", "HEAD"]);
    Some((dir, head))
}

const REMOTE: &str = "https://github.com/acme/ingest.git";

#[test]
fn the_same_repo_at_the_same_commit_matches() {
    let outer = tempfile::tempdir().expect("a temp dir");
    let Some((dir, head)) = repo(outer.path(), "https://ada:token@github.com/acme/ingest.git")
    else {
        return;
    };
    let ceiling = std::fs::canonicalize(outer.path()).expect("resolves");
    let packed = Repo {
        remote: Some(REMOTE.to_string()),
        commit: Some(head.clone()),
    };

    let Match::Same { commit, root } = matching(&dir.join("src"), Some(&packed), Some(&ceiling))
    else {
        panic!("credentials in the reader's remote are cleaned off before comparing");
    };
    assert_eq!(commit, head);
    assert_eq!(
        std::fs::canonicalize(root).expect("resolves"),
        std::fs::canonicalize(&dir).expect("resolves"),
        "the root is the top of the checkout, not the folder asked from"
    );
}

#[test]
fn the_same_repo_at_another_commit_has_moved() {
    let outer = tempfile::tempdir().expect("a temp dir");
    let Some((dir, first)) = repo(outer.path(), REMOTE) else {
        return;
    };
    std::fs::write(dir.join("src/batch.ts"), "zero\none\n").expect("moved");
    run_git(outer.path(), &dir, &["commit", "-qam", "second"]);
    let head = run_git(outer.path(), &dir, &["rev-parse", "HEAD"]);
    let ceiling = std::fs::canonicalize(outer.path()).expect("resolves");
    let packed = Repo {
        remote: Some(REMOTE.to_string()),
        commit: Some(first.clone()),
    };

    let Match::Moved {
        packed: was,
        head: now,
        root,
    } = matching(&dir, Some(&packed), Some(&ceiling))
    else {
        panic!("the code has moved");
    };
    assert_eq!(was, first);
    assert_eq!(now, head);
    assert_eq!(
        std::fs::canonicalize(root).expect("resolves"),
        std::fs::canonicalize(&dir).expect("resolves")
    );
}

#[test]
fn another_remote_or_none_never_matches() {
    let outer = tempfile::tempdir().expect("a temp dir");
    let Some((dir, head)) = repo(outer.path(), "https://github.com/someone/else.git") else {
        return;
    };
    let ceiling = std::fs::canonicalize(outer.path()).expect("resolves");
    let packed = Repo {
        remote: Some(REMOTE.to_string()),
        commit: Some(head.clone()),
    };
    assert_eq!(
        matching(&dir, Some(&packed), Some(&ceiling)),
        Match::Elsewhere
    );

    // A deck that never said where it came from is not this repo either,
    // even at the very same commit.
    let silent = Repo {
        remote: None,
        commit: Some(head),
    };
    assert_eq!(
        matching(&dir, Some(&silent), Some(&ceiling)),
        Match::Elsewhere
    );
    assert_eq!(matching(&dir, None, Some(&ceiling)), Match::Elsewhere);

    // And outside any repo at all.
    let empty = outer.path().join("empty");
    std::fs::create_dir(&empty).expect("a folder");
    assert_eq!(
        matching(&empty, Some(&packed), Some(&ceiling)),
        Match::Elsewhere
    );
}
