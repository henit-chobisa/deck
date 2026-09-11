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

<img width="1379" height="1027" alt="image" src="https://github.com/user-attachments/assets/a5340bfc-50b8-4ecd-a7c9-930b9af8ad96" />


<!--
  The hero goes here, and it is the most important thing on this page: a reader
  learns what deck is from the picture faster than from any paragraph on it.

  What belongs here is a deck OPEN — two panes of code with the narration band
  above them, around 1600 wide. Drop it at assets/deck.png and uncomment:

<img src="assets/deck.png" alt="A deck open on two panes of code with the narration above them">

<p align="center"><i>An agent points at the lines it means. You walk them, comment, and submit.</i></p>
-->

<div align="center">

[Why](#why) • [How it works](#how-it-works) • [Demo](#demo) • [Install](#install) • [In the window](#in-the-window) • [Diagrams](#diagrams) • [Theming](#theming)

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



https://github.com/user-attachments/assets/ea4f8fe3-50b2-4412-917c-882b536a538a



<!--
  Drop the video in here.

  GitHub will not play a video committed to the repo. The way that works is to
  open this file in the web editor, drag the file into the text area, and let
  GitHub upload it — it inserts a user-attachments URL that renders as a player.
  MP4 or MOV, under 10 MB.

  Then delete this comment and write a sentence under it saying what is on
  screen. A silent video with no caption makes the reader work out what they
  are watching, which is the thing this whole page is against.
-->

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

## In the window

### Walk the argument, not the diff

A deck is a sequence of groups, and each group is one thing the agent is saying
with the code that shows it. `n` and `p` move between them. The panes are the
evidence for that one claim — an enum and the column that stores it, a writer and
the reader that consumes it — so the relationship is on screen rather than in
your head.

<!--
  A screenshot here: a group open, both panes lit, the narration above them.
  Caption it with what the two panes have to do with each other — that is the
  thing a still picture cannot say on its own.
-->

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

<!--
  A GIF still wanted here: a flow playing, with the rest of the diagram receding. This one
  has to move — the travelling is the whole point and a still frame of it is
  just a diagram with some boxes a different colour.
-->

There are no colours, sizes or positions in the format, on purpose. A node says
what it *is* and how much it matters, and deck owns every pixel — so two decks
drawing the same idea come out looking the same.

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
