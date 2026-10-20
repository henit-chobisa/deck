//! What a page answers to.
//!
//! A page is markup the agent wrote, and a point reaches it by name. Which
//! names it answers to is read straight out of that markup — a scan, not a
//! parse, because the answer is needed before anything has rendered the page
//! and by readers that never will: the command line checking a group on its
//! way in, and an export with no window at all.

/// The names a page answers to: its ids, whatever it declares in
/// `deck-points`, and every name its elements light or show on —
/// `data-on`, `data-show`, `data-from`.
///
/// Shared by the window, which needs the answer before a page has a view to
/// ask, by the command line, which warns about a point no page answers to, and
/// by [`crate::prose::resolve_refs`] — so none of them can disagree about which
/// page a point is in.
#[must_use]
pub fn answered_by(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(at) = html.find("name=\"deck-points\"")
        && let Some(from) = html[at..].find("content=\"")
        && let Some(end) = html[at + from + 9..].find('"')
    {
        let start = at + from + 9;
        out.extend(
            html[start..start + end]
                .split_whitespace()
                .map(str::to_string),
        );
    }
    for attribute in ["id=\"", "data-on=\"", "data-show=\"", "data-from=\""] {
        let mut rest = html;
        while let Some(at) = rest.find(attribute) {
            // `data-id="…"` is not an id, nor `grid="…"` one of these.
            let whole =
                at == 0 || !rest[..at].ends_with(|ch: char| ch.is_alphanumeric() || ch == '-');
            rest = &rest[at + attribute.len()..];
            let Some(end) = rest.find('"') else {
                break;
            };
            if whole {
                out.extend(rest[..end].split_whitespace().map(str::to_string));
            }
            rest = &rest[end + 1..];
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_answers_to_what_its_parts_light_on() {
        let page = r#"<meta name="deck-points" content="rest hop">
            <div id="box" class="node" data-on="hop settle">a</div>
            <div data-show="arrive" data-from="leave">b</div>
            <div data-id="not-a-name" grid="nor-this">c</div>"#;
        let names = answered_by(page);
        for name in ["rest", "hop", "box", "settle", "arrive", "leave"] {
            assert!(names.contains(&name.to_string()), "{name}");
        }
        assert!(!names.contains(&"not-a-name".to_string()));
        assert!(!names.contains(&"nor-this".to_string()));
    }
}
