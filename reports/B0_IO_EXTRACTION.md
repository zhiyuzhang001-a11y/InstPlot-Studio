# B0.3 — `instplot-io` extraction

Status: **DONE**

Completed: 2026-09-22

## Source and scope

- Repository: `zhiyuzhang001-a11y/InstPlot-Lite`
- Starting commit: `f0644cf`
- Result commit: `fdcaf08` (`refactor: extract shared data IO`)
- Package: `instplot-lite 0.3.7`
- Preserved unrelated state: the pre-existing untracked
  `tests/fixtures/fitted-curves.csv` remains unmodified and uncommitted.

This increment mechanically moved existing import/export code. It did not
change file formats, parsing rules, error codes, source/fit identity, numerical
data, application UI or Studio behavior.

## Result

- Added `crates/instplot-io` as an independent Rust library crate.
- Moved TXT/CSV/DAT/TSV parsing, encoding detection and XLS/XLSX import from the
  former Lite `data` module.
- Moved text, selected-column, combined/separate and XLSX export from the former
  Lite `data_export` module.
- Moved all 42 I/O unit and round-trip tests with their implementation.
- Kept thin Lite compatibility modules so existing application call sites retain
  their names.
- Moved `calamine`, `chardetng`, `csv`, `encoding_rs` and `rust_xlsxwriter` out
  of Lite's direct dependency list and into `instplot-io`.
- Kept the resolved `encoding_rs 0.8.35` and `rust_xlsxwriter 0.99.0` versions
  exact in the standalone crate test graph.

The moved source differs from its previous location only in crate imports and
two repository-relative fixture paths.

## Verification

The following checks pass on macOS arm64 with Rust 1.98.0:

```sh
cargo fmt --all --check
cargo fmt --manifest-path crates/instplot-core/Cargo.toml --check
cargo fmt --manifest-path crates/instplot-io/Cargo.toml --check
cargo test --locked
CARGO_TARGET_DIR=target cargo test --locked \
  --manifest-path crates/instplot-core/Cargo.toml
CARGO_TARGET_DIR=target cargo test --locked \
  --manifest-path crates/instplot-io/Cargo.toml
cargo clippy --locked --all-targets -- -D warnings
CARGO_TARGET_DIR=target cargo clippy --locked \
  --manifest-path crates/instplot-core/Cargo.toml --all-targets -- -D warnings
CARGO_TARGET_DIR=target cargo clippy --locked \
  --manifest-path crates/instplot-io/Cargo.toml --all-targets -- -D warnings
cargo build --release --locked
target/release/instplot-lite --check tests/fixtures/smoke.csv
```

Results:

- Lite application/support tests: 69 passed;
- core tests: 5 passed;
- I/O tests: 42 passed;
- combined coverage: 116 passed, 0 failed, 0 ignored;
- Clippy: zero warnings for all three packages;
- all CSV/TSV/TXT/DAT and XLS/XLSX round-trip tests pass;
- smoke import: 7 rows, 2 columns, UTF-8 comma-delimited input;
- `instplot-io` dependency tree contains no eframe, egui, dialog, raster,
  processing or fitting dependency.

## Size and memory comparison

- stripped release executable before B0.3: 7,011,856 bytes;
- stripped release executable after B0.3: 7,011,856 bytes;
- maximum resident set size in the post-change smoke run: 7,913,472 bytes;
- mandatory assets unchanged.

The extraction adds no release-size cost and does not add Studio dependencies to
Lite.

## Product-boundary note

`instplot-core + instplot-io` are the shared capabilities Studio V1 actually
needs to accept Lite data correctly. Extracting processing and fitting next does
not commit Studio to exposing those operations or linking them by default; they
remain shared algorithm packages available only to applications that select
them.

## Next task

B0.4 begins `instplot-processing` with the independent numerical algorithms and
types. The formula branch remains a thin Lite adapter until the single existing
expression implementation moves with `instplot-fitting` in B0.5.
