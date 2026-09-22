# B3.4 — Formal linear line and marker artists

Date: 2026-09-22  
Status: **DONE locally; cross-platform CI pending implementation push**

## Boundary established

The formal Figure Document adapter now resolves the B3.4 artist slice from the
single project axes: linear line artists become stroked layout series and
scatter artists become filled marker series. The adapter transfers stable IDs,
embedded X/Y columns, palette colors, stroke width/dash class, marker shape and
marker size into `instplot-layout`.

Line plus marker uses the existing version-1 composition model: a line artist
and a scatter artist may reference the same data binding. Their independently
stable nodes resolve to identical point geometry and are overlaid by the one
Display List. No new project variant or schema migration is needed.

Only embedded project columns are converted in this increment. An artist bound
to an external source that has not been loaded fails with a named
`ExternalDataUnavailable` error; missing sources, columns, colors and mismatched
column lengths likewise fail explicitly. They never silently produce an empty
series.

The GUI preview now calls `layout_figure`, so the formal axes, inward four-spine
ticks, grid, resolved publication text, line and scatter all travel through the
same layout/result/preview path. Error bars, reference/baseline artists,
annotations and legends remain B3.5.

## Verification

`python3 scripts/validate_b3_4.py` runs formatting, all workspace tests, the
resolved UI preview suite, Clippy with warnings denied, a release build and
contract checks for data, styles, identity, schema and size.

Focused tests prove that the fixed line and scatter create formal series and
data-point hit targets; both IDs remain reversible; the Display List validates;
and matching line/scatter bindings have identical mapped point geometry.

## Acceptance result

- embedded linear line: **PASS**;
- embedded scatter: **PASS**;
- composed line plus marker: **PASS**;
- palette/stroke/marker mapping: **PASS**;
- stable artist identity and hit geometry: **PASS**;
- explicit unavailable/invalid data errors: **PASS**;
- schema remains version 1: **PASS**.

## Next dependency

Proceed to **B3.5**: add log-axis artist coverage, error bars, fit/theory/
reference/baseline roles, annotations and legend.
