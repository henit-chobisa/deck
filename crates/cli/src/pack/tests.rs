//! `deck pack` and `deck unpack`, against real folders and real repos.
//!
//! Every repo is a fresh `git init` in a temporary directory. A machine with
//! no git skips the tests that need one rather than failing them: packing
//! without git is supported, and is tested on its own.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use super::*;
use deck_core::protocol::{Comment, Moment};

/// When every test pack happened.
const CREATED: &str = "2026-10-20T09:30:00Z";

/// A home directory that is nobody's, so the test does not depend on whose
/// machine it runs on.
const HOME: &str = "/home/ada";

/// Options for a test whose temporary directory is `dir`: a fixed time, a
/// home that is nobody's, and git never looking above `dir` for a repo, so a
/// temp directory that happens to sit inside one is not taken for part of it.
fn options(dir: &Path) -> Options {
    Options {
        created: Some(CREATED.to_string()),
        home: Some(PathBuf::from(HOME)),
        ceiling: Some(std::fs::canonicalize(dir).expect("the temp dir resolves")),
        ..Options::default()
    }
}

fn has_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Run git in `dir`, as somebody with no settings of their own: no global or
/// system config, none of the caller's `GIT_*` variables, and no looking
/// above the temporary directory `dir` is in for some other repo.
fn run_git(dir: &Path, args: &[&str]) {
    let outer = dir.parent().expect("a temp dir above the repo");
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
            shown(&std::fs::canonicalize(outer).expect("the temp dir resolves")),
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
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `lines` numbered lines, as a file.
fn lines(lines: u32) -> String {
    (1..=lines).map(|n| format!("line {n}\n")).collect()
}

/// Write `files` under `dir`, making directories as needed.
fn write(dir: &Path, files: &[(&str, &str)]) {
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(path, text).expect("written");
    }
}

/// A repo at `dir` with `files` committed, or `false` when there is no git.
fn repo(dir: &Path, files: &[(&str, &str)]) -> bool {
    if !has_git() {
        eprintln!("no git on this machine: skipped");
        return false;
    }
    write(dir, files);
    run_git(dir, &["init", "-q"]);
    run_git(dir, &["add", "-A"]);
    run_git(dir, &["commit", "-qm", "first"]);
    true
}

/// A deck folder in `at`, written against `cwd`, with these groups.
fn deck(at: &Path, cwd: Option<&Path>, groups: &[serde_json::Value]) -> PathBuf {
    let root = at.join("d-1.deck");
    std::fs::create_dir_all(&root).expect("a deck folder");
    let mut header = json!({ "v": 1, "id": "d-1", "title": "Why the batch stalls" });
    if let Some(cwd) = cwd {
        header["cwd"] = json!(cwd);
    }
    std::fs::write(root.join("deck.json"), header.to_string()).expect("a header");
    for (n, group) in groups.iter().enumerate() {
        std::fs::write(root.join(format!("g{}.json", n + 1)), group.to_string()).expect("a group");
    }
    root
}

/// One group pointing at `refs`, each `(file, first, last)`.
fn pointing(refs: &[(&Path, u32, u32)]) -> serde_json::Value {
    let refs: Vec<serde_json::Value> = refs
        .iter()
        .enumerate()
        .map(|(n, (file, first, last))| {
            json!({ "id": format!("g1r{}", n + 1), "file": file, "range": [first, last] })
        })
        .collect();
    json!({ "id": "g1", "ord": 1, "say": "look", "refs": refs })
}

fn text(bundle: &Bundle, name: &str) -> String {
    String::from_utf8(bundle.files[name].clone()).expect("text")
}

/// A run of letters and digits, `n` long, for building keys that are not
/// written out whole anywhere in this file.
fn filler(n: usize) -> String {
    "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
        .chars()
        .cycle()
        .take(n)
        .collect()
}

#[test]
fn only_a_decks_own_files_are_taken() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("a.rs", &lines(10))]);
    let group = json!({ "id": "g1", "say": "look", "asks": ["What is a batch?"],
        "refs": [{ "id": "g1r1", "file": "a.rs", "range": [2, 3] }] });
    let root = deck(dir.path(), Some(&project), &[group]);
    for stray in [
        "g01.json",
        "draft.json",
        "asks-g1.json",
        "listening",
        "answering",
        "closed",
        "owner.json",
        "notes.txt",
    ] {
        std::fs::write(root.join(stray), "{}").expect("a stray file");
    }
    std::fs::create_dir(root.join("cache")).expect("a stray folder");
    std::fs::write(root.join("done"), "anything").expect("sealed");

    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let names: Vec<&str> = packing.bundle.files.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "deck/deck.json",
            "deck/done",
            "deck/g1.json",
            "snap/1.txt",
            "snap/index.json"
        ]
    );
    assert!(packing.bundle.files["deck/done"].is_empty());
    for stray in [
        "g01.json",
        "draft.json",
        "asks-g1.json",
        "listening",
        "cache/",
    ] {
        assert!(packing.left.iter().any(|left| left == stray), "{stray}");
    }
    // The pills are for the reader who was there.
    let group: Group = serde_json::from_slice(&packing.bundle.files["deck/g1.json"]).unwrap();
    assert!(group.asks.is_empty());
    assert!(!text(&packing.bundle, "deck/g1.json").contains("asks"));
    let header = packing.bundle.header().expect("a header");
    assert!(header.cwd.is_none());
    assert!(packing.refusal().is_none());
}

#[test]
fn every_free_text_field_loses_this_machines_paths() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("src/a.rs", &lines(10))]);
    let here = project.display().to_string();
    let said = format!("see {here}/src/a.rs and {HOME}/notes.md");
    let wanted = "see src/a.rs and ~/notes.md";
    let group = json!({
        "id": "g1", "title": said, "say": said,
        "confidence": { "level": "medium", "why": said },
        "refs": [
            { "id": "g1r1", "file": "src/a.rs", "range": [2, 3], "note": said, "name": said,
              "after": said,
              "values": [{ "line": 2, "at": said, "is": said,
                           "keys": [{ "key": "k", "is": said, "why": said }] }] },
            { "id": "g1r2", "file": "src/a.rs", "range": [5, 5], "before": said },
            { "id": "g1d1", "note": said, "name": said, "diagram": {
                "title": said,
                "nodes": [{ "id": said, "label": said, "note": said }, { "id": "b", "label": "b" }],
                "edges": [{ "from": said, "to": "b", "label": said }],
                "clusters": [{ "label": said, "nodes": [said] }],
                "flows": [{ "name": said, "steps": [said, { "node": said }, "b"] }] } },
            { "id": "g1p1", "page": format!("<p>{said}</p>"), "note": said, "name": said }
        ]
    });
    let root = deck(dir.path(), Some(&project), &[group]);
    let mut header: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("deck.json")).unwrap()).unwrap();
    header["title"] = json!(said);
    std::fs::write(root.join("deck.json"), header.to_string()).unwrap();
    let review = json!({ "v": 1, "deck": "d-1",
        "comments": [{ "group": "g1", "ref": "g1r1", "file": project.join("src").join("a.rs"),
                       "range": [2, 2], "source": "diff", "quote": said, "text": said }],
        "transcript": [{ "at_ms": 1, "what": "said", "text": said }] });
    std::fs::write(crate::review_path(&root), review.to_string()).unwrap();

    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let bundle = &packing.bundle;
    for (name, bytes) in &bundle.files {
        let raw = String::from_utf8_lossy(bytes);
        assert!(!raw.contains(HOME), "{name}: {raw}");
        let escaped = serde_json::to_string(&here).unwrap();
        assert!(!raw.contains(escaped.trim_matches('"')), "{name}: {raw}");
    }
    let header = bundle.header().unwrap();
    let group = &bundle.groups().unwrap()[0];
    let review = bundle.review().unwrap().expect("a review");
    let Ref::Code(code) = &group.refs[0] else {
        panic!("code")
    };
    let Ref::Code(gone) = &group.refs[1] else {
        panic!("code")
    };
    let Ref::Diagram(drawn) = &group.refs[2] else {
        panic!("a diagram")
    };
    let Ref::Page(page) = &group.refs[3] else {
        panic!("a page")
    };
    let value = &code.values[0];
    let picture = &drawn.diagram;
    let every = [
        &header.title,
        group.title.as_ref().unwrap(),
        &group.say,
        group.confidence.as_ref().unwrap().why.as_ref().unwrap(),
        code.note.as_ref().unwrap(),
        code.after.as_ref().unwrap(),
        gone.before.as_ref().unwrap(),
        &value.at,
        &value.is,
        value.keys[0].is.as_ref().unwrap(),
        value.keys[0].why.as_ref().unwrap(),
        drawn.note.as_ref().unwrap(),
        picture.title.as_ref().unwrap(),
        &picture.nodes[0].label,
        picture.nodes[0].note.as_ref().unwrap(),
        picture.edges[0].label.as_ref().unwrap(),
        &picture.clusters[0].label,
        &picture.flows[0].name,
        page.note.as_ref().unwrap(),
        &review.comments[0].text,
        &review.comments[0].quote,
        &review.transcript[0].text,
    ];
    for (n, field) in every.iter().enumerate() {
        assert_eq!(field.as_str(), wanted, "field {n}");
    }
    // Pane names, and node ids with everything that names them, so the
    // picture still joins up.
    let named = [
        code.name.as_deref().unwrap(),
        drawn.name.as_deref().unwrap(),
        page.name.as_deref().unwrap(),
        picture.nodes[0].id.as_str(),
        picture.edges[0].from.as_str(),
        picture.clusters[0].nodes[0].as_str(),
        picture.flows[0].steps[0].node(),
        picture.flows[0].steps[1].node(),
    ];
    for (n, field) in named.iter().enumerate() {
        assert_eq!(*field, wanted, "name {n}");
    }
    assert_eq!(page.page, format!("<p>{wanted}</p>"));
    assert_eq!(
        review.comments[0].file.as_deref(),
        Some(Path::new("src/a.rs"))
    );
}

#[test]
fn a_path_is_only_swapped_where_it_stands_as_a_path() {
    assert_eq!(swap("/home/ada/x", "/home/ada", "~/", "~"), "~/x");
    assert_eq!(swap("cd /home/ada.", "/home/ada", "~/", "~"), "cd ~.");
    assert_eq!(swap("/home/adam/x", "/home/ada", "~/", "~"), "/home/adam/x");
    assert_eq!(
        swap("/x/home/ada/y", "/home/ada", "~/", "~"),
        "/x/home/ada/y"
    );
    assert_eq!(
        swap("/home/ada.old/y", "/home/ada", "~/", "~"),
        "/home/ada.old/y"
    );
    assert_eq!(swap(r"C:\p\src\a.rs", r"C:\p", "", "."), r"src\a.rs");
    assert_eq!(swap("(/p) and /p/a", "/p", "", "."), "(.) and a");
}

#[test]
fn paths_in_the_repo_become_relative_with_forward_slashes() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    if !repo(
        &project,
        &[("src/a.rs", &lines(60)), ("src/b.rs", &lines(60))],
    ) {
        return;
    }
    let decks = dir.path().join("decks");
    let a = project.join("src").join("a.rs");
    let root = deck(
        &decks,
        Some(&project),
        &[pointing(&[(&a, 10, 12), (Path::new("src/b.rs"), 5, 5)])],
    );
    let review = json!({ "v": 1, "deck": "d-1",
        "comments": [{ "group": "g1", "file": a, "range": [11, 11], "source": "diff",
                       "quote": "line 11", "text": "why?" }],
        "transcript": [{ "at_ms": 3, "file": a, "range": [30, 30] }] });
    std::fs::write(crate::review_path(&root), review.to_string()).unwrap();

    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let group = &packing.bundle.groups().unwrap()[0];
    let files: Vec<PathBuf> = group
        .refs
        .iter()
        .map(|one| match one {
            Ref::Code(code) => code.file.clone(),
            _ => panic!("code"),
        })
        .collect();
    assert_eq!(
        files,
        [PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]
    );
    let review = packing.bundle.review().unwrap().unwrap();
    let Comment { file, .. } = &review.comments[0];
    assert_eq!(file.as_deref(), Some(Path::new("src/a.rs")));
    let Moment { file, .. } = &review.transcript[0];
    assert_eq!(file.as_deref(), Some(Path::new("src/a.rs")));
    assert!(packing.outside.is_empty());
    let snapped: Vec<&str> = packing.snapped.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(snapped, ["src/a.rs", "src/b.rs"]);
    // The comment and the moment add ranges to a.rs rather than a third file.
    assert_eq!(packing.snapped[0].kept, [LineRange::new(1, 50)]);
    let repo = packing.bundle.manifest.repo.as_ref().expect("a repo");
    let commit = repo.commit.as_ref().expect("a commit");
    assert_eq!(commit.len(), 40);
    assert_eq!(packing.snapped[0].commit.as_ref(), Some(commit));
    assert!(repo.remote.is_none());
    assert!(packing.bundle.header().unwrap().cwd.is_none());
    for file in &packing.snapped {
        assert!(!file.path.contains('\\'));
    }
}

#[test]
fn a_file_outside_the_repo_is_named_but_never_read() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("a.rs", &lines(10))]);
    let elsewhere = tempfile::tempdir().expect("another temp dir");
    let stray = elsewhere.path().join("stray.rs");
    std::fs::write(&stray, lines(10)).unwrap();
    let root = deck(dir.path(), Some(&project), &[pointing(&[(&stray, 2, 2)])]);

    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let Ref::Code(code) = &packing.bundle.groups().unwrap()[0].refs[0] else {
        panic!("code")
    };
    assert_eq!(code.file, PathBuf::from("stray.rs"));
    assert_eq!(packing.outside, std::slice::from_ref(&stray));
    assert!(packing.snapped.is_empty());
    assert!(packing.notes()[0].contains("outside the repo"));
    assert!(packing.dry_run().unwrap().contains("never read"));
    let why = packing.refusal().expect("refused");
    assert!(why.contains("outside the repo"), "{why}");
}

#[test]
fn a_remote_loses_its_credentials_and_a_local_one_goes() {
    assert_eq!(
        clean_remote("https://ada:s3cr3t-value@github.com/acme/ingest.git").as_deref(),
        Some("https://github.com/acme/ingest.git")
    );
    assert_eq!(
        clean_remote("ssh://git@github.com/acme/ingest.git").as_deref(),
        Some("ssh://git@github.com/acme/ingest.git")
    );
    assert_eq!(
        clean_remote("https://ada@github.com/acme/ingest.git?private_token=s3cr3t#top").as_deref(),
        Some("https://github.com/acme/ingest.git")
    );
    assert_eq!(
        clean_remote("ada:s3cr3t@corp.example:org/repo.git").as_deref(),
        Some("corp.example:org/repo.git")
    );
    assert_eq!(
        clean_remote("ada@corp.example:org/repo.git").as_deref(),
        Some("corp.example:org/repo.git")
    );
    assert_eq!(
        clean_remote("corp.example:org/repo.git").as_deref(),
        Some("corp.example:org/repo.git")
    );
    assert_eq!(
        clean_remote("git@github.com:acme/ingest.git").as_deref(),
        Some("git@github.com:acme/ingest.git")
    );
    for local in [
        "/srv/git/ingest.git",
        "../ingest",
        "./ingest",
        r"C:\git\ingest",
        "C:/git/ingest",
        "file:///srv/git/ingest.git",
        "",
    ] {
        assert_eq!(clean_remote(local), None, "{local}");
    }

    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    if !repo(&project, &[("a.rs", &lines(10))]) {
        return;
    }
    run_git(
        &project,
        &[
            "remote",
            "add",
            "origin",
            "https://ada:s3cr3t-value@github.com/acme/ingest.git",
        ],
    );
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[(Path::new("a.rs"), 1, 1)])],
    );
    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let repo = packing.bundle.manifest.repo.clone().expect("a repo");
    assert_eq!(
        repo.remote.as_deref(),
        Some("https://github.com/acme/ingest.git")
    );

    run_git(&project, &["remote", "set-url", "origin", "../elsewhere"]);
    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let repo = packing.bundle.manifest.repo.clone().expect("a repo");
    assert!(repo.remote.is_none());
    assert!(repo.commit.is_some());
}

#[test]
fn snapshots_merge_their_spans_and_say_how_each_file_stands() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    if !repo(&project, &[("a.rs", &lines(100)), ("b.rs", &lines(30))]) {
        return;
    }
    std::fs::write(project.join("b.rs"), lines(31)).unwrap();
    std::fs::write(project.join("c.rs"), lines(5)).unwrap();
    let a = Path::new("a.rs");
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[
            (a, 40, 42),
            (Path::new("b.rs"), 3, 3),
            (a, 50, 51),
            (Path::new("c.rs"), 2, 2),
        ])],
    );

    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let [first, second, third] = &packing.snapped[..] else {
        panic!("three files: {:?}", packing.snapped)
    };
    assert_eq!(
        (first.path.as_str(), first.entry.as_str()),
        ("a.rs", "snap/1.txt")
    );
    assert_eq!(first.kept, [LineRange::new(20, 71)]);
    assert_eq!(first.state, snap::State::Clean);
    assert_eq!(first.sha256, bundle::sha256(lines(100).as_bytes()));
    assert_eq!(second.state, snap::State::Modified);
    assert_eq!(third.state, snap::State::Untracked);
    assert_eq!(third.entry, "snap/3.txt");
    let snapshots = packing.bundle.snapshots().expect("snapshots");
    assert_eq!(snapshots[0].line(41), Some("line 41"));
    let listing = packing.dry_run().unwrap();
    assert!(listing.contains("a.rs  lines 20-71  clean"), "{listing}");
    assert!(listing.contains("b.rs  lines 1-23  modified"), "{listing}");
    assert!(listing.contains("snap/index.json"), "{listing}");

    let tight = gather(
        &root,
        &Options {
            context: 0,
            ..options(dir.path())
        },
    )
    .expect("gathers");
    assert_eq!(
        tight.snapped[0].kept,
        [LineRange::new(40, 42), LineRange::new(50, 51)]
    );
    let whole = gather(
        &root,
        &Options {
            whole_files: true,
            ..options(dir.path())
        },
    )
    .expect("gathers");
    assert_eq!(whole.snapped[0].kept, [LineRange::new(1, 100)]);
    assert_eq!(text(&whole.bundle, "snap/1.txt"), lines(100));
}

#[test]
fn a_file_that_cannot_be_read_stops_the_pack_unless_allowed() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("a.rs", &lines(10))]);
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[
            (Path::new("gone.rs"), 1, 2),
            (Path::new("a.rs"), 1, 1),
        ])],
    );
    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let why = packing.refusal().expect("refused");
    assert!(
        why.contains("gone.rs") && why.contains("--allow-missing"),
        "{why}"
    );
    let out = dir.path().join("out.deck");
    assert!(pack(&root, Some(&out), &options(dir.path())).is_err());
    assert!(!out.exists());

    let allowing = Options {
        allow_missing: true,
        ..options(dir.path())
    };
    let packed = pack(&root, Some(&out), &allowing).expect("packs");
    assert!(packed.notes.iter().any(|note| note.contains("left out")));
    let (opened, _) = Bundle::open(&std::fs::read(&out).unwrap()).expect("opens");
    let snapshots = opened.snapshots().unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].file.entry, "snap/1.txt");
}

/// One true positive per rule, built so that no key appears whole in this
/// file.
fn true_positives() -> Vec<(&'static str, String)> {
    vec![
        (
            "aws-access-key-id",
            format!("key = {}{}", "AKIA", "IOSFODNN7EXAMPLE"),
        ),
        ("github-pat", format!("ghp{}{}", "_", filler(36))),
        ("github-oauth", format!("gho{}{}", "_", filler(36))),
        ("github-app-token", format!("ghs{}{}", "_", filler(36))),
        ("github-refresh-token", format!("ghr{}{}", "_", filler(36))),
        (
            "github-fine-grained-pat",
            format!("github{}{}", "_pat_", filler(82)),
        ),
        ("gitlab-pat", format!("glpat{}{}", "-", filler(20))),
        (
            "slack-token",
            format!("xoxb{}{}", "-1234567890-", filler(24)),
        ),
        (
            "slack-webhook",
            format!(
                "https://hooks.slack.com/services/{}/{}/{}",
                "T0000AAAA",
                "B0000BBBB",
                filler(24)
            ),
        ),
        ("stripe-key", format!("sk{}{}", "_live_", filler(24))),
        ("google-api-key", format!("AIza{}", filler(35))),
        (
            "anthropic-key",
            format!("sk-ant{}{}", "-api03-", filler(40)),
        ),
        ("openai-key", format!("sk-proj{}{}", "-", filler(40))),
        (
            "private-key",
            format!("-----BEGIN {}-----", "RSA PRIVATE KEY"),
        ),
        (
            "jwt",
            format!(
                "{}.{}.{}",
                "eyJhbGciOiJIUzI1NiJ9",
                "eyJzdWIiOiIxMjM0NTY3ODkwIn0",
                "dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
            ),
        ),
        (
            "generic-secret",
            format!("password = \"{}\"", "correct-horse-battery"),
        ),
    ]
}

#[test]
fn every_rule_finds_its_secret_and_only_that_rule_does() {
    let samples = true_positives();
    for rule in rules() {
        assert!(
            samples.iter().any(|(id, _)| *id == rule),
            "no sample for {rule}"
        );
    }
    for (rule, sample) in &samples {
        let hits = scan("snap/1.txt", format!("first line\n  {sample}\n").as_bytes());
        assert_eq!(hits.len(), 1, "{rule}: {hits:?}");
        let hit = &hits[0];
        assert_eq!(hit.rule, *rule);
        assert_eq!(hit.at, "snap/1.txt:2");
        assert_eq!(hit.redacted.len(), 8);
        assert!(hit.redacted.ends_with("****"));
        assert_eq!(hit.fingerprint.len(), 12);
    }
    // Both GitHub app prefixes, and a classic OpenAI key.
    assert_eq!(
        scan("e", format!("ghu{}{}", "_", filler(36)).as_bytes())[0].rule,
        "github-app-token"
    );
    assert_eq!(
        scan("e", format!("sk-{}", filler(48)).as_bytes())[0].rule,
        "openai-key"
    );
    // A secret inside a protocol file is found through its escaping.
    let escaped =
        serde_json::to_vec_pretty(&json!({ "say": format!("token: \"{}\"", filler(12)) })).unwrap();
    let hits = scan("deck/g1.json", &escaped);
    assert_eq!(hits[0].rule, "generic-secret");
    assert_eq!(hits[0].at, "deck/g1.json say");
}

#[test]
fn ordinary_code_is_not_a_secret() {
    let corpus = [
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "commit 3261dcd8f0a1b2c3d4e5f60718293a4b5c6d7e8f",
        "id: 123e4567-e89b-12d3-a456-426614174000",
        r#"let icon = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";"#,
        r#"password: """#,
        r#"token = env("X")"#,
        r#"api_key = os.environ["API_KEY"]"#,
        "secret: process.env.SECRET",
        r#"token: "short""#,
        r#"password: "${DB_PASSWORD}""#,
        r#"secret = "<your secret here>""#,
        r#"api_key = "{{ api_key }}""#,
        "token_count = 12345678",
        "password_hash = hash(input)",
        "-----BEGIN PUBLIC KEY-----",
        "-----BEGIN CERTIFICATE-----",
        r#"<div class="sk-button sk-primary">"#,
        "ghp_short and gho_ and AKIA alone",
        r#"header = "eyJhbGciOiJIUzI1NiJ9""#,
        "Authorization: Bearer ${token}",
        "let sk = keys.sk_test_only;",
        "https://hooks.slack.com/",
        "AIzaShort",
    ];
    for line in corpus {
        assert!(
            scan("snap/1.txt", line.as_bytes()).is_empty(),
            "{line}: {:?}",
            scan("snap/1.txt", line.as_bytes())
        );
    }
}

#[test]
fn a_secret_refuses_the_pack_until_it_is_allowed_by_fingerprint() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    let key = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    write(&project, &[("a.rs", &format!("{}aws = {key}\n", lines(4)))]);
    let mut group = pointing(&[(Path::new("a.rs"), 5, 5)]);
    group["say"] = json!(format!(
        "it reads password = \"{}\"",
        "correct-horse-battery"
    ));
    let root = deck(dir.path(), Some(&project), &[group]);
    let out = dir.path().join("out.deck");

    let err = pack(&root, Some(&out), &options(dir.path()))
        .expect_err("refused")
        .to_string();
    let packing = gather(&root, &options(dir.path())).unwrap();
    assert_eq!(packing.hits.len(), 2, "{:?}", packing.hits);
    let in_snapshot = packing
        .hits
        .iter()
        .find(|hit| hit.entry == "snap/1.txt")
        .expect("found in the code");
    assert_eq!(
        (in_snapshot.at.as_str(), in_snapshot.rule),
        ("a.rs:5", "aws-access-key-id")
    );
    assert_eq!(in_snapshot.redacted, "AKIA****");
    for hit in &packing.hits {
        assert!(
            err.contains(&format!("--allow {}", hit.fingerprint)),
            "{err}"
        );
        assert!(err.contains(&hit.redacted));
    }
    assert!(!err.contains(&key), "the refusal must not print the secret");
    assert!(!out.exists());

    // Letting one through is not letting both through.
    let one = Options {
        allow: vec![in_snapshot.fingerprint.clone()],
        ..options(dir.path())
    };
    assert!(pack(&root, Some(&out), &one).is_err());

    let mut allow: Vec<String> = packing.hits.iter().map(|h| h.fingerprint.clone()).collect();
    allow.push("000000000000".to_string());
    let both = Options {
        allow: allow.clone(),
        ..options(dir.path())
    };
    let packed = pack(&root, Some(&out), &both).expect("packs");
    assert!(packed.notes.iter().any(|n| n.contains("000000000000")));
    let (opened, _) = Bundle::open(&std::fs::read(&out).unwrap()).expect("opens");
    allow.pop();
    allow.sort();
    assert_eq!(opened.manifest.allowed, allow);
}

#[test]
fn the_same_deck_packs_to_the_same_bytes() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("a.rs", &lines(40))]);
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[(Path::new("a.rs"), 5, 6)])],
    );
    std::fs::write(root.join("done"), "").unwrap();
    let stamped = Options {
        created: Some(stamp(Some("1760952600")).unwrap()),
        ..options(dir.path())
    };
    let one = pack(&root, Some(&dir.path().join("one.deck")), &stamped).expect("packs");
    let two = pack(&root, Some(&dir.path().join("two.deck")), &stamped).expect("packs");
    assert_eq!(
        std::fs::read(&one.path).unwrap(),
        std::fs::read(&two.path).unwrap()
    );
    let (opened, _) = Bundle::open(&std::fs::read(&one.path).unwrap()).expect("opens");
    assert_eq!(opened.manifest.created, "2025-10-20T09:30:00Z");
    assert_eq!(
        opened.manifest.generator,
        format!("deck {}", env!("CARGO_PKG_VERSION"))
    );
    // A directory as the output gets the file under its own name.
    let folder = dir.path().join("out");
    std::fs::create_dir(&folder).unwrap();
    let named = pack(&root, Some(&folder), &stamped).expect("packs");
    assert_eq!(
        named.path,
        folder.join("2025-10-20-why-the-batch-stalls.deck")
    );
}

#[test]
fn a_time_is_written_to_the_second_in_utc() {
    assert_eq!(stamp(Some("0")).unwrap(), "1970-01-01T00:00:00Z");
    assert_eq!(stamp(Some("951782400")).unwrap(), "2000-02-29T00:00:00Z");
    assert_eq!(stamp(Some("4107542399")).unwrap(), "2100-02-28T23:59:59Z");
    assert!(stamp(Some("yesterday")).is_err());
    assert!(stamp(Some("-1")).is_err());
    let now = stamp(None).unwrap();
    assert_eq!(now.len(), 20);
    assert!(now.ends_with('Z'));
}

#[test]
fn a_deck_is_named_by_its_day_and_its_title() {
    assert_eq!(
        default_name("Why the batch stalls", CREATED),
        "2026-10-20-why-the-batch-stalls.deck"
    );
    assert_eq!(
        default_name("  `retry()` runs — twice?!  ", CREATED),
        "2026-10-20-retry-runs-twice.deck"
    );
    assert_eq!(default_name("—", CREATED), "2026-10-20-deck.deck");
    assert!(default_name(&"a".repeat(200), CREATED).len() <= 10 + 1 + 60 + 5);
    assert_eq!(
        unpacked_name(Path::new("x/2026-10-20-a.deck")),
        PathBuf::from("2026-10-20-a.unpacked")
    );
}

#[test]
fn unpack_and_pack_are_inverses() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("src/a.rs", &lines(40))]);
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[(Path::new("src/a.rs"), 5, 6)])],
    );
    std::fs::write(root.join("done"), "").unwrap();
    std::fs::write(
        crate::review_path(&root),
        json!({ "v": 1, "deck": "d-1", "comments": [] }).to_string(),
    )
    .unwrap();
    let packed = pack(
        &root,
        Some(&dir.path().join("a.deck")),
        &options(dir.path()),
    )
    .expect("packs");
    let bytes = std::fs::read(&packed.path).unwrap();

    // unpack(pack(d)) is d, scrubbed: every entry as gathering made it.
    let into = dir.path().join("a.unpacked");
    let unpacked = unpack(&packed.path, Some(&into)).expect("unpacks");
    assert!(unpacked.ignored.is_empty());
    let gathered = gather(&root, &options(dir.path())).unwrap();
    let mut written = BTreeMap::new();
    let mut left = Vec::new();
    walk(&into, "", &mut written, &mut left).unwrap();
    assert!(left.is_empty(), "{left:?}");
    let manifest = written.remove("manifest.json").expect("the manifest");
    assert_eq!(written, gathered.bundle.files);
    assert!(written.contains_key("review.json"));
    let (opened, _) = Bundle::open(&bytes).unwrap();
    assert_eq!(
        serde_json::from_slice::<Manifest>(&manifest).unwrap(),
        opened.manifest
    );

    // pack(unpack(f)) is f, byte for byte, and nothing is scrubbed twice.
    let again = gather(&into, &options(dir.path())).expect("gathers");
    assert!(again.repacked);
    let repacked = pack(
        &into,
        Some(&dir.path().join("b.deck")),
        &options(dir.path()),
    )
    .expect("packs");
    assert_eq!(std::fs::read(&repacked.path).unwrap(), bytes);
}

#[test]
fn nothing_is_overwritten_without_being_asked() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("a.rs", &lines(4))]);
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[(Path::new("a.rs"), 1, 1)])],
    );
    let out = dir.path().join("taken.deck");
    std::fs::write(&out, "mine").unwrap();
    let err = pack(&root, Some(&out), &options(dir.path()))
        .expect_err("refused")
        .to_string();
    assert!(err.contains("--force"), "{err}");
    assert_eq!(std::fs::read(&out).unwrap(), b"mine");
    let forced = Options {
        force: true,
        ..options(dir.path())
    };
    pack(&root, Some(&out), &forced).expect("replaces");
    assert!(Bundle::open(&std::fs::read(&out).unwrap()).is_ok());

    let busy = dir.path().join("busy");
    write(&busy, &[("keep.txt", "mine")]);
    let err = unpack(&out, Some(&busy)).expect_err("refused").to_string();
    assert!(err.contains("not empty"), "{err}");
    assert!(!busy.join("manifest.json").exists());
    // An empty one is fine.
    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    unpack(&out, Some(&empty)).expect("unpacks");
    assert!(empty.join("deck").join("deck.json").is_file());
}

#[test]
fn a_folder_that_is_not_a_deck_is_refused() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let err = gather(dir.path(), &options(dir.path()))
        .expect_err("refused")
        .to_string();
    assert!(err.contains("no deck.json"), "{err}");
    let garbage = dir.path().join("x.deck");
    std::fs::write(&garbage, "not a zip").unwrap();
    assert!(unpack(&garbage, Some(&dir.path().join("x"))).is_err());
}

#[test]
fn a_deck_with_no_repo_still_packs() {
    // No `git init` here, and git is told not to look above the temp
    // directory, so this holds wherever TMPDIR is and whether or not git is
    // installed: it packs, unscoped and untracked.
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("src/a.rs", &lines(4))]);
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[(&project.join("src").join("a.rs"), 1, 1)])],
    );
    let packing = gather(&root, &options(dir.path())).expect("gathers");
    assert!(packing.bundle.manifest.repo.is_none());
    assert_eq!(packing.snapped[0].path, "src/a.rs");
    assert_eq!(packing.snapped[0].state, snap::State::Untracked);
    assert!(packing.outside.is_empty());
}

/// Nothing outside the root is ever read, however the deck spells its way
/// there: up and out, an absolute path, or a link in the repo pointing out.
#[test]
fn a_ref_that_leaves_the_repo_is_never_read() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("a.rs", &lines(4))]);
    let private = "PRIVATE-CREDENTIALS-NOBODY-ELSE-SEES";
    write(
        dir.path(),
        &[
            ("secrets/up", &format!("{private}\n")),
            ("secrets/absolute", &format!("{private}\n")),
            ("secrets/linked", &format!("{private}\n")),
        ],
    );
    let absolute = dir.path().join("secrets").join("absolute");
    let mut refs: Vec<(PathBuf, u32, u32)> = vec![
        (PathBuf::from("../secrets/up"), 1, 1),
        (absolute, 1, 1),
        (PathBuf::from("a.rs"), 1, 1),
    ];
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            dir.path().join("secrets").join("linked"),
            project.join("link.rs"),
        )
        .expect("a link out of the repo");
        refs.push((PathBuf::from("link.rs"), 1, 1));
    }
    let pointed: Vec<(&Path, u32, u32)> =
        refs.iter().map(|(p, a, b)| (p.as_path(), *a, *b)).collect();
    let root = deck(dir.path(), Some(&project), &[pointing(&pointed)]);

    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let snapped: Vec<&str> = packing.snapped.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(snapped, ["a.rs"]);
    assert_eq!(packing.outside.len(), refs.len() - 1);
    for bytes in packing.bundle.files.values() {
        assert!(!String::from_utf8_lossy(bytes).contains(private));
    }
    let why = packing.refusal().expect("refused");
    assert!(why.contains("outside the repo"), "{why}");
    let out = dir.path().join("out.deck");
    assert!(pack(&root, Some(&out), &options(dir.path())).is_err());

    // Allowed to go without them, they go without them.
    let allowing = Options {
        allow_missing: true,
        ..options(dir.path())
    };
    pack(&root, Some(&out), &allowing).expect("packs");
    let bytes = std::fs::read(&out).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(private));
}

#[test]
fn a_token_right_after_an_escape_is_found() {
    let github = format!("ghp{}{}", "_", filler(36));
    let aws = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    let group = json!({ "id": "g1", "say": format!("first\n{github}"),
                        "title": format!("tab\t{aws}") });
    let bytes = serde_json::to_vec_pretty(&group).unwrap();
    let hits = scan("deck/g1.json", &bytes);
    let mut found: Vec<(&str, &str)> = hits.iter().map(|h| (h.rule, h.at.as_str())).collect();
    found.sort_unstable();
    assert_eq!(
        found,
        [
            ("aws-access-key-id", "deck/g1.json title"),
            ("github-pat", "deck/g1.json say line 2"),
        ]
    );
    // Glued to a word in plain text, a distinctive prefix is still found.
    assert_eq!(
        scan("snap/1.txt", format!("x = \"\\n{github}\"").as_bytes())[0].rule,
        "github-pat"
    );
}

#[test]
fn a_secret_under_a_longer_name_is_found() {
    let value = "Zq8mVt4LpW2xRn7k";
    for line in [
        format!("client_secret = \"{value}\""),
        format!("AWS_SECRET_ACCESS_KEY=\"{value}\""),
        format!("DB_PASSWORD: '{value}'"),
        format!("access_token = \"{value}\""),
        format!("\"apiKey\": \"{value}\""),
    ] {
        let hits = scan("snap/1.txt", line.as_bytes());
        assert_eq!(hits.len(), 1, "{line}: {hits:?}");
        assert_eq!(hits[0].rule, "generic-secret");
        assert_eq!(
            hits[0].redacted, "Zq8m****",
            "only the value, never the name"
        );
    }
}

/// The fingerprint is the value's, not the quotes' or the name's, so one
/// `--allow` covers the same secret in the narration and in the code.
#[test]
fn one_secret_has_one_fingerprint_wherever_it_is() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    let value = "Zq8mVt4LpW2xRn7k";
    write(
        &project,
        &[(
            "a.rs",
            &format!("{}let db_password = '{value}';\n", lines(3)),
        )],
    );
    let mut group = pointing(&[(Path::new("a.rs"), 4, 4)]);
    group["say"] = json!(format!("it reads `secret: \"{value}\"`"));
    let root = deck(dir.path(), Some(&project), &[group]);
    let packing = gather(&root, &options(dir.path())).expect("gathers");
    assert_eq!(packing.hits.len(), 2, "{:?}", packing.hits);
    assert_eq!(packing.hits[0].fingerprint, packing.hits[1].fingerprint);
    let allowing = Options {
        allow: vec![packing.hits[0].fingerprint.clone()],
        ..options(dir.path())
    };
    let let_through = gather(&root, &allowing).expect("gathers");
    assert!(let_through.hits.is_empty());
    assert_eq!(let_through.allowed.len(), 2);
    assert_eq!(let_through.bundle.manifest.allowed.len(), 1);
}

#[test]
fn a_hit_in_a_snapshot_names_the_source_line() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    let mut source = lines(120);
    source = source.replacen(
        "line 101\n",
        &format!("aws = {}{}\n", "AKIA", "IOSFODNN7EXAMPLE"),
        1,
    );
    write(&project, &[("src/a.rs", &source)]);
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[
            (Path::new("src/a.rs"), 10, 10),
            (Path::new("src/a.rs"), 100, 102),
        ])],
    );
    let packing = gather(&root, &options(dir.path())).expect("gathers");
    assert_eq!(packing.hits.len(), 1);
    assert_eq!(packing.hits[0].entry, "snap/1.txt");
    assert_eq!(packing.hits[0].at, "src/a.rs:101");
    assert!(packing.refusal().unwrap().contains("src/a.rs:101"));
}

/// A deck written against `/tmp/repo/sub` names its root `/tmp/repo`, though
/// git and the disk both call it `/private/tmp/repo`. Every spelling goes.
#[cfg(unix)]
#[test]
fn the_root_as_the_deck_spells_it_is_scrubbed() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let real = dir.path().join("real");
    if !repo(&real, &[("src/a.rs", &lines(4)), ("sub/b.rs", &lines(4))]) {
        return;
    }
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("a link to the repo");
    let cwd = link.join("sub");
    let mut group = pointing(&[(Path::new("../src/a.rs"), 1, 1)]);
    group["say"] = json!(format!("see {}/src/a.rs", link.display()));
    let root = deck(dir.path(), Some(&cwd), &[group]);
    let packing = gather(&root, &options(dir.path())).expect("gathers");
    let group = &packing.bundle.groups().unwrap()[0];
    assert_eq!(group.say, "see src/a.rs");
    assert_eq!(packing.snapped[0].path, "src/a.rs");
}

#[test]
fn the_home_directory_goes_in_every_spelling() {
    let scrub = Scrub {
        roots: Vec::new(),
        homes: variants(vec![r"C:\Users\ada\".to_string()]),
    };
    for written in [
        r"C:\Users\ada\x",
        "C:/Users/ada/x",
        r"c:\users\ada\x",
        "c:/Users/ada/x",
    ] {
        let mut text = written.to_string();
        scrub.text(&mut text);
        assert_eq!(text, "~/x", "{written}");
    }
    let scrub = Scrub {
        roots: Vec::new(),
        homes: variants(vec!["/home/ada/".to_string()]),
    };
    let mut text = "/home/ada/x and /home/ada".to_string();
    scrub.text(&mut text);
    assert_eq!(text, "~/x and ~");
}

/// `src/[id].tsx` is that file, not a pattern that matches `src/i.tsx`.
#[test]
fn a_path_with_glob_characters_is_taken_literally() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    if !repo(
        &project,
        &[("src/i.tsx", &lines(4)), ("src/[id].tsx", &lines(4))],
    ) {
        return;
    }
    std::fs::write(project.join("src").join("i.tsx"), lines(5)).unwrap();
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[(Path::new("src/[id].tsx"), 1, 1)])],
    );
    let packing = gather(&root, &options(dir.path())).expect("gathers");
    assert_eq!(packing.snapped[0].path, "src/[id].tsx");
    assert_eq!(packing.snapped[0].state, snap::State::Clean);
}

/// A repack takes the names a `.deck` file holds and nothing else: a Finder
/// `.DS_Store`, a note, or a link is left behind and said, never packed and
/// never a reason to fail.
#[test]
fn a_repack_takes_only_what_a_deck_file_holds() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let project = dir.path().join("project");
    write(&project, &[("a.rs", &lines(10))]);
    let root = deck(
        dir.path(),
        Some(&project),
        &[pointing(&[(Path::new("a.rs"), 2, 2)])],
    );
    let packed = pack(
        &root,
        Some(&dir.path().join("a.deck")),
        &options(dir.path()),
    )
    .expect("packs");
    let into = dir.path().join("a.unpacked");
    unpack(&packed.path, Some(&into)).expect("unpacks");
    write(
        &into,
        &[
            (".DS_Store", "finder"),
            ("deck/notes.txt", "mine"),
            ("mimetype", "x"),
        ],
    );
    let mut stray = vec![".DS_Store", "deck/notes.txt", "mimetype"];
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(project.join("a.rs"), into.join("deck").join("g9.json"))
            .expect("a link");
        stray.push("deck/g9.json");
    }
    let packing = gather(&into, &options(dir.path())).expect("gathers");
    let mut left: Vec<&str> = packing.left.iter().map(String::as_str).collect();
    left.sort_unstable();
    stray.sort_unstable();
    assert_eq!(left, stray);
    let again = pack(
        &into,
        Some(&dir.path().join("b.deck")),
        &options(dir.path()),
    )
    .expect("packs");
    assert_eq!(
        std::fs::read(&again.path).unwrap(),
        std::fs::read(&packed.path).unwrap()
    );
}
