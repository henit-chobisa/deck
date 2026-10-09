"""Write the guides, their hub, the sitemap and robots.txt.

    python3 site/tools/build.py

Plain Python, no dependencies. The guides' text lives in guides.py; this turns
it into pages that share the site's layout, each with its own title, summary,
canonical address, social card and structured data (Article, FAQPage and
BreadcrumbList), linked to each other, to the docs and to the demo.
"""

import html
import json
import os
import re
import sys
from datetime import date

sys.path.insert(0, os.path.dirname(__file__))
from guides import GUIDES  # noqa: E402

SITE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ORIGIN = "https://trydeck.dev"
TODAY = date.today().isoformat()
AUTHOR = {"@type": "Person", "name": "Henit Chobisa", "url": "https://github.com/henit-chobisa"}


def inline(text: str) -> str:
    """The inline marks the guides use: code, bold and links."""
    out = html.escape(text, quote=False)
    out = re.sub(r"`([^`]+)`", r"<code>\1</code>", out)
    out = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", out)
    out = re.sub(r"(?<![\w*])\*([^*\n]+)\*(?![\w*])", r"<em>\1</em>", out)
    out = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", lambda m: f'<a href="{m.group(2)}">{m.group(1)}</a>', out)
    return out


def blocks(md: str) -> str:
    """Headings, paragraphs and lists: all a guide needs."""
    out, para, items, kind = [], [], [], None

    def flush():
        nonlocal para, items, kind
        if para:
            out.append("<p>" + inline(" ".join(para)) + "</p>")
            para = []
        if items:
            tag = "ol" if kind == "ol" else "ul"
            out.append(f"<{tag}>" + "".join(f"<li>{inline(i)}</li>" for i in items) + f"</{tag}>")
            items, kind = [], None

    for line in md.strip().splitlines():
        line = line.rstrip()
        if not line:
            flush()
            continue
        if line.startswith("## "):
            flush()
            text = line[3:]
            anchor = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
            out.append(f'<h2 id="{anchor}">{inline(text)}</h2>')
            continue
        m = re.match(r"^(\d+)\.\s+(.*)$", line)
        if m:
            if para:
                flush()
            kind = "ol"
            items.append(m.group(2))
            continue
        if line.startswith("- "):
            if para:
                flush()
            kind = "ul"
            items.append(line[2:])
            continue
        if items:
            flush()
        para.append(line)
    flush()
    return "\n".join(out)


def plain(md_inline: str) -> str:
    """Text without marks, for structured data."""
    return re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", md_inline).replace("**", "").replace("`", "")


HEAD = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<meta name="description" content="{description}">
<link rel="canonical" href="{url}">
<meta property="og:type" content="{og_type}">
<meta property="og:site_name" content="deck">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{description}">
<meta property="og:url" content="{url}">
<meta property="og:image" content="{origin}/assets/og.png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{title}">
<meta name="twitter:description" content="{description}">
<meta name="twitter:image" content="{origin}/assets/og.png">
<meta name="theme-color" content="#0d0f0f">
<link rel="icon" href="/assets/mark.svg" type="image/svg+xml">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
<link rel="stylesheet" href="/css/site.css">
<link rel="stylesheet" href="/css/docs.css">
<link rel="stylesheet" href="/css/guide.css">
<script type="application/ld+json">{ld}</script>
</head>
<body class="docs">
<a class="skip" href="#doc">Skip to content</a>
<header class="nav solid">
  <a class="brand" href="/" aria-label="deck, home"><img src="/assets/mark.svg" alt="" width="28" height="28"><span>deck</span></a>
  <nav aria-label="Main">
    <a href="/#agent">How it works</a>
    <a href="/#play" class="nav-try"><span class="live-dot" aria-hidden="true"></span>Try it<span class="wide"> live</span></a>
    <a href="/docs/">Docs</a>
    <a href="/guides/"{guides_current}>Guides</a>
    <a href="https://github.com/henit-chobisa/deck">GitHub</a>
    <a class="nav-install" href="/#install">Install</a>
  </nav>
</header>
"""

FOOT = """<footer class="foot">
  <div class="wrap foot-base">
    <span>deck · free and open source · Apache-2.0</span>
    <span>Made for people who still prefer quality.</span>
  </div>
</footer>
</body>
</html>
"""

CTA = """<aside class="cta" aria-label="Try deck">
  <div>
    <p class="cta-title">See your agent's decisions on the lines.</p>
    <p>deck is free and open source. It works with Claude Code, Codex, Cursor, Amp and any agent with a shell.</p>
  </div>
  <div class="cta-actions">
    <a class="star" href="/#install">Install deck</a>
    <a class="cta-alt" href="/#play">Try a real deck in your browser</a>
  </div>
</aside>"""


def guide_page(g: dict, related: list) -> str:
    url = f"{ORIGIN}/{g['slug']}/"
    faq = g.get("faq", [])
    ld = [
        {
            "@context": "https://schema.org",
            "@type": "Article",
            "headline": g["h1"],
            "description": g["description"],
            "author": AUTHOR,
            "publisher": {"@type": "Organization", "name": "deck", "url": ORIGIN, "logo": {"@type": "ImageObject", "url": f"{ORIGIN}/assets/mark.svg"}},
            "image": f"{ORIGIN}/assets/og.png",
            "mainEntityOfPage": url,
            "dateModified": TODAY,
        },
        {
            "@context": "https://schema.org",
            "@type": "BreadcrumbList",
            "itemListElement": [
                {"@type": "ListItem", "position": 1, "name": "deck", "item": f"{ORIGIN}/"},
                {"@type": "ListItem", "position": 2, "name": "Guides", "item": f"{ORIGIN}/guides/"},
                {"@type": "ListItem", "position": 3, "name": g["h1"], "item": url},
            ],
        },
    ]
    if faq:
        ld.append({
            "@context": "https://schema.org",
            "@type": "FAQPage",
            "mainEntity": [{"@type": "Question", "name": q, "acceptedAnswer": {"@type": "Answer", "text": plain(a)}} for q, a in faq],
        })
    head = HEAD.format(
        title=html.escape(g["title"]), description=html.escape(g["description"]), url=url, origin=ORIGIN,
        og_type="article", ld=json.dumps(ld, ensure_ascii=False).replace("</", "<\\/"), guides_current="",
    )
    faq_html = ""
    if faq:
        faq_html = '<section class="g-faq" aria-labelledby="faq"><h2 id="faq">Questions</h2>' + "".join(
            f"<details><summary>{html.escape(q)}</summary><p>{inline(a)}</p></details>" for q, a in faq
        ) + "</section>"
    rel = "".join(f'<li><a href="/{r["slug"]}/">{html.escape(r["h1"])}</a></li>' for r in related)
    return head + f"""<main class="g-wrap" id="doc">
  <nav class="crumbs" aria-label="Breadcrumb"><a href="/">deck</a> <span>/</span> <a href="/guides/">Guides</a></nav>
  <article class="doc g-doc">
    <p class="eyebrow accent">{html.escape(g['eyebrow'])}</p>
    <h1>{html.escape(g['h1'])}</h1>
    <p class="lede">{html.escape(g['description'])}</p>
    {blocks(g['body'])}
    {CTA}
    {faq_html}
    <section class="g-related" aria-labelledby="more"><h2 id="more">Keep reading</h2><ul>{rel}</ul></section>
  </article>
</main>
""" + FOOT


def hub_page() -> str:
    url = f"{ORIGIN}/guides/"
    ld = {
        "@context": "https://schema.org",
        "@type": "CollectionPage",
        "name": "Guides: working with coding agents without losing the plot",
        "url": url,
        "hasPart": [{"@type": "Article", "headline": g["h1"], "url": f"{ORIGIN}/{g['slug']}/"} for g in GUIDES],
    }
    head = HEAD.format(
        title="Guides: reviewing and understanding code your AI agent wrote | deck",
        description="Practical guides for engineers working with coding agents: AI code review, large pull requests, comprehension debt, review fatigue, and keeping your judgement.",
        url=url, origin=ORIGIN, og_type="website", ld=json.dumps(ld, ensure_ascii=False).replace("</", "<\\/"),
        guides_current=' aria-current="page"',
    )
    cards = "".join(
        f'<a class="g-card" href="/{g["slug"]}/"><span class="eyebrow accent">{html.escape(g["eyebrow"])}</span>'
        f'<span class="g-card-t">{html.escape(g["h1"])}</span><span class="g-card-d">{html.escape(g["description"])}</span></a>'
        for g in GUIDES
    )
    return head + f"""<main class="g-wrap wide" id="doc">
  <header class="g-hub-head">
    <p class="eyebrow accent">Guides</p>
    <h1>Working with your agent, without losing the plot.</h1>
    <p class="lede">Agents write code faster than anyone can follow. These guides are about the other half: reviewing it, understanding it, and keeping the decisions yours.</p>
  </header>
  <div class="g-grid">{cards}</div>
  {CTA}
</main>
""" + FOOT


def main() -> None:
    for i, g in enumerate(GUIDES):
        related = [GUIDES[(i + k) % len(GUIDES)] for k in (1, 2, 3)]
        os.makedirs(os.path.join(SITE, g["slug"]), exist_ok=True)
        with open(os.path.join(SITE, g["slug"], "index.html"), "w") as f:
            f.write(guide_page(g, related))
    os.makedirs(os.path.join(SITE, "guides"), exist_ok=True)
    with open(os.path.join(SITE, "guides", "index.html"), "w") as f:
        f.write(hub_page())

    pages = ["", "docs/", "changelog/", "guides/"] + [g["slug"] + "/" for g in GUIDES]
    urls = "".join(
        f"  <url><loc>{ORIGIN}/{p}</loc><lastmod>{TODAY}</lastmod><priority>{'1.0' if p == '' else '0.8' if p in ('docs/', 'guides/') else '0.7'}</priority></url>\n"
        for p in pages
    )
    with open(os.path.join(SITE, "sitemap.xml"), "w") as f:
        f.write(f'<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n{urls}</urlset>\n')
    with open(os.path.join(SITE, "robots.txt"), "w") as f:
        f.write(f"User-agent: *\nAllow: /\n\nSitemap: {ORIGIN}/sitemap.xml\n")
    print(f"{len(GUIDES)} guides, the hub, {len(pages)} pages in the sitemap")


if __name__ == "__main__":
    main()
