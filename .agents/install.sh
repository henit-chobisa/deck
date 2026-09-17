#!/bin/bash
# Install a locally built deck without macOS killing it.
#
# Copying over a binary that is currently mapped invalidates its cached code
# signature, and the next launch dies with `Taskgated Invalid Signature` — a
# SIGKILL with no window, no error, and nothing in the terminal. It cost an
# afternoon of "the walkthrough never happens".
set -e
cd "$(dirname "$0")/.."
cargo build --release -p deck-app
pkill -f "deck open" 2>/dev/null || true
sleep 1
rm -f ~/.local/bin/deck
cp target/release/deck ~/.local/bin/deck
codesign -s - --force ~/.local/bin/deck
codesign --verify ~/.local/bin/deck
~/.local/bin/deck --version
