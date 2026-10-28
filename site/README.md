# site

deck's website: `index.html` (the home page), `docs/` (how to use it), the
changelog, the guides, and nothing to build. Plain HTML, CSS and JavaScript,
no libraries.

The home page is `index.html`, `css/landing.css`, `js/landing.js` and two
images in `assets/`: `deck-live.webp`, the real deck window in the hero, and
`henit.jpg`. The inner pages use `css/site.css`.

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
pages. Set `SCREENSHOTS` to an existing directory to save captures. WebKit
checks do not replace testing Safari on a physical iPhone.

## What is real

- **The hero** is a screenshot of deck itself, on group 4 of a real deck.
- **The posts** in the LGTM section are real posts by developers, linked,
  quoted verbatim. None of them endorse deck.
- **The windows** in `#x3` to `#x6` are drawn to deck's own window: its band,
  panes and footer. The cache stampede in them is an example; `#x3` and `#x4`
  say so under the window.
