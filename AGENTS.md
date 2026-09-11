# Working on deck

Instructions for an agent changing this repository. A human should read
[CONTRIBUTING.md](CONTRIBUTING.md), which says the same things at more length
and with the reasons attached.

## What this is

Deck is a review surface: an agent points at real lines, a person walks them,
comments, and submits, and the whole review comes back at once. Four crates —
`deck-core` (the model, no I/O and no UI), `deck-cli` (writing a deck to disk),
`deck-theme` (reading a palette out of an editor), `deck-app` (the window).

[docs/how-it-works.md](docs/how-it-works.md) is the architecture.
[PROTOCOL.md](PROTOCOL.md) is the wire format.

## Checks

Run all three before you say you are finished. CI runs them with warnings fatal,
so run clippy the same way or you will find out later:

```sh
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
cargo test --workspace
```

## Rules that are not negotiable

**Comments say why, never what.** A comment restating the line below it is
deleted on sight. A comment explaining why the code has this shape — what was
tried, what broke, what it looked like when it was wrong — stays forever,
because it is the only place that information exists. If your change fixes a
bug, the comment explains the bug.

**Share the model, not the view.** `deck-core` has no view trait and never
learns what a widget is. Layout arithmetic returns cells.

**The protocol is frozen at v1.** Additions are allowed — a new optional field,
a new enum value — because a v1 client ignores what it does not recognise.
Changing or removing anything already in `PROTOCOL.md` is not.

**Never read the window during render.** In `deck-app`, a pane asking the entity
that is drawing it is a second borrow and it aborts the process rather than
failing — usually inside a mouse handler, which cannot unwind. Everything a pane
needs comes down through `Slot`. Handlers are fine; they run later.

**Do not weaken a test to make it pass.** The assertions in
`crates/app/src/skill.rs` that check for specific sentences look strange and are
deliberate: each one is a rule that a rewrite quietly dropped once.

## Verifying a change

Deck is a window. Most of what it does cannot be asserted — synthetic clicks do
not reach it, and it cannot be screenshotted from a script. So:

1. Test the part that is a function: layout arithmetic, relocation, palette
   derivation, protocol parsing.
2. For anything visual, write a deck and open it:

```sh
D=$(cargo run -q -p deck-app -- new --title "Checking a change" --total 1)
cargo run -q -p deck-app -- group "$D" --say "Here." --ref "src/main.rs:1-10 the change"
cargo run -q -p deck-app -- seal "$D"
cargo run -q -p deck-app -- open "$D"
```

A bar appears; the person you are working with presses Open. **You cannot open
the deck yourself and must not try** — there is no flag for it, on purpose.

3. **Say which of these you did.** "I could not check this by hand and here is
   why" is a fine answer. Claiming a visual change works when you only compiled
   it is not — see the rule below.

## Reporting honestly

Deck exists because *looks good* on work nobody verified is expensive. Do not do
it here.

- If tests fail, say so and paste the output.
- If you changed something visual and could not see it, say that plainly.
- An unrunnable check is a fact to report, not a step to skip quietly.

## Committing

Subject in the imperative, no prefix, no ticket number. The body explains why:
what was wrong, what you tried, what you decided against. Long bodies are
welcome — `git log` is read more often than anyone expects.

Do not add a co-author trailer.

## Use deck on deck

This repository carries deck's own skill at `.agents/skills/deck/`. If you are
explaining a change here and your answer would name two or more `file:line`
locations, build a deck instead of writing the answer. That is what it is for,
and this is the codebase where a bad one gets noticed fastest.
