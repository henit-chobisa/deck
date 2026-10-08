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
```

`site/frames/` is ignored, and `.github/workflows/site.yml` does the same copy
when it publishes to GitHub Pages.

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
recorder's icon painted out in the band's own colour, and written as WebP at
1200 and 720 wide (`l` and `s`; phones and Save-Data get `s`).

| frames | from | shows |
| --- | --- | --- |
| 0–137 | the stampede deck, 2–48 s, 3 a second | the walk, the pile-up, the fix |
| 138–233 | the payments chapter, 40–72 s, 3 a second | a timeline, then a diff |
| 234–305 | the search deck, 98–116 s, 4 a second | a comment typed and sent |
| 306–359 | the search deck, 132–150 s, 3 a second | the answer, as a chart |

The captions change at frames 0, 30, 88, 138, 234 and 306 (`CUTS` in
`js/main.js`). Recut the frames and those move with them.
