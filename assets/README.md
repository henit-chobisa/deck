# assets

`mark-dark.svg`, `mark-light.svg` — the deck mark: a page with one line lit on
it. Drawn from the way `pill.rs` paints it in the bar, so the logo and the thing
it stands for are the same shape rather than two designs that have to be kept in
step. Gruvbox, because that is deck's own palette.

`themes.gif` — one deck in twenty editor themes, a frame each, the theme named
in the footer. Captured by opening the deck with `--theme` and cutting the
window out by its own bounds.

To remake it: `DECK_SHOW_ME=1` on a **debug** build opens a deck without the
bar, which is the only way to photograph the window. It does not exist in a
release binary, which is the point — see `main.rs`.

## The demo video

It does not live here. GitHub will not play a video committed to the repo — the
only form that renders as a player is an upload. Open the README in the web
editor, drag the file in, and GitHub inserts a `user-attachments` URL for it.
MP4 or MOV, under 10 MB.

## Screenshots

The hero slot is the commented-out block near the top of the README. What
belongs there is a deck **open** — two panes of code with the narration band
above them, around 1600 wide. A reader learns what deck is from that picture
faster than from any paragraph on the page.

Two more places want one, and each has a comment in the README marking the spot:

- **In the window** — walking a group, or a comment being written.
- **Diagrams** — a flow playing, which has to be a GIF: the travelling is the
  part prose cannot carry.

Caption anything that goes in. Every project worth copying puts a sentence under
its screenshot, because a picture of an interface means nothing to somebody who
does not yet know what they are looking at.

Keep them small. A README that takes a second to load is a README with a
screenshot nobody optimised.
