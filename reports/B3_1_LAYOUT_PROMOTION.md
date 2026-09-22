# B3.1 — Production layout-module promotion

Date: 2026-09-22  
Status: **DONE locally; cross-platform CI pending implementation push**

## Boundary established

The accepted A7 implementation now lives in the production workspace crate
`crates/instplot-layout`. It owns:

- linear and log10 transforms;
- automatic and fixed major locators plus minor locators;
- automatic/shared-exponent, explicit decimal and explicit scientific
  formatting with negative-zero suppression;
- shaped tick-label collision stride;
- the bounded four-pass single-axes layout loop and conservative warning path;
- point-space axes, labels, artists, clipping and Display List generation;
- deterministic legend scoring and outside fallback;
- the geometry-derived hit map.

The old `layout-engine-spike` package no longer contains a second implementation.
It re-exports `instplot-layout`, so the original A7 tests, snapshots and example
generator now exercise production code directly. This keeps Part A evidence
live without allowing prototype and product algorithms to diverge.

`instplot-studio` declares the formal crate as a workspace dependency. B3.2 will
connect the B2 Figure Document axes configuration to this compiler; B3.1 does
not prematurely replace the B1 preview with a partially adapted artist model.

## Verification

The formal workspace tests run the seven original A7 layout/scale tests plus the
new explicit formatter test. The compatibility package independently runs the
same eight tests against the formal crate. The publication snapshot remains
unchanged, including the deterministic axes rectangle, tick geometry, legend
fallback, Display List and hit-map structure.

`python3 scripts/validate_b3_1.py` provides the repeatable phase audit. It checks
formatting, workspace tests, Clippy, the compatibility package, release build,
workspace/dependency topology, single implementation ownership, dependency
licenses and the 12 MiB release-size ceiling.

## Acceptance result

- A7 scale/locator/formatter implementation promoted: **PASS**;
- bounded layout implementation promoted: **PASS**;
- no duplicate prototype implementation remains: **PASS**;
- accepted A7 snapshots execute against production code: **PASS**;
- Studio depends on the formal crate: **PASS**;
- backend-specific layout remains absent: **PASS**.

## Next dependency

Proceed to **B3.2**: adapt the versioned Figure Document axes to the formal
compiler and freeze axes rectangle, spines, ticks, grid, labels and clipping.
