---
name: change-the-protocol
description: >-
  Add something to deck's wire format — a field on a group, a ref, a diagram
  node, a review comment. Use before editing PROTOCOL.md or the types in
  crates/core/src/protocol.rs and diagram.rs. Version 1 is frozen, so what is
  allowed is narrower than it looks, and a change has to land in five places to
  be real.
---

# Changing the protocol

`PROTOCOL.md` is version 1 and stays that way. A deck is written by one program
and read by another, often not the same version, so every field in it is a
promise to somebody else's client.

## What is allowed

**Additions.** A new optional field, a new enum value. A v1 client ignores what
it does not recognise, and unknown enum values read as that field's default —
which is what makes this safe.

**Not changes or removals.** Renaming a field, changing its type, making an
optional field required, or removing anything already there breaks every client
that trusted the old shape.

If you need one of those, the answer is a v2 and a conversation, not a quiet
edit.

## The five places

A field added in one place and not the others is a field that half exists.

1. **The type**, in `crates/core/src/protocol.rs` or `diagram.rs`:

```rust
/// What this is for, in the reader's terms.
///
/// Optional, and absent is the ordinary case — say why somebody would set it.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub color: Option<Rgb>,
```

`skip_serializing_if` matters: a deck that does not use the field must not grow
an empty one on the way out, or every existing document changes shape.

2. **`PROTOCOL.md`** — the table for that object, plus a sentence on what a
   client does with it. Somebody is writing a second client from this document
   alone.

3. **The skill**, `crates/app/skill/SKILL.md` — an agent cannot use a field it
   has never been told about. This is the step most often missed, and the
   symptom is a feature nothing ever emits.

4. **The app**, so the field does something.

5. **A test** that the old shape still parses:

```rust
#[test]
fn a_diagram_with_no_flows_reads_and_writes_without_them() {
    let plain: Diagram = serde_json::from_str(
        r#"{ "nodes": [ { "id": "a", "label": "A" } ] }"#,
    ).expect("a diagram with no flows is a diagram");
    assert!(plain.flows.is_empty());

    let back = serde_json::to_string(&plain).expect("it serialises");
    assert!(!back.contains("flows"), "and says nothing about them");
}
```

That test is the whole promise of "additions are safe", written down.

## Two habits worth copying

**A lenient enum.** Unknown values become the default rather than an error, so a
v1 client meeting a v2 document carries on:

```rust
#[serde(rename_all = "lowercase", from = "String")]
pub enum Role { … }
```

**A shape that can grow.** A step in a flow is written as a bare id *or* as an
object, so the common case stays short and the rare one is expressible:

```rust
#[serde(untagged)]
pub enum Step {
    Node(String),
    Lit { node: String, color: Option<Rgb> },
}
```

```json
"steps": ["a", { "node": "b", "color": "#c33" }]
```

## Before you send it

A pull request touching `PROTOCOL.md` gets a CI comment quoting the diff and the
freeze rule. That is a prompt to say in the description **which kind of change
this is** — addition, or something that needs a conversation — not a refusal.

```sh
cargo test -p deck-core
```
