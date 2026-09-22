# B4 — Palette, semantics and Publication Check closeout

Date: 2026-09-22  
Status: **DONE locally; cross-platform CI pending implementation push**

## Registries

The built-in palette catalog is versioned independently from the project-file
schema. It records the active publication subset plus Tol Bright, Tol High
Contrast, Tol BuRd and SciPlot neutral metadata. Each entry includes source and
version, fixture reference and SHA-256, provenance class, palette type,
recommendation, sampling rule, derived-subset identity, ordering or diverging
center/direction, and separate CVD, print and monochrome review states.

The versioned semantic registry defines object-identity colour, marker and line
channels, neutral theory/reference roles, and redundant non-colour encodings.
Because these are built-in policies resolved by the existing schema-v1 palette
and artist identities, B4 does not invalidate B2 projects or silently alter
their stored data.

## Publication Check

One rule engine now evaluates physical size, font size, stroke width, PDF font
embedding, clipping, legend overlap, colour-only encoding, palette/data
relationship, grayscale distinguishability, deuteranopia risk, raster DPI and
pixels, transparency, and provenance completeness. Reports use Error, Warning
and Information levels, carry the rules version and raster DPI, and attach
warnings to stable project nodes.

The live Inspector and the `--publication-check` headless command use this same
engine. Checks rerun when the Figure Document changes. A generic project
override can suppress a finding only when its value records a non-empty
`reason`; the report retains that reason.

## Verification

`python3 scripts/validate_b4.py` runs formatting, workspace and UI regressions,
Clippy, release build, project creation, the structured headless report,
registry/checksum contracts, complete rule-set checks, rule fixtures and the
release-size ceiling. Named logs and `target/b4-validation/summary.json` show
the precise failure when any step does not pass.

The fixed publication fixture has no Publication Check errors. Its current
legend/data intersection is deliberately reported as a node-specific warning;
this is a real styling issue for later visual adjustment, not a framework
failure or a hidden pass.

## Acceptance result

- palette registry and provenance: **PASS**;
- semantic role registry: **PASS**;
- all 13 V1 rule categories: **PASS**;
- node-specific warnings: **PASS**;
- reasoned overrides: **PASS**;
- document/export-parameter re-evaluation: **PASS**;
- live UI and structured headless report: **PASS**.

## Next dependency

Proceed to **B5**: Lite → Studio handoff, without writing Studio styling back
into Lite source files.
