---
name: no-unnecessary-comments
description: 'Use when writing or editing code. Keeps comments limited to information the code cannot express, and removes comments that restate the implementation.'
metadata:
    namespace: system
---
# No unnecessary comments

Do not add comments that restate what the code already says.

## Rules

- Do not narrate what a line or block does when the code is self-explanatory.
- Do not add section banners, obvious doc comments, or comments that repeat a function or type name.
- Do not leave commented-out code.
- Do not add comments explaining a change you just made; the diff already shows it.
- Keep a comment only when it records something the code cannot: a non-obvious constraint, an invariant, a workaround, an external bug reference, or a reason that is not visible from the implementation.
- Keep any necessary comment short and place it next to the code it constrains.
- Remove stale or redundant comments in code you touch.
- Prefer clearer names and structure over explanatory comments.
