//! Where an export's links go, outside it.
//!
//! Two kinds. "Powered by Deck" goes to deck's site, so a reader who has
//! never heard of it can find out what made the page. And a code pane's
//! header goes to its lines in the repo, at the commit the deck was packed
//! at, so a reader can see them in place — but only where that is honest:
//! a host whose links deck knows how to write, a commit to write them at,
//! and a file that was clean against that commit. A file that was modified
//! or not in the repo at all shows lines that are at no commit, and a link
//! to the commit would show the reader something else under the same name.

use deck_core::bundle::Repo;
use deck_core::bundle::snap::State;
use deck_core::document::Code;

/// Where "Powered by Deck" goes.
pub const DECK: &str = "https://trydeck.dev/?ref=export";

/// A host deck can write a link to lines for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Host {
    GitHub,
    GitLab,
}

/// The host and the project path of `remote`, as `acme/ingest`: from an
/// https, http, ssh or scp-style remote, its credentials and `.git` gone.
fn project(remote: &str) -> Option<(Host, String)> {
    let remote = remote.trim().trim_end_matches('/');
    let remote = remote.strip_suffix(".git").unwrap_or(remote);
    let (host, path) = if let Some((_, rest)) = remote.split_once("://") {
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit('@').next()?;
        // A port is not part of which host this is.
        (host.split(':').next()?, path)
    } else {
        // `git@github.com:acme/ingest`.
        let (authority, path) = remote.split_once(':')?;
        (authority.rsplit('@').next()?, path)
    };
    let host = match host.to_ascii_lowercase().as_str() {
        "github.com" | "www.github.com" => Host::GitHub,
        "gitlab.com" | "www.gitlab.com" => Host::GitLab,
        _ => return None,
    };
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    // GitHub is always an owner and a repo; GitLab can nest groups.
    let fits = match host {
        Host::GitHub => parts.len() == 2,
        Host::GitLab => parts.len() >= 2,
    };
    fits.then(|| (host, parts.join("/")))
}

/// `segment` with everything but the unreserved characters percent-encoded.
fn encoded(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The lines `code` shows, at the deck's commit on its host, when deck can
/// say honestly where they are.
#[must_use]
pub fn source(repo: Option<&Repo>, code: &Code) -> Option<String> {
    if code.missing || code.state != Some(State::Clean) {
        return None;
    }
    let repo = repo?;
    let commit = repo
        .commit
        .as_deref()
        .filter(|commit| !commit.is_empty() && commit.bytes().all(|b| b.is_ascii_hexdigit()))?;
    let (host, project) = project(repo.remote.as_deref()?)?;
    let path = code
        .file
        .split('/')
        .map(encoded)
        .collect::<Vec<_>>()
        .join("/");
    let (first, last) = (code.range.first, code.range.last);
    Some(match host {
        Host::GitHub if first == last => {
            format!("https://github.com/{project}/blob/{commit}/{path}#L{first}")
        }
        Host::GitHub => {
            format!("https://github.com/{project}/blob/{commit}/{path}#L{first}-L{last}")
        }
        Host::GitLab if first == last => {
            format!("https://gitlab.com/{project}/-/blob/{commit}/{path}#L{first}")
        }
        Host::GitLab => {
            format!("https://gitlab.com/{project}/-/blob/{commit}/{path}#L{first}-{last}")
        }
    })
}

#[cfg(test)]
mod tests {
    use deck_core::line::LineRange;

    use super::*;

    fn code(file: &str, state: Option<State>) -> Code {
        Code {
            id: "g1r1".to_string(),
            name: None,
            note: None,
            file: file.to_string(),
            range: LineRange::new(139, 146),
            change: None,
            lines: Vec::new(),
            missing: false,
            state,
        }
    }

    fn repo(remote: &str, commit: Option<&str>) -> Repo {
        Repo {
            remote: Some(remote.to_string()),
            commit: commit.map(str::to_string),
        }
    }

    const SHA: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";

    #[test]
    fn github_and_gitlab_link_to_the_lines_at_the_commit() {
        let clean = code("src/batch.ts", Some(State::Clean));
        for remote in [
            "https://github.com/acme/ingest.git",
            "git@github.com:acme/ingest.git",
            "ssh://git@github.com/acme/ingest",
            "http://github.com/acme/ingest/",
        ] {
            assert_eq!(
                source(Some(&repo(remote, Some(SHA))), &clean).as_deref(),
                Some(
                    format!("https://github.com/acme/ingest/blob/{SHA}/src/batch.ts#L139-L146")
                        .as_str()
                ),
                "{remote}"
            );
        }
        assert_eq!(
            source(
                Some(&repo("git@gitlab.com:acme/tools/ingest.git", Some(SHA))),
                &clean
            )
            .as_deref(),
            Some(
                format!("https://gitlab.com/acme/tools/ingest/-/blob/{SHA}/src/batch.ts#L139-146")
                    .as_str()
            )
        );
    }

    #[test]
    fn another_host_a_dirty_file_or_no_commit_has_no_link() {
        let clean = code("src/batch.ts", Some(State::Clean));
        assert_eq!(
            source(
                Some(&repo("https://git.example.com/acme/ingest", Some(SHA))),
                &clean
            ),
            None
        );
        for state in [Some(State::Modified), Some(State::Untracked), None] {
            let file = code("src/batch.ts", state);
            assert_eq!(
                source(
                    Some(&repo("https://github.com/acme/ingest", Some(SHA))),
                    &file
                ),
                None
            );
        }
        assert_eq!(
            source(Some(&repo("https://github.com/acme/ingest", None)), &clean),
            None
        );
        assert_eq!(source(None, &clean), None);
    }

    #[test]
    fn each_part_of_the_path_is_encoded() {
        let file = code("src/a dir/#weird?.ts", Some(State::Clean));
        let link = source(
            Some(&repo("https://github.com/acme/ingest", Some(SHA))),
            &file,
        )
        .expect("a link");
        assert!(
            link.ends_with("/src/a%20dir/%23weird%3F.ts#L139-L146"),
            "{link}"
        );
    }
}
