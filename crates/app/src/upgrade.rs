//! Replace deck with a newer deck.
//!
//! # Why this can swap a binary that is running
//!
//! Overwriting a running executable on macOS kills it. `cp` truncates and
//! rewrites the *same* inode, so the pages behind the executing image stop
//! matching the signature they were loaded under and the kernel takes the
//! process out — SIGKILL, no message, nothing in the log that says why.
//!
//! `rename` does not do that. It swaps a directory entry; the running process
//! holds the inode and carries on. So the new deck goes into place beside the
//! old one and is moved over it, and whatever windows are open are untouched.
//! The next deck anybody opens is the new one.
//!
//! # Why there is no notarisation here
//!
//! The binary is signed ad-hoc — `codesign -s -`, no identity, no Apple
//! Developer account. On Apple Silicon an executable has to carry *some*
//! signature to run at all, and ad-hoc satisfies that. What it does not satisfy
//! is Gatekeeper's quarantine — which is set by browsers, and not by this.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};

use crate::setup::{accent, bold, dim, mark};

/// How long to wait on the network before deciding there is not one.
///
/// Short on purpose. Somewhere with no route out should find that out quickly
/// rather than watching deck think about it.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(10);

/// Where the releases are.
const RELEASES: &str = "https://api.github.com/repos/henit-chobisa/deck/releases";

/// What this build is.
const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// A release, as much of one as this needs.
struct Release {
    tag: String,
    prerelease: bool,
    tarball: Option<String>,
    sums: Option<String>,
}

/// Fetch, verify and swap.
///
/// `unstable` is the answer to *do you want prereleases too*, or `None` to ask.
pub fn run(unstable: Option<bool>) -> anyhow::Result<()> {
    // The same opening `deck setup` wears. A command that prints in its own
    // style reads as a different program, and this one did.
    println!();
    println!("  {}  {}", mark(), bold("deck upgrade"));
    println!();

    let here = installed()?;
    if let Some(owner) = managed(&here) {
        bail!(
            "deck was installed by {owner}, so {owner} should be the one to \
             replace it — run `{owner} upgrade deck`"
        );
    }

    let unstable = match unstable {
        Some(yes) => yes,
        None => ask_unstable()?,
    };

    println!("  {}", dim("looking"));
    let releases = releases()?;
    let Some(release) = pick(&releases, unstable) else {
        bail!("no release has a binary attached to it yet");
    };

    let there = release.tag.trim_start_matches('v');
    if !newer(there, RUNNING) {
        println!(
            "  {} {}",
            accent("✓"),
            dim(&format!("deck {RUNNING} is the newest there is"))
        );
        println!();
        return Ok(());
    }

    println!("  {}", dim(&format!("deck {there}, and this is {RUNNING}")));
    swap(&here, release)?;

    println!("  {} deck {}", accent("✓"), bold(there));
    println!("  {}", dim("Open a deck and it will be the new one."));
    println!();
    println!(
        "  {}",
        dim(&format!(
            "What changed: https://github.com/henit-chobisa/deck/releases/tag/{}",
            release.tag
        ))
    );
    println!();
    Ok(())
}

/// Where deck is, with every symlink followed.
fn installed() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe().context("deck cannot find its own binary")?;
    Ok(exe.canonicalize().unwrap_or(exe))
}

/// The package manager that owns this copy, if one does.
///
/// Replacing a file Homebrew put there wins until the next `brew upgrade`
/// disagrees, and then the two of them take turns. Better to say so and stop.
fn managed(exe: &Path) -> Option<&'static str> {
    let path = exe.to_string_lossy();
    (path.contains("/Cellar/") || path.contains("/homebrew/")).then_some("brew")
}

/// Ask whether prereleases count.
fn ask_unstable() -> anyhow::Result<bool> {
    use std::io::IsTerminal as _;

    // Nobody to ask. Stable is the answer that cannot surprise somebody.
    if !std::io::stdin().is_terminal() {
        return Ok(false);
    }
    print!("  include prereleases? [no] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

/// Every release, newest first.
fn releases() -> anyhow::Result<Vec<Release>> {
    let body: serde_json::Value = ureq::get(RELEASES)
        .config()
        .timeout_global(Some(PATIENCE))
        .build()
        // GitHub refuses a request with no user agent, and says so in a way
        // that reads like the repository is missing.
        .header("User-Agent", concat!("deck/", env!("CARGO_PKG_VERSION")))
        .call()
        .context("could not reach GitHub — if there is no network, there is nothing to do")?
        .body_mut()
        .read_json()
        .context("GitHub answered with something that is not a release list")?;

    Ok(body
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|it| {
            let find = |suffix: &str| {
                it["assets"].as_array()?.iter().find_map(|asset| {
                    let name = asset["name"].as_str()?;
                    name.ends_with(suffix)
                        .then(|| asset["browser_download_url"].as_str())?
                        .map(ToString::to_string)
                })
            };
            Some(Release {
                tag: it["tag_name"].as_str()?.to_string(),
                prerelease: it["prerelease"].as_bool().unwrap_or(false),
                tarball: find(".tar.gz"),
                sums: find(".sha256"),
            })
        })
        .collect())
}

/// Whether `candidate` is strictly newer than `running`.
///
/// By version rather than by difference. Asking only whether the two strings
/// disagree calls a downgrade an upgrade: somebody who built 0.1.4 from source
/// would be handed 0.1.3 because that is the newest release with a binary on
/// it, and told it was an improvement.
///
/// Anything that will not parse is not newer. A tag nobody can order is not a
/// thing to replace a working deck with.
pub(crate) fn newer(candidate: &str, running: &str) -> bool {
    use semver::Version;

    match (Version::parse(candidate), Version::parse(running)) {
        (Ok(there), Ok(here)) => there > here,
        _ => false,
    }
}

/// The newest one worth installing.
///
/// A release with no binary attached is not an upgrade anybody can take, which
/// is every release before this was written.
fn pick(releases: &[Release], unstable: bool) -> Option<&Release> {
    releases
        .iter()
        .filter(|it| it.tarball.is_some())
        .find(|it| unstable || !it.prerelease)
}

/// Put the new one where the old one is.
///
/// Everything happens beside the installed binary rather than in a temporary
/// directory, because `rename` is only atomic within one filesystem and
/// `/tmp` is not guaranteed to be the same one.
fn swap(here: &Path, release: &Release) -> anyhow::Result<()> {
    let (Some(tarball), Some(sums)) = (&release.tarball, &release.sums) else {
        bail!("{} has no binary attached to it", release.tag);
    };
    let beside = here
        .parent()
        .context("deck is not in a directory, which should not be possible")?;
    writable(beside)?;

    let archive = carry(tarball)?;
    let wanted = fetch(sums)?;
    checked(&archive, &wanted)?;

    // Exclusive, and the lock as well as the workspace. `create_dir` fails if
    // something is already there, so two upgrades running at once cannot
    // delete each other's staged binary — which could otherwise install a file
    // the other one had not finished checking.
    //
    // A directory left behind by a crash has to be cleared by hand. That is
    // the right trade: the alternative is removing one that a live upgrade is
    // halfway through using.
    let staging = beside.join(".deck-upgrade");
    std::fs::create_dir(&staging).with_context(|| {
        format!(
            "{} already exists — another upgrade is running, or one stopped \
             partway and left it behind",
            staging.display()
        )
    })?;
    let tidy = Tidy(staging.clone());

    let holding = staging.join("deck.tar.gz");
    std::fs::write(&holding, &archive).context("could not write the download")?;
    run_it(
        "tar",
        &[
            "-xzf",
            &holding.to_string_lossy(),
            "-C",
            &staging.to_string_lossy(),
        ],
    )?;

    let fresh = staging.join("deck");
    if !fresh.exists() {
        bail!("the archive did not contain a deck");
    }

    // Ad-hoc, because an unsigned binary does not run on Apple Silicon at all,
    // and because extracting it is the step that drops whatever signature it
    // arrived with.
    run_it(
        "codesign",
        &["--force", "--sign", "-", &fresh.to_string_lossy()],
    )?;

    // The last check before it becomes the deck everybody gets: it has to run.
    // A release that cannot start its own binary should go no further than
    // this directory.
    let said = std::process::Command::new(&fresh)
        .arg("--version")
        .output()
        .context("the new deck would not start")?;
    if !said.status.success() {
        bail!("the new deck would not start, so it is not going in");
    }

    // The one it replaces, kept — as a hard link rather than a move.
    //
    // Moving it first leaves nothing at the install path until the second
    // rename lands, and anything that goes wrong in that gap leaves the
    // machine with no deck at all: a failed rename, a full disk, a signal. A
    // link gives the old binary a second name while the first one still works,
    // so the only moment anything changes is the rename, which is atomic.
    //
    // And the backup is checked. Replacing a deck without keeping the one it
    // replaced is the case somebody needs most when a release is bad and they
    // have no network left to fetch the old one again.
    let previous = beside.join("deck.prev");
    let _ = std::fs::remove_file(&previous);
    std::fs::hard_link(here, &previous)
        .context("could not keep a copy of the deck being replaced")?;

    std::fs::rename(&fresh, here).context("could not put the new deck in place")?;
    drop(tidy);
    Ok(())
}

/// Take the staging directory away whatever happens.
struct Tidy(PathBuf);

impl Drop for Tidy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Fail early and clearly rather than halfway through a download.
fn writable(dir: &Path) -> anyhow::Result<()> {
    let probe = dir.join(".deck-writable");
    std::fs::write(&probe, b"").with_context(|| {
        format!(
            "{} is not writable, so deck cannot replace itself there",
            dir.display()
        )
    })?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// Get the bytes at a url.
fn fetch(url: &str) -> anyhow::Result<Vec<u8>> {
    let mut body = ureq::get(url)
        .config()
        .timeout_global(Some(PATIENCE))
        .build()
        .header("User-Agent", concat!("deck/", env!("CARGO_PKG_VERSION")))
        .call()
        .with_context(|| format!("could not download {url}"))?;
    body.body_mut()
        .with_config()
        .limit(256 * 1024 * 1024)
        .read_to_vec()
        .with_context(|| format!("the download of {url} stopped early"))
}

/// Get the bytes, and show them arriving.
///
/// Twenty-five megabytes behind the word *downloading* is a long silence, and a
/// silence is indistinguishable from a hang. This reads the body in pieces and
/// draws how far along it is.
fn carry(url: &str) -> anyhow::Result<Vec<u8>> {
    use std::io::{IsTerminal as _, Read as _, Write as _};

    let mut body = ureq::get(url)
        .config()
        .timeout_global(Some(PATIENCE))
        .build()
        .header("User-Agent", concat!("deck/", env!("CARGO_PKG_VERSION")))
        .call()
        .with_context(|| format!("could not download {url}"))?;

    let whole = body.body().content_length();
    let mut reader = body.body_mut().as_reader();
    let mut got: Vec<u8> =
        Vec::with_capacity(usize::try_from(whole.unwrap_or(0)).unwrap_or(0).min(LIMIT));
    let mut chunk = [0u8; 64 * 1024];

    // A terminal, not a colour terminal. Somebody with `NO_COLOR` set still
    // wants to see the download move; gating on colour put the silence back
    // for exactly them.
    let live = std::io::stdout().is_terminal();
    let mut drawn = false;
    // Redrawn on a clock rather than on every chunk: sixty-four kilobytes at a
    // time is hundreds of writes a second, and a bar nobody can read flickering
    // is worse than no bar.
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);

    // Whatever goes wrong mid-download, the next thing printed starts on its
    // own line. Without this the error was written onto the end of the bar.
    let off_the_bar = |drawn: bool| {
        if drawn {
            println!();
        }
    };

    loop {
        let read = match reader.read(&mut chunk) {
            Ok(read) => read,
            Err(err) => {
                off_the_bar(drawn);
                return Err(err).with_context(|| format!("the download of {url} stopped early"));
            }
        };
        if read == 0 {
            break;
        }
        // The cap the plain fetch has always had. A response that keeps coming
        // is not a deck, and reading it to the end is how memory runs out.
        if got.len() + read > LIMIT {
            off_the_bar(drawn);
            bail!("the download of {url} is larger than any deck should be, so it is not going in");
        }
        got.extend_from_slice(&chunk[..read]);
        if live && last.elapsed() >= std::time::Duration::from_millis(80) {
            draw(got.len() as u64, whole);
            drawn = true;
            last = std::time::Instant::now();
        }
    }

    if live {
        draw(got.len() as u64, whole);
        println!();
    }
    let _ = std::io::stdout().flush();
    Ok(got)
}

/// The most a download may be. Ten times the binary, and nowhere near a
/// machine's memory.
const LIMIT: usize = 256 * 1024 * 1024;

/// Wide enough to read as movement, narrow enough that the whole line stays
/// inside an 80 column terminal — the indent and `downloading` take 14, the bar
/// 28, and `24.0 MB of 24.0 MB` another 20, which is 62. Wider and a terminal
/// at its default size wraps the line, and a carriage return then redraws only
/// the second half of it.
const WIDE: usize = 28;

/// One frame of the bar, over the top of the last one.
fn draw(got: u64, whole: Option<u64>) {
    use std::io::Write as _;

    print!("\r  {} {}", dim("downloading"), frame(got, whole));
    let _ = std::io::stdout().flush();
}

/// How many of the cells are filled, or `None` with nothing to measure against.
///
/// On its own so it can be asserted directly. Counting glyphs in the drawn
/// frame proved nothing while filled and empty were the same glyph in two
/// colours, and a colourless test cannot see colour.
fn filled(got: u64, whole: Option<u64>) -> Option<usize> {
    let whole = whole.filter(|whole| *whole > 0)?;
    // Clamped, because a body longer than its promised length would otherwise
    // ask for more cells than the track has and panic on the subtraction.
    let along = (got as f64 / whole as f64).clamp(0., 1.);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a fraction of the track, clamped to it"
    )]
    Some((along * WIDE as f64).round() as usize)
}

/// What a frame says.
///
/// Filled and empty are different glyphs, not one glyph in two colours. With
/// `NO_COLOR` set — or piped, or read by a screen reader — colour is all that
/// would have told them apart, and a bar whose cells all look alike shows no
/// progress at all.
fn frame(got: u64, whole: Option<u64>) -> String {
    let mb = |bytes: u64| format!("{:.1} MB", bytes as f64 / 1_048_576.0);
    match (filled(got, whole), whole) {
        (Some(cells), Some(whole)) => format!(
            "{}{}  {}",
            accent(&"━".repeat(cells)),
            dim(&"─".repeat(WIDE - cells)),
            dim(&format!("{} of {}", mb(got), mb(whole)))
        ),
        // No length to measure against, so no bar to draw — say what has
        // arrived and leave it at that.
        _ => dim(&mb(got)),
    }
}

/// Refuse anything that is not byte for byte what was published.
fn checked(archive: &[u8], sums: &[u8]) -> anyhow::Result<()> {
    use sha2::{Digest as _, Sha256};

    let wanted = String::from_utf8_lossy(sums);
    let wanted = wanted
        .split_whitespace()
        .next()
        .context("the published checksum is empty")?;

    let got = Sha256::digest(archive);
    let got = got.iter().fold(String::new(), |mut all, byte| {
        use std::fmt::Write as _;
        let _ = write!(all, "{byte:02x}");
        all
    });

    if got != wanted {
        bail!("the download does not match its checksum, so it is not going in");
    }
    Ok(())
}

/// Run something, and say what it said if it fails.
fn run_it(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let done = std::process::Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("could not run {program}"))?;
    if !done.status.success() {
        bail!(
            "{program} failed: {}",
            String::from_utf8_lossy(&done.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool, binary: bool) -> Release {
        Release {
            tag: tag.into(),
            prerelease,
            tarball: binary.then(|| "https://example/deck.tar.gz".to_string()),
            sums: binary.then(|| "https://example/deck.tar.gz.sha256".to_string()),
        }
    }

    #[test]
    fn a_release_with_nothing_attached_is_not_an_upgrade() {
        // Every release before the workflow that builds one carries a demo
        // video and no binary. Offering those as an upgrade would download
        // nothing and replace deck with it.
        let only_media = [release("v0.1.2", false, false)];
        assert!(pick(&only_media, false).is_none());
        assert!(pick(&only_media, true).is_none(), "and not for prereleases");
    }

    #[test]
    fn prereleases_are_opt_in() {
        // Newest first, as GitHub returns them.
        let releases = [
            release("v0.2.0-rc.1", true, true),
            release("v0.1.3", false, true),
        ];

        assert_eq!(
            pick(&releases, false).map(|it| it.tag.as_str()),
            Some("v0.1.3"),
            "the newest stable, stepping over the newer prerelease"
        );
        assert_eq!(
            pick(&releases, true).map(|it| it.tag.as_str()),
            Some("v0.2.0-rc.1"),
            "and the prerelease when asked for"
        );
    }

    #[test]
    fn a_copy_a_package_manager_put_there_is_left_alone() {
        // Replacing it wins until the next `brew upgrade` disagrees, and then
        // the two of them take turns.
        assert_eq!(
            managed(Path::new("/opt/homebrew/Cellar/deck/0.1.2/bin/deck")),
            Some("brew")
        );
        assert_eq!(
            managed(Path::new("/usr/local/homebrew/bin/deck")),
            Some("brew")
        );
        assert_eq!(
            managed(Path::new("/Users/someone/.local/bin/deck")),
            None,
            "but one the install script put there is ours"
        );
    }

    #[test]
    fn a_download_that_does_not_match_its_checksum_is_refused() {
        // sha256 of "deck", by hand rather than by the same code being tested.
        let sum = "1a3fbd4a8d3c5e9c0e2c0b2aeb4d6b9a07f0dbd0b9e5e6a7c8d9e0f1a2b3c4d5";
        assert!(checked(b"deck", format!("{sum}  deck.tar.gz").as_bytes()).is_err());

        // And the real one round-trips, so the comparison is not simply always
        // failing.
        use sha2::{Digest as _, Sha256};
        let real = Sha256::digest(b"deck");
        let real = real.iter().fold(String::new(), |mut all, byte| {
            use std::fmt::Write as _;
            let _ = write!(all, "{byte:02x}");
            all
        });
        assert!(checked(b"deck", format!("{real}  deck.tar.gz").as_bytes()).is_ok());
    }

    /// The bar, with the colour taken out, so the shape is what is asserted.
    fn bare(got: u64, whole: Option<u64>) -> String {
        let line = frame(got, whole);
        let mut out = String::new();
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn the_bar_fills_as_the_bytes_arrive() {
        // Raised in review: the first version of this test counted 28 glyphs
        // after stripping the colour that told filled from empty, so a broken
        // fill would have passed. The count is asserted directly now.
        let whole = 28 * 1_048_576;
        assert_eq!(filled(0, Some(whole)), Some(0), "nothing yet");
        assert_eq!(filled(whole / 2, Some(whole)), Some(14), "half");
        assert_eq!(filled(whole, Some(whole)), Some(28), "all of it");

        // And the drawn frame carries that split in its glyphs, so it reads
        // with the colour gone — which is the point of using two glyphs.
        let half = bare(whole / 2, Some(whole));
        assert_eq!(half.matches('━').count(), 14, "filled: {half:?}");
        assert_eq!(half.matches('─').count(), 14, "empty: {half:?}");
        assert!(half.contains("14.0 MB of 28.0 MB"));
    }

    #[test]
    fn with_nothing_to_measure_against_there_is_no_bar() {
        assert_eq!(filled(1_048_576, None), None, "no length sent");
        assert_eq!(filled(1_048_576, Some(0)), None, "a length of nothing");

        let unknown = bare(1_048_576, None);
        assert!(
            !unknown.contains('━') && !unknown.contains('─'),
            "{unknown:?}"
        );
        assert_eq!(unknown.trim(), "1.0 MB");
    }

    #[test]
    fn a_download_longer_than_promised_does_not_overflow_the_bar() {
        // Without the clamp this asks for 280 filled cells of a 28 cell track
        // and panics on the subtraction for the empty ones.
        assert_eq!(filled(100, Some(10)), Some(28));
        let line = bare(100, Some(10));
        assert_eq!(line.matches('━').count(), 28);
        assert_eq!(line.matches('─').count(), 0);
    }

    #[test]
    fn only_a_strictly_newer_release_is_an_upgrade() {
        // Raised in review. Asking only whether the two strings disagree calls
        // a downgrade an upgrade: somebody running a 0.1.4 they built from
        // source would be handed 0.1.3, because that is the newest release
        // with a binary attached.
        assert!(!newer("0.1.3", "0.1.4"), "a downgrade is not an upgrade");
        assert!(!newer("0.1.4", "0.1.4"), "and neither is the same version");
        assert!(newer("0.1.4", "0.1.3"));
        assert!(newer("0.2.0", "0.1.9"), "minor over patch");
        assert!(newer("1.0.0", "0.9.9"), "and major over minor");

        // Prereleases order below the release they lead to, which is what
        // stops an rc replacing the thing it was an rc for.
        assert!(newer("0.1.4", "0.1.4-rc.1"), "the release beats its rc");
        assert!(!newer("0.1.4-rc.1", "0.1.4"), "and not the other way");
        assert!(newer("0.1.4-rc.2", "0.1.4-rc.1"), "rc 2 over rc 1");

        // A tag nobody can order is not a thing to replace a working deck
        // with.
        assert!(!newer("tip", "0.1.3"));
        assert!(!newer("0.1.4", "whatever this build is"));
    }

    #[test]
    fn an_empty_checksum_file_is_not_a_pass() {
        assert!(checked(b"deck", b"").is_err());
        assert!(checked(b"deck", b"   \n").is_err());
    }
}
