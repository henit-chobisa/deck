# Live conversation checks

These checks exercise issue #11 as a conversation, not just speech playback.
The automated suite covers the state and transport. A person must check the
native window; passing tests do not establish visual quality or input latency.

## Session boundaries

- Open a deck from its bar and press `l`.
- Send a reply with `deck say <deck> --silent --text "A reply to keep."`.
- Select the reply, write a comment, and send it. Start another comment but do
  not send it. Choose Interrupt for the draft.
- Hide with `h`, then reopen from the bar. The transcript, sent comment, draft,
  draft anchor and delivery choice must remain. Speech must not restart.
- Repeat with a code selection, code scroll position, narration selection,
  resized rail and a conversation scrolled away from the latest turn.
- Submit. The review must include conversation turns from before and after
  hiding, in order, with timestamps on the same clock.
- Make the review destination unwritable and attempt submission. Restore access
  and retry. A failed write must not empty the transcript.

## Taking turns

- While speech is active, queue two comments. Both appear immediately in the
  record, but reach the agent at the next gap, in order.
- Queue another comment, then press `stop`. Speech stops; both the earlier
  queued comment and the interruption reach the agent, in order.
- Queue a comment and hide or change groups before speech finishes. It must
  still reach the agent; cancellation is also a gap.
- With voice disabled, comments reach the agent without an audio timer.
- A silent agent produces a waiting label, then `no answer yet` after two
  minutes. Deck must not claim that the agent is thinking or connected merely
  because a comment exists.
- Send `deck doing <deck> --text "reading the retry loop"` while a comment is
  unanswered. The panel shows those words, and a second note replaces them. The
  waiting bar starts again from each one, so an agent sending notes never
  reaches `no answer yet`; once one does go stale the label returns and the note
  is kept below it in the past tense. `deck say` clears both.

## Reading while the agent presents

- A new reply follows the transcript tail. Scroll up, then send another reply:
  it must not pull the reader away. `Latest` returns to the tail.
- Select an older turn, then change groups. Replying still quotes that turn and
  names its original group. Selecting code switches the target back to code.
- Use `deck show`, hide, then reopen. The applied spotlight remains. Navigate
  away and return control with `f`; asking for the old stage must restore it,
  not report an unchanged stage against a different visible group.
- A command waiting for the view when it hides is refused, never replayed on
  reopen. A command already applied still returns its applied result.

## Agent loop

- Start `deck wait` before opening the owner. A later reader comment must wake
  it. Answer with `say` or move evidence with `show`, then wait again.
- Replace the owner while waiting. The waiter must reconnect rather than
  remain attached to an obsolete generation.
- Break stdout before an event is printed. The next waiter must still receive
  that event. Delivery is at-least-once if printing succeeds but saving the
  cursor fails; consumers must not assume exactly-once delivery.
- Submit with unread queued events. The final review wins over another nudge.

## Limits not covered by this change

The session handoff is in-memory, not crash recovery. Replies arriving while
hidden are still refused; the agent must handle that outcome. Agent presence,
explicit working/yielded states, atomic show-and-say turns, diagram control,
streaming speech and transcript virtualization remain separate work. Neither
these tests nor compilation establish that every live edge case is resolved.
