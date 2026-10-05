# Diagrams and pages

## Diagrams

Some things are in no file at all — how a click reaches a controller, the order
four services touch one request. A group can carry a picture beside its code, and
the picture can be walked.

```json
{
  "title": "what a Run click does",
  "nodes": [
    { "id": "run", "label": "Run", "role": "actor" },
    { "id": "q", "label": "another job in progress?", "role": "decision" },
    { "id": "queued", "label": "status QUEUED, no task" }
  ],
  "edges": [{ "from": "run", "to": "q" }, { "from": "q", "to": "queued", "label": "yes" }],
  "flows": [
    { "name": "where it stalls", "color": "#d29922", "steps": ["run", "q", "queued"] }
  ]
}
```

A **flow** is a named path through the picture. Press it and a current travels
the route while the rest of the diagram recedes — several to a picture, so the
happy path and the one that stalls can share the same seven boxes. Drag the
drawing anywhere; hold `⌘` or `ctrl` and scroll, or pinch, to zoom.

A picture is a pane like any other: it takes a `[name]` the prose can say, and a
point can land in it. `[point checkout]` lights the block whose `id` is
`checkout` and steps the rest of the picture back — and `[point 44-46 checkout]`
lights those lines **and** that block from one sentence, which is the thing
neither a diff nor a diagram can do alone.

```sh
deck group <path> \
  --say 'You press Connect and nothing happens. [point q] In [flow] the request
stops at the installed check. [pause] [point 44-46 q] And [guard] is that check.' \
  --diagram 'connect.json [flow] where the click stops' \
  --ref 'web/connect.tsx:40-52 [guard] the check that returns early'
```

There are no colours, sizes or positions in the format, on purpose. A node says
what it *is* and how much it matters, and deck owns every pixel — so two decks
drawing the same idea come out looking the same.

## Pages, for the idea that only moves

Some things are neither a file nor a picture. Two workers racing for one count, a
queue filling until the producer is told to stop, a pointer walking a list, a
latency chart bending when the cache goes cold — the argument *is* the movement,
and a still picture of it shows the start and the end with the interesting part
missing.

A group can carry a page: HTML the agent writes, rendered in a pane of the deck,
wearing the deck's own colours and hearing the same points the code does. It is
for something the reader watches happen. Boxes and arrows are a diagram.

```bash
deck group <path> \
  --say '[point read-a] Worker A reads the count: 5. [point read-b] Worker B reads
it too, and also sees 5. [point write-b] Both write 6, and one increment is gone —
[race], and [bump] is where it happens.' \
  --page 'race.html [race] two workers, one count' \
  --ref 'src/counter.ts:12-14 [bump] read, add one, write'
```

It is deliberately narrow. At most forty words may show — numbers and units are
free, so an axis costs nothing, but a page with sentences in it is a second
narration competing with the band. Nothing is fetched from anywhere; the page is
handed deck's palette as CSS variables rather than choosing its own. The skill
asks for it to be drawn for the pane's real size, with text at the window's own
size.

What it gets in return is the thing an artifact cannot have: it is told where the
reader is. `[point read-b]` reaches the page as an event, so the animation is bound
to the sentence being read rather than to a clock.

<p align="center">
  <img width="420" alt="rows shifting under a paged query, bound to the narration"
       src="https://github.com/henit-chobisa/deck/releases/download/v0.1.2/pages-demo.gif">
</p>

---

[← README](../README.md) · [How it works](how-it-works.md) · [The walk](the-walk.md) · [Diagrams and pages](diagrams-and-pages.md) · [Theming](theming.md) · [The catch](the-catch.md)
