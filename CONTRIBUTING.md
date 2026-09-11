# Contributing

Thanks for looking. Deck is small and the conventions are unusual enough to be
worth twenty minutes of reading before you write anything.

## Getting it running

```sh
git clone https://github.com/henit-chobisa/deck
cd deck
cargo test --workspace
cargo run -p deck-app -- --help
```

The window is developed on macOS. Linux and Windows are built and tested on
every push — see [the workflows](.github/workflows/) for what runs where.

To try a change, write a deck and open it:

```sh
D=$(cargo run -q -p deck-app -- new --title "A thing I changed" --total 1)
cargo run -q -p deck-app -- group "$D" --say "Here it is." --ref "src/main.rs:1-10 the change"
cargo run -q -p deck-app -- seal "$D"
cargo run -q -p deck-app -- open "$D"
```

A bar appears at the bottom of the screen. Deck never opens the window for you,
so you press Open — that is the product working, not something to route around.

## Before you send it

```sh
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

CI runs these with `RUSTFLAGS=-D warnings`, so a warning that is fine locally
will stop a pull request. Running clippy the same way first saves a round trip:

```sh
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
```

## Comments say why, never what

This is the convention that will surprise you, and it is the one that matters
most. A comment that restates the code is deleted on sight. A comment that says
why the code is the shape it is stays forever, because it is the only place that
information exists.

Most comments here exist because something was tried and did not work. From
`chart.rs`:

```rust
// A flex row, not the block a div is by default. Under block layout the
// child fills the pane's width whatever its content measures, so a drawing
// wider than the pane had nowhere to be except centred inside a box too
// small for it — cut off at both ends, and not reachable by scrolling
// either.
```

Somebody wrote `div()`, watched a diagram vanish, and left behind the reason so
the next person does not spend that hour. That is the bar. If your change fixes
a bug, the comment explains the bug — not the fix, which is already on the line
below.

Module headers do the same job for a whole file: read the top of
[`crates/core/src/relocate.rs`](crates/core/src/relocate.rs) for what one looks
like when the idea is subtle.

## Tests pin behaviour, not implementation

A test is named as a sentence about what is true, and its comment says which
mistake it is standing guard over:

```rust
#[test]
fn a_corrupt_file_costs_the_window_size_and_nothing_else() {
    // Whatever is on disk, opening the deck has to work — so a file that
    // will not parse reads as "nothing remembered" rather than as a
    // failure to be reported.
```

Not every part of deck can be tested. It is a window, and most of what it does
is pixels; synthetic clicks do not reach it. So test the part that is a
function — the layout arithmetic, the relocation, the palette derivation, the
protocol — and check the rest by opening a deck and using it. Say which you did
in the pull request. "I could not check this by hand and here is why" is a fine
answer; silence is not.

## The one architectural rule

**Share the model, not the view.** `deck-core` has no view trait and knows
nothing about a renderer. Layout arithmetic returns cells, not widgets. The
moment it knows what a pane is, a second client cannot use it.

[`docs/how-it-works.md`](docs/how-it-works.md) has the rest of the shape.

## The protocol is frozen

`PROTOCOL.md` is version 1 and stays that way. A deck is written by one program
and read by another, often not the same version, so every field in it is a
promise to somebody else's client.

- **Additions are fine.** A new optional field, a new enum value. A v1 client
  ignores what it does not recognise, which is what makes this safe.
- **Changes and removals are not.** They break every client that trusted the old
  shape.

A pull request touching `PROTOCOL.md` gets a CI comment saying so. That is a
prompt to explain the change, not a refusal.

## Commit messages

The subject says what changed, in the imperative, with no prefix and no ticket
number. The body says why — what was wrong, what you tried, what you decided
against. Recent ones:

```
Do not ask the window a question while it is drawing
Light the last node, colour the paths, and choose the speed
Put the picture back, and the flows in a row
```

Long bodies are welcome. A commit is the best place to put the reasoning that
does not fit in a comment, and `git log` is read far more often than anybody
expects.

## Changing the skill

`crates/app/skill/SKILL.md` is what tells an agent deck exists and how to write
a good deck. It is compiled into the binary, so a change to it ships with a
release rather than as a file anybody copies.

Tests in [`crates/app/src/skill.rs`](crates/app/src/skill.rs) assert that
specific sentences are present. That looks odd until you have watched a rewrite
quietly drop the rule that made the thing work — each of those assertions is a
rule that was lost once.

## Skills, if you work with an agent

`.agents/skills/` holds four workflows from this repository that are easy to get
wrong — changing the window, changing the protocol, changing the skill, and
cutting a release. Each is a plain Markdown file with the reasoning attached, so
they are worth reading yourself even if nothing else does.

## What to work on

- **Bugs you hit while using it.** Deck is used by the people writing it, and
  that is where nearly every fix so far has come from.
- **A deck opened on Linux or Windows.** The window is developed on macOS, so a
  note saying what it looked like elsewhere is worth a lot. If a theme fails to
  import, `cargo run -p deck-theme --example paths` prints every directory deck
  checked.
- Before building anything large, open an issue. Deck has opinions and it is
  kinder to disagree before the work than after it.

## Licence

Apache-2.0. By contributing you agree your work ships under it.
