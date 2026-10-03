//! What changed in a release, read inside deck.
//!
//! # Where the words come from
//!
//! The notes are the release's own body on GitHub — the same text the release
//! page shows, so there is one place they are written. Each version's notes
//! are fetched once and kept in `~/.deck/notes/<version>.md`: a release's
//! notes do not change after it ships, and a deck opened on a plane should
//! still be able to say what it is.
//!
//! # When they are shown
//!
//! When somebody presses the version in the foot, or the *available* beside
//! it. And once, by itself, the first time a deck opens after it has been
//! upgraded — the one moment the reader certainly has not read them yet.
//!
//! # Only the blocks a release uses
//!
//! Headings, paragraphs, bullets and fenced code. The inline marks inside
//! them are the narration's, and are drawn by [`crate::prose`]. Links keep
//! their words and lose their address; an image is dropped. A document
//! renderer would draw tables nobody writes and still not look like deck.

use std::time::Duration;

/// How long to wait for GitHub before saying the notes could not be had. A
/// little longer than the update check: somebody is looking at this one.
const PATIENCE: Duration = Duration::from_secs(5);

/// What this build is.
pub const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// One block of a release's notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading(String),
    Para(String),
    Item(String),
    Code(String),
}

/// Where a version's notes stand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notes {
    /// Being fetched.
    Reading,
    /// Here.
    Read(Vec<Block>),
    /// Not cached, and GitHub did not answer. The page link still works for
    /// whenever the reader is back online.
    Away,
}

/// The release page, for the link in the modal's corner.
#[must_use]
pub fn page(version: &str) -> String {
    format!("https://github.com/henit-chobisa/deck/releases/tag/v{version}")
}

/// The notes for `version`, from the cache if they are there.
#[must_use]
pub fn cached(version: &str) -> Option<Notes> {
    let text = std::fs::read_to_string(store(version)?).ok()?;
    Some(Notes::Read(blocks(&text)))
}

/// Ask GitHub for `version`'s notes, and keep them if they came.
///
/// Blocking. Called from the background executor.
#[must_use]
pub fn fetch(version: &str) -> Notes {
    let url = format!("https://api.github.com/repos/henit-chobisa/deck/releases/tags/v{version}");
    let reply: Option<serde_json::Value> = ureq::get(&url)
        .config()
        .timeout_global(Some(PATIENCE))
        .build()
        .header("User-Agent", concat!("deck/", env!("CARGO_PKG_VERSION")))
        .call()
        .ok()
        .and_then(|mut reply| reply.body_mut().read_json().ok());
    let Some(body) = reply.as_ref().and_then(|reply| reply["body"].as_str()) else {
        return Notes::Away;
    };
    keep(version, body);
    Notes::Read(blocks(body))
}

/// Whether the first window should open on this deck's notes.
static ANNOUNCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// At startup: is this the first launch since an upgrade? Remembered for the
/// first window to ask, by [`announcing`].
///
/// Here and not in the window, so that only a running app — never a test
/// building a window — writes down that a version has been seen.
pub fn look_back() {
    ANNOUNCE.store(first_since_upgrade(), std::sync::atomic::Ordering::Relaxed);
}

/// Whether this window should open on the notes. True for one window only.
#[must_use]
pub fn announcing() -> bool {
    ANNOUNCE.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// Whether this is the first launch since deck was upgraded, and note that it
/// has now been seen either way.
///
/// A fresh install is not an upgrade: there is nothing it is newer than, and
/// a modal before the reader has done anything would be in the way. Going
/// back to an older version is not one either.
#[must_use]
fn first_since_upgrade() -> bool {
    let Some(path) = deck_core::home::deck().map(|deck| deck.join("seen")) else {
        return false;
    };
    let before = std::fs::read_to_string(&path).ok();
    if before.as_deref().map(str::trim) != Some(RUNNING) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, RUNNING);
    }
    upgraded(before.as_deref().map(str::trim), RUNNING)
}

/// Whether going from `before` to `now` is an upgrade worth announcing.
fn upgraded(before: Option<&str>, now: &str) -> bool {
    before.is_some_and(|before| crate::upgrade::newer(now, before))
}

fn store(version: &str) -> Option<std::path::PathBuf> {
    // A version is a path segment here, so nothing that could leave the
    // directory gets to be one.
    let safe = version
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+'));
    if !safe || version.starts_with('.') {
        return None;
    }
    deck_core::home::deck().map(|deck| deck.join("notes").join(format!("{version}.md")))
}

fn keep(version: &str, body: &str) {
    let Some(path) = store(version) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let partial = path.with_extension(format!("md.{}", std::process::id()));
    if std::fs::write(&partial, body).is_ok() && std::fs::rename(&partial, &path).is_err() {
        let _ = std::fs::remove_file(&partial);
    }
}

/// The blocks of a release's markdown.
#[must_use]
pub fn blocks(markdown: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    let mut fence: Option<Vec<&str>> = None;

    let flush = |para: &mut Vec<&str>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Para(plain(&para.join(" "))));
            para.clear();
        }
    };

    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(code) = &mut fence {
            if trimmed.starts_with("```") {
                out.push(Block::Code(code.join("\n")));
                fence = None;
            } else {
                code.push(line);
            }
            continue;
        }
        if trimmed.starts_with("```") {
            flush(&mut para, &mut out);
            fence = Some(Vec::new());
        } else if trimmed.is_empty() || is_rule(trimmed) || trimmed.starts_with("<!--") {
            flush(&mut para, &mut out);
        } else if let Some(heading) = heading(trimmed) {
            flush(&mut para, &mut out);
            out.push(Block::Heading(plain(heading)));
        } else if let Some(item) = item(trimmed) {
            flush(&mut para, &mut out);
            out.push(Block::Item(plain(item)));
        } else if let (true, true, Some(Block::Item(last))) =
            (para.is_empty(), line.starts_with(' '), out.last_mut())
        {
            // An item's text wrapped onto an indented line of its own.
            last.push(' ');
            last.push_str(&plain(trimmed));
        } else {
            para.push(trimmed);
        }
    }
    if let Some(code) = fence {
        out.push(Block::Code(code.join("\n")));
    }
    flush(&mut para, &mut out);
    out.retain(|block| match block {
        Block::Para(text) | Block::Item(text) | Block::Heading(text) => !text.is_empty(),
        Block::Code(_) => true,
    });
    out
}

fn heading(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    (1..=6)
        .contains(&hashes)
        .then(|| &line[hashes..])
        .filter(|rest| rest.starts_with(' '))
        .map(str::trim)
}

fn item(line: &str) -> Option<&str> {
    ["- ", "* ", "+ "]
        .iter()
        .find_map(|bullet| line.strip_prefix(bullet))
        .or_else(|| {
            let digits = line.chars().take_while(char::is_ascii_digit).count();
            (digits > 0)
                .then(|| &line[digits..])
                .and_then(|rest| rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")))
        })
        .map(str::trim)
}

fn is_rule(line: &str) -> bool {
    line.len() >= 3
        && ["-", "*", "_"]
            .iter()
            .any(|mark| line.chars().all(|ch| ch.to_string() == *mark || ch == ' '))
}

/// A line with its links reduced to their words and its images gone.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let image = rest[..open].ends_with('!');
        let before = if image {
            &rest[..open - 1]
        } else {
            &rest[..open]
        };
        let Some((words, after)) = link(&rest[open..]) else {
            out.push_str(&rest[..=open]);
            rest = &rest[open + 1..];
            continue;
        };
        out.push_str(before);
        if !image {
            out.push_str(words);
        }
        rest = after;
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `[words](address)` at the start of `text`: the words, and what follows.
fn link(text: &str) -> Option<(&str, &str)> {
    let close = text.find("](")?;
    let words = &text[1..close];
    if words.contains('[') {
        return None;
    }
    let after = &text[close + 2..];
    let end = after.find(')')?;
    Some((words, &after[end + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_reads_as_its_blocks() {
        let notes = "The same deck as `rc.1`.\nIt exists to test upgrading.\n\n\
            ## What this exercises\n\n```sh\ndeck upgrade --prerelease\n```\n\n\
            - fetch, and\n  checksum\n- swap\n\n---\n\n1. one\n2) two\n";
        assert_eq!(
            blocks(notes),
            vec![
                Block::Para("The same deck as `rc.1`. It exists to test upgrading.".into()),
                Block::Heading("What this exercises".into()),
                Block::Code("deck upgrade --prerelease".into()),
                Block::Item("fetch, and checksum".into()),
                Block::Item("swap".into()),
                Block::Item("one".into()),
                Block::Item("two".into()),
            ]
        );
    }

    #[test]
    fn links_keep_their_words_and_images_go() {
        assert_eq!(
            plain("See [the page](https://x.y/z) and ![demo](a.gif) here."),
            "See the page and here."
        );
        assert_eq!(plain("a [bracket] alone"), "a [bracket] alone");
        assert_eq!(plain("[unclosed](nowhere"), "[unclosed](nowhere");
    }

    #[test]
    fn a_hash_without_a_space_is_not_a_heading() {
        assert_eq!(
            blocks("#14 is fixed"),
            vec![Block::Para("#14 is fixed".into())]
        );
        assert_eq!(blocks("### Small"), vec![Block::Heading("Small".into())]);
    }

    #[test]
    fn an_unclosed_fence_still_shows_its_code() {
        assert_eq!(blocks("```\nx = 1"), vec![Block::Code("x = 1".into())]);
    }

    #[test]
    fn only_an_upgrade_is_announced() {
        assert!(upgraded(Some("0.1.2"), "0.1.3"));
        assert!(upgraded(Some("0.1.3-rc.2"), "0.1.3"));
        assert!(!upgraded(None, "0.1.3"), "a fresh install");
        assert!(!upgraded(Some("0.1.3"), "0.1.3"), "the same again");
        assert!(!upgraded(Some("0.1.4"), "0.1.3"), "going back");
    }

    #[test]
    fn a_version_cannot_leave_the_notes_directory() {
        assert!(store("../../etc/passwd").is_none());
        assert!(store("..").is_none());
        assert!(store("0.1/3").is_none());
        if deck_core::home::deck().is_some() {
            assert!(store("0.1.3-rc.2").is_some());
        }
    }
}
