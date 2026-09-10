# deck

**Your agent points at code. You walk it, comment, submit.**

Deck is a review surface for agent work. Instead of a summary in chat that names
files you then have to go and find, your agent opens a window that shows the
actual lines, with its reasoning beside them. You walk the pages, comment where
you disagree, and press submit. Every comment goes back at once, pinned to the
lines it was about.

```
agent  →  deck new / deck group / deck seal      writes a deck
agent  →  deck open                              a bar says one is ready
you    →  walk it, comment, submit
agent  →  deck wait exits with the whole review
```

Nothing runs in the background. There is no daemon, no editor plugin, and no
protocol to speak — four commands and a wait, which is why it works the same
with Claude Code, Codex, Cursor, Amp, or whatever comes next.

---

## Why

Review is the bottleneck. People run four to eight agents at once now, and their
own reading speed is the ceiling — the agents finish and then wait on a human
reading diffs.

Deck is built for that moment. It is not a diff viewer and not a chat window: it
is the surface where an agent makes an argument about code and a person answers
it, in the fewest keystrokes that can carry the answer.

---

## Install

**macOS and Linux**

```sh
brew install henit-chobisa/deck/deck
```

**Windows, and anywhere with a Rust toolchain**

```sh
cargo install --git https://github.com/henit-chobisa/deck deck-app
```

Windows also needs the MSVC toolchain — Visual Studio Build Tools with the
"Desktop development with C++" workload — because that is what supplies the
linker.

Both build from source: deck compiles nineteen tree-sitter grammars into the
binary, so the first install takes a few minutes. Building locally is also why
neither platform asks about an unsigned binary — nothing was downloaded, so
there is nothing for Gatekeeper or SmartScreen to hold.

Windows is **not yet tested**. It builds and the paths are written for it, but
nobody has run a deck there. `cargo run -p deck-theme --example paths` prints
what deck resolved, which is the useful thing to send back if it misbehaves.

Then, once:

```sh
deck setup
```

which asks two things — whether to borrow your editor's colours, and which of
your agents should be told deck exists — and writes the answers to
`~/.deck/config.toml`.

---

## Using it

Most of the time you will not run deck at all. `deck setup` installs a skill
into your agents, and they reach for it on their own when they have something
worth showing you.

When you want to drive it yourself:

```sh
deck new --title "The batch counter stalls at 63" --total 2
# prints the deck's path

deck group <path> \
  --say "The counter is decremented on the **error path** too." \
  --ref "src/batch.ts:140-148 decremented *twice* when the write fails" \
  --ref "src/queue.ts:88 the only caller"

deck seal <path>
deck open <path>
```

`deck wait <path>` blocks until you submit and prints the review as JSON.

### In the window

| key | does |
| --- | --- |
| `n` `p` | next / previous group |
| `c` | comment on the selected lines, or on the group |
| `r` | turn the panes |
| `h` | put the deck away — it comes back on the bar |
| `s` | submit the review |
| `q` | close without answering |

Click and drag to select lines. Drag a seam to resize a pane. Drag a diagram to
move it, and hold `⌘` or `ctrl` with the wheel — or pinch — to take it in and out. `⌘↩` saves a comment, `esc` discards it.

---

## What a deck is

A directory, and that shape does something: the agent writes the header, then a
group at a time, then `done`. You can start reading group one while group four is
still being written.

```
d-1788265010-8842.deck/
  deck.json      the header — title, project, how many groups are coming
  g1.json        one claim, and the code that shows it
  g2.json
  done           written last
d-1788265010-8842.review    your answer, written beside it
```

A **group** is one thing the agent wants to say. Its **refs** are the evidence —
a file and a tight line range, or a diagram for structure that lives in no single
file. The window puts each ref in its own pane and lights the range.

`PROTOCOL.md` is the full specification, frozen at version 1.

---

## Theming

Deck borrows your editor's colours rather than shipping a look you have to
tolerate beside your work.

```toml
# ~/.deck/config.toml
[theme]
editor = "vscode"   # nvim | zed | vscode | cursor | windsurf
```

It takes the page, the text, an accent, and whatever syntax colours the theme
has. **Never its chrome** — a theme designs its own status bar and tab strip,
and those are answers to questions deck is not asking. Everything else is
derived from those few colours, so a deck always looks like a deck.

To try one on without changing your editor:

```sh
deck open <path> --theme "vscode:Solarized Dark"
```

Anything can be overridden by hand:

```toml
[theme.colors]
accent = "#af3a03"
```

---

## Building

```sh
cargo build --release
cargo test --workspace
```

| crate | what it is |
| ----- | ---------- |
| `deck-core` | the model: protocol types, relocation, layout, theme derivation. No I/O, no UI. |
| `deck-cli` | writing a deck to disk |
| `deck-theme` | reading a palette out of an editor |
| `deck-app` | the window, and the `deck` command |

The rule that keeps them apart: **share the model, not the view.** `deck-core`
has no view trait and knows nothing about a renderer.

---

## License

Apache-2.0. See [LICENSE](LICENSE).
