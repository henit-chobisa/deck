# The deck protocol, version 1

A deck is a directory an agent writes and a reader opens. This is what goes in
it, what comes back, and what a client has to do with both.

Everything here is frozen at `v: 1`. Anything not described is not part of the
protocol, and a client is free to ignore it.

**Who reads this.** People writing clients: deck's own window, deck.nvim, and
anything else that renders a deck or writes a review back. Two clients exist
today, which is why this document is normative rather than a description of
whatever the window happens to do.

**Agents do not read this.** An agent writes a deck through `deck new`,
`deck group` and `deck seal`, and the arguments to those three commands are the
whole of what it has to know — a few hundred tokens, not four hundred lines. If
a model ever needs this document in front of it to produce a deck, the command
surface is wrong and the fix belongs there, not here.

---

## The shape

```
<somewhere>/
  d-1788265010-8842.deck/
    deck.json          the header, written first
    g1.json            one group
    g2.json
    done               written last, and empty
  d-1788265010-8842.review
```

The directory is named `<id>.deck`. The review is written beside it as
`<id>.review`, not inside it — a deck is what the agent said, and the review is
what the reader answered; putting the answer inside the question makes the deck
directory mean two things.

**A deck is a directory because it fills up over time.** The agent writes the
header, then groups as it works them out, then `done`. A reader can start on
group one while group four is still being written. A client that reads the
directory once and never looks again is a conforming client, but it is throwing
away the reason for the shape.

Every file is UTF-8 JSON. Every document carries `v`.

---

## `deck.json` — the header

```json
{
  "v": 1,
  "id": "d-1788265010-8842",
  "cwd": "/Users/you/project",
  "title": "The batch counter stalls at 63",
  "total": 3
}
```

| field   | type            | required | meaning |
| ------- | --------------- | -------- | ------- |
| `v`     | integer         | yes      | Protocol version. `1`. |
| `id`    | string          | yes      | The deck's identity. Also its directory name and its review's filename. |
| `cwd`   | string          | no       | The project this deck was written against. |
| `title` | string          | yes      | The question or claim the deck answers. |
| `total` | integer         | no       | How many groups the agent intends to write. |

**`cwd` scopes the deck to a checkout.** Every relative path in a ref resolves
against it. Absent means unscoped: any project may render the deck, and paths
resolve against the deck directory itself. Two projects cannot steal each
other's decks by accident.

**`title` is a heading, in sentence case.** *The review path drops a comment*,
not *review path bug*. It is read cold — a client may show it before anything
else about the deck is on screen, with no context in front of it. A client
capitalises the first letter for display, because models are inconsistent about
it and a heading that starts lower-case reads as a fragment; the rest is left
alone, since a title may name code.

**`total` is a promise, not a count.** It is what the agent intends to write, so
a client can say *2 of 4* while it waits. It may be absent for a deck that
arrived whole. If more groups arrive than `total` claims, the deck is what
arrived; a count that a file can arrive and make a lie of is worse than no
count.

The header alone is enough to open a window. A deck with a header and no groups
yet is not an error — it is the normal first moment of a deck's life.

---

## `g<n>.json` — a group

One thing the agent wants to say, and the evidence for it.

```json
{
  "id": "g1",
  "ord": 1,
  "say": "The counter is decremented on the **error path** too.",
  "refs": [
    {
      "id": "g1r1",
      "file": "src/batch.ts",
      "range": [140, 148],
      "note": "decremented *twice* when the write fails"
    }
  ]
}
```

| field  | type     | required | meaning |
| ------ | -------- | -------- | ------- |
| `id`   | string   | yes      | Identifies the group within its deck. |
| `ord`  | integer  | no       | Position in the story, counting from 1. |
| `title` | string  | no       | What this group shows, as a one-line heading. See below. |
| `confidence` | object | no | How good the agent's basis for the claim is: `{ "level": "high" \| "medium" \| "low", "why"?: string }`. See below. |
| `say`  | string   | yes      | The narration. Markdown; see below. |
| `refs` | array    | no       | The evidence. One pane each. Defaults to empty. |
| `asks` | array    | no       | Up to five questions the reader can press to ask the agent. Defaults to empty. |

The filename must start with `g` and end with `.json`. Nothing else about it is
read — order comes from `ord`, not from the name and not from the filesystem.

### A group's title

`title` heads the group while it is on screen. The deck's `title` is the
question the whole deck answers; a group's is the step of the answer in front
of the reader. A client shows the group's title where it would otherwise show
the deck's, capitalised at the front like the deck's, and keeps the deck's title
somewhere quieter: deck's own window sets it in a small line above the group's
title, beside the group's number, and keeps it there for every group. A group
without one is headed by the deck's title, as every group was before the field
existed.

`deck group --title` trims it and refuses one that is empty, runs over a line, or
is longer than 80 characters.

### Confidence

`confidence` says how good the agent's basis for the claim is. It is not a probability. The level starts from the strongest kind of evidence the agent used, then drops one step for each problem:

- Start at `high` if it watched the behaviour (ran it, reproduced it, ran a test covering this case, read a trace from the system the claim is about), `medium` if it read every line the claim depends on without running it, `low` if it inferred it. Each gap counts once.
- Drop one level for each that applies: indirect (evidence from another environment, commit or input), incomplete (a config value, another service, or a function not seen), conflicting (something seen disagrees and is not explained), or a never/always claim resting on examples.
- A second, independent kind of evidence that agrees cancels one drop it does not share; repeating the same kind never raises it; below `low`, the claim is not made.

`why` is the basis in one line the reader can check: on `high`, what showed it; below, the drop or what is missing. `deck group --confidence` refuses medium or low without `--why`, a `--why` without a level, a level other than the three, and a basis that is empty, runs over a line, or is longer than 140 characters. deck's own window shows the level as three bars and its name after the group's title, and on a rest a card with what the level means and the basis. A group without `confidence` shows nothing; a confidence a client cannot read is treated as none.

### Questions to ask

`asks` is a list of up to five strings, each one question on one line, in the
order the agent wants them offered. A client shows them beside the group and,
when one is pressed, puts it to the agent as the reader's own question about
the group's claim, the way a remark sent at once is put. A client that does not
show them ignores the field; a group without it offers none.

`deck group` refuses more than five, an empty one, one over ninety characters,
one running over a line, and the same question twice.

### Questions renewed after an answer

A group's questions can be replaced after the agent has answered one, with
`deck asks <deck> --group <id> --ask …`. The new row is written beside the
group as `asks-<id>.json`, `{"asks": [...]}`, and the group file itself is never
rewritten. When that file exists, its `asks` are the group's questions. It is
not named `g*.json`, because a client takes any such file for a group.

The row is written in the order it will be shown: the questions that stay, in
their old order, then the new ones. `deck asks` refuses until the agent has
answered with `deck say`, and takes the same rules as `deck group --ask`, plus
at least one question. It works on a sealed deck.

A client that shows questions watches for the file while an agent is present,
sealed or not. It waits for an answer being spoken to finish before it shows
the renewed row.

### Order, and the gap rule

Group files land in whatever order the agent finishes them. The story does not:
the prose of group 3 assumes group 2 has been read.

**A group that arrives with a gap in front of it is kept, counted, and not
shown, until the gap fills.** A client shows the unbroken run from `ord: 1`.
Group 3 arriving before group 2 is held; when group 2 lands, both appear.

A deck whose groups have no `ord` at all is not claiming an order, and every
group it has is readable.

### The narration

`say` is Markdown, and only the inline part of it: `**bold**`, `*emphasis*`,
`` `code` ``, and a blank line between paragraphs. No headings, no lists, no
tables — a group is a claim, not a document.

Emphasis means *look at this word*. A client is free to render it in its accent
rather than in italics, which is what deck's own window does.

Three bracketed directives may also appear in `say`, and a client that does not
implement them must take them out rather than show them: `[pause]`, `[pause
short]` and `[pause long]` are beats for a voice; `[point …]` says where to
light while the words after it are read (see "Points"); and `[a-name]` matching
a ref's `name` is that pane, which a client may render as something the reader
can press. A directive it cannot parse is prose and stays as written.

### Points

`[point …]` holds one or two parts, separated by a space or a comma, in either
order:

- **lines**: `106-110` or `106`, the file's own line numbers. `+106-110` means
  the arriving half of a code ref with `after` or `before`: the code as it
  would be, numbered as it will be. A bare range means the code as it is.
- **a word**: letters, digits, `-` and `_`. It names a node of a diagram, by
  the node's `id`, or a name a page answers to (see "A page ref").

At most one of each, so `[point 106-110 checkout]` lights lines and a node
together. Anything else — two ranges, a `+` on a word, three parts — is not a
point, and stays in the prose as words.

**A point finds its pane by what the pane is showing**, never by its place:

- lines light the first code ref in the group whose `range` overlaps them, and
  nothing at all when none does;
- a word lights the first diagram with a node of that `id`, and the first page
  that answers to it — both, when both do;
- for a word, a pane brought in during a live walk is asked before the group's
  own, so an answer's points land in the picture or page carried in to answer
  it.

A point holds from where it is written until the next point. A client with a
voice moves the light as the words after it are spoken; a client without one
lights a point when the reader is on its sentence, by stepping, scrolling or
picking it. Either way, that moment is when the narration *reaches* the point.

---

## Refs

A ref is one pane of evidence. There are three kinds, and they are told apart by
what they carry: an entry with a `file` is code, an entry with a `diagram` is a
picture, and an entry with a `page` is markup the agent wrote. There is no
`kind` field. An entry should carry only one of the three; one that carries
more is read as the first it has, in that order — `file`, then `diagram`, then
`page`.

### A pane's name

Any ref may carry a `name`: what the narration calls that pane. `[retry]` in
`say` means the pane named `retry`, wherever the layout has put it. A name is
how prose says which pane it means without saying where it sits, which is the
window's decision and changes with its width.

A name is one lowercase ASCII word: a letter, then letters, digits or dashes,
not ending in a dash, at most 24 characters. `pause` and `point` are not names,
because `[pause]` and `[point …]` are already directives. Two refs in one group
never share a name; a client writing a deck is expected to refuse it.

A name in `say` that no ref of its group carries is prose, and stays as written.
A ref with no name is still a pane; the prose just cannot address it.

### A code ref

```json
{
  "id": "g1r1",
  "file": "src/batch.ts",
  "range": [140, 148],
  "note": "decremented *twice* when the write fails",
  "after": "  if (!ok) return;\n  pending -= 1;\n"
}
```

| field   | type              | required | meaning |
| ------- | ----------------- | -------- | ------- |
| `id`    | string            | yes      | Identifies the ref within its deck. |
| `file`  | string            | yes      | Relative to `cwd`, or absolute. |
| `range` | `[first, last]`   | yes      | Lines to light up. |
| `note`  | string            | no       | A short label. Inline `*emphasis*` only. |
| `name`  | string            | no       | What the narration calls this pane. See "A pane's name". |
| `after` | string            | no       | A proposed replacement for the range. |
| `before`| string            | no       | What the range replaced, for a change already made. |
| `values`| array of value    | no       | What lines in the range held when the code ran. |

**`range` is one-based and inclusive**, written as a two-element array.
`[140, 140]` is a single line. A client corrects a backwards or zero range
rather than rejecting the deck.

**Ranges are tight.** A two hundred line range is not a highlight, it is a
shrug. Nothing enforces this; it is what makes a deck worth reading.

**Two code refs in one group should not be the same file close together.** A
client writing a deck is expected to refuse it: two panes a few lines apart show
one block twice, the second opening below the first and repeating most of it.
Far-apart ranges in one file are a different thing and are allowed.

**`after` is a change that has not been made.** The range reads as what would
go, and `after` as what would replace it. The file on disk is never touched by
deck. A client that does not render `after` shows the range and ignores it.

**`before` is a change that already has been.** It is the mirror, and it exists
because the two halves of a change are never both on disk: once the edit is
written, a ref on those lines is a highlight of the result. So the range reads
as what arrived and `before` as what it replaced, drawn above it. The old text
has no line numbers, because the file does not contain those lines any more.

**Never both on one ref.** A range is what is going or what arrived and cannot
be drawn as both; a client writing a deck is expected to refuse it.

**`values` are what the code held, written on the lines that held it.** A claim
about a bug is usually a claim about a value, and a value on its line is
evidence the reader can check against the code beside it. Each is an object:

```json
{ "line": 6, "at": "orderId", "is": "undefined", "lit": true }
```

| field  | type            | required | meaning |
| ------ | --------------- | -------- | ------- |
| `line` | integer         | yes      | The file line, one-based, inside `range`. |
| `at`   | string          | yes      | The name on that line the value belongs to, as written there. |
| `is`   | string          | yes      | The value as the language prints it, or a one-line preview for one with `keys`. |
| `lit`  | boolean         | no       | The claim turns on this value. Defaults to false. |
| `after`| boolean         | no       | What the *proposed* code would hold, on a ref with `after`. Defaults to false. |
| `keys` | array of key    | no       | The keys inside, in the value's own order. |

A key is `{ "key", "is", "why", "lit" }` for one the claim names, or
`{ "more": 12 }` for a run of keys left out. A client draws a value without
`keys` right after the first whole-word occurrence of `at` on its line, or at
the line's end when `at` is not there. A value with `keys` is drawn under its
line as the preview in `is`, which the reader can open to the named keys and a
single count of the rest. On a ref with `after`, values describe the range as it
is, and values with `"after": true` describe the replacement, numbered as it will
be from the range's first line; a client draws them on the arriving lines, set
apart as predicted. On a ref with `before`, values describe the lines that
arrived. A client that does not render
`values` ignores them, and a ref without them is unchanged.

Only values the agent has seen belong here: from a run, a log, a payload or
arithmetic on the code. A value drawn on a line reads as observed. The one
exception is a worked example, for code that has not run: the narration says it
is one, and the ref's `note` starts `example:`, so the line says so too.

### A diagram ref

```json
{
  "id": "g1d1",
  "name": "flow",
  "note": "where the count is *lost*",
  "diagram": { "...": "see below" }
}
```

| field     | type    | required | meaning |
| --------- | ------- | -------- | ------- |
| `id`      | string  | yes      | Identifies the ref within its deck. |
| `diagram` | object  | yes      | The picture. |
| `note`    | string  | no       | A short label, as a code ref's is. |
| `name`    | string  | no       | What the narration calls this pane, as a code ref's `name` is. |

A picture is a pane like any other, so the prose has to be able to say which one
it means. Added after code panes got names, which had left a diagram as the one
kind of pane a sentence could not address.

### A page ref

```json
{
  "id": "g2p1",
  "name": "race",
  "note": "two workers, one count",
  "page": "<meta name=\"deck-points\" content=\"read-a read-b write-b\"><svg>…</svg><script>…</script>"
}
```

| field  | type   | required | meaning |
| ------ | ------ | -------- | ------- |
| `id`   | string | yes      | Identifies the ref within its deck. |
| `page` | string | yes      | The document as the agent wrote it, markup, style and script: a fragment or a whole HTML document. |
| `note` | string | no       | A short label, as a code ref's is. |
| `name` | string | no       | What the narration calls this pane, as a code ref's `name` is. |

A page is for the one thing in a deck that only makes sense moving: two workers
racing on a timeline, a queue filling, a structure changing. It is not a
document and not a diagram drawn in HTML: the prose carries the argument, and
boxes and arrows are a diagram. A client writing a deck is expected to refuse a
page that shows more than forty words: the words of its text outside `<script>`
and `<style>`, where a word is a run of characters between spaces whose first
letter or digit is a letter — so `200ms`, `64` and `-12%` are not counted.

**`page` is complete in itself.** A page must not load anything: no remote
scripts, styles, fonts or images, no requests, and no navigation away from the
document. A client renders it as if there were no network, and may refuse any
request a page makes. A client puts its own style and script in front of the
document; a page does not rely on what comes before it.

**A page answers to names, and a point moves it.** The names a page answers to
are the `id`s of its elements, every name listed in
`<meta name="deck-points" content="…">` (written with `name` before `content`),
and every name in a `data-on`, `data-show` or `data-from` attribute. A point can
only reach a name made of letters, digits, `-` and `_`. `[point read-b]` in
`say` lands in the page that answers to `read-b`. A client that shows pages
gives the document, before any of its own script runs:

- `window.deck.at`: the last of its names this page was told, or `null` before
  the first;
- a `deck:point` event on `window`, whose `detail` is that name, each time the
  narration reaches one of the page's names; and once more when the document
  has loaded, if `window.deck.at` is not `null`, so a page opened halfway
  through a walk shows the step the narration is on;
- a `deck:remark` event whose `detail` is the text of a comment the reader has
  just written about this page, so it can show what the comment is about;
- the class `on` on an element while the point is one of its `data-on` names;
  `data-show` elements shown only while the point is one of their names; and a
  `data-from` element shown from its first name onwards, in the order
  `deck-points` lists — a `data-from` naming something `deck-points` does not
  list is never shown.

A page is only ever moved forward. When the narration leaves it, it is told
nothing, `window.deck.at` keeps its last name, and the page keeps the moment it
is showing. It starts at rest and never plays on a clock of its own. A client
may offer to play a page again from the start; that reloads it, with
`window.deck.at` back to `null`.

**A page wears the reader's colours.** A client provides these CSS custom
properties, in the theme the rest of its window is in:

```
--deck-bg  --deck-fg  --deck-accent  --deck-on-accent  --deck-muted
--deck-edge  --deck-wash  --deck-add  --deck-del  --deck-comment
```

The page's background is transparent and its font is the client's monospace,
so a page that uses only these looks like part of the window in any theme.

A comment on a page carries `ref` and no `range`, as one on a diagram does, and
its `quote` is the ref's `name`, or `page` when it has none.

---

## The diagram format

A code ref can only point at code that exists. How two crates depend on each
other, what happens between a header landing and a review being written, why one
shape becomes another — that is structure *between* files, and prose carries it
badly.

**The format exists so that decoration is not expressible.** An agent handed a
blank canvas draws three rounded boxes and an arrow restating the sentence above
it. There are no colours here, no sizes, no positions: a node says what it *is*
and how much it matters, and the client owns every pixel. Two decks drawing the
same idea come out looking the same.

```json
{
  "title": "how a count goes missing",
  "flow": "down",
  "nodes": [
    { "id": "w", "label": "write", "note": "the happy path" },
    { "id": "q", "label": "ok?", "role": "decision", "weight": "accent" },
    { "id": "c", "label": "counter", "role": "store" }
  ],
  "edges": [
    { "from": "w", "to": "q" },
    { "from": "q", "to": "c", "label": "twice", "line": "dashed" }
  ],
  "clusters": [{ "label": "batch.ts", "nodes": ["w", "q"] }]
}
```

**Diagram**

| field      | type   | required | default |
| ---------- | ------ | -------- | ------- |
| `title`    | string | no       | — |
| `flow`     | enum   | no       | `down` |
| `nodes`    | array  | yes      | — |
| `edges`    | array  | no       | empty |
| `clusters` | array  | no       | empty |
| `flows`    | array  | no       | empty |

`flow` is `down` or `right`: which way later things sit from earlier ones.

**Node**

| field    | type    | required | default  |
| -------- | ------- | -------- | -------- |
| `id`     | string  | yes      | —        |
| `label`  | string  | yes      | —        |
| `note`   | string  | no       | —        |
| `role`   | enum    | no       | `step`   |
| `weight` | enum    | no       | `normal` |
| `lane`   | integer | no       | —        |

`role` is `step`, `decision`, `store`, `terminal` or `actor` — what the thing
*is*, which decides its shape. `weight` is `normal`, `accent` or `muted` — how
much it matters, which decides its colour.

`label` is a name: a type, a crate, a step. `note` is one line under it, and a
client may cut it short; a label may wrap, since cutting a name short loses the
one thing the node is for.

`lane` pins a node to a fixed track across the flow. It is what makes a sequence
diagram a sequence diagram: each participant keeps its column while time runs
down.

**Edge**

| field   | type   | required | default |
| ------- | ------ | -------- | ------- |
| `from`  | string | yes      | —       |
| `to`    | string | yes      | —       |
| `label` | string | no       | —       |
| `line`  | enum   | no       | `solid` |

`line` is `solid` or `dashed`: a real dependency, or a conditional one. `label`
is a word or two — `writes`, `on submit`, `anchor()`. A client keeps it to the
space between the nodes, so anything longer wraps or is cut.

An edge naming a node that does not exist is ignored, not fatal.

**Flow**

A path through the picture that the reader can play. Optional, and a diagram
with none is the ordinary case.

| field   | type   | required |
| ------- | ------ | -------- |
| `name`  | string | yes      |
| `steps` | array  | yes      |
| `color` | string | no       |

A step is a node id, or an object `{ "node", "color" }` when that one step wants
a colour of its own. `color` on the flow lights the whole path; on a step it
lights that step alone and beats the flow's. Both are hex, as elsewhere. Without
either, a flow uses the client's own accent.

`steps` are node ids, in the order they happen. A client shows one button per
flow and lights the steps in turn when it is played: the nodes on the path keep
their own appearance while everything else recedes, so a flow changes what
stands out rather than recolouring the picture.

The edge between two consecutive steps belongs to the second of them and lights
with it. Steps naming a node that does not exist are skipped, as an edge naming
a missing node is.

```json
"flows": [
  { "name": "cancel check", "steps": ["run", "worker", "cache"] },
  { "name": "re-run", "steps": ["run", "queued"] }
]
```

**Cluster**

| field   | type   | required |
| ------- | ------ | -------- |
| `label` | string | yes      |
| `nodes` | array  | yes      |

Containment, not layout: the nodes inside are still ranked by their edges.

**Cycles are allowed.** A graph that cannot be ranked has one edge of each cycle
set aside, and a client draws those differently — an arrow that visibly returns
rather than one that appears to point the wrong way by mistake. Which edge is
chosen follows declaration order, so an agent writing a state machine's steps in
order gets its return arrow picked.

---

## `done` — the seal

An empty file. Its presence means the agent has finished writing: there are no
more groups coming.

Until it exists, a client should assume more is on the way, and may say so. A
client that polls for new groups stops when it appears.

---

## `<id>.review` — the answer

Written beside the deck directory when the reader submits.

```json
{
  "v": 1,
  "deck": "d-1788265010-8842",
  "comments": [
    {
      "group": "g1",
      "ref": "g1r1",
      "file": "src/batch.ts",
      "range": [143, 143],
      "source": "extmark",
      "kind": "must-fix",
      "quote": "  pending -= 1;",
      "text": "This runs on the error path too."
    },
    {
      "group": "g2",
      "kind": "question",
      "quote": "The retry backoff is unbounded.",
      "text": "Is this true when the queue is empty?"
    }
  ]
}
```

**Review**

| field      | type    | required |
| ---------- | ------- | -------- |
| `v`        | integer | yes      |
| `deck`     | string  | yes      |
| `comments` | array   | yes      |

**Comment**

| field    | type            | required | meaning |
| -------- | --------------- | -------- | ------- |
| `group`  | string          | yes      | The group the comment was made in. |
| `ref`    | string          | no       | The ref it was made on. |
| `file`   | string          | no       | Repeated here so a comment is legible without the deck. |
| `range`  | `[first, last]` | no       | Where the comment sits now. |
| `source` | enum            | no       | How much to trust `range`. |
| `kind`   | enum            | no       | What the reader wants done. Defaults to `question`. |
| `quote`  | string          | no       | The text the comment was pinned to. Defaults to empty. |
| `text`   | string          | yes      | What the reader said. |

**`ref` and `range` are absent together** for a remark about the group itself.
That kind matters more than its size here suggests: *this whole approach is
wrong* is the most valuable thing a reader can say, and with only line-pinned
comments it has nowhere to go — it ends up pinned to some arbitrary line, hoping
the agent understands the remark is not about that line. For such a comment,
`quote` is the sentence being answered.

A comment on a diagram carries `ref` and no `range`: a picture has no lines.
`quote` is the label of whichever node was picked, or the diagram's title.

**`source` says how the range was arrived at**, and so how far to trust it:

| value         | meaning |
| ------------- | ------- |
| `extmark`     | Tracked through every edit. Exact. |
| `diff`        | Replayed through a diff of the file. Reliable. |
| `fingerprint` | Found again by matching surrounding lines. A good guess. |
| `stale`       | Could not be confirmed. Search for `quote` instead. |

Absent means the client did not say, which is not the same as any of the four —
search for `quote` rather than assume.

**`kind` is `must-fix`, `question` or `nit`.** Terse remarks read as neutral,
and an agent left to infer tone from prose gets it wrong. Naming it costs the
reader one keystroke and is the difference between a blocker and an aside.

---

## What a client must do

- **Read the header first.** A deck without a readable `deck.json` cannot be
  opened, and that is the only fatal parse failure.
- **Skip a group it cannot parse**, and show the rest. Losing one group of a
  story beats refusing to show any of it.
- **Hold a group that arrives behind a gap** in `ord`, and show it when the gap
  fills.
- **Resolve relative paths against `cwd`**, or against the deck directory when
  `cwd` is absent.
- **Never write to a file a ref points at.** Deck reads code. `after` is a
  proposal, not an edit.
- **Say what it could not do.** A ref whose file is missing is shown as a pane
  saying so, and so is a kind of ref the client does not draw: a client that
  cannot show pages shows a pane saying a page is there, rather than skipping
  the group that has one. A deck that silently drops the evidence for its own claim is worse
  than one that admits the file has moved.
- **Fail loudly to the agent.** A deck with a wrong `cwd` that is ignored in
  silence leaves the agent believing it presented something the reader never
  saw. A command has an exit code; use it.

## Forward compatibility

- A document with a `v` this client does not know is refused, not guessed at.
- Unknown fields are ignored. A future version may add them, and a v1 client
  reading a v1 document must not break on one it does not recognise.
- Unknown enum values — a `role`, a `kind`, a `source` — are treated as the
  default for that field rather than as an error.

## Not in version 1

Named here so nobody has to guess whether they were forgotten:

- **Threads.** A comment is one remark. There is no reply, and no comment on a
  comment.
- **Attachments.** No images, no files, no links beyond what prose can say.
- **Anything about which agent wrote the deck.** The protocol is a format, not a
  handshake; a deck says nothing about who produced it.
