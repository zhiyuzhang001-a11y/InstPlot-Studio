# B0.4 — `instplot-processing` first-pass extraction

Status: **DONE**

Completed: 2026-09-22

## Source and scope

- Repository: `zhiyuzhang001-a11y/InstPlot-Lite`
- Starting commit: `fdcaf08`
- Result commit: `7e0239d` (`refactor: extract shared processing algorithms`)
- Package: `instplot-lite 0.3.7`
- Preserved unrelated state: the pre-existing untracked
  `tests/fixtures/fitted-curves.csv` remains unmodified and uncommitted.

This increment moved only existing processing types and numerical algorithms.
It did not change algorithm parameters, numerical behavior, validation codes,
Lite UI, edit history or Studio features.

## Result

- Added `crates/instplot-processing` as an independent Rust library crate.
- Moved `ProcessingOperation`, `Anchor`, `ProcessingMetadata`,
  `ProcessingResult` and `ProcessingError`.
- Moved centering, normalization, polynomial-background subtraction, local
  flattening and Savitzky–Golay denoising.
- Moved eight direct numerical and validation tests with the algorithms.
- Kept the existing dataset dispatcher and formula connection as a thin Lite
  adapter, with its five behavioral tests.
- The shared processing crate has no third-party, GUI, I/O, fitting or core
  dependency in this first pass.

The `Formula` operation and metadata variants already use the shared types, but
formula evaluation still calls the one existing Lite fitting implementation.
No expression code was copied.

## Verification

Formatting, locked tests and Clippy `-D warnings` pass separately for Lite,
`instplot-core`, `instplot-io` and `instplot-processing`.

Results:

- Lite application and adapter tests: 61 passed;
- core tests: 5 passed;
- I/O tests: 42 passed;
- shared processing algorithm tests: 8 passed;
- combined coverage: 116 passed, 0 failed, 0 ignored;
- Clippy: zero warnings for all four packages;
- processing dependency tree: only `instplot-processing` itself;
- smoke import: 7 rows, 2 columns, UTF-8 comma-delimited input.

## Size and memory comparison

- stripped release executable before B0.4: 7,011,856 bytes;
- stripped release executable after B0.4: 7,011,856 bytes;
- maximum resident set size in the post-change smoke run: 7,929,856 bytes;
- mandatory assets unchanged.

The extraction has no release-size cost. Studio does not link this crate merely
because it exists; applications select it only when they need processing.

## Next task

B0.5 extracts fitting and the single expression implementation into
`instplot-fitting`. B0.6 can then reconnect the shared processing formula branch
without duplicating expression parsing or numerical code.
