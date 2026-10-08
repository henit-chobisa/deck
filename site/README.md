# site

deck's website: `index.html` (the scroll story), `docs/` (how to use it), and
nothing to build. Plain HTML, CSS and JavaScript; the one library is Lenis, for
smooth scrolling, and the page works without it.

## Run it

```sh
cd site && python3 -m http.server 8000
```

The frames are not in this branch. Copy them in first:

```sh
git fetch origin pr-assets
git archive FETCH_HEAD site-frames | tar -x -C /tmp
mkdir -p site/frames && cp -r /tmp/site-frames/l /tmp/site-frames/s site/frames/
# Once the Retina export is published on pr-assets:
cp -r /tmp/site-frames/retina site/frames/
```

To try the changelog, write its releases out the way the workflow does:

```sh
gh api -H "Accept: application/vnd.github.html+json" "repos/henit-chobisa/deck/releases?per_page=100" \
  --jq '[.[] | select(.draft | not) | {tag: .tag_name, name, prerelease, published: .published_at, url: .html_url, html: .body_html}]' \
  > site/changelog/releases.json
```

`site/frames/` and `site/changelog/releases.json` are ignored, and `.github/workflows/site.yml` does the same copy
when it publishes to GitHub Pages.

## Check the layouts

After copying the frames above, run:

```sh
cd site/tools
npm ci
npx playwright install chromium webkit
npm test
```

The checks start their own local server on port 8765. They cover narrow phones,
iPhone portrait and landscape sizes, desktop WebKit and Chromium, reduced motion,
story controls, commenting, installation tabs, and navigation on the inner pages.
Set `SCREENSHOTS` to an existing directory to save full-page captures. WebKit
checks do not replace testing Safari on a physical iPhone.

Phones and desktop share the pinned scroll story, stars, and background text.
Only the system's reduced-motion setting uses Previous/Next controls instead.
The stacked layout centres the caption and recording vertically as one group;
text stays left-aligned.

## What is real

- **The reel** (*In the window*) is 360 stills from screen recordings of deck
  0.1.4, played by scroll. Nothing in it is drawn for the site.
- **Try it** is the stampede deck from that recording — the same title,
  narration, points, code, map and page — running in the browser.
  `js/requests.js` is the deck's page, `requests.html`, mounted in place of a
  pane. The answers to the three questions are written in advance, and the
  page says so.
- **The bar** in *Same question, same agent* is drawn to `pill.rs`: 560 by 48,
  the same type sizes, padding and colours.

## The frames

Four stretches of three recordings, cropped to the window, the screen
recorder's icon painted out in the band's own colour. The legacy exports are
1200 and 720 pixels wide. The new lossless exports in `retina/l` and `retina/h`
are 1200 pixels and the native crop width (2326–2374 pixels).

The player chooses by painted width × device pixel ratio, not by a phone
breakpoint. It keeps at most eight decoded frames and three pending requests,
rather than decoding the entire Retina recording into several GB of memory.
The old 1200px frames are a fallback while the new assets are being published.

To export from the original October 5 recordings (requires FFmpeg and Pillow):

```sh
python3 site/tools/export-frames.py ~/Desktop --output site/frames/retina
```

Publish that directory as `site-frames/retina/` on the **pr-assets** branch before
deploying the site. Do not commit these large generated files to `main`.
The site workflow copies both Retina tiers and checks their frame counts.

| frames | from | shows |
| --- | --- | --- |
| 0–137 | the stampede deck, 2–48 s, 3 a second | the walk, the pile-up, the fix |
| 138–233 | the payments chapter, 40–72 s, 3 a second | a timeline, then a diff |
| 234–305 | the search deck, 98–116 s, 4 a second | a comment typed and sent |
| 306–359 | the search deck, 132–150 s, 3 a second | the answer, as a chart |

The captions change at frames 0, 30, 88, 138, 234 and 306 (`CUTS` in
`js/main.js`). Recut the frames and those move with them.
