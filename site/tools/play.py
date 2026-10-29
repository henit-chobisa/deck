"""Write /play/: the cache-stampede deck from the home page, playable on its own.

    python3 site/tools/play.py

The three windows are taken from index.html as they are (the values tile of #x6, #x3 and #x4),
so the demo and the home page never drift apart; landing.js drives them, stepped by hand
(html.play), and js/play.js adds the deck around them: the groups, n/p/c, and the recorded
answers. Run it after changing those sections of index.html; the site workflow checks it is fresh.
"""

import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
import shell  # noqa: E402

SITE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ORIGIN = "https://trydeck.dev"
TITLE = "Try a deck in your browser: an agent explains a cache fix | deck"
DESC = ("A recorded deck you can play: an agent walks a cache-stampede fix claim by claim, lights the code, "
        "plays the flow, shows the values, and answers the questions you would ask. No install, no model.")


def section(html: str, sid: str) -> str:
    i = html.rfind("<section", 0, html.index(f'id="{sid}"'))
    return html[i:html.index("</section>", i) + len("</section>")]


def windows(html: str) -> tuple:
    x6 = section(html, "x6")
    # the values tile only: drop the second tile and the links under the tiles (the headline stays,
    # hidden by play.css: landing.js watches it)
    second = x6.index('<div class="x6pan o2t">', x6.index('<div class="x6pan o2t">') + 1)
    x6 = x6[:second].rstrip() + "\n  </div>\n </div>\n</section>"
    x6 = x6.replace(' aria-labelledby="x6h"', ' aria-label="Group 1: the values each claim rests on"')
    # the link to this page, from the home page's #x3, is not needed on this page
    x3 = section(html, "x3").replace(' <p class="x3try"><a href="/play/">Try a full deck <i aria-hidden="true">→</i></a></p>\n', "").replace(' aria-labelledby="x3h"', ' aria-label="Group 2: the fix, claim by claim"')
    x4 = section(html, "x4").replace(' aria-labelledby="x4h"', ' aria-label="Group 2: ask, and push back"')
    return x6, x3, x4


def page() -> str:
    html = open(os.path.join(SITE, "index.html"), encoding="utf-8").read()
    x6, x3, x4 = windows(html)
    url = f"{ORIGIN}/play/"
    ld = {"@context": "https://schema.org", "@type": "WebPage", "name": TITLE, "url": url, "description": DESC,
          "isPartOf": {"@type": "WebSite", "name": "deck", "url": f"{ORIGIN}/"}}
    return f"""<!doctype html>
<html lang="en" class="play">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{TITLE}</title>
<meta name="description" content="{DESC}">
<link rel="canonical" href="{url}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="deck">
<meta property="og:title" content="{TITLE}">
<meta property="og:description" content="{DESC}">
<meta property="og:url" content="{url}">
<meta property="og:image" content="{ORIGIN}/assets/og.png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{TITLE}">
<meta name="twitter:description" content="{DESC}">
<meta name="twitter:image" content="{ORIGIN}/assets/og.png">
<meta name="theme-color" content="#070707">
<script type="application/ld+json">{json.dumps(ld, ensure_ascii=False)}</script>
<link rel="icon" href="/assets/mark.svg" type="image/svg+xml">
<!-- shell:head -->
{shell.FONTS}
<!-- /shell:head -->
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Caveat:wght@600&amp;display=swap">
<link rel="stylesheet" href="/css/landing.css">
<link rel="stylesheet" href="/css/play.css">
</head>
<body>
<a class="skip" href="#deck">Skip to the deck</a>
<!-- shell:nav -->
{shell.nav()}
<!-- /shell:nav -->
<main id="main">
<header class="play-head">
 <p class="eyebrow accent">Try a full deck</p>
 <h1>Cache stampede on user lookups</h1>
 <p class="play-note"><b>A recorded example.</b> An agent wrote this deck about a fix it made; you read two of its four groups here, the way you would in deck. Nothing on this page runs a model: the answers to the questions are recorded with the deck.</p>
 <nav class="play-steps" aria-label="Groups">
  <button type="button" data-view="0" aria-current="step"><b>1</b>The values each claim rests on</button>
  <button type="button" data-view="1"><b>2</b>The fix, claim by claim</button>
  <button type="button" data-view="2"><b>2</b>Ask, and push back</button>
 </nav>
 <p class="play-keys"><kbd>n</kbd> next <kbd>p</kbd> previous <kbd>c</kbd> comment, or click</p>
</header>
<div class="play-deck" id="deck" tabindex="-1">
<div class="play-view" data-view="0">
{x6}
</div>
<div class="play-view" data-view="1" hidden>
{x3}
</div>
<div class="play-view" data-view="2" hidden>
{x4}
<form class="play-cmp" id="play-cmp" hidden>
 <label for="play-text">Your comment, on get-user.ts:19</label>
 <textarea id="play-text" rows="2" placeholder="Write a comment"></textarea>
 <div class="play-cmp-row"><span class="play-cmp-note" id="play-cmp-note" role="status"></span><button type="submit" class="btn solid">Send</button></div>
</form>
</div>
<div class="play-nav">
 <button type="button" class="btn ghost" id="play-prev">← previous</button>
 <span class="play-at" id="play-at" aria-live="polite"></span>
 <button type="button" class="btn ghost" id="play-next">next →</button>
</div>
<div class="play-end">
 <p>That was a deck. In yours, the agent is the one you already use, the answers are its own, and your comments go back to it, pinned to the lines.</p>
 <div class="play-end-btns"><a class="btn solid" href="/#install">Install deck</a><a class="btn ghost" href="/docs/">Read the docs</a></div>
</div>
</div>
</main>
<!-- shell:foot -->
{shell.FOOT}
<!-- /shell:foot -->
<script src="/js/landing.js" defer></script>
<script src="/js/play.js" defer></script>
</body>
</html>
"""


def main(check: bool) -> None:
    out = os.path.join(SITE, "play", "index.html")
    new = page()
    if check:
        old = open(out, encoding="utf-8").read() if os.path.exists(out) else ""
        if old != new:
            sys.exit("play/index.html is stale: run python3 site/tools/play.py")
        print("play/index.html is fresh")
        return
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w", encoding="utf-8") as f:
        f.write(new)
    print("wrote", out)


if __name__ == "__main__":
    main("--check" in sys.argv)
