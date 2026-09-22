# B3.3 — Resolved publication-text preview

Date: 2026-09-22  
Status: **DONE locally; cross-platform CI pending implementation push**

## Boundary established

The Studio canvas now starts from the formal B3.2 project layout, resolves its
Display List through the same publication-text resolver used by export, and
passes a `ResolvedDisplayList` to the egui adapter. The UI no longer flattens
semantic labels to a single system-font string.

All four bundled TeX Gyre Heros faces are registered with egui under their
PostScript identities. Every resolved run preserves its selected face, font
size, horizontal start, subscript/superscript baseline shift and text range.
The adapter rotates each run around the Display List text pivot, so the Y label
reaches egui as a −90-degree `TextShape`. Clip pushes and pops continue to be
applied by the resolved Display List traversal.

The B1 fixed compiler remains available for compatibility tests and the old
fixed PDF export. It is no longer the GUI preview source. Artist population of
the formal layout begins in B3.4, in the order fixed by the Part B plan.

## Verification

`python3 scripts/validate_b3_3.py` runs formatting, workspace tests, the full
UI-shell publication-stack suite, Clippy with warnings denied, a release build
and source-contract checks. The focused tests prove that:

- the formal grid is present in the resolved GUI preview;
- unresolved `GlyphRun` items do not reach the preview adapter;
- the Y label remains −90 degrees after formal layout and resolution;
- resolved labels select only bundled TeX Gyre Heros faces;
- italic variables and nonzero scientific baseline shifts survive resolution;
- a real headless egui pass emits a rotated `TextShape`.

The native app also reached `FIRST_CANVAS` locally. The macOS session was locked,
so no claim of a human screenshot review is made; the rendering-layer assertion
above is the repeatable acceptance evidence for this increment.

## Acceptance result

- preview consumes formal layout: **PASS**;
- preview consumes resolved text runs: **PASS**;
- bundled regular/italic/bold/bold-italic registration: **PASS**;
- variable and sub/superscript run positioning: **PASS**;
- rotated Y label reaches egui: **PASS**;
- layout clipping traversal retained: **PASS**.

## Next dependency

Proceed to **B3.4**: populate the formal layout from the Figure Document with
linear line, scatter and combined line-plus-marker artists.
