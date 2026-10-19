# Pictures

§4 of the deck skill: `--diagram`, the JSON a picture is written in, and flows. Read it before you draw one.

§1–§2 and §6–§8 are in [SKILL.md](../SKILL.md), §3 in [groups.md](groups.md), §4 in [pictures.md](pictures.md), §5 in [pages.md](pages.md). "Plan the context" and the other sections on how to tell it are in [writing.md](writing.md).

## Contents

- 4. When the answer is a picture

## 4. When the answer is a picture

Some things are in no file at all. How a click reaches a controller, what calls what, the
order four services touch one request, the states a job moves through — that is structure
*between* files, and a code ref cannot point at it.

**A question about a flow is a diagram.** If answering means naming three or more places
and the order they run in, draw it. A deck that answers a flow question with six code refs
and no picture has made the reader assemble the diagram in their own head — which is the
work they asked you to do.

**Put the picture and the code in the same group.** The diagram says *where* in the flow
you are, the refs say *what* the code does there. Apart they are two things to hold;
together they are one.

```bash
deck group <path> \
  --title 'The installed check returns early, silently' \
  --say 'You press Connect and nothing happens — no error, no spinner, nothing.

[point q] In [flow], the request stops at the installed check before it ever
leaves the browser. [pause] [point 44-46] And [guard] is that check: it returns
early when the app is missing, without saying so to anybody.' \
  --diagram 'connect.json [flow] where the click stops' \
  --ref 'web/silo/connect.tsx:40-52 [guard] the check that returns early'
```

**A picture is a pane, so it takes a name and a point like any other.** The
`--diagram` argument has the same shape as a `--ref`: the file, then an optional
`[name]`, then a note. The prose says [flow] and the reader can press it. And
[point q] lights the block whose `id` is `q` — the id you wrote in the JSON is
the name a sentence can carry, which is a reason to give nodes ids that mean
something rather than `n1`, `n2`, `n3`.

While a point is on one block, the rest of the picture steps back — the same
thing a spotlight does to the lines around it. So a diagram is walked the way
code is: one block at a time, in the order the sentences go.

```json
{
  "title": "connect to controller",
  "flow": "down",
  "nodes": [
    { "id": "b", "label": "Connect", "role": "actor" },
    { "id": "h", "label": "onConnect", "note": "web/silo/connect.tsx" },
    { "id": "q", "label": "installed?", "role": "decision", "weight": "accent" },
    { "id": "c", "label": "JiraController", "note": "apps/api" },
    { "id": "s", "label": "workspace_credentials", "role": "store" }
  ],
  "edges": [
    { "from": "b", "to": "h" },
    { "from": "h", "to": "q", "label": "POST /connect" },
    { "from": "q", "to": "c", "label": "yes" },
    { "from": "c", "to": "s", "label": "upsert", "line": "dashed" }
  ],
  "clusters": [{ "label": "browser", "nodes": ["b", "h"] }]
}
```

Only `nodes` is required.

- `flow` — `down` or `right`. Default `down`.
- `role` — `step` (default), `decision`, `store`, `terminal`, `actor`. What the thing
  *is*. It decides the shape.
- `weight` — `normal` (default), `accent`, `muted`. How much it matters. One accented
  node is usually enough.
- `note` — one line under the label. A file path or a qualifier, not a sentence.
- `lane` — an integer pinning a node to a column, so a sequence keeps its tracks.
- `line` on an edge — `solid` (default), or `dashed` for a conditional one.
- Edge `label` is a word or two: `POST /connect`, `on submit`. Longer gets cut.
- `clusters` are containment, for a boundary worth seeing: browser against server.
- Cycles are fine. The returning edge is drawn as one that visibly comes back.
- `flows` are paths through the picture the reader can play, and they are how one
  diagram answers more than one question. Each is `{ "name", "steps" }` where the
  steps are node ids in the order they happen. A button per flow sits in the
  drawing's top-right; pressing it sends a current along the path while
  everything else recedes.

  Give a flow a `color` when there are two, so they can be told apart without
  pressing both. A step can be written `{ "node": "x", "color": "#c33" }` to
  light one stop differently from the rest — the place it goes wrong, in a path
  that is otherwise calm.

  Flows and points share the picture and do not fight over it. A point is your
  finger while you talk; a flow is the reader pressing play, and theirs wins for
  as long as it runs. Write both: the flows let them retrace it on their own
  after you have stopped talking, which is when most of the understanding
  happens.

```json
"flows": [
  { "name": "cancel check", "steps": ["run", "worker", "cache"] },
  { "name": "re-run", "steps": ["run", "queued"] }
]
```

  Reach for them when a picture holds two paths that a reader would otherwise
  have to trace with a finger — the happy path and the one that stalls, the
  first run and the second. Two flows over one diagram beat two diagrams,
  because the thing being compared is what they share.

There are no colours, sizes or positions, on purpose. A node says what it is and how much
it matters; deck owns every pixel. **Six to ten nodes** — twenty is a wall. Every node
earns its place: cut the ones that only pass a value along.

**Every node needs an edge.** A box with nothing joined to it is the reader
stopping mid-sentence to work out what it is doing there. If something belongs in
the story but not in the flow, say it in the prose and leave it out of the
picture. And a picture brought again under a name already borrowed replaces the
one that was there, so *draw it better* is a revision rather than a second
drawing beside the first.

Code refs are always laid out before diagrams whatever order you typed them. If the
picture has to be read first, it needs a group of its own.
