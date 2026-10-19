# Pages

§5 of the deck skill: `--page`, markup that moves when the narration points. Read it before you write one.

§1–§2 and §6–§8 are in [SKILL.md](../SKILL.md), §3 in [groups.md](groups.md), §4 in [pictures.md](pictures.md), §5 in [pages.md](pages.md). "Plan the context" and the other sections on how to tell it are in [writing.md](writing.md).

## Contents

- 5. When the answer only makes sense moving
  - What pages are for
  - A catalog of shapes that have worked
  - It starts at rest, and moves when you point
  - Move it so they can follow it
  - Move it smoothly
  - Real numbers, in real units
  - Draw for the room you have
  - Forty words, and that is the whole rule
  - Say what you answer to
  - Wear the deck's colours
  - Nothing loads from anywhere
  - Do it again, in the same place
  - What a good page looks like
  - Bringing one mid-walk

## 5. When the answer only makes sense moving

**A page is something they watch happen.** A race between two workers on a time axis.
A queue filling until the producer is told to stop. A pointer walking a list and the
node it unlinks. A latency chart that bends when the cache goes cold. The reader sees
the thing happen, in time with the sentence that describes it, and understands it
the way they would if they had stepped through it in a debugger themselves.

It is not a diagram drawn in HTML. **Boxes and arrows are a picture (§4) — always.**
Deck draws pictures itself, laid out, themed, folding, with blocks a point can light.
A page that is a row of rounded rectangles with labels in them is a worse picture,
and it is the page agents write by default. Do not write it.

**If you would have to draw it twice, it is a page.** That is the test. A before and
an after, with the interesting part in the gap between them — that gap is the whole
answer, and a still picture cannot hold it. If the drawing would look the same at
every sentence, it is structure, and structure is a picture. If something moves,
grows, fills, races or changes value when you point, it is a page.

```bash
deck group <path> \
  --title 'Two workers, one lost increment' \
  --say '[point read-a] Worker A reads the count: 5. [point read-b] Worker B reads
it too, and also sees 5. [point write-b] Both write 6, and one increment is gone —
[race], and [bump] is where it happens.' \
  --page 'race.html [race] two workers, one count' \
  --ref 'src/counter.ts:12-14 [bump] read, add one, write'
```

`--page` takes the same shape `--ref` and `--diagram` do: the file, then an optional
`[name]`, then a note. It is a pane like any other — it takes a name the prose can say,
it folds, and a point can land in it. An agent reaches for a file without thinking and
a picture when prompted; this is the pane that gets forgotten, so ask the question on
purpose.

### What pages are for

Each of these is a page, and each says what moves:

- **A timeline.** Lanes for the actors — threads, requests, services — on a time axis
  in real units; one actor is one row. Concurrency, races, deadlocks, retries and
  backoff, where the latency went. Bars grow as time runs; the sentence decides how far
  it has run.
- **Something filling.** A queue, a buffer, a pool, a token bucket, a rate limit.
  Items arrive and leave; the level is the story; the moment it is full is the point.
- **A structure changing.** An array with an index walking it, a linked list being
  spliced, a tree rebalancing, a cache evicting its oldest entry, a hash table
  resizing. The same elements, moved — never redrawn.
- **A chart that bends.** Before and after a change: latency, memory, a distribution,
  a counter over time. The axes stay put and the data moves between them.
- **A value through time.** Every variable that matters as a track, and the moment one
  goes wrong — the count that should be 7 and is 6.

### A catalog of shapes that have worked

The list above is by what moves; this one is by the question the group answers. They
are options that have landed with readers, not a template. Pick by the question, not by
habit. The timeline is the commonest one to overuse: lanes are for two or more actors,
and one actor is one row. Each entry says what moves and what it needs. The sentences
move the drawing, and the code sits beside it ("Where it goes", in "Plan the context").

1. **Why does this break only under load or concurrency?** The timeline above, with the
   shared value as a track beneath it. "What a good page looks like" below is this one.
   The fix is the next group: the same page brought back under the same name, the
   second actor refused. Needs: the order of reads and writes, and how long the slow
   step takes.

2. **Should we make this change?** For a plan or a proposal, not ship mode. A chart that
   bends, in production numbers: the latency percentiles today move to what the option
   gives. If the option moves cost somewhere else, a worker or a queue, that is the next
   group, as something filling over a busy day. Needs: real percentiles from traces, and
   the peak the system sees.

3. **What does the input become on its way through?** Where it stops is a flow on the
   picture. The page is the one record: its fields filling in or dropping out at each
   step. Needs: one real input and what each step does to it.

4. **Why is this number what it is?** Asked mid-walk, answered with evidence: the
   measurement the decision was made on, and a marker that moves along it to the value
   that shipped, then past it to where it gets worse. Needs: the data, not a guess.

5. **Where does the time go?** One request, its calls in one row as they run. An N+1
   grows call by call; the fix collapses it to one, and the total falls with it. Needs:
   the call count, the time per call, any pool limit.

6. **Why does the total not add up?** A running total: the opening value, each movement
   applied in turn, then what it should be beside what it is, and the gap as the bad
   outcome. Needs: the figures the books or the counter report.

7. **Why does the user see an old value?** Two values through time, the source and the
   copy the user reads, moving apart at the change and meeting again when the copy
   expires. The window between them is the bug. Needs: when the value changed and when
   the copy expires.

A question about how a system is put together is not on this list. It is a picture
with a flow per journey (the system-sized rules in "Plan the context", and §4). When
nothing here fits, name the question first. Something changing over time, a number
before and after, or a quantity is a page. One thing moving through others is a picture
with a flow. Two paths through the same parts are one picture with two flows. How a
whole splits into parts, or how two things relate, is a picture or no drawing at all.

### It starts at rest, and moves when you point

A page must come up still. No autoplay, no timer, no `setTimeout` chain that starts on
load. The reader is listening to a sentence; if the animation is running on its own clock
it is already three steps past the words, and they are watching one thing while hearing
another.

What moves it is a point:

```js
window.deck = window.deck || { at: null }   // deck installs this before your script runs

addEventListener('deck:point', (e) => {
  render(e.detail)        // e.detail is one of your names: the one pointed at
})

addEventListener('deck:remark', (e) => {
  // e.detail is the text of the comment the reader just pressed in the rail
})
```

`window.deck.at` holds the last name the page was told, and `deck:point` fires each time
the narration points at a new one of *yours* — including once more when the document
finishes loading, so a page brought in halfway through a walk comes up on the step the
narration is actually on rather than its first frame.

It only ever moves forward. When the narration points into another pane, or the voice
stops, the page is told nothing and keeps the moment it is showing — so do not reset on
anything but a name you know. `again` reloads the page, which is the way back to rest.

So write the page as a **function of the point**, not as a sequence that plays. Given
`hop`, draw the state at `hop`. A page written that way can
be walked forwards, walked backwards, and re-entered in the middle, which is what the
reader will actually do to it.

For marking the moment inside a scene that already moves — the bar that just landed, the
label that appears when the count goes wrong — no script is needed: `data-on="hop
settle"` puts the class `on` on an element while the point is either name,
`data-show="hop"` shows it only then, and `data-from="hop"` shows it from `hop` onward
in the order `deck-points` lists. Style `.on` yourself. A page whose parts only light, and
otherwise sit still, is a picture: write the picture.

`deck:remark` is the other half: the reader pressing a comment in the rail, saying *this
part*, about something that moves. Nothing else they could have written it against can
hear them.

### Move it so they can follow it

How motion explains, and how it stops explaining, is well studied. Four rules:

- **The same thing stays the same thing.** The request that was in the queue at one
  sentence is the same dot at the next — it moves there. Redraw the whole scene with
  new shapes and the reader has to find everything again; move the shapes they already
  know and their eye follows by itself.
- **One kind of change per sentence.** Time advancing, *or* a value changing, *or*
  something arriving — not all three at once. Staged changes are measurably easier to
  follow, and people prefer them slower than whoever made them expects: 400 to 900
  milliseconds a change, eased, never a snap.
- **Stop just before it goes wrong.** Give the moment before the outcome its own
  point and the outcome the next one. The reader predicts what happens, and then
  watches whether they were right.
- **Light the one thing the sentence is about.** In `--deck-accent`, and only that;
  everything else in `--deck-edge` and `--deck-muted`. When the sentence moves on, the
  light moves with it.

- **Make the thing travel.** When an element changes place — a pane sliding from slot 3
  to slot 2, an entry leaving the cache — animate it from where it was to where it goes.
  Swapping the label in a box that stays still is a redraw, and the reader has to work
  out what moved.

And honour `prefers-reduced-motion`: jump straight to the state instead of moving there.

### Move it smoothly

Motion that jitters reads as broken, and these are the ways it does:

- **Build once, then only change.** Make the elements when the page loads and keep them;
  each frame sets attributes on what is already there. Clearing the drawing and building
  it again every frame is the commonest jitter there is.
- **Ease over time, not over frames.** Take `now` from `requestAnimationFrame`, start the
  clock on the first frame you draw, and run a smoothstep from where you were to where
  you are going over 400 to 900 milliseconds. `x += (target - x) * 0.1` each frame snaps
  on the first frame, runs twice as fast on a 120 Hz screen, and stalls on a dropped one.
- **Carry on from where it is.** A point that arrives mid-movement starts from the
  current state, never from zero and never from the last target.
- **Whole pixels.** Round every position to the device pixel; put a 1-pixel line on the
  half pixel. Text never slides continuously — it fades in place, or moves by a whole
  pixel at a time.
- **Never animate an HTML element's layout.** Not `left`, `top`, `width`, `height` or a
  margin — each frame of those re-lays out the page. Move an element with `transform`,
  fade it with `opacity`. On SVG or a canvas, anything goes: change any attribute of an
  element that persists, or repaint the canvas every frame.

### Real numbers, in real units

Use the numbers the code uses — the 30-second timeout, the queue of 64, the 2 ms the
read takes — and put them on the axis in their units. A timeline with no scale is a
cartoon of the bug; one with `0ms … 12ms` under it is the bug. Numbers and units are
free against the word limit below.

### Draw for the room you have

A page gets a whole pane, and a pane is whatever shape the window left it — often tall
and narrow, sometimes wide and short. **Measure it and draw for it**: read the size of
the drawing area in a `ResizeObserver`, and lay out from that — never by measuring inside
the animation loop.

**Fill it.** The drawing should take most of the pane: lanes as tall as the room allows,
the axis across the whole width. A strip of drawing along the top of a tall, empty pane
reads as something that has not finished loading.

**Never scale text with a `viewBox`.** An SVG stretched to fill the pane scales its text
with it, so the labels come out three times the size of everything else in deck, too
wide for the boxes they were written to fit — the page that started this section's
rewrite looked exactly like that. Text is 11 to 13 pixels, set in CSS, always; the
geometry grows to the room, and the words stay the size of the window's words.

**No text touches other text.** The pane is often narrower than the one you pictured, so
every label has to survive the narrow case: a legend, a counter and a title each get a
band of their own rather than sharing a line; axis labels are spaced from the measured
width and thinned when they would meet; a label that does not fit its bar goes beside it
or nowhere. Before you send it, picture the page at about 400 pixels wide. Text drawn
over text is the first thing a reader sees, and it tells them nobody looked at the page.

### Forty words, and that is the whole rule

`deck group` counts the words a page shows and refuses it over forty. Numbers and
measurements — `200ms`, `64`, `-12%` — are not counted. Not a style note — the command
fails and you write it again.

The prose carries the argument. Words on the page are a second narration competing with
the band, read at a different speed by the same reader. **Labels, numbers, a field name.**
Not sentences, not a heading and three bullets, not an explanation of what they are
looking at. If the page needs a paragraph to make sense, the paragraph belongs in the
`say`.

### Say what you answer to

A point finds its pane by name, and a page answers to the `id` of any element in it, any
name in a `data-on`, `data-show` or `data-from`, and whatever it declares here:

```html
<meta name="deck-points" content="rest merge hop settle">
```

Declare them whenever the interesting names are moments rather than things on screen —
and on a page that moves, they nearly always are: `hop` is not an element, it is what
the page looks like at a moment.

Without that, `[point hop]` names nothing, and `deck group` says so while you are still
running and can fix it. The failure it was added for is worth knowing, because it is
silent from the reader's side: the code lit up beside a page that never moved.

### Wear the deck's colours

Deck hands every page its palette as CSS variables, before your markup:

```
--deck-bg  --deck-fg  --deck-accent  --deck-on-accent  --deck-muted
--deck-edge  --deck-wash  --deck-add  --deck-del  --deck-comment
```

Use them and nothing else. **Never write a literal colour.** The reader chose their
editor's theme and the rest of the window is honouring it; one page with `#1e1e1e` hard
coded in it is the pane that looks like it came from somewhere else. `--deck-add` and
`--deck-del` are for good and bad outcomes — the write that was lost, the request that
got through.

The background is transparent and the font is already set to the window's mono. Build on
that rather than reasserting it.

### Nothing loads from anywhere

There is no network behind a page. No CDN, no chart library, no Google Fonts, no `fetch`,
no image URL. Every script and style is inline in the file you write.

This is deliberate and not a limitation to work around: a surface for reading your own
code should not be making requests while you read it. SVG, a canvas, and
`requestAnimationFrame` are the whole toolkit, and they are enough for every page above.

### Do it again, in the same place

Asked for a clearer one? **Bring it again under the same `[name]`.**

```bash
deck bring <path> --page 'race.html [race] the same race, slower'   # and again, revised
```

A page or a picture brought under a name that is already borrowed replaces what
was there, keeping its place in the room and the fold it already had. Two
drawings of one idea side by side is the reader deciding which one you meant,
which is worse than the first drawing was.

### What a good page looks like

The race behind a lost increment: two workers on a timeline, and the count as a track
under them. Each sentence runs time forward to its moment — the same bars grow, the
playhead moves, and the one bar the sentence is about is lit — until the count reads 6
where it should read 7.

```bash
deck group <path> \
  --title 'Two workers, one lost increment' \
  --say '[point read-a] Worker A reads the count: 5. [point read-b] Before A is done,
worker B reads it too, and also sees 5. [point write-a] A writes 6. [point write-b] Then
B writes 6 over it, and one increment is gone. That is [race], and [bump] is
where it happens: a read and a write with nothing holding them together.' \
  --page 'race.html [race] two workers, one count' \
  --ref 'src/counter.ts:12-14 [bump] read, add one, write'
```

```html
<meta name="deck-points" content="read-a read-b write-a write-b">
<style>
  body { margin: 0; height: 100vh }
  svg  { display: block; width: 100%; height: 100% }
  text { font: 12px ui-monospace, monospace; fill: var(--deck-muted) }
  .tick { text-anchor: middle }  .lost { text-anchor: end }
  .name { fill: var(--deck-fg) }
  .bar { fill: var(--deck-edge); transition: fill .4s }
  .bar.lit { fill: var(--deck-accent) }
  .label { transition: opacity .4s, fill .4s }  .label.lit { fill: var(--deck-on-accent) }
  .lost { fill: var(--deck-del); font-weight: 600; transition: opacity .4s }
</style>
<svg id="s"></svg>
<script>
// Milliseconds, from the code: a read takes 2, the work 6, the write 2.
const spans = [['A', 0, 2, 'read 5'], ['A', 2, 8, ''], ['A', 8, 10, 'write 6'],
               ['B', 2, 4, 'read 5'], ['B', 4, 10, ''], ['B', 10, 12, 'write 6']]
const at  = { 'read-a': 2, 'read-b': 4, 'write-a': 10, 'write-b': 12 }
const lit = { 'read-a': 0, 'read-b': 3, 'write-a': 2, 'write-b': 5 }   // one thing each
const svg = document.getElementById('s'), NS = 'http://www.w3.org/2000/svg'
const make = (tag, cls, text) => {
  const e = document.createElementNS(NS, tag)
  if (cls) e.setAttribute('class', cls)
  if (text) e.textContent = text
  return svg.appendChild(e)
}

// Built once. Every frame after this only moves what is already here.
const ticks = [0, 2, 4, 6, 8, 10, 12].map(ms => make('text', 'tick', ms + 'ms'))
const names = ['worker A', 'worker B', 'count'].map(n => make('text', 'name', n))
const bars = spans.map(() => make('rect', 'bar'))
const labels = spans.map(s => make('text', 'label', s[3]))
const count = make('text', 'name', '5'), lost = make('text', 'lost', 'should be 7')
const head = make('line'); head.setAttribute('stroke', 'var(--deck-accent)')

let W = 0, H = 0, shown = 0, point = null, from = 0, to = 0, t0 = null, frame = 0
const px = v => Math.round(v * devicePixelRatio) / devicePixelRatio  // whole pixels

function place(t) {                        // the scene at time t, in the room there is
  // The room, filled: lanes as tall as the pane allows, bars more than half a lane.
  const lane = Math.max(28, (H - 80) / 3.4), bar = px(Math.min(lane * 0.55, 56))
  const x = ms => px(110 + (W - 150) * ms / 12)
  const top = px(30), row = { A: top, B: top + lane }, mid = bar / 2 + 4
  ticks.forEach((e, i) => { e.setAttribute('x', x(i * 2)); e.setAttribute('y', top + lane * 3.4) })
  names.forEach((e, i) => { e.setAttribute('x', 16); e.setAttribute('y', (i === 2 ? top + lane * 2.5 : row['AB'[i]]) + mid) })
  spans.forEach(([who, a, b, label], i) => {
    const end = Math.max(a, Math.min(b, t)), done = t >= b, on = lit[point] === i && done
    const width = Math.max(0, x(end) - x(a) - 2)
    bars[i].setAttribute('x', x(a) + 1); bars[i].setAttribute('y', row[who])
    bars[i].setAttribute('width', width); bars[i].setAttribute('height', bar)
    bars[i].setAttribute('class', on ? 'bar lit' : 'bar')
    labels[i].setAttribute('x', x(a) + 7); labels[i].setAttribute('y', row[who] + mid)
    labels[i].setAttribute('class', on ? 'label lit' : 'label')
    // A label fades in once its bar is whole, and only if it fits inside it.
    labels[i].style.opacity = done && label.length * 7.5 + 14 < width ? 1 : 0
  })
  count.textContent = t >= 10 ? '6' : '5'
  count.setAttribute('x', 110); count.setAttribute('y', top + lane * 2.5 + mid)
  lost.setAttribute('x', x(12)); lost.setAttribute('y', top + lane * 2.5 + mid)
  lost.style.opacity = point === 'write-b' && t >= 12 ? 1 : 0
  const h = x(t) + 0.5                     // a 1px line on the half pixel stays sharp
  head.setAttribute('x1', h); head.setAttribute('x2', h)
  head.setAttribute('y1', top - 12); head.setAttribute('y2', top + lane * 3)
}

function run(now) {                        // eased over time, never over frames
  if (t0 === null) t0 = now                // starts on the first frame it draws
  const k = Math.min(1, (now - t0) / 700), e = k * k * (3 - 2 * k)
  shown = from + (to - from) * e
  place(shown)
  frame = k < 1 ? requestAnimationFrame(run) : 0
}

addEventListener('deck:point', (e) => {
  if (!(e.detail in at)) return            // not one of ours: keep the moment
  point = e.detail
  from = shown; to = at[point]; t0 = null  // carry on from wherever it is
  cancelAnimationFrame(frame)
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) { shown = to; place(to) }
  else frame = requestAnimationFrame(run)
})
new ResizeObserver(() => { W = svg.clientWidth; H = svg.clientHeight; place(shown) }).observe(svg)
</script>
```

Two lanes, a track, four moments, one thing lit at each, built once and eased over time,
and every colour borrowed —
with the three lines that cause it beside it, so the reader sees the race and the code
that allows it in the same breath. Nothing in it is a box with
a label for a box's sake: every shape is something happening, at the time it happens.
That is the size and the kind of thing that works. A page with a legend, a title and six
controls is a small web app, and the reader did not open deck to use a small web app.

### Bringing one mid-walk

When they ask about something that only makes sense moving, write it there and then:

```bash
deck bring <path> --page 'live/evict.html [evict] what happens to the third entry' --fold-group
```

Borrowed like any other pane — it carries a cross, turning to another group takes it away,
and `[point …]` lands in it while it is there. If a group already has a page and the
borrowed one answers to the same name, the borrowed one wins for as long as it is up.
