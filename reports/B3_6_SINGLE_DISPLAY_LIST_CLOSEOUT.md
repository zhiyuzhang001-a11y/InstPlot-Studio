# B3.6 — Single Display List pipeline closeout

Date: 2026-09-22  
Status: **DONE locally; cross-platform CI pending implementation push**

## Boundary established

Preview, PDF and PNG now enter the rendering pipeline through one
`resolve_document` function. It creates the formal `DocumentLayout` once and
resolves that layout's Display List for all consumers. The GUI no longer uses
the historical fixed compiler; its PDF and PNG actions export the current
Figure Document shown by Preview, while both formats also retain headless smoke
commands.

The earlier B1/B2 compatibility compiler remains covered by tests, but is not
used by product preview or export. Layout identity and resolved backend items
are retained together in `ResolvedFigure` so later inspection and publication
checks do not need a second layout state.

## Verification

`python3 scripts/validate_b3_6.py` runs formatting, workspace and prototype
regressions, Clippy, release build, headless PDF and PNG exports, source-level
single-entry checks, artifact structure checks and the release-size ceiling.
Every failing command writes a named log and the run writes a structured
`target/b3-6-validation/summary.json` report.

The local PDF was also independently opened and rendered: it is one page,
uses the expected physical page size, has extractable scientific text, and
contains embedded TeX Gyre Heros regular and italic font subsets. The rendered
PDF and direct PNG preserve the same axes, artists, legend and annotations.

## Acceptance result

- formal layout/resolution entry point: **PASS**;
- GUI Preview consumer: **PASS**;
- PDF consumer: **PASS**;
- PNG consumer: **PASS**;
- headless artifact structure and embedded fonts: **PASS**;
- compatibility regressions and release-size ceiling: **PASS**.

Fine visual styling remains intentionally deferred; it does not create a
second rendering path and does not block the completed B3 framework.

## Next dependency

Proceed to **B4**: palette and semantic registries plus Publication Check.
