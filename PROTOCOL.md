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
| `say`  | string   | yes      | The narration. Markdown; see below. |
| `refs` | array    | no       | The evidence. One pane each. Defaults to empty. |

The filename must start with `g` and end with `.json`. Nothing else about it is
read — order comes from `ord`, not from the name and not from the filesystem.

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

---

## Refs

A ref is one pane of evidence. There are two kinds, and they are told apart by
what they carry: an entry with a `file` is code, an entry with a `diagram` is a
picture. There is no `kind` field, and no entry is both.

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
| `after` | string            | no       | A proposed replacement for the range. |

**`range` is one-based and inclusive**, written as a two-element array.
`[140, 140]` is a single line. A client corrects a backwards or zero range
rather than rejecting the deck.

**Ranges are tight.** A two hundred line range is not a highlight, it is a
shrug. Nothing enforces this; it is what makes a deck worth reading.

**`after` is a change that has not been made.** The range reads as what would
go, and `after` as what would replace it. The file on disk is never touched by
deck. A client that does not render `after` shows the range and ignores it.

### A diagram ref

```json
{
  "id": "g1d1",
  "note": "where the count is *lost*",
  "diagram": { "...": "see below" }
}
```

| field     | type    | required | meaning |
| --------- | ------- | -------- | ------- |
| `id`      | string  | yes      | Identifies the ref within its deck. |
| `diagram` | object  | yes      | The picture. |
| `note`    | string  | no       | A short label, as a code ref's is. |

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
  saying so. A deck that silently drops the evidence for its own claim is worse
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
