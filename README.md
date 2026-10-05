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

Focused on engineers, built for agents.

</div>

<img src="https://raw.githubusercontent.com/henit-chobisa/deck/pr-assets/readme/race.gif" alt="A deck walking a race condition: two requests read the same balance, the code lights line by line while a timeline plays both requests until the money goes missing">

<p align="center"><i>An agent leverages deck to walk you through intent, one claim at a time, until the whole decision is yours. Argue, decide and lock.</i></p>

<div align="center">

[Why](#why) • [Install](#install) • [Where deck helps](#where-deck-helps) • [How it works](#how-it-works) • [In the window](#in-the-window) • [Docs](#docs)

</div>

## Why

Your agent finishes a change and writes you a paragraph. It names four files and
six line numbers. Now you open each one, find the line, hold the argument in your
head while you go and look at the next one, and try to remember what the third
one was for. By the time you have the whole picture you have done the work of
assembling it yourself — which is the work you asked for. Then you type *looks
good*, and neither of you is quite sure what you approved.

Or you ask it to explain something, or for a plan. Maybe it tells you the
limitations, maybe it tells you where the architecture is broken. But it
**tells** you. It never **shows** you. And you are too tired to read all of it,
so you say "do what's good and move on".

My favourite: code reviews. A PR comes to you, you ask the agent to review it,
and it gives you five paragraphs in a language you understand fifty per cent of.
The PR is 20,000 lines, by the way. No architecture was explained. No root
context was given. There are only two outcomes. Either you become a proxy,
"okay", and get an alert at three in the morning. Or you discard the review and
dig in yourself, coffee in hand, rewriting everything you thought you knew.

Code is now written faster than anyone can keep up with it. Nobody decides to
stop understanding their own system; it is given up, one "looks good" at a time,
and in a team building seven things at once, soon nobody knows what is happening
behind the scenes. So instead of another harness for agents, deck is **focused on
engineers, but built for agents**: the agent does the explaining, and you keep
the understanding.

Explanation as *text* is the bottleneck now. People run four, six, eight agents
at once, and their own reading speed is the ceiling. Deck is built for that
moment. It is not a diff viewer and not a chat window: it is the surface where an
agent makes an argument about code, draws the flow it is describing, and a
person answers it in the fewest keystrokes that can carry the answer.

## Install

One command. It downloads the deck built for your machine, checks it, puts it
on your PATH, clears away any other deck on it — a Homebrew one, a `cargo
install`, a binary you were handed — and runs `deck setup`. Run it as yourself,
not with `sudo`.

**macOS and Linux**

```sh
curl -fsSL https://raw.githubusercontent.com/henit-chobisa/deck/main/install | sh
```

**Windows** (PowerShell)

```powershell
irm https://raw.githubusercontent.com/henit-chobisa/deck/main/install | iex
```

Nothing is compiled and nothing else is installed. Your settings and decks in
`~/.deck` are kept. After that, deck keeps itself current on macOS and Linux;
on Windows, `deck upgrade` brings in the next release when you ask.

<details>
<summary>Choices, and building it yourself</summary>

- `DECK_VERSION=v0.1.3` installs that release rather than the newest.
- `DECK_INSTALL=<dir>` puts it somewhere other than `~/.local/bin`
  (`%LOCALAPPDATA%\Programs\deck` on Windows).
- `DECK_NO_SETUP=1` stops before `deck setup`.

On Linux the window needs WebKitGTK; the installer says which package if it is
missing. To build from source instead:

```sh
cargo install --git https://github.com/henit-chobisa/deck deck-app
```

</details>

## Where deck helps

### Debugging a race condition: the books don't balance

Two requests read the same balance, both write, and one deposit is gone. The
agent puts the lines beside a timeline of both requests and plays it, sentence by
sentence, until the money goes missing. That is the recording at the top.

### Planning with your agent: decide before a line is written

Checkout is slow and the fix is to move work off the request. Before touching
anything, the agent shows the plan against today's code, projects the latency it
buys, and the backlog it costs on the busiest day. You decide with the numbers in
front of you.

<img src="https://raw.githubusercontent.com/henit-chobisa/deck/pr-assets/readme/planning.gif" alt="A plan walked as a deck: today's checkout code beside the proposed change, a latency chart moving from today to the plan, and a day of traffic building a backlog">

### Learning a system you didn't build

One search query, end to end: the map of the service, the code at each stop, and
the query taken apart as it goes. Ask why popularity is weighted 0.3 and the
answer comes back inside the deck, with the experiment that decided it.

<img src="https://raw.githubusercontent.com/henit-chobisa/deck/pr-assets/readme/learning.gif" alt="A search request walked end to end, then a question asked mid-walk answered with a chart of the experiment behind the number">

### Reviewing a 20,000-line PR

You say *review this branch*. The agent reads it, proposes the review as
chapters — the layers the branch was built in, inside out — and asks before it
starts. Chapter one is a deck: the map of the branch, a retried payment played
out in milliseconds, the test, and what the test does not prove. Your review of
it shapes the next chapter.

<img src="https://raw.githubusercontent.com/henit-chobisa/deck/pr-assets/readme/review.gif" alt="An agent proposes a 20,000-line review as three chapters, then the first chapter opens as a deck: a map of the branch, the middleware beside a timeline of a retried payment, and the test">

## How it works

Four commands and a wait. No daemon, no editor plugin, no protocol to speak —
which is why it works the same with **Claude Code, Codex, Cursor and Amp**, or
whatever comes next. A deck is a review, a walkthrough, a plan, or an
explanation — whatever an agent would otherwise have written as prose.

```sh
deck new --title "The batch counter stalls at 63" --total 2
# prints the deck's path

deck open <path>          # a bar appears; you open it when you are ready.
                          # holds until you answer, then prints the review

deck group <path> \
  --say "The counter is decremented on the **error path** too." \
  --ref "src/batch.ts:140-148 decremented *twice* when the write fails" \
  --ref "src/queue.ts:88 the only caller"

deck seal <path>
```

Most of the time you will not run any of this. `deck setup` installs a skill into
your agents and they reach for it on their own.

<details>
<summary>Why <code>deck open</code> is also the listener</summary>

`deck open` is the listener as well as the window, and it ends — handing the
agent what happened — on every way you can reach it: a review, a question asked
mid-walk, an interruption, or closing it without answering. So there is no
second command for an agent to forget. `-d` detaches it if you would rather do
the listening yourself, and `deck wait` is there for that case.

The rule underneath is that **a process wakes an agent by ending**, so whatever
wakes it has to be something that can afford to die. A window cannot — somebody
asking a question still wants the deck in front of them — so the window is a
process of its own and the listener is the one you ran.

</details>

<details>
<summary>A deck is a directory</summary>

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

[`PROTOCOL.md`](PROTOCOL.md) is the full specification, frozen at version 1.

</details>

The architecture is in [`docs/how-it-works.md`](docs/how-it-works.md).

## In the window

A deck is a sequence of groups, and each group is one thing the agent is saying
with the code that shows it. The light moves with the sentence being read, not
with your scrolling. Drag across lines, or across the narration itself, and press
`c` to disagree; **Ask now** puts it to the agent straight away, **Add to review**
holds it until you submit.

| key | does |
| --- | --- |
| `n` `p` | next / previous group |
| `c` | comment on the selection, or on the group |
| `w` | walk: read the deck aloud, and its answers |
| `t` | turn the panes |
| `h` | put the deck away |
| `s` | submit the review |
| `q` | close without answering |

More in [the window](docs/the-window.md) and [the walk](docs/the-walk.md).

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

## Docs

| | |
| --- | --- |
| [The window](docs/the-window.md) | walking groups, commenting, putting a deck away |
| [The walk](docs/the-walk.md) | the voice, what the agent can do while you watch, the transcript that comes back |
| [Diagrams and pages](docs/diagrams-and-pages.md) | pictures with flows, and pages for the idea that only moves |
| [Theming](docs/theming.md) | your editor's colours, twenty themes |
| [The catch](docs/the-catch.md) | the hook that sends a prose reply back to become a deck |
| [How it works](docs/how-it-works.md) | the architecture |
| [`PROTOCOL.md`](PROTOCOL.md) | the wire format, frozen at v1 |

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
