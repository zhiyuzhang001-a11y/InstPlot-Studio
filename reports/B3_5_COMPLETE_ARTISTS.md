# B3.5 — Complete single-axes artist framework

Date: 2026-09-22  
Status: **DONE locally; cross-platform CI pending implementation push**

## Boundary established

The formal Figure Document adapter now covers the remaining V1 single-axes
artist structure: error bars with cap/stroke style, horizontal and vertical
reference or baseline lines, semantic annotations in figure-point coordinates,
and an explicitly identified and positioned legend. Fit and theory lines use
the same formal line path and retain their project artist identities.

Annotation labels remain semantic `Label` trees rather than flattened strings.
Legend entries come only from the project legend artist, so unlabeled helper
artists do not appear accidentally. Positive line, scatter, error-bar and
reference data are verified on log10 axes. Invalid style/position/data values
fail layout validation.

## Verification

`python3 scripts/validate_b3_5.py` runs formatting, workspace tests, the A7
layout regression, Clippy, release build, contract checks and the size ceiling.
Focused tests verify every fixed-project artist and hit-map role, semantic
annotation and legend labels, stable IDs, and positive log-axis rendering.

## Acceptance result

- log-axis artist path: **PASS**;
- error bars: **PASS**;
- fit/theory/reference/baseline line framework: **PASS**;
- semantic annotation: **PASS**;
- explicit legend identity, entries and position: **PASS**;
- validation and clipping remain active: **PASS**.

## Next dependency

Proceed to **B3.6**: make Preview, PDF and PNG consume the same formal resolved
Display List and close the B3 vertical slice.
