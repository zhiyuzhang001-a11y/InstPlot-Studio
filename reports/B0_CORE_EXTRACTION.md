# B0.2 — `instplot-core` extraction

Status: **DONE**

Completed: 2026-09-22

## Source and scope

- Repository: `zhiyuzhang001-a11y/InstPlot-Lite`
- Starting commit: `b2fecd1cc46517ee013c836a4427d6609b9494d8`
- Result commit: `f0644cf` (`refactor: extract shared data core`)
- Package: `instplot-lite 0.3.7`
- Preserved unrelated state: the pre-existing untracked
  `tests/fixtures/fitted-curves.csv` remains unmodified and uncommitted.

This increment moved only the existing data model, identity implementation and
UI-independent `DataSet` methods. Import, export, processing, fitting, history,
fonts, image export, updater and GUI behavior were not moved or redesigned.

## Result

- Added `crates/instplot-core` as an independent Rust library crate.
- Moved `NumericColumn`, `FitLink`, `DataSetKind`, `DataSet`, stable `plot_id`
  generation, row iteration/selection, deletion/restoration and min/max
  decimation into the shared crate.
- Kept `src/data.rs` as the importer and compatibility namespace by re-exporting
  the core types. Existing Lite call sites therefore retain their names.
- Added a direct regression test for the existing identity byte contract.
- Added only one production dependency: the local `instplot-core` path crate,
  which itself has no third-party dependencies.

## Verification

The following checks pass on macOS arm64 with the repository-pinned Rust 1.98.0:

```sh
cargo fmt --all --check
cargo fmt --manifest-path crates/instplot-core/Cargo.toml --check
cargo test --locked
CARGO_TARGET_DIR=target cargo test --locked \
  --manifest-path crates/instplot-core/Cargo.toml
cargo clippy --locked --all-targets -- -D warnings
CARGO_TARGET_DIR=target cargo clippy --locked \
  --manifest-path crates/instplot-core/Cargo.toml --all-targets -- -D warnings
cargo build --release --locked
target/release/instplot-lite --check tests/fixtures/smoke.csv
```

Results:

- Lite tests: 111 passed;
- core tests: 5 passed, including the new identity-contract test;
- combined behavior coverage: 116 passed, 0 failed, 0 ignored;
- Clippy: zero warnings for Lite and core;
- smoke import: 7 rows, 2 columns, UTF-8 comma-delimited input;
- core dependency tree: `instplot-core` only, with no GUI or third-party crate.

## Size and memory comparison

Before extraction:

- stripped release executable: 7,011,840 bytes;
- maximum resident set size for the smoke check: 8,028,160 bytes;
- mandatory font assets: 1,182,516 bytes.

After extraction:

- stripped release executable: 7,011,856 bytes, a 16-byte increase;
- maximum resident set size for the smoke check: 8,044,544 bytes, a 16 KiB
  increase within single-run measurement noise;
- mandatory assets unchanged.

No Studio dependency entered the Lite graph, and the executable/asset boundary
remains far below the 15 MiB architecture limit.

## Next task

B0.3 extracts import and data export into `instplot-io`, moves their 42 existing
tests, and proves text plus XLS/XLSX round trips without changing formats or
error behavior.
