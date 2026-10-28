"""The site's shell, in one place: the head lines every inner page shares, the nav and the footer.

build.py writes them into the generated pages. The hand-written pages (docs, changelog, telemetry,
works-with, 404) carry the same markup between <!-- shell:NAME --> markers, and

    python3 site/tools/shell.py

rewrites those blocks from here, so no page keeps its own copy. Run it after changing the shell;
the site workflow's check fails when a page's blocks differ from this file.
"""

import pathlib
import re
import sys

MARK = (
    '<svg viewBox="0 0 128 128" aria-hidden="true"><rect x="1.5" y="1.5" width="125" height="125" rx="35.6" '
    'fill="#2f2f2f" stroke="#4a4542" stroke-width="3"/><rect x="35.5" y="39.1" width="56.9" height="14.2" rx="7.1" '
    'fill="#7c6f64"/><rect x="35.5" y="74.6" width="56.9" height="14.2" rx="7.1" fill="#fe8019"/></svg>'
)
GITHUB = (
    '<svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 '
    '6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28'
    '-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64'
    '-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 '
    '1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 '
    '1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>'
)
REPO = "https://github.com/henit-chobisa/deck"

FONTS = (
    '<link rel="preconnect" href="https://fonts.googleapis.com">\n'
    '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>\n'
    '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600'
    '&amp;family=IBM+Plex+Sans:wght@400;500;600&amp;family=JetBrains+Mono:wght@400;500&amp;display=swap">\n'
    '<link rel="stylesheet" href="/css/site.css">'
)


def nav(current: str = "") -> str:
    """current: '/docs/' or '/changelog/' marks that link as the page you are on."""
    def link(href, text):
        cur = ' aria-current="page"' if href == current else ""
        return f'<a href="{href}"{cur}>{text}</a>'
    return (
        '<header class="nav" id="nav"><div class="wrap">\n'
        f' <a class="brand" href="/" aria-label="deck, home">{MARK}deck</a>\n'
        f' <nav class="links" aria-label="Main">{link("/docs/", "Docs")}{link("/changelog/", "Changelog")}'
        f'<a class="gh" href="{REPO}" aria-label="deck on GitHub">{GITHUB}</a><a class="cta" href="/#install">Install</a></nav>\n'
        '</div></header>'
    )


FOOT = (
    '<footer class="foot">\n'
    ' <div class="foot-fin">\n'
    '  <div class="foot-left">\n'
    f'   <a class="foot-brand" href="/" aria-label="deck, home">{MARK}deck<small>Make your agent answer for your understanding.</small></a>\n'
    '  </div>\n'
    '  <nav class="foot-cols" aria-label="Footer">\n'
    '   <div><p class="foot-ch">Product</p><a href="/#install">Install</a><a href="/docs/">Docs</a><a href="/guides/">Guides</a>'
    f'<a href="/changelog/">Changelog</a><a href="{REPO}/releases">Releases</a><a href="/telemetry/">Telemetry</a></div>\n'
    f'   <div><p class="foot-ch">Open source</p><a href="{REPO}">GitHub</a><a href="{REPO}/blob/main/LICENSE">License · Apache-2.0</a>'
    f'<a href="{REPO}/blob/main/README.md">README</a></div>\n'
    '   <div><p class="foot-ch">Henit</p><a href="https://henit.blog">Blog</a><a href="/#note">Why I\'m building it</a>'
    '<button type="button" class="foot-cl" data-email="chobisa.henit@gmail.com">Email<span class="foot-cs" aria-live="polite"></span></button></div>\n'
    '  </nav>\n'
    '  <div class="foot-base"><span>© 2026 Henit Chobisa · Apache-2.0 · macOS, Linux and Windows</span></div>\n'
    ' </div>\n'
    '</footer>\n'
    '<script src="/js/site.js" defer></script>'
)

BLOCK = re.compile(r"<!-- shell:(head|nav|foot)( current=\"[^\"]*\")? -->.*?<!-- /shell:\1 -->", re.S)


def stamp(html: str) -> str:
    def fill(m):
        name, cur = m.group(1), (m.group(2) or "")
        current = cur[len(' current="'):-1] if cur else ""
        body = {"head": FONTS, "nav": nav(current), "foot": FOOT}[name]
        return f"<!-- shell:{name}{cur} -->\n{body}\n<!-- /shell:{name} -->"
    return BLOCK.sub(fill, html)


def main(root: str, check: bool) -> None:
    stale = []
    for page in sorted(pathlib.Path(root).rglob("*.html")):
        html = page.read_text(encoding="utf-8")
        if "<!-- shell:" not in html:
            continue
        new = stamp(html)
        if new != html:
            stale.append(str(page))
            if not check:
                page.write_text(new, encoding="utf-8")
    if check and stale:
        sys.exit("the shell differs from tools/shell.py in: " + ", ".join(stale))
    print(("stale: " if check else "stamped: ") + str(len(stale)))


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--check"]
    main(args[0] if args else str(pathlib.Path(__file__).resolve().parent.parent), "--check" in sys.argv)
