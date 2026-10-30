# site

deck's website: `index.html` (the home page), `docs/` (how to use it), the
changelog, the guides, and nothing to build. Plain HTML, CSS and JavaScript,
no libraries.

Every page shares one shell: `css/site.css` (the tokens, the `--u` scale, the
nav, the footer and the reading type) and `js/site.js` (the footer's Email).
Its markup lives once, in `tools/shell.py`: `tools/build.py` writes it into the
generated guides, and `python3 site/tools/shell.py` stamps it into the other
pages between `<!-- shell: -->` markers (`--check` fails on a stale page).

- The home page adds `css/landing.css`, `js/landing.js`, and two images in
  `assets/`: `deck-live.webp`, the real deck window in the hero, and `henit.jpg`.
- `/play/` opens on an intro; Start goes full screen (or fills the window
  when fullscreen is refused) onto a desktop drawn for it, where a recorded
  deck plays in ten steps: the agent's summary in a terminal, deck's real
  commands, the bar, the deck window, the question, the decision. Phones and
  windows up to 1100px get a still of it. `python3 site/tools/play.py` writes
  the page (the wallpaper, menu bar and dock are drawn there), `js/play-deck.js`
  is the deck, and `js/play.js` draws its window the way deck draws it and
  plays the steps. `tools/play-still.cjs` captures the still. The app in it is
  made up, and the page says so on every step.
- The docs, the guides and the telemetry page add `css/docs.css` (and
  `css/guide.css`); the changelog adds `css/changelog.css` and `js/changelog.js`.
- The guides are written in `tools/guides.py`; `python3 site/tools/build.py`
  writes them, their hub, `sitemap.xml` and `robots.txt`.

## Run it

```sh
cd site && python3 -m http.server 8000
```

To try the changelog, write its releases out the way the workflow does:

```sh
gh api -H "Accept: application/vnd.github.html+json" "repos/henit-chobisa/deck/releases?per_page=100" \
  --jq '[.[] | select(.draft | not) | {tag: .tag_name, name, prerelease, published: .published_at, url: .html_url, html: .body_html}]' \
  > site/changelog/releases.json
```

`site/changelog/releases.json` is ignored, and `.github/workflows/site.yml`
writes it when it publishes to GitHub Pages. The workflow also checks every
local link and every `#anchor` on the home page, and adds the visit counter
(`tools/analytics.py`) to every page.

## Check the layouts

With `releases.json` written as above, run:

```sh
cd site/tools
npm ci
npx playwright install chromium webkit
npm test
```

The checks start their own local server on port 8765. They cover narrow
phones, iPhone portrait and landscape sizes, a tall 1100×1700 window, desktop
WebKit and Chromium, and reduced motion: the links, the pinned steps of
`#x3` and `#x4` by Space and Page Down (by their buttons on phones and with
reduced motion), the `#x5` features by keyboard, the install tabs and copy
buttons, horizontal overflow, browser errors, and navigation on the inner
pages, and every step of `/play/`'s desktop by keys and by clicks, in and out of full screen. Set `SCREENSHOTS` to an existing directory to save captures. WebKit
checks do not replace testing Safari on a physical iPhone.

## What is real

- **The hero** is a screenshot of deck itself, on group 4 of a real deck.
- **The posts** in the LGTM section are real posts by developers, linked,
  quoted verbatim. None of them endorse deck.
- **The windows** in `#x3` to `#x6` are drawn to deck's own window: its band,
  panes and footer. The cache stampede in them is an example; `#x3` and `#x4`
  say so under the window.
- **The window in `/play/`** is drawn to deck's own: the band, the review and
  the conversation down the side, whole files with their range lit, a proposed
  change as rows going and arriving, values on the lines, the strip and the
  asks, at the sizes in `crates/render/src/metrics.rs` and in the colours of
  the hero's screenshot. The commands it types are deck's real ones. The app,
  its code, its test runs and the answers are made up, and the page says so.
