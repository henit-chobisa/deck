"""Write /play/: a recorded deck, played full screen on a desktop drawn for it.

    python3 site/tools/play.py            # write site/play/index.html
    python3 site/tools/play.py --check    # fail if it is stale (the site workflow runs this)

The page opens on an intro; Start goes full screen (or fills the window when fullscreen is refused)
onto a desktop: a wallpaper, a menu bar and a dock, all drawn here. On it, ten steps: an agent's fix
for random logouts in a terminal, the deck it writes with deck's real commands, the bar, the deck
window, the question that finds what the summary left out, and the reader's decision. Phones and
narrow screens get a still of it instead. js/play-deck.js is the deck (groups, refs, values,
recorded answers); js/play.js draws its window the way deck draws it and plays the steps;
css/play.css is both. The app and its code are made up and every answer is recorded, and the page
says so. assets/play-still.webp is a capture of step 7 (see tools/play-still.cjs).
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import shell  # noqa: E402

SITE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ORIGIN = "https://trydeck.dev"
TITLE = "Try deck in your browser: the decision inside a one-line fix | deck"
DESC = ("Watch a recorded deck on a desktop: an agent fixes random logouts, shows the change claim by claim on "
        "the lines, and when you ask about logout, finds the decision its summary never mentioned. No install, no model.")

# (rail label, caption)
STEPS = [
    ("The problem", "The agent says it is done, in a paragraph. <b>A decision you would sign off on without seeing it.</b>"),
    ("It writes a deck", "Instead of a summary, it writes a deck with deck's own commands: one claim per group, pinned to the lines."),
    ("The bar", "A small bar comes up with the deck's title. <b>Only you open it</b>: the agent cannot."),
    ("The change", "Each sentence lights the lines it is about. The change is drawn on today's code, before anything is edited."),
    ("The flow", "The path the code takes, beside the code. Play a route, and the same step lights in both."),
    ("The timeline", "An idea that only makes sense moving gets a page: a session's life, before and now."),
    ("The values", "What each line held when the test ran, written on the line, like a debugger."),
    ("You ask", "Push back where you doubt it: hold a line and press <kbd>c</kbd>, or pick a question under the window."),
    ("You decide", "You decide, in a comment on the line, and export the deck with the argument in it."),
    ("That's deck", "Your agent shows you. You take part. What ships is something you both chose."),
]

MARK = '<span class="bm" aria-hidden="true"><i></i><i></i></span>'
DECK_MARK = ('<svg viewBox="0 0 128 128" aria-hidden="true"><rect x="1.5" y="1.5" width="125" height="125" rx="35.6" fill="#2f2f2f" '
             'stroke="#4a4542" stroke-width="3"/><rect x="35.5" y="39.1" width="56.9" height="14.2" rx="7.1" fill="#7c6f64"/>'
             '<rect x="35.5" y="74.6" width="56.9" height="14.2" rx="7.1" fill="#fe8019"/></svg>')


def ridge(seed: int, base: float, amp: float, rough: float, n: int = 64) -> str:
    """A mountain line, by midpoint displacement from a fixed seed: the same range on every build."""
    s = seed

    def rnd():
        nonlocal s
        s = (s * 9301 + 49297) % 233280
        return s / 233280 - 0.5

    pts = [base + rnd() * amp, base + rnd() * amp]
    spread = amp
    while len(pts) < n:
        spread *= rough
        nxt = []
        for a, b in zip(pts, pts[1:]):
            nxt += [a, (a + b) / 2 + rnd() * spread]
        pts = nxt + [pts[-1]]
    step = 1600 / (len(pts) - 1)
    return " ".join(f"{i * step:.0f},{y:.0f}" for i, y in enumerate(pts))


def wallpaper() -> str:
    """A calm beach by day, in layers: sky and soft clouds, a hazy headland, the sea, its foam, the sand."""
    head = ridge(11, 520, 70, 0.55, 32)
    cloud = lambda x, y, s, o: (f'<g transform="translate({x} {y}) scale({s})" opacity="{o}"><ellipse cx="0" cy="0" rx="120" ry="26"/>'
                                f'<ellipse cx="-60" cy="-14" rx="62" ry="30"/><ellipse cx="40" cy="-22" rx="74" ry="38"/></g>')
    return f"""<svg class="wall" viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
<defs>
<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#7fc3ea"/><stop offset=".35" stop-color="#a9d8f2"/><stop offset=".56" stop-color="#e4f3fa"/></linearGradient>
<radialGradient id="sun" cx="1220" cy="150" r="460" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#fffbe8" stop-opacity=".95"/><stop offset=".2" stop-color="#fff4d1" stop-opacity=".45"/><stop offset="1" stop-color="#fff4d1" stop-opacity="0"/></radialGradient>
<linearGradient id="head" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#8fb6b4"/><stop offset="1" stop-color="#b9d3d0"/></linearGradient>
<linearGradient id="sea" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#5aa9cf"/><stop offset=".55" stop-color="#5fbfd6"/><stop offset="1" stop-color="#8fdad9"/></linearGradient>
<linearGradient id="sand" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#efe0bf"/><stop offset="1" stop-color="#e6cf9f"/></linearGradient>
<linearGradient id="wet" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#d8c7a0"/><stop offset="1" stop-color="#e9d8b2" stop-opacity="0"/></linearGradient>
<clipPath id="land"><rect width="1600" height="566"/></clipPath>
</defs>
<rect width="1600" height="1000" fill="url(#sky)"/>
<rect width="1600" height="1000" fill="url(#sun)"/>
<g fill="#fff">{cloud(300, 170, 1, .75)}{cloud(760, 110, .7, .6)}{cloud(1480, 260, .85, .55)}{cloud(1040, 300, .55, .45)}</g>
<g clip-path="url(#land)"><polygon points="900,1000 {" ".join(p for p in head.split() if float(p.split(",")[0]) >= 900)} 1600,1000" fill="url(#head)"/></g>
<rect y="566" width="1600" height="300" fill="url(#sea)"/>
<g fill="none" stroke="#ffffff" stroke-linecap="round" opacity=".55"><path d="M120 620h180M520 650h240M980 610h160M1300 680h200M240 720h260M760 760h220" stroke-width="2"/></g>
<path d="M0 820 C 200 800, 360 840, 560 822 S 960 800, 1160 826 S 1460 808, 1600 820 V 1000 H 0 Z" fill="url(#sand)"/>
<path d="M0 820 C 200 800, 360 840, 560 822 S 960 800, 1160 826 S 1460 808, 1600 820 V 860 H 0 Z" fill="url(#wet)"/>
<path d="M0 818 C 200 798, 360 838, 560 820 S 960 798, 1160 824 S 1460 806, 1600 818" fill="none" stroke="#fffdf6" stroke-width="5" stroke-linecap="round" opacity=".9"/>
</svg>"""


# The dock: generic tools drawn here, and deck's own mark.
ICONS = {
    "Terminal": '<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="it" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3a3d42"/><stop offset="1" stop-color="#16181b"/></linearGradient></defs><rect x="2" y="2" width="60" height="60" rx="14" fill="url(#it)"/><rect x="2.5" y="2.5" width="59" height="59" rx="13.5" fill="none" stroke="#ffffff1f"/><path d="M15 22l9 8-9 8" fill="none" stroke="#9fe08c" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/><rect x="28" y="36" width="16" height="4" rx="2" fill="#9fe08c"/></svg>',
    "Browser": '<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="ib" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#7cc4ff"/><stop offset="1" stop-color="#2a6fd6"/></linearGradient></defs><rect x="2" y="2" width="60" height="60" rx="14" fill="url(#ib)"/><circle cx="32" cy="32" r="17" fill="none" stroke="#fff" stroke-width="3"/><ellipse cx="32" cy="32" rx="7" ry="17" fill="none" stroke="#fff" stroke-width="2.4"/><path d="M15 32h34M18 23h28M18 41h28" stroke="#fff" stroke-width="2.4"/></svg>',
    "Editor": '<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="ie" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3b4a6b"/><stop offset="1" stop-color="#1c2335"/></linearGradient></defs><rect x="2" y="2" width="60" height="60" rx="14" fill="url(#ie)"/><path d="M24 22l-10 10 10 10M40 22l10 10-10 10" fill="none" stroke="#8ab4ff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/><path d="M35 18l-6 28" stroke="#ffb86b" stroke-width="3.6" stroke-linecap="round"/></svg>',
    "Notes": '<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="in" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffe08a"/><stop offset="1" stop-color="#f4b740"/></linearGradient></defs><rect x="2" y="2" width="60" height="60" rx="14" fill="#fffaf0"/><path d="M2 16a14 14 0 0 1 14-14h32a14 14 0 0 1 14 14v4H2z" fill="url(#in)"/><path d="M13 32h38M13 41h38M13 50h26" stroke="#d9cfbd" stroke-width="2.4" stroke-linecap="round"/></svg>',
    "Files": '<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="if" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#8fd0ff"/><stop offset="1" stop-color="#3b8be8"/></linearGradient></defs><path d="M6 18a6 6 0 0 1 6-6h13l6 6h21a6 6 0 0 1 6 6v24a6 6 0 0 1-6 6H12a6 6 0 0 1-6-6z" fill="#2f78d4"/><rect x="6" y="22" width="52" height="32" rx="6" fill="url(#if)"/></svg>',
    "deck": DECK_MARK,
}


def dock() -> str:
    items = []
    for name, svg in ICONS.items():
        cls = " on" if name == "Terminal" else ""
        sep = '<i class="dsep" aria-hidden="true"></i>' if name == "deck" else ""
        items.append(f'{sep}<span class="di{cls}" data-app="{name}" title="{name}">{svg}<b aria-hidden="true"></b></span>')
    return '<div class="dock" aria-hidden="true">' + "".join(items) + "</div>"


MENU = """<div class="mb" aria-hidden="true">
 <span class="mb-l"><svg viewBox="0 0 16 16"><path d="M8 1.5l5.6 3.25v6.5L8 14.5l-5.6-3.25v-6.5z" fill="none" stroke="currentColor" stroke-width="1.5"/><circle cx="8" cy="8" r="2" fill="currentColor"/></svg><b id="mb-app">Terminal</b><span>File</span><span>Edit</span><span>View</span><span>Window</span><span>Help</span></span>
 <span class="mb-r"><svg viewBox="0 0 18 14"><path d="M1 5a12 12 0 0 1 16 0M4 8a7.5 7.5 0 0 1 10 0M7 11a3 3 0 0 1 4 0" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg><svg viewBox="0 0 26 12"><rect x=".75" y=".75" width="21.5" height="10.5" rx="3" fill="none" stroke="currentColor" stroke-width="1.2" opacity=".7"/><rect x="2.5" y="2.5" width="15" height="7" rx="1.6" fill="currentColor"/><rect x="23.4" y="4" width="1.8" height="4" rx=".9" fill="currentColor" opacity=".7"/></svg><span id="mb-clock">Thu 10:42</span></span>
</div>"""

TERM = """<section class="win tw" id="tw" aria-label="The agent's terminal">
 <div class="wbar"><span class="lights" aria-hidden="true"><i></i><i></i><i></i></span><span class="wt">agent — ~/shop-api</span></div>
 <div class="tm-tx" id="tm-tx">
  <pre class="tu" data-ty="p"><span>›</span> users keep getting logged out at random. fix it</pre>
  <pre class="ta" data-out="a"><span>⏺</span> Read <b>src/auth/session.ts</b></pre>
  <pre class="ta" data-out="a"><span>⏺</span> Read <b>src/auth/login.ts</b></pre>
  <pre class="ta" data-out="a"><span>⏺</span> Ran <b>npm test</b> <em>· 42 passed</em></pre>
  <pre class="tsum" data-out="a" id="tm-sum"><span>⏺</span> <b>Done.</b> Users were getting logged out randomly. Fixed by extending the session refresh. Tests pass.</pre>
  <div class="tm-deck" id="tm-deck">
   <pre class="ta" data-out="d"><span>⏺</span> Writing it up as a deck, so you can check it on the code.</pre>
   <pre class="tc" data-ty="d"><span class="tp">$</span> <b>deck new</b> --title <s>"Fix random logouts"</s> --total 4</pre>
   <pre class="to" data-out="d">~/.deck/decks/d-1793261705-4127.deck</pre>
   <pre class="tc" data-ty="d" data-bar="0"><span class="tp">$</span> <b>deck open</b> $deck   <u># in the background: the bar goes up</u></pre>
   <pre class="tc" data-ty="d" data-bar="1"><span class="tp">$</span> <b>deck group</b> $deck \\
    --title <s>"The refresh now extends the session's lifetime"</s> \\
    --confidence high \\
    --say <s>'Users who leave a tab for an hour come back signed out. [point 9-11 refresh] …'</s> \\
    --ref <s>'src/auth/session.ts:7-15 [refresh] where a session is kept alive'</s> \\
    --ref <s>'src/auth/session.ts:5 [ttl] a proposed change'</s> \\
    --after <s>'export const SESSION_TTL = 30 * 24 * 60 * 60;  // 30 days'</s></pre>
   <pre class="tc" data-ty="d" data-bar="2"><span class="tp">$</span> <b>deck group</b> $deck --title <s>'A refresh is what keeps you signed in'</s> --diagram flow.json …</pre>
   <pre class="tc" data-ty="d" data-bar="3"><span class="tp">$</span> <b>deck group</b> $deck --title <s>'Now only logout signs you out'</s> --page life.html …</pre>
   <pre class="tc" data-ty="d" data-bar="4"><span class="tp">$</span> <b>deck group</b> $deck --title <s>'Tested: a refresh lands thirty days out'</s> \\
    --values <s>'[{"line":12,"at":"expiresAt","is":"2026-11-28T10:14:07Z","lit":true}, …]'</s> \\
    --ask <s>'Does logout still kill the session?'</s> …</pre>
   <pre class="tc" data-ty="d" data-bar="seal"><span class="tp">$</span> <b>deck seal</b> $deck</pre>
  </div>
 </div>
</section>"""

BAR = """<div class="pill" id="pl-bar" role="group" aria-label="deck's bar">
 <span class="bar-brand">""" + MARK + """<span>deck</span><i aria-hidden="true"></i></span>
 <span class="bar-said"><span class="bar-t">Fix random logouts</span><i class="bar-dot" aria-hidden="true"></i><span class="bar-c" id="bar-c">Getting deck ready… 0/4</span></span>
 <button type="button" class="bar-open" id="bar-open">Open</button>
 <button type="button" class="bar-x" id="bar-x" aria-label="Put the deck away"><svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 3l6 6M9 3l-6 6"/></svg></button>
</div>"""

DECKWIN = """<section class="win dw" id="dw" aria-label="The deck window">
 <div class="wbar"><span class="lights" aria-hidden="true"><i></i><i></i><i></i></span><span class="wt">deck — Fix random logouts</span></div>
 <div class="dk" id="dk" tabindex="-1" aria-label="deck: Fix random logouts"></div>
 <div class="dk-asks" id="dk-asks" role="group" aria-label="Questions to ask"></div>
 <div class="xs" id="xs" role="dialog" aria-label="Export this deck" hidden>
  <div class="xs-h">Export this deck<small>Fix random logouts · 4 groups</small></div>
  <div class="xs-l">Format</div>
  <div class="xs-seg"><span class="on">PDF</span><span>HTML</span><span>Markdown</span></div>
  <div class="xs-l">Layout</div>
  <div class="xs-seg"><span>Portrait</span><span class="on">Landscape</span></div>
  <div class="xs-ck"><i aria-hidden="true">✓</i>Include comments<small>Each group's comments and answers, after it.</small></div>
  <div class="xs-b"><span class="xs-st" id="xs-st" aria-live="polite"></span><button type="button" class="xs-c" id="xs-c">Cancel</button><button type="button" class="xs-go" id="xs-go">Export PDF</button></div>
 </div>
</section>"""


def rail() -> str:
    return "\n".join(
        f'  <button type="button" data-go="{i}" data-cap="{cap.replace(chr(34), "&quot;")}" aria-label="Step {i + 1} of {len(STEPS)}: {label}">'
        f'<i></i><span>{label}</span></button>' for i, (label, cap) in enumerate(STEPS))


def desk() -> str:
    return f"""<div class="desk" id="desk" tabindex="-1" role="dialog" aria-modal="true" aria-label="deck on a desktop, a recorded example" hidden>
{wallpaper()}
{MENU}
<div class="dt-file" id="dt-file" hidden><svg viewBox="0 0 48 60" aria-hidden="true"><path d="M4 4h28l12 12v40H4z" fill="#f4f1ea"/><path d="M32 4v12h12" fill="#d8d2c4"/><rect x="9" y="38" width="30" height="12" rx="2" fill="#da3633"/><text x="24" y="47.5" text-anchor="middle" font-size="9" font-weight="700" fill="#fff" font-family="system-ui,sans-serif">PDF</text></svg><span>fix-random-logouts.pdf</span></div>
{TERM}
{DECKWIN}
{BAR}
{dock()}
<nav class="srail" id="pl-rail" aria-label="Steps">
{rail()}
</nav>
<div class="cap" id="cap">
 <p class="cap-t" aria-live="polite"><b id="pl-no">1 / {len(STEPS)}</b> <span id="pl-say">{STEPS[0][1]}</span></p>
 <div class="cap-b"><button type="button" class="cb" id="pl-prev">Back</button><button type="button" class="cb solid" id="pl-next">Next</button><button type="button" class="cb auto" id="pl-auto" aria-pressed="false"><span class="pl-ai" aria-hidden="true"></span><span class="pl-at">Play it for me</span></button></div>
</div>
<p class="dhonest"><b>A recorded example:</b> nothing here runs a model · example app, fictional code</p>
<button type="button" class="dx" id="dx" aria-label="Leave full screen">✕</button>
<div class="endc" id="endc" hidden>
 <p class="end-t">That's deck.</p>
 <p class="end-s">Your agent shows you. You take part. What ships is something you both chose.</p>
 <div class="end-b"><a class="btn solid" href="/#install">Install deck</a><a class="btn ghost" href="{shell.REPO}">View on GitHub</a><button type="button" class="btn ghost" id="pl-replay">Replay</button></div>
</div>
<svg class="cursor" id="cursor" viewBox="0 0 22 26" aria-hidden="true"><path d="M3 2 L3 20 L7.5 15.8 L10.6 23 L13.6 21.7 L10.6 14.8 L16.6 14.6 Z" fill="#f7f7f5" stroke="#111" stroke-width="1.3" stroke-linejoin="round"/></svg>
</div>"""


INTRO = """<section class="pi" aria-labelledby="pi-h">
 <div class="pi-head">
  <p class="eyebrow accent">Try a full deck</p>
  <h1 id="pi-h">Fixed the random logouts</h1>
  <p class="pi-line">An agent's one-line fix, on a desktop like yours: it writes a deck, you open it, question it and decide. About a minute and a half.</p>
 </div>
 <div class="pi-frame">
  <img class="pi-still" src="/assets/play-still.webp" srcset="/assets/play-still-800.webp 800w, /assets/play-still.webp 1440w" sizes="(max-width: 1100px) 100vw, 1200px" width="1440" height="900" alt="A desktop with the deck window open on group 4: the values each line held, the review down the side, and the questions under the window." decoding="async">
  <button type="button" class="pi-start" id="pi-start"><span class="pi-play" aria-hidden="true"></span><span>Start</span><small>full screen</small></button>
  <div class="pi-phone">
   <p><b>Best on a desktop.</b> Open trydeck.dev/play on your computer to watch it full screen.</p>
   <div class="pi-links"><a class="btn solid" href="/#install">Install deck</a><a class="btn ghost" href="REPO">View on GitHub</a></div>
  </div>
 </div>
 <p class="pi-honest"><b>A recorded example:</b> nothing here runs a model, and the app in it is made up.</p>
</section>""".replace("REPO", shell.REPO)


def page() -> str:
    url = f"{ORIGIN}/play/"
    ld = {"@context": "https://schema.org", "@type": "WebPage", "name": TITLE, "url": url, "description": DESC,
          "isPartOf": {"@type": "WebSite", "name": "deck", "url": f"{ORIGIN}/"},
          "about": {"@type": "SoftwareApplication", "name": "deck", "applicationCategory": "DeveloperApplication",
                    "operatingSystem": "macOS, Linux, Windows", "url": f"{ORIGIN}/"}}
    # outside the f-string: a backslash inside one is a SyntaxError before Python 3.12
    ld_json = json.dumps(ld, ensure_ascii=False).replace("</", "<\\/")
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
<script type="application/ld+json">{ld_json}</script>
<link rel="icon" href="/assets/mark.svg" type="image/svg+xml">
<!-- shell:head -->
{shell.FONTS}
<!-- /shell:head -->
<link rel="stylesheet" href="/css/play.css">
</head>
<body>
<a class="skip" href="#main">Skip to the demo</a>
<!-- shell:nav -->
{shell.nav()}
<!-- /shell:nav -->
<main id="main">
{INTRO}
</main>
<!-- shell:foot -->
{shell.FOOT}
<!-- /shell:foot -->
{desk()}
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
