## What this changes

<!-- One or two sentences. What is different after this lands. -->

## Why

<!-- The reason, not the change. What was wrong, or what became possible. -->

## How it was checked

<!--
Deck is a window. Most of what it does cannot be asserted in a test, so say
plainly which of these is true:

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets` is clean
- [ ] I opened a deck and used the thing I changed
- [ ] I could not check it by hand, and here is why
-->

## Protocol

<!--
Delete this section unless PROTOCOL.md changed.

Version 1 is frozen. Additions are fine — a new optional field, a new enum
value — because a v1 client ignores what it does not recognise. Anything else
breaks clients that trusted the old shape.
-->
