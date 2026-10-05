# The walk

The rail sits beside the panes with everything that has been said in it, and
the agent that wrote the deck can move your eyes while it talks, whether or not
anybody is listening.

`w` adds the voice. The narration is read out loud, the code lights up under the
sentence being said — sentence by sentence, from the sound's own clock — and
panes fold down to a spine when they are not the point, unfolding again when
something points at them. `w` again leaves the walk.

While walking, a thin track under the narration fills as it is read, cut where
each sentence starts. Hover a stretch of it and its sentence lights up above;
press it to hear the prose from there. On its left, **▶ / ❚❚** reads or holds,
and **↺** starts again from the top. A group already read is not read again
when you come back to it, so you can walk just to follow the conversation.

The voice wants a Google API key, set once:

```sh
deck walk
```

It lists every Chirp 3 HD voice and plays one before you choose it. Until that
is done the window does not offer the key at all, and everything else works
without it.

A comment is a comment. There is nothing to grade it with and nothing to choose
before you type — `c`, write, send. What you do choose is *when* it is heard:
now (**Ask now**) or with the review (**Add to review**).

## What the agent can do while you watch

```sh
deck show  <deck> --ref "src/view.rs:106-110"  # move my eyes here
deck say   <deck> --text "…"                   # say something, spoken if you have a voice
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

While the agent works on an answer the panel shows a slow pulse and nothing
else. The agent is not asked to report its progress: a note a step made every
answer later, to say it was on its way.

Movement is yours the moment you take it. Scroll, select, or start typing and
the agent stops moving you; the rail says **paused** while that holds. It lapses
on its own after a few seconds of stillness, and `f` takes it back immediately.
A composer holds it until you close it, because nobody should have the ground
moved while they are writing about it.

Asked something mid-walk, it can answer with a pane rather than a paragraph —
written there and then, folded in beside what you were already looking at.

## What comes back

`deck open` (or `deck wait`, if you detached it) returns the comments as always,
and for a walked deck it also returns a `transcript`: what you were shown, in
what order, and what you said about each part. Not that a review happened —
**what the reviewer actually looked at.**

## The voice

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

---

[← README](../README.md) · [The window](the-window.md) · [Diagrams and pages](diagrams-and-pages.md) · [Theming](theming.md) · [The catch](the-catch.md) · [How it works](how-it-works.md)
