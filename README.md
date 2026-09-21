<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/mark-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="assets/mark-light.svg">
  <img alt="deck" height="72" src="assets/mark-light.svg">
</picture>

<h1>deck</h1>

[![Build](https://github.com/henit-chobisa/deck/actions/workflows/build.yml/badge.svg)](https://github.com/henit-chobisa/deck/actions/workflows/build.yml)
[![Test](https://github.com/henit-chobisa/deck/actions/workflows/test.yml/badge.svg)](https://github.com/henit-chobisa/deck/actions/workflows/test.yml)
[![Release](https://img.shields.io/github/v/release/henit-chobisa/deck?include_prereleases&color=d65d0e)](https://github.com/henit-chobisa/deck/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

A presentation surface for agents, written in Rust.

</div>

<img width="1292" height="1346" alt="file-91edb215334a29711dd9a11712a287e7" src="https://github.com/user-attachments/assets/108158d4-5486-4a3d-a8d3-562d315fcc5c" />

<!--
  The hero goes here, and it is the most important thing on this page: a reader
  learns what deck is from the picture faster than from any paragraph on it.

  What belongs here is a deck OPEN — two panes of code with the narration band
  above them, around 1600 wide. Drop it at assets/deck.png and uncomment:

<img src="assets/deck.png" alt="A deck open on two panes of code with the narration above them">

<p align="center"><i>An agent points at the lines it means. You walk them, comment, and submit.</i></p>
-->

<div align="center">

[Why](#why) • [How it works](#how-it-works) • [Demo](#demo) • [Install](#install) • [In the window](#in-the-window) • [The walk](#the-walk) • [Diagrams](#diagrams) • [Theming](#theming)

</div>

## Why

Your agent finishes a change and writes you a paragraph. It names four files and
six line numbers. Now you open each one, find the line, hold the argument in your
head while you go and look at the next one, and try to remember what the third
one was for. By the time you have the whole picture you have done the work of
assembling it yourself — which is the work you asked for. Then you type *looks
good*, and neither of you is quite sure what you approved.

Now another one. Say you ask the agent to explain something to you, or a plan —
maybe it tells you the limitations, maybe it tells you where the architecture is
broken. But the problem is that it **tells** you. It never **shows** you. And
then you are too tired to read all of that, so you say "do what's good and move
on".

My favourite: code reviews. A PR comes to you, you ask the agent to review it,
and it gives you five paragraphs in a language you understand fifty per cent of.
Now you are too tired to go and sniff into the code — and the PR is 20,000 lines
to sniff into, by the way. No architecture was explained. No root context was
given. There are only two outcomes. Either you become a proxy, "okay", and get
an alert at three in the morning. Or you discard the review and dig in yourself,
coffee in hand, 20,000 lines, rewriting everything you thought you knew.

Explanation as *text* is the bottleneck now. People run four, six, eight agents
at once, and their own reading speed is the ceiling — the agents finish and then
wait on a human reading prose. Deck is built for that moment. It is not a diff
viewer and not a chat window: it is the surface where an agent makes an argument
about code, draws the flow it is describing, and a person answers it in the
fewest keystrokes that can carry the answer.

## How it works

Four commands and a wait. No daemon, no editor plugin, no protocol to speak —
which is why it works the same with **Claude Code, Codex, Cursor and Amp**, or
whatever comes next. A deck is a review, a walkthrough, a plan, or an
explanation — whatever an agent would otherwise have written as prose.

```sh
deck new --title "The batch counter stalls at 63" --total 2
# prints the deck's path

deck open <path>          # a bar appears; you open it when you are ready

deck group <path> \
  --say "The counter is decremented on the **error path** too." \
  --ref "src/batch.ts:140-148 decremented *twice* when the write fails" \
  --ref "src/queue.ts:88 the only caller"

deck seal <path>
deck wait <path>          # blocks until you submit, prints the review as JSON
```

Most of the time you will not run any of this. `deck setup` installs a skill into
your agents and they reach for it on their own.

## Demo

The whole loop in one take: the agent writes a deck while you watch, you walk it,
comment where you disagree — and it answers you inside the deck, without either of
you going back to the chat window.


<img width="1200" height="696" alt="3 00 40" src="https://github.com/user-attachments/assets/26043479-32bc-4dc6-8e52-a433c3059379" />

<img width="1200" height="696" alt="3 02 50" src="https://github.com/user-attachments/assets/3a203e8a-9ad3-411c-a730-4facdd5bc943" />

![the whole loop](https://github.com/henit-chobisa/deck/releases/download/v0.1.1/interaction.gif)


## Install

**macOS and Linux**

```sh
brew install henit-chobisa/deck/deck
```

**Windows, and anywhere with a Rust toolchain**

```sh
cargo install --git https://github.com/henit-chobisa/deck deck-app
```

Then once, to choose how deck looks and to tell your agents it exists:

```sh
deck setup
```

Both install from source — deck compiles nineteen tree-sitter grammars into the
binary, so the first build takes a few minutes. It is also why neither platform
asks about an unsigned binary: nothing was downloaded, so there is nothing for
Gatekeeper or SmartScreen to hold.

On Windows this needs the MSVC toolchain — Visual Studio Build Tools with the
"Desktop development with C++" workload — for the linker.

## The catch

`deck setup` installs a skill telling your agents when to build a deck. A skill
is a suggestion, and an agent that has just finished an investigation has
enormous momentum toward typing out what it found. Suggestions lose to that
often enough that you can install deck and then go a week without seeing one.

So setup also offers to turn on the catch. It reads each finished reply, and
when one names two or more `file:line` locations it sends the reply back and
says to build the deck instead — while the agent is still mid-turn and still
holding everything it just learned.

```
That reply names 3 code locations (view.rs:1109, protocol.rs:253,
protocol.rs:155). Locations in prose are the thing deck exists to
replace — the reader has to go and open each one and hold the
argument together themselves.
```

Two locations, not one: a single pointer is a sentence, and opening a window for
it would be ceremony. Two is an argument, and assembling it is the work deck
does for you.

It stays quiet whenever it might be wrong — a deck already built this turn, a
reply about deck itself, the same location cited twice, a version number or a
time of day that only looks like a citation. A hook that fires when it should
not is worse than one that never fires, because you turn it off.

Claude Code only, for now; it is the one agent with a hook that fires on the
reply itself. Say no at the prompt and everything else still works. Turn it on
later with `deck setup`, or take the `Stop` entry out of
`~/.claude/settings.json` to turn it off.

## In the window

### Walk the argument, not the diff

A deck is a sequence of groups, and each group is one thing the agent is saying
with the code that shows it. `n` and `p` move between them. The panes are the
evidence for that one claim — an enum and the column that stores it, a writer and
the reader that consumes it — so the relationship is on screen rather than in
your head.

The light moves with the sentence being read, not with your scrolling: press a
line of the narration and the code it is about lights up, in whichever pane is
showing it.

![walking a deck](https://github.com/henit-chobisa/deck/releases/download/v0.1.1/3.00.40.gif)

### Comment where you disagree

Drag across lines and press `c`. Drag across the narration itself to answer a
sentence rather than a line, which is where *this whole approach is wrong* goes.
`⌘↩` saves, `esc` discards. Every comment goes back at once, pinned to the lines
it was about, with the text it was written against — so it survives the file
moving underneath it.

### Put it away without losing it

A deck arrives as a bar at the bottom of the screen, not a window across the
middle of your work. Open it when you are ready; press `h` and it goes back to
the bar with your comments still in it. The agent cannot open the deck itself,
and there is no flag that lets it.

### Let it read itself to you

With a voice set up, `w` reads the deck aloud and the light walks the code in
time with the words — timed from the sound itself, not from a guess at how long
a sentence takes. Press a sentence and the code it is about lights up, whether
or not anybody is listening.

<!-- MEDIA · no clip for the voice yet: none of the five shows `w` being pressed. -->

### Arrange it the way you read

Drag a seam to resize a pane, `r` to turn the panes a quarter, and the shape is
remembered for next time.

| key | does |
| --- | --- |
| `n` `p` | next / previous group |
| `c` | comment on the selection, or on the group |
| `r` | turn the panes |
| `h` | put the deck away |
| `s` | submit the review |
| `q` | close without answering |

## How do you know the agent did not make it up

You do not have to take its word for the half that can be checked. `deck group`
opens every file a ref points at, and refuses the group if the file is not there
or the range runs past the end of it — so a deck cannot be written against code
that does not exist, and the agent is told while it is still running and can go
and look.

That leaves the other half: whether the *claim* about those lines is true. Deck's
answer is that you are looking at the lines while you read the claim, which is
the whole shape of the thing. A summary in chat asks you to believe it. A deck
puts the evidence next to the argument and gives you a key to disagree on.

And when the file moves underneath you, comments come back with how far to trust
them — `diff`, `fingerprint`, or `stale` — rather than a line number that may
have drifted.

## The walk

There is no mode to enter. The rail sits beside the panes with everything that
has been said in it, and the agent that wrote the deck can move your eyes while
it talks, whether or not anybody is listening.

`w` adds the voice. The narration is read out loud, the code lights up under the
sentence being said — sentence by sentence, from the sound's own clock — and
panes fold down to a spine when they are not the point, unfolding again when
something points at them. `w` again stops it.

The voice wants a Google API key, set once:

```sh
deck walk
```

It lists every Chirp 3 HD voice and plays one before you choose it. Until that
is done the window does not offer the key at all, and everything else works
without it.

Three keys answer, and they cost one keystroke each:

```
1   noted
2   wait, what?
3   that's wrong
```

Tapping one leaves a reaction pinned to whatever is on screen — no typing, no
composer, nothing to close. It waits for the review like every other remark, so
the agent is not off working on half a thought; the composer offers *wait for a
gap* and *interrupt* when you want it heard sooner. `c` is still there when you have actual words, and
while the composer is open those same three keys set what the comment is asking
for instead of leaving a bare reaction.

This is what `Kind` in the protocol has always been for. Every comment used to
go back marked `question` whatever you meant, which left the agent guessing your
tone from your prose.

### What the agent can do while you watch

```sh
deck show  <deck> --ref "src/view.rs:106-110"  # move my eyes here
deck say   <deck> --text "…"                   # say something, spoken if you have a voice
deck doing <deck> --text "reading the retry loop"  # what it is doing, while it does it
deck next  <deck> --after <cursor>             # block until the reader does something
deck fold  <deck> --pane protocol              # fold that pane away to its spine
deck bring <deck> --ref "src/live.rs:40-60"    # borrow a file the group does not carry
deck bring <deck> --diagram "flow.json [flow] the path"  # or a picture, written now
deck clear <deck>                              # put the borrowed panes back
```

All shell commands, so this works the same with Claude Code, Codex, Cursor and
Amp — the same reason the other verbs do. `deck next` returns a cursor, so an
agent that was busy or restarting reads from where it got to rather than losing
what you pressed while it was away.

The panel cannot tell a working agent from a dead one, so `deck doing` is how it
finds out: the words go where the waiting label is, and each one restarts the
two minutes of silence after which deck admits it has no idea. Notes are not
turns — nothing is read aloud, and nothing reaches the review.

Movement is yours the moment you take it. Scroll, select, or start typing and
the agent stops moving you; the rail says **paused** while that holds. It lapses
on its own after a few seconds of stillness, and `f` takes it back immediately.
A composer holds it until you close it, because nobody should have the ground
moved while they are writing about it.

Asked something mid-walk, it can answer with a pane rather than a paragraph —
written there and then, folded in beside what you were already looking at.

![answering mid-walk](https://github.com/henit-chobisa/deck/releases/download/v0.1.1/3.56.56.gif)

### What comes back

`deck wait` returns the comments as always, and for a walked deck it also
returns a `transcript`: what you were shown, in what order, and what you said
about each part. Not that a review happened — **what the reviewer actually
looked at.**

### The voice

Reading aloud is optional and off in one line. It speaks with Google's Chirp 3:
HD and nothing else — `deck walk` sets the key and picks the voice:

```toml
[speech]
aloud = true
voice = "en-US-Chirp3-HD-Kore"
rate  = 182
pause = 420
```

The key lives in `DECK_SPEECH_KEY` if you would rather it were not in a file
that ends up in every backup.

One voice rather than a choice of them, because the ones a machine already has
are the reason people switch narration off after a paragraph — and because only
a rendered passage can be timed exactly, which is what lets the light move with
the words instead of near them. Chirp 3: HD includes a million characters a
month and renews; a five-group deck is about five thousand.

Worth naming plainly: the narration is sent to Google to be turned into sound.
The narration, not your code — the prose the agent wrote about it. Nothing is
sent until a key is set up, and a deck with no key still walks, in silence.

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

Pressing a flow sends a current down the route while everything off it recedes;
a second flow over the same boxes shows the path that stalls.

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

<!-- MEDIA · a flow playing. 3.16.53.gif is 55s and wants cutting to ~25s first. -->

There are no colours, sizes or positions in the format, on purpose. A node says
what it *is* and how much it matters, and deck owns every pixel — so two decks
drawing the same idea come out looking the same.

<!--
  NOT SHIPPED YET. Uncomment this section in the release that carries pages —
  today the binary on brew is 0.1.1, which has diagrams and no pages, and a
  README that describes a pane nobody can make is a README that lies.

## Pages, for the idea that only moves

Some things are neither a file nor a picture. Two rows merging, a queue filling
up, a node coming out of a chain — the argument *is* the movement, and a diagram
of it is a diagram of the start and the end with the interesting part missing.

A group can carry a page: HTML the agent writes, rendered in a pane of the deck,
wearing the deck's own colours and hearing the same points the code does.

```bash
deck group <path> \
  --say 'Two rows, same key. [point 24-26 hop] The later one lands on the
earlier, and [merge] shows which field survives.' \
  --page 'merge.html [merge] two rows meeting' \
  --ref 'src/merge.ts:24-26 [code] the line that decides'
```

It is deliberately narrow. At most forty words may show — the prose carries the
argument and a page with paragraphs in it is a second narration competing with
the band — nothing is fetched from anywhere, and the page is handed deck's
palette as CSS variables rather than choosing its own.

What it gets in return is the thing an artifact cannot have: it is told where the
reader is. `[point rows]` reaches the page as an event, so the animation is bound
to the sentence being read rather than to a clock.

MEDIA · 4.26.13.gif — rows shifting under a paged query. Upload it when pages ship.
-->

## Theming

Deck borrows your editor's colours rather than shipping a look you have to
tolerate beside your work.

<img src="assets/themes.gif" alt="One deck rendered in twenty editor themes, one after another">

<i>One deck, twenty themes, nothing changed but the config line. The page, the
text, the accent and the syntax come from the editor; the shapes and the spacing
are always deck's.</i>

```toml
# ~/.deck/config.toml
[theme]
editor = "vscode"   # nvim | zed | vscode | cursor | windsurf
```

It takes the page, the text, an accent, and whatever syntax colours the theme
has. Never its chrome — a theme designs its own status bar and tab strip, and
those are answers to questions deck is not asking. To try one on without changing
your editor:

```sh
deck open <path> --theme "vscode:Solarized Dark"
```

Light or dark follows the machine unless you say otherwise. Worth pinning for a
screenshot or a recording, which would otherwise change colour depending on what
time of day somebody plays it back:

```sh
deck open <path> --mode dark --paper warm
```

## A deck is a directory

That shape does something: the agent writes the header, then a group at a time,
then `done`. You can start reading group one while group four is still being
written.

```
d-1788265010-8842.deck/
  deck.json      the header — title, project, how many groups are coming
  g1.json        one claim, and the code that shows it
  g2.json
  done           written last
d-1788265010-8842.review    your answer, written beside it
```

[`PROTOCOL.md`](PROTOCOL.md) is the full specification, frozen at version 1, and
[`docs/how-it-works.md`](docs/how-it-works.md) is the shape of the rest.

## Contributing

```sh
cargo test --workspace
```

- [CONTRIBUTING.md](CONTRIBUTING.md) — conventions, and what needs doing
- [docs/how-it-works.md](docs/how-it-works.md) — the architecture
- [PROTOCOL.md](PROTOCOL.md) — the wire format, frozen at v1
- [AGENTS.md](AGENTS.md) — the same, for an agent changing this repo

Deck is used by the people writing it, and nearly every fix so far has come from
somebody hitting something while reading a deck of their own.

## License

Apache-2.0. See [LICENSE](LICENSE).
