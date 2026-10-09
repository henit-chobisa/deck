# The window

## Walk the argument, not the diff

A deck is a sequence of groups, and each group is one thing the agent is saying
with the code that shows it. `n` and `p` move between them. The panes are the
evidence for that one claim — an enum and the column that stores it, a writer and
the reader that consumes it — so the relationship is on screen rather than in
your head.

The light moves with the sentence being read, not with your scrolling: press a
line of the narration and the code it is about lights up, in whichever pane is
showing it.

## Comment where you disagree

Drag across lines and press `c`. Drag across the narration itself to answer a
sentence rather than a line, which is where *this whole approach is wrong* goes.
Finish it one of two ways: **Ask now** (`⌘↩`, or `Ctrl+Enter` on Windows and
Linux) puts the question to the agent straight away, and **Add to review**
(`⇧⌘↩` / `Ctrl+Shift+Enter`) holds it until you submit. `esc` discards. When
the agent is not listening, **Ask now** is off and says why; the review still
takes the comment. Every
comment is pinned to the lines it was about, with the text it was written
against — so it survives the file moving underneath it.

## Questions worth asking

Each group can offer up to five questions, written by the agent: the basics
the claim stands on, why it matters at all, what the code does when something
goes wrong, and whether the claim holds. They sit in a row of pills just below
the deck window. On Linux, in full screen, or when there is no room below the
window, they sit inside it above the footer instead.

Click one and it is asked, straight away, as if you had typed it and pressed
**Ask now**. Hold `⌘` (`Ctrl` on Windows and Linux) as you click, and it goes
into the comment box instead, for you to change first. The arrows at either
end move through them. A question you have asked stays marked.

## Put it away without losing it

A deck arrives as a bar at the bottom of the screen, not a window across the
middle of your work. Open it when you are ready; press `h` and it goes back to
the bar with your comments still in it. The agent cannot open the deck itself,
and there is no flag that lets it.

## Let it read itself to you

With a voice set up, `w` reads the deck aloud and the light walks the code in
time with the words — timed from the sound itself, not from a guess at how long
a sentence takes. Press a sentence and the code it is about lights up, whether
or not anybody is listening.

## Arrange it the way you read

Drag a seam to resize a pane, `t` to turn the panes a quarter, and the shape is
remembered for next time.

| key | does |
| --- | --- |
| `n` `p` | next / previous group |
| `c` | comment on the selection, or on the group |
| `w` | walk: read the deck aloud, and its answers |
| `t` | turn the panes |
| `h` | put the deck away |
| `s` | submit the review |
| `q` | close without answering |

---

[← README](../README.md) · [The walk](the-walk.md) · [Diagrams and pages](diagrams-and-pages.md) · [Theming](theming.md) · [The catch](the-catch.md) · [How it works](how-it-works.md)
