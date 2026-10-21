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
    assert_eq!(origin(&opened.dir, &file), Some(opened.origin.clone()));
    assert_eq!(opened.origin.sha256, sha);
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
    assert_eq!(opened.origin.beside, None);
}

#[test]
fn two_copies_of_the_same_bytes_each_keep_their_own_review() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    std::fs::create_dir_all(&a).expect("a");
    std::fs::create_dir_all(&b).expect("b");
    let bytes = packed(None);
    std::fs::write(a.join("x.deck"), &bytes).expect("x");
    std::fs::write(b.join("y.deck"), &bytes).expect("y");
    let cache = dir.path().join("opened");

    let first = open(&a.join("x.deck"), &cache).expect("opens");
    let second = open(&b.join("y.deck"), &cache).expect("opens");

    // One deck, shared, read-only.
    assert_eq!(first.deck, second.deck);
    // Two answers, each beside its own file — and the first is not
    // redirected by the second having been opened after it.
    assert!(
        first.origin.review.ends_with("a/x.review"),
        "{:?}",
        first.origin
    );
    assert!(
        second.origin.review.ends_with("b/y.review"),
        "{:?}",
        second.origin
    );
    assert_eq!(origin(&first.dir, &a.join("x.deck")), Some(first.origin));
    assert_eq!(origin(&second.dir, &b.join("y.deck")), Some(second.origin));
}

#[test]
fn a_review_of_another_copy_is_kept_and_this_one_goes_beside_it() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let file = file_in(dir.path());
    let sha = bundle::sha256(&std::fs::read(&file).expect("read"));
    // The review of an earlier copy of the same deck: same id, other bytes.
    let earlier = dir.path().join("2026-10-20-batch.review");
    let old = r#"{"v":1,"deck":"d-1","comments":[],"deck_sha256":"0000000"}"#;
    std::fs::write(&earlier, old).expect("an old review");

    let opened = open(&file, &dir.path().join("opened")).expect("opens");

    let short = &sha[..7];
    assert!(
        opened
            .origin
            .review
            .ends_with(format!("2026-10-20-batch.{short}.review")),
        "{:?}",
        opened.origin.review
    );
    assert_eq!(
        opened
            .origin
            .beside
            .as_deref()
            .map(std::fs::canonicalize)
            .transpose()
            .ok()
            .flatten(),
        Some(std::fs::canonicalize(&earlier).expect("resolves"))
    );
    assert_eq!(
        std::fs::read_to_string(&earlier).expect("there"),
        old,
        "untouched"
    );
}

#[test]
fn a_review_counts_as_this_files_only_with_its_id_and_its_hash() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let at = dir.path().join("x.review");
    let write = |json: &str| std::fs::write(&at, json).expect("written");

    write(r#"{"v":1,"deck":"d-1","comments":[],"deck_sha256":"abc"}"#);
    assert!(answers(&at, "d-1", "abc"));
    assert!(!answers(&at, "d-2", "abc"), "another deck");
    assert!(!answers(&at, "d-1", "abd"), "another copy");
    write(r#"{"v":1,"deck":"d-1","comments":[]}"#);
    assert!(
        answers(&at, "d-1", "abc"),
        "written before reviews named a file"
    );
    assert!(!answers(&at, "d-2", "abc"), "but still only this deck's");
    write("not json");
    assert!(!answers(&at, "d-1", "abc"));
}

#[cfg(unix)]
#[test]
fn a_link_in_the_cache_is_never_followed_and_the_cache_is_rebuilt() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let file = file_in(dir.path());
    let cache = dir.path().join("opened");
    let first = open(&file, &cache).expect("opens");

    // Somewhere outside the cache, with a read-only file in it that must be
    // left exactly as it is.
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).expect("a folder");
    let precious = outside.join("precious.txt");
    std::fs::write(&precious, "keep").expect("written");
    let mut permissions = std::fs::metadata(&precious).expect("there").permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&precious, permissions).expect("locked");
    std::os::unix::fs::symlink(&outside, first.dir.join("snap").join("away")).expect("a link");

    let second = open(&file, &cache).expect("opens again");

    assert!(!second.reused, "a cache with a link in it is not trusted");
    assert!(!second.dir.join("snap").join("away").exists());
    assert!(
        std::fs::metadata(&precious)
            .expect("still there")
            .permissions()
            .readonly(),
        "the target was neither unlocked nor removed"
    );
}

#[test]
fn every_spelling_of_one_remote_is_one_repo() {
    let key = Some("github.com/acme/ingest".to_string());
    for spelling in [
        "git@github.com:acme/ingest.git",
        "git@github.com:acme/ingest",
        "ssh://git@github.com/acme/ingest.git",
        "ssh://git@github.com:22/acme/ingest",
        "https://github.com/acme/ingest",
        "https://github.com/acme/ingest.git",
        "https://GitHub.com/acme/ingest/",
        "https://ada:token@github.com/acme/ingest.git",
    ] {
        assert_eq!(remote_key(spelling), key, "{spelling}");
    }
    assert_ne!(remote_key("https://github.com/acme/other"), key);
    assert_ne!(remote_key("https://gitlab.com/acme/ingest"), key);
    assert_eq!(
        remote_key("/srv/git/ingest.git"),
        None,
        "a path names no repo"
    );
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
fn an_ordinary_deck_folder_has_its_review_beside_it() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = dir.path().join("d-1.deck");
    std::fs::create_dir(&root).expect("a deck folder");
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

/// [`matching`], with git reading an empty file as its global settings, so
/// these pass whatever the machine running them has set.
fn hermetic(cwd: &Path, repo: Option<&Repo>, ceiling: Option<&Path>) -> Match {
    let config = ceiling
        .expect("every test gives a ceiling")
        .join("empty.gitconfig");
    std::fs::write(&config, "").expect("an empty config");
    matching_with(cwd, repo, ceiling, Some(&config))
}

/// What git says when asked, the way [`matching`] asks, for the top of the
/// checkout at `dir` and its remote: the reason, when a match that should
/// have been found was not.
fn why(dir: &Path, ceiling: &Path, config: Option<&Path>) -> String {
    let empty = ceiling.join("empty.gitconfig");
    std::fs::write(&empty, "").expect("an empty config");
    let config = config.unwrap_or(&empty);
    [
        "rev-parse --show-toplevel",
        "config --get remote.origin.url",
        "remote get-url origin",
    ]
    .iter()
    .map(|args| {
        let args: Vec<&str> = args.split(' ').collect();
        format!(
            "git {}: {:?}",
            args.join(" "),
            local_git(dir, Some(ceiling), Some(config), &args)
        )
    })
    .collect::<Vec<_>>()
    .join("\n")
}

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

    let Match::Same { commit, root } = hermetic(&dir.join("src"), Some(&packed), Some(&ceiling))
    else {
        panic!(
            "credentials in the reader's remote are cleaned off before comparing\n{}",
            why(&dir, &ceiling, None)
        );
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
    } = hermetic(&dir, Some(&packed), Some(&ceiling))
    else {
        panic!("the code has moved\n{}", why(&dir, &ceiling, None));
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
        hermetic(&dir, Some(&packed), Some(&ceiling)),
        Match::Elsewhere
    );

    // A deck that never said where it came from is not this repo either,
    // even at the very same commit.
    let silent = Repo {
        remote: None,
        commit: Some(head),
    };
    assert_eq!(
        hermetic(&dir, Some(&silent), Some(&ceiling)),
        Match::Elsewhere
    );
    assert_eq!(hermetic(&dir, None, Some(&ceiling)), Match::Elsewhere);

    // And outside any repo at all.
    let empty = outer.path().join("empty");
    std::fs::create_dir(&empty).expect("a folder");
    assert_eq!(
        hermetic(&empty, Some(&packed), Some(&ceiling)),
        Match::Elsewhere
    );
}

#[test]
fn a_checkout_cloned_over_ssh_matches_a_deck_packed_over_https() {
    let outer = tempfile::tempdir().expect("a temp dir");
    let Some((dir, head)) = repo(outer.path(), "git@GitHub.com:acme/ingest.git") else {
        return;
    };
    let ceiling = std::fs::canonicalize(outer.path()).expect("resolves");
    let packed = Repo {
        remote: Some("https://github.com/acme/ingest".to_string()),
        commit: Some(head),
    };
    assert!(
        matches!(
            hermetic(&dir, Some(&packed), Some(&ceiling)),
            Match::Same { .. }
        ),
        "{}",
        why(&dir, &ceiling, None)
    );
}

#[test]
fn a_review_written_before_files_were_named_is_reused_not_orphaned() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let file = file_in(dir.path());
    let earlier = dir.path().join("2026-10-20-batch.review");
    std::fs::write(&earlier, r#"{"v":1,"deck":"d-1","comments":[]}"#).expect("written");

    let opened = open(&file, &dir.path().join("opened")).expect("opens");

    assert_eq!(
        std::fs::canonicalize(&opened.origin.review).expect("resolves"),
        std::fs::canonicalize(&earlier).expect("resolves")
    );
    assert_eq!(opened.origin.beside, None);
}

#[cfg(unix)]
#[test]
fn a_cache_folder_that_is_a_link_is_removed_not_followed() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().expect("a temp dir");
    let file = file_in(dir.path());
    let cache = dir.path().join("opened");
    std::fs::create_dir_all(&cache).expect("a cache");
    let sha = bundle::sha256(&std::fs::read(&file).expect("read"));
    // The victim: somebody's folder, a read-only file in it.
    let victim = dir.path().join("victim");
    std::fs::create_dir(&victim).expect("a folder");
    let kept = victim.join("kept.txt");
    std::fs::write(&kept, "mine").expect("written");
    std::fs::set_permissions(&kept, std::fs::Permissions::from_mode(0o444)).expect("0444");
    // Planted where the cache folder goes, and where this process writes
    // its part.
    std::os::unix::fs::symlink(&victim, cache.join(&sha)).expect("a link");
    std::os::unix::fs::symlink(
        &victim,
        cache.join(format!("{sha}.{}.part", std::process::id())),
    )
    .expect("a link");

    let opened = open(&file, &cache).expect("opens");

    assert!(!opened.reused);
    assert!(
        !std::fs::symlink_metadata(&opened.dir)
            .expect("there")
            .file_type()
            .is_symlink(),
        "the link was replaced by a real folder"
    );
    let mode = std::fs::metadata(&kept)
        .expect("still there")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o444, "the victim's file was not unlocked");
    assert_eq!(std::fs::read_to_string(&kept).expect("there"), "mine");
}

#[test]
fn owner_and_repo_are_compared_without_case() {
    assert_eq!(
        remote_key("https://github.com/Acme/Ingest.git"),
        remote_key("git@github.com:acme/ingest")
    );
}

#[test]
fn a_checkout_whose_remote_is_rewritten_matches_either_spelling() {
    let outer = tempfile::tempdir().expect("a temp dir");
    let Some((dir, head)) = repo(outer.path(), "gh:acme/ingest") else {
        return;
    };
    let ceiling = std::fs::canonicalize(outer.path()).expect("resolves");
    // The reader's own settings say `gh:` means GitHub over ssh.
    let config = ceiling.join("rewrite.gitconfig");
    std::fs::write(&config, "[url \"git@github.com:\"]\n\tinsteadOf = gh:\n").expect("config");
    let packed = Repo {
        remote: Some("https://github.com/acme/ingest".to_string()),
        commit: Some(head),
    };
    assert!(
        matches!(
            matching_with(&dir, Some(&packed), Some(&ceiling), Some(&config)),
            Match::Same { .. }
        ),
        "{}",
        why(&dir, &ceiling, Some(&config))
    );
    // Without the rewrite, `gh:acme/ingest` is host `gh`: another repo.
    assert_eq!(
        hermetic(&dir, Some(&packed), Some(&ceiling)),
        Match::Elsewhere
    );
}
