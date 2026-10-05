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

## The recordings

`hero.gif` — the README's hero: five seconds, under 2 MB, so it is kept here
and always loads.

The four use-case recordings are GIFs too, and they do not live here: each is
several MB, and committed they would be in every clone forever. They sit on the
`pr-assets` branch under `readme/`, linked by commit so a later push to that
branch cannot break them.

They are cut from screen recordings of the deck window at 1165×954 (the size in
`~/.deck/window.json`): 15 frames a second, 1400–1600 wide, waits removed, and a
256-colour palette with a bayer dither, which keeps one under 10 MB.

Caption anything that goes in. A picture of an interface means nothing to
somebody who does not yet know what they are looking at.

`hero.png` is the still the hero used before the recordings; nothing links to it
now.
