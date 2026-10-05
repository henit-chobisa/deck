# The catch

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

---

[← README](../README.md) · [The window](the-window.md) · [The walk](the-walk.md) · [Diagrams and pages](diagrams-and-pages.md) · [Theming](theming.md) · [How it works](how-it-works.md)
