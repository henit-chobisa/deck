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
//! upgraded — the one moment the reader certainly has not read them yet. With
//! `[updates] automatic = false` that only happens if the notes are already
//! kept, since fetching them would be a request nobody asked for.
//!
//! # Only the blocks a release uses
//!
//! Headings, paragraphs, bullets, fenced code and images. The inline marks
//! inside them are the narration's, and are drawn by [`crate::prose`]. Links
//! keep their words and lose their address. A document renderer would draw
//! tables nobody writes and still not look like deck.
//!
//! # Images
//!
//! A release shows what changed, and a screenshot says it faster than a
//! paragraph. An image on a line of its own — markdown or an `<img>` tag — is
//! fetched with the notes and kept beside them, in `~/.deck/notes/<version>/`,
//! so it is there offline too. One inside a sentence is dropped: there is no
//! good way to set a picture in the middle of a line.

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
    /// A picture on a line of its own. `width` is the width the release asked
    /// for, in points, when it said.
    Image {
        url: String,
        alt: String,
        width: Option<u32>,
    },
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
    let reply = ureq::get(&url)
        .config()
        .timeout_global(Some(PATIENCE))
        .build()
        .header("User-Agent", concat!("deck/", env!("CARGO_PKG_VERSION")))
        .call();
    let reply: serde_json::Value = match reply {
        Ok(mut reply) => match reply.body_mut().read_json() {
            Ok(reply) => reply,
            Err(_) => return Notes::Away,
        },
        // GitHub answered, and there is no such release — a build from a
        // version nobody tagged. That is no notes, not no network.
        Err(ureq::Error::StatusCode(404)) => return Notes::Read(Vec::new()),
        Err(_) => return Notes::Away,
    };
    // A release with no description comes back with `"body": null`. That is
    // an answer too, and one worth keeping so it is not asked again.
    let body = reply["body"].as_str().unwrap_or_default();
    keep(version, body);
    // The words only. The pictures follow on their own (`fetch_images`), so
    // the card has something to read before the slowest screenshot is down.
    Notes::Read(blocks(body))
}

/// The longest an image may take, and the most it may weigh.
const IMAGE_PATIENCE: Duration = Duration::from_secs(10);
const IMAGE_LIMIT: u64 = 20 * 1024 * 1024;

/// Fetch whichever of these images are not kept yet. Says whether any
/// arrived, so a card already on screen knows to draw them.
///
/// Blocking. Called from the background executor. Only over https, and a
/// failure is an image that is not drawn — the words are what matter.
pub fn fetch_images(version: &str, blocks: &[Block]) -> bool {
    let mut arrived = false;
    for block in blocks {
        let Block::Image { url, .. } = block else {
            continue;
        };
        let Some(path) = image_file(version, url) else {
            continue;
        };
        if path.exists() || !url.starts_with("https://") {
            continue;
        }
        let Ok(mut reply) = ureq::get(url)
            .config()
            .timeout_global(Some(IMAGE_PATIENCE))
            .build()
            .header("User-Agent", concat!("deck/", env!("CARGO_PKG_VERSION")))
            .call()
        else {
            continue;
        };
        let Ok(bytes) = reply
            .body_mut()
            .with_config()
            .limit(IMAGE_LIMIT)
            .read_to_vec()
        else {
            continue;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let partial = path.with_extension(partial_suffix());
        if std::fs::write(&partial, bytes).is_ok() && std::fs::rename(&partial, &path).is_ok() {
            arrived = true;
        } else {
            let _ = std::fs::remove_file(&partial);
        }
    }
    arrived
}

/// A name for a file being written that no other write is using.
///
/// The process alone is not enough: every deck window is one process, and two
/// of them can fetch the same picture at the same moment.
fn partial_suffix() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let ours = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("part.{}.{ours}", std::process::id())
}

/// Whether any of these images still has to be fetched.
#[must_use]
pub fn missing_images(version: &str, blocks: &[Block]) -> bool {
    blocks.iter().any(|block| match block {
        Block::Image { url, .. } => {
            url.starts_with("https://")
                && image_file(version, url).is_some_and(|path| !path.exists())
        }
        _ => false,
    })
}

/// A kept image, and its size in pixels when it can be read from the file.
///
/// Asked on every frame the card is drawn — sixty a second while it is
/// scrolled — so the size is read once, from the header alone, and
/// remembered. Reading the whole file each time is what made scrolling the
/// card lag.
#[must_use]
pub fn image(version: &str, url: &str) -> Option<(std::path::PathBuf, Option<(u32, u32)>)> {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    type Known = HashMap<std::path::PathBuf, Option<(u32, u32)>>;
    static SIZES: LazyLock<Mutex<Known>> = LazyLock::new(Mutex::default);

    let path = image_file(version, url)?;
    if let Some(size) = SIZES
        .lock()
        .ok()
        .and_then(|sizes| sizes.get(&path).copied())
    {
        return Some((path, size));
    }
    if !path.exists() {
        return None;
    }
    let size = header(&path).and_then(|bytes| pixel_size(&bytes));
    if let Ok(mut sizes) = SIZES.lock() {
        sizes.insert(path.clone(), size);
    }
    Some((path, size))
}

/// The start of a file: enough for an image to say how big it is. A PNG or a
/// GIF says so in its first few bytes; a JPEG can say so a little further in.
fn header(path: &std::path::Path) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(64 * 1024)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

/// An image's width and height in pixels, whichever of the usual kinds it is.
///
/// By its bytes, not its name: a picture uploaded to GitHub has an address
/// with no extension, and is kept as `.png` whatever it really is.
fn pixel_size(bytes: &[u8]) -> Option<(u32, u32)> {
    png_size(bytes)
        .or_else(|| gif_size(bytes))
        .or_else(|| jpeg_size(bytes))
}

/// A GIF's width and height, from its header.
fn gif_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 10 || !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return None;
    }
    let width = u16::from_le_bytes([bytes[6], bytes[7]]);
    let height = u16::from_le_bytes([bytes[8], bytes[9]]);
    Some((u32::from(width), u32::from(height)))
}

/// A JPEG's width and height, from the first frame header it reaches.
fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut at = 2;
    while at + 9 < bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        // A start-of-frame marker, which holds the size. The others in that
        // range — C4, C8 and CC — are tables, not frames.
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let height = u16::from_be_bytes([bytes[at + 5], bytes[at + 6]]);
            let width = u16::from_be_bytes([bytes[at + 7], bytes[at + 8]]);
            return Some((u32::from(width), u32::from(height)));
        }
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        at += 2 + length;
    }
    None
}

/// Where an image from `url` is kept. Named by a hash of the address, which
/// stays put across runs, with the extension the address had.
fn image_file(version: &str, url: &str) -> Option<std::path::PathBuf> {
    let notes = store(version)?;
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|name| name.split(['?', '#']).next())
        .and_then(|name| {
            name.rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
        })
        .filter(|ext| matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp"))
        .unwrap_or_else(|| "png".into());
    // FNV-1a: tiny, and the same answer in every build, unlike the standard
    // hasher, which would fetch every image again after an upgrade.
    let hash = url.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    Some(notes.with_extension("").join(format!("{hash:016x}.{ext}")))
}

/// A PNG's width and height, from its header.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || !bytes.starts_with(SIGNATURE) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
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

/// Whether this window should open on the notes. True for one window only,
/// and only then is the new version written down as seen.
///
/// Not at startup: deck often starts with no window at all — the pill, or a
/// command that quits — and an announcement marked seen there would be gone
/// before anybody could see it.
#[must_use]
pub fn announcing() -> bool {
    let due = ANNOUNCE.swap(false, std::sync::atomic::Ordering::Relaxed);
    if due {
        see();
        crate::usage::record_soon(crate::usage::Count::Upgrade);
    }
    due
}

/// Whether this is the first launch since deck was upgraded.
///
/// A fresh install is not an upgrade: there is nothing it is newer than, and
/// a modal before the reader has done anything would be in the way. Going
/// back to an older version is not one either. Both of those are written down
/// at once; an upgrade waits for [`announcing`].
#[must_use]
fn first_since_upgrade() -> bool {
    let before = seen_file().and_then(|path| std::fs::read_to_string(path).ok());
    let before = before.as_deref().map(str::trim);
    let upgrade = upgraded(before, RUNNING);
    if !upgrade && before != Some(RUNNING) {
        see();
    }
    upgrade
}

fn seen_file() -> Option<std::path::PathBuf> {
    deck_core::home::deck().map(|deck| deck.join("seen"))
}

/// Write down that this version's notes have had their moment.
fn see() {
    let Some(path) = seen_file() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, RUNNING);
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
    let mut comment = false;

    let flush = |para: &mut Vec<&str>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Para(plain(&para.join(" "))));
            para.clear();
        }
    };

    for line in markdown.lines() {
        let trimmed = line.trim();
        // HTML is for the release page. A comment — often several lines, in a
        // release template — and a line that is nothing but a tag, like
        // `<details>`, would otherwise be drawn as words.
        if comment {
            comment = !trimmed.contains("-->");
            continue;
        }
        if fence.is_none() && trimmed.starts_with("<!--") {
            flush(&mut para, &mut out);
            comment = !trimmed.contains("-->");
            continue;
        }
        if fence.is_none()
            && let Some(picture) = picture(trimmed)
        {
            flush(&mut para, &mut out);
            out.push(picture);
            continue;
        }
        if fence.is_none() && tagged(trimmed) {
            flush(&mut para, &mut out);
            let words = untagged(trimmed);
            if !words.is_empty() {
                out.push(Block::Para(plain(&words)));
            }
            continue;
        }
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
        } else if trimmed.is_empty() || is_rule(trimmed) {
            flush(&mut para, &mut out);
        } else if let Some(heading) = heading(trimmed) {
            flush(&mut para, &mut out);
            out.push(Block::Heading(unmarked(&plain(heading))));
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
        Block::Image { url, .. } => !url.is_empty(),
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

/// A heading's words without their inline marks.
///
/// A heading is set in capitals in the accent, which a code chip or emphasis
/// inside it would only interrupt — so the marks go and the words stay,
/// rather than the backticks being drawn as backticks.
fn unmarked(text: &str) -> String {
    text.chars().filter(|ch| !matches!(ch, '`' | '*')).collect()
}

/// `[words](address)` at the start of `text`: the words, and what follows.
fn link(text: &str) -> Option<(&str, &str)> {
    let close = text.find("](")?;
    let words = &text[1..close];
    if words.contains('[') {
        return None;
    }
    let after = &text[close + 2..];
    // The address can hold brackets of its own — a Wikipedia page often does —
    // so it ends at the `)` that balances, not the first one.
    let mut depth = 0usize;
    let end = after.char_indices().find_map(|(at, ch)| match ch {
        '(' => {
            depth += 1;
            None
        }
        ')' if depth == 0 => Some(at),
        ')' => {
            depth -= 1;
            None
        }
        _ => None,
    })?;
    Some((words, &after[end + 1..]))
}

/// An image on a line of its own: `![alt](url)` or `<img src="…">`.
fn picture(line: &str) -> Option<Block> {
    if let Some(rest) = line.strip_prefix('!')
        && let Some((alt, after)) = link(rest)
        && after.trim().is_empty()
    {
        let open = rest.find("](")? + 2;
        let close = open + rest[open..].rfind(')')?;
        // `![alt](url "title")` and `![alt](<url>)` are both markdown; the
        // address is the first word, without its brackets.
        let address = rest[open..close].split_whitespace().next()?;
        let address = address.trim_start_matches('<').trim_end_matches('>');
        return Some(Block::Image {
            url: address.to_string(),
            alt: alt.to_string(),
            width: None,
        });
    }
    let tag = line.strip_prefix("<img")?;
    if !tag.trim_end().ends_with('>') {
        return None;
    }
    Some(Block::Image {
        url: attribute(tag, "src")?,
        alt: attribute(tag, "alt").unwrap_or_default(),
        width: attribute(tag, "width").and_then(|width| width.trim_end_matches("px").parse().ok()),
    })
}

/// The value of `name="…"` in a tag.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let mut rest = tag;
    while let Some(at) = rest.find(name) {
        let before = rest[..at].chars().last();
        let after = &rest[at + name.len()..];
        if before.is_some_and(char::is_whitespace)
            && let Some(value) = after.trim_start().strip_prefix('=')
        {
            let value = value.trim_start();
            let quote = value
                .chars()
                .next()
                .filter(|ch| *ch == '"' || *ch == '\'')?;
            let value = &value[1..];
            return Some(value[..value.find(quote)?].to_string());
        }
        rest = after;
    }
    None
}

/// A line of HTML: it opens with a tag and closes with one, like
/// `<details>` or `<summary>More</summary>`.
fn tagged(line: &str) -> bool {
    line.len() > 2
        && line.starts_with('<')
        && line.ends_with('>')
        && line[1..].starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '/')
}

/// The words of a line of HTML, without its tags.
fn untagged(line: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for ch in line.chars() {
        match ch {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(ch),
            _ => {}
        }
    }
    out.trim().to_string()
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
        assert_eq!(
            plain("[spec](https://en.wikipedia.org/wiki/Foo_(bar)) here"),
            "spec here",
            "an address with brackets of its own"
        );
    }

    #[test]
    fn html_is_left_to_the_release_page() {
        let notes = "Intro.\n\n<!-- template:\nfill this in\n-->\n\n\
            <details>\n<summary>More</summary>\n\nHidden words.\n\n</details>\n\
            <!-- one line -->\nAfter.";
        assert_eq!(
            blocks(notes),
            vec![
                Block::Para("Intro.".into()),
                Block::Para("More".into()),
                Block::Para("Hidden words.".into()),
                Block::Para("After.".into()),
            ]
        );
        // Something that only looks like a tag is still prose.
        assert_eq!(
            blocks("<3 this release"),
            vec![Block::Para("<3 this release".into())]
        );
    }

    #[test]
    fn a_hash_without_a_space_is_not_a_heading() {
        assert_eq!(
            blocks("#14 is fixed"),
            vec![Block::Para("#14 is fixed".into())]
        );
        assert_eq!(blocks("### Small"), vec![Block::Heading("Small".into())]);
        assert_eq!(
            blocks("## Fix `deck open` **now**"),
            vec![Block::Heading("Fix deck open now".into())],
            "a heading's marks go, its words stay"
        );
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

    #[test]
    fn a_picture_on_its_own_line_is_kept() {
        let notes = "Words.\n\n![Before and after](https://x.y/ba.png)\n\n\
            <img src=\"https://x.y/quit.png\" width=\"520\" alt=\"The quit card\">\n\n\
            See ![inline](https://x.y/i.png) here.";
        assert_eq!(
            blocks(notes),
            vec![
                Block::Para("Words.".into()),
                Block::Image {
                    url: "https://x.y/ba.png".into(),
                    alt: "Before and after".into(),
                    width: None
                },
                Block::Image {
                    url: "https://x.y/quit.png".into(),
                    alt: "The quit card".into(),
                    width: Some(520)
                },
                Block::Para("See here.".into()),
            ]
        );
    }

    #[test]
    fn a_kept_image_is_named_by_its_address() {
        if deck_core::home::deck().is_none() {
            return;
        }
        let a = image_file("0.1.3", "https://x.y/a.png?raw=1").expect("a path");
        let b = image_file("0.1.3", "https://x.y/b.jpg").expect("a path");
        assert_ne!(a, b);
        assert_eq!(a.extension().and_then(|it| it.to_str()), Some("png"));
        assert_eq!(b.extension().and_then(|it| it.to_str()), Some("jpg"));
        assert_eq!(
            a,
            image_file("0.1.3", "https://x.y/a.png?raw=1").expect("again")
        );
        assert!(a.parent().is_some_and(|dir| dir.ends_with("notes/0.1.3")));
        assert!(image_file("../x", "https://x.y/a.png").is_none());
    }

    #[test]
    fn a_png_says_how_big_it_is() {
        let mut header = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
        header.extend_from_slice(&1040u32.to_be_bytes());
        header.extend_from_slice(&880u32.to_be_bytes());
        assert_eq!(png_size(&header), Some((1040, 880)));
        assert_eq!(png_size(b"GIF89a"), None);
    }

    #[test]
    fn a_gif_or_a_jpeg_says_how_big_it_is_too() {
        let gif = b"GIF89a\x40\x00\x20\x00";
        assert_eq!(pixel_size(gif), Some((64, 32)));

        // Start of image, one table segment of four bytes, then a frame.
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x01,
            0x2C, 0x01, 0x90, 0x03,
        ];
        assert_eq!(pixel_size(&jpeg), Some((400, 300)));
        assert_eq!(pixel_size(b"not an image"), None);
    }

    #[test]
    fn a_title_is_not_part_of_the_address() {
        assert_eq!(
            blocks(r#"![shot](https://x.y/a.png "The window")"#),
            vec![Block::Image {
                url: "https://x.y/a.png".into(),
                alt: "shot".into(),
                width: None
            }]
        );
        assert_eq!(
            blocks("![shot](<https://x.y/b.png>)"),
            vec![Block::Image {
                url: "https://x.y/b.png".into(),
                alt: "shot".into(),
                width: None
            }]
        );
    }
}
