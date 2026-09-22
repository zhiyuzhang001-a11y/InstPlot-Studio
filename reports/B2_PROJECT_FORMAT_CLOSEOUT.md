# B2 — Formal Figure Document and project format closeout

Date: 2026-09-22  
Status: **DONE locally; cross-platform CI pending implementation push**

## Scope delivered

InstPlot Studio now owns a formal schema-versioned Figure Document rather than
serializing GUI widgets or the B1 runtime wrapper. Schema 1 records:

- producer and schema versions;
- stable figure, axes, axis and artist identities;
- linear/log scale, locator and formatter settings;
- typed line, scatter, error-bar, reference-line, annotation and legend records,
  including data bindings, semantic roles, styles and placements;
- external and embedded data-source records plus fit identity;
- semantic label nodes;
- the exact TeX Gyre Heros four-face provenance and typography scales;
- palette identity and RGBA entries;
- overrides, export preferences and operation provenance.

The B1 runtime document now owns this formal project model. Axis edits update
the formal schema, and compilation bridges it to the validated Part A Display
List. A save/open round trip produces the same resolved Display List.

ADR-022 selects a strict UTF-8 JSON `.instplot` file for V1. Unknown fields in a
supported schema and future schema versions are rejected explicitly. Schema 0
is accepted only through a tested migration that appends provenance. A future
ZIP/binary container requires a new decision backed by real asset-size needs.

## Persistence and recovery

- saves use same-directory atomic replacement on supported platforms;
- replacing a valid project first writes its prior bytes to `<name>.bak`;
- an invalid existing project is never overwritten silently;
- opening a corrupt or unreadable primary may recover the validated backup, but
  the result is clearly identified as backup state and the GUI requires Save As;
- external files are fingerprinted by byte length and SHA-256;
- missing or changed sources produce visible warnings and do not update the
  stored fingerprint, source identity or project automatically;
- embedded columns validate row counts, finite values and their content digest.

The GUI exposes Open project, Save project and Save project as actions. The
release binary also exposes `--create-project` and `--check-project` before any
window or GPU context is created, so project-format validation remains suitable
for CI and recovery diagnostics.

## Automated verification

`python3 scripts/validate_b2.py` writes structured evidence under
`target/b2-validation/` and covers **13** independent checks:

1. workspace formatting;
2. all workspace tests;
3. Clippy for all targets/features with warnings denied;
4. stripped release build;
5. headless project creation;
6. exact schema-root, typography, palette and export contract inspection;
7. headless current-project opening and compilation;
8. atomic backup creation;
9. recovery after deliberate corruption of the generated primary;
10. checked-in schema-0 fixture migration;
11. 12 MiB release-size ceiling;
12. complete dependency license metadata;
13. ADR and migration-fixture presence.

The Rust suite has **18 passing tests** across the library and binary. B2 tests
cover exact round trips, resolved-output equality, strict unknown fields,
future-schema rejection, source/fit identity, font and palette provenance,
old-fixture migration, source changed/missing states, backup recovery and
headless command routing.

Regression remains closed: the B1 validator passes all **11** checks and the
Part A validator passes all **21** checks. The fixed PDF exported by the final
B2 release is byte-for-byte identical to the B1 reference (SHA-256
`d3fada2b9cb3b44b2b98c2e693f9e7c6288b8033897ac780f7075779f101d3a3`).

The final local release executable is **6,088,256 bytes** (5.81 MiB), below the
12 MiB soft ceiling. `cargo audit` reports no known vulnerabilities across 395
locked packages. The existing `RUSTSEC-2026-0192` maintenance warning for
`ttf-parser 0.25.1` remains the already recorded B2/B3 watch item; it is not a
security vulnerability and was not introduced by the project-format work.

## B2 acceptance result

- create/save/open round trip: **PASS**;
- old fixture migration with audit provenance: **PASS**;
- source and fit identity retained: **PASS**;
- typography and palette provenance retained: **PASS**;
- missing/changed external data produces warnings without replacement: **PASS**;
- identical project state produces an identical resolved Display List: **PASS**;
- schema is stable enough to support B3/B4/B5: **PASS**.

## Next dependency

Proceed to **B3.1**: promote the validated A7 scale, locator, formatter and
bounded iterative layout into a formal production module. Do not introduce a
second UI-owned layout model or backend-specific geometry.
