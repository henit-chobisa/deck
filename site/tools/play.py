"""Write /play/: a recorded deck you can play, from "Tests pass." to the decision the summary hid.

    python3 site/tools/play.py            # write site/play/index.html
    python3 site/tools/play.py --check    # fail if it is stale (the site workflow runs this)

The page is the shell (tools/shell.py) around ten steps: an agent's fix for random logouts, the deck
it writes, the bar, the deck window, the question that finds what the summary left out, and the
reader's decision. js/play-deck.js is the deck itself (its groups, refs, values and recorded
answers); js/play.js draws its window the way deck draws it (crates/app: the band, the panes, the
rail, the strip, the asks) and plays the steps; css/play.css is both. The app and its code are made
up, and every answer is recorded; the page says so on every step.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import shell  # noqa: E402

SITE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ORIGIN = "https://trydeck.dev"
TITLE = "Try deck in your browser: the decision inside a one-line fix | deck"
DESC = ("Play a recorded deck: an agent fixes random logouts, shows the change claim by claim on the lines, "
        "and when you ask about logout, finds the decision its summary never mentioned. No install, no model.")

# (rail label, caption under the stage)
STEPS = [
    ("The problem", "The agent says it is done, in a paragraph. <b>A decision you would sign off on without seeing it.</b>"),
    ("It writes a deck", "Instead of a summary, it writes a deck: one claim per group, each pinned to the lines it is about."),
    ("The bar", "A small bar comes up with the deck's title. <b>Only you open it</b>: the agent cannot."),
    ("The change", "Each sentence lights the lines it is about. Click one. The change is drawn on today's code, before anything is edited."),
    ("The flow", "The path the code takes, beside the code. Play a route, and the same step lights in both."),
    ("The timeline", "An idea that only makes sense moving gets a page: a session's life, before and now."),
    ("The values", "What each line held when the test ran, written on the line, like a debugger."),
    ("You ask", "Push back where you doubt it: select a line and press <kbd>c</kbd>, or tap a question under the window."),
    ("You decide", "You decide, in a comment on the line. The deck exports with the argument, for the pull request."),
    ("That's deck", "Your agent shows you. You take part. What ships is something you both chose."),
]

MARK = ('<span class="bm" aria-hidden="true"><i></i><i></i></span>')

TERM = """<section class="sc sc-term" id="sc-term" aria-label="The agent's terminal">
 <div class="tm">
  <div class="tm-tb" aria-hidden="true"><i></i><i></i><i></i><span>claude · ~/shop-api</span></div>
  <div class="tm-tx" id="tm-tx">
   <p class="tu"><span>›</span> users keep getting logged out at random. fix it</p>
   <p class="ta"><span>⏺</span> Read <b>src/auth/session.ts</b>, <b>src/auth/login.ts</b></p>
   <p class="ta"><span>⏺</span> Ran <b>npm test</b> <em>· 42 passed</em></p>
   <div class="tm-done" id="tm-done">
    <p class="tsum"><span>⏺</span> Users were getting logged out randomly. Fixed by extending the session refresh. Tests pass.</p>
    <div class="tm-pr"><span class="pr-n">#318 Fix random logouts <small>+1 −1</small></span><span class="pr-ok" id="pr-ok">Approve · LGTM<i class="cur" aria-hidden="true"></i></span></div>
   </div>
   <div class="tm-deck" id="tm-deck">
    <p class="ta"><span>⏺</span> Writing it up as a deck, so you can check it on the code.</p>
    <pre class="tc" data-ty><span class="tp">$</span> <b>deck new</b> --title <s>"Fix random logouts"</s> --total 4</pre>
    <pre class="to" data-out>~/.deck/decks/d-1793261705-4127.deck</pre>
    <pre class="tc" data-ty data-bar="0"><span class="tp">$</span> <b>deck open</b> $deck   <u># in the background: the bar goes up</u></pre>
    <pre class="tc" data-ty data-bar="1"><span class="tp">$</span> <b>deck group</b> $deck \\
    --title <s>"The refresh now extends the session's lifetime"</s> \\
    --confidence high \\
    --say <s>'Users who leave a tab for an hour come back signed out. [point 9-11 refresh] …'</s> \\
    --ref <s>'src/auth/session.ts:7-15 [refresh] where a session is kept alive'</s> \\
    --ref <s>'src/auth/session.ts:5 [ttl] a proposed change'</s> \\
    --after <s>'export const SESSION_TTL = 30 * 24 * 60 * 60;  // 30 days'</s></pre>
    <pre class="tc" data-ty data-bar="2"><span class="tp">$</span> <b>deck group</b> $deck --title <s>'A refresh is what keeps you signed in'</s> --diagram flow.json …</pre>
    <pre class="tc" data-ty data-bar="3"><span class="tp">$</span> <b>deck group</b> $deck --title <s>'Now only logout signs you out'</s> --page life.html …</pre>
    <pre class="tc" data-ty data-bar="4"><span class="tp">$</span> <b>deck group</b> $deck --title <s>'Tested: a refresh lands thirty days out'</s> \\
    --values <s>'[{"line":12,"at":"expiresAt","is":"2026-11-28T10:14:07Z","lit":true}, …]'</s> \\
    --ask <s>'Does logout still kill the session?'</s> …</pre>
    <pre class="tc" data-ty data-bar="seal"><span class="tp">$</span> <b>deck seal</b> $deck</pre>
   </div>
  </div>
 </div>
 <div class="pill" id="pl-bar" role="group" aria-label="deck's bar">
  <span class="bar-brand">""" + MARK + """<span>deck</span><i aria-hidden="true"></i></span>
  <span class="bar-said"><span class="bar-t">Fix random logouts</span><i class="bar-dot" aria-hidden="true"></i><span class="bar-c" id="bar-c">Getting deck ready… 0/4</span></span>
  <button type="button" class="bar-open" id="bar-open">Open</button>
  <button type="button" class="bar-x" id="bar-x" aria-label="Later"><svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 3l6 6M9 3l-6 6"/></svg></button>
 </div>
</section>"""

WIN = """<section class="sc sc-win" id="sc-win" aria-label="The deck window">
 <div class="dk" id="dk" tabindex="-1" aria-label="deck: Fix random logouts"></div>
 <div class="dk-asks" id="dk-asks" role="group" aria-label="Questions to ask"></div>
 <div class="xs" id="xs" role="dialog" aria-label="Export this deck" hidden>
  <div class="xs-h">Export this deck<small>Fix random logouts · 4 groups</small></div>
  <div class="xs-l">Format</div>
  <div class="xs-seg"><span class="on">PDF</span><span>HTML</span><span>Markdown</span></div>
  <div class="xs-l">Layout</div>
  <div class="xs-seg"><span>Portrait</span><span class="on">Landscape</span></div>
  <div class="xs-ck"><i aria-hidden="true">✓</i>Include comments<small>Each group's comments and answers, after it.</small></div>
  <div class="xs-b"><span class="xs-st" id="xs-st" aria-live="polite"></span><span class="xs-c">Cancel</span><button type="button" class="xs-go" id="xs-go">Export PDF</button></div>
 </div>
 <div class="pr" id="pr" hidden>
  <div class="pr-h"><b>Fix random logouts</b> <span>#318</span><em>Changes requested</em></div>
  <div class="pr-c">
   <div class="pr-w"><b>you</b> requested changes · now</div>
   <p>Keep the 1h TTL and revoke the refresh token on logout instead. The argument is in the deck.</p>
   <div class="pr-f"><b>PDF</b><span>fix-random-logouts.pdf<small>4 groups · with comments</small></span></div>
  </div>
  <p class="pr-n">Nothing is downloaded here: the export is part of the recording.</p>
 </div>
</section>"""

END = """<section class="sc sc-end" id="sc-end" aria-label="That's deck">
 <div class="end">
  <p class="end-t">That's deck.</p>
  <p class="end-s">Your agent shows you. You take part. What ships is something you both chose.</p>
  <div class="end-b"><a class="btn solid" href="/#install">Install deck</a><a class="btn ghost" href="https://github.com/henit-chobisa/deck">View on GitHub</a><button type="button" class="btn ghost" id="pl-replay">Replay</button></div>
 </div>
</section>"""


def rail() -> str:
    out = []
    for i, (label, cap) in enumerate(STEPS):
        out.append(f'   <li><button type="button" data-go="{i}" data-cap="{cap.replace(chr(34), "&quot;")}" '
                   f'aria-label="Step {i + 1} of {len(STEPS)}: {label}"><i></i><span>{label}</span></button></li>')
    return "\n".join(out)


def story() -> str:
    first = STEPS[0][1]
    return f"""<div class="pl" id="pl" tabindex="-1">
 <div class="pl-top">
  <ol class="pl-rail" id="pl-rail" aria-label="Steps">
{rail()}
  </ol>
  <button type="button" class="pl-auto" id="pl-auto" aria-pressed="false"><span class="pl-ai" aria-hidden="true"></span><span class="pl-at">Play it for me</span></button>
 </div>
 <p class="pl-honest"><b>A recorded example:</b> nothing here runs a model. <span>example app, fictional code</span></p>
 <div class="pl-stage" id="pl-stage" data-step="1">
{TERM}
{WIN}
{END}
 </div>
 <div class="pl-foot">
  <p class="pl-cap" id="pl-cap" aria-live="polite"><b id="pl-no">1 / {len(STEPS)}</b> <span id="pl-say">{first}</span></p>
  <div class="pl-nav"><button type="button" class="btn ghost" id="pl-prev">Back</button><button type="button" class="btn solid" id="pl-next">Next</button></div>
 </div>
 <p class="pl-keys"><kbd>n</kbd> or <kbd>→</kbd> next · <kbd>p</kbd> or <kbd>←</kbd> back · <kbd>c</kbd> comment<span> · on a phone, tap the frame</span></p>
 <noscript><p class="pl-nojs">The demo needs JavaScript. <a href="/docs/">The docs</a> show the same deck in words.</p></noscript>
</div>"""


def page() -> str:
    url = f"{ORIGIN}/play/"
    ld = {"@context": "https://schema.org", "@type": "WebPage", "name": TITLE, "url": url, "description": DESC,
          "isPartOf": {"@type": "WebSite", "name": "deck", "url": f"{ORIGIN}/"},
          "about": {"@type": "SoftwareApplication", "name": "deck", "applicationCategory": "DeveloperApplication",
                    "operatingSystem": "macOS, Linux, Windows", "url": f"{ORIGIN}/"}}
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
<script type="application/ld+json">{json.dumps(ld, ensure_ascii=False).replace("</", "<\\/")}</script>
<link rel="icon" href="/assets/mark.svg" type="image/svg+xml">
<!-- shell:head -->
{shell.FONTS}
<!-- /shell:head -->
<link rel="stylesheet" href="/css/play.css">
</head>
<body>
<a class="skip" href="#pl">Skip to the demo</a>
<!-- shell:nav -->
{shell.nav()}
<!-- /shell:nav -->
<main id="main">
<header class="play-head">
 <p class="eyebrow accent">Try a full deck</p>
 <h1>Fixed the random logouts</h1>
 <p class="play-lede">An agent's one-line fix, as the deck it would write you. Ten steps, about a minute and a half, at your pace.</p>
</header>
{story()}
</main>
<!-- shell:foot -->
{shell.FOOT}
<!-- /shell:foot -->
<script src="/js/play-deck.js" defer></script>
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
