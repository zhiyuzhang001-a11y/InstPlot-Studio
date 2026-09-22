# B0.1 — InstPlot Lite shared-core inventory

Status: **DONE; migration boundaries frozen for the first extraction**

Captured: 2026-09-22

## 1. Source snapshot

- Source repository: `zhiyuzhang001-a11y/InstPlot-Lite`
- Branch: `main`
- Commit: `b2fecd1cc46517ee013c836a4427d6609b9494d8`
- Package: `instplot-lite 0.3.7`
- Rust edition: 2024
- Working tree note: the pre-existing untracked
  `tests/fixtures/fitted-curves.csv` was not modified or added.
- Difference from the A0 source snapshot: the application moved from 0.3.4 to
  0.3.7. Changes since the recorded A0 commit affect release/update plumbing,
  documentation and `main.rs`/`updater.rs`; the candidate shared data,
  processing, fitting and export modules are unchanged.

The current locked test suite passes: **115 passed, 0 failed, 0 ignored**.

## 2. Current dependency shape

```text
main
  -> app
  -> data (headless --check path)
  -> fonts / updater

app
  -> data
  -> data_export
  -> processing
  -> fitting
  -> edit_history
  -> fonts / image_export / updater

data_export -> data
edit_history -> data + processing
processing -> data + fitting expression functions

data, fitting
  -> no application module
```

The current package has only a binary crate. All modules are private at the
crate root even when individual items are declared `pub`.

## 3. Module inventory and ownership

### `instplot-core`

Move from the model portion of `src/data.rs`:

- `NumericColumn`;
- `FitLink`;
- `DataSetKind` and `metadata_value`;
- `DataSet`;
- stable `plot_id` generation;
- `display_name`, finite/alive row iteration, rectangular selection, deletion
  and restoration;
- the current peak-preserving min/max decimation method.

Although decimation and the Chinese unnamed-data fallback are not ideal core
policy, moving the complete existing `DataSet` implementation is the smallest
behavior-preserving first step. They may be separated only after B0, with their
existing tests retained.

The first cross-crate API addition should be only the identity function needed
by `instplot-io`. Do not add constructors, serialization or Figure Document
types during this move.

### `instplot-io`

Move the importer portion of `src/data.rs` plus all of `src/data_export.rs`:

- TXT/CSV/DAT/TSV decoding and strict parsing;
- XLS/XLSX import;
- sectioned InstPlot source/fit metadata;
- `ImportError`;
- `TextExportFormat`, `ExportSummary` and `FitCurveExport`;
- selected, combined and separate text export;
- XLSX export and source/fit round trips.

Direct third-party dependencies belong here: `calamine`, `chardetng`, `csv`,
`encoding_rs` and `rust_xlsxwriter`. This crate depends on `instplot-core` and
must not depend on either application or a GUI crate.

### `instplot-processing`

Move from `src/processing.rs`:

- `ProcessingOperation`, `Anchor`, `ProcessingMetadata`, `ProcessingResult` and
  `ProcessingError`;
- centering, normalization, polynomial background removal, local flattening and
  Savitzky–Golay denoising;
- dataset dispatch after its dependencies are available.

The non-formula algorithms depend only on `instplot-core`. The `Formula`
operation currently calls expression functions in `fitting`; it must not be
copied. Start this extraction with the independent algorithms and keep the
existing Lite dispatcher as a temporary adapter. Finish the formula branch
after `instplot-fitting` is extracted.

### `instplot-fitting`

Move all of `src/fitting.rs`:

- `FitMethod`, `FitResult`, `FitError` and `FormulaAxis`;
- polynomial, exponential, logarithmic, power and custom fitting;
- row formula evaluation and output-axis detection;
- constant-expression evaluation;
- equation and display-equation generation.

Direct third-party dependencies belong here: `fasteval2` and `nalgebra`.
Expression evaluation remains owned by this crate, matching the product
boundary's “fitting and expression” rule.

### Remain in InstPlot Lite during B0

- `app.rs`: all eframe/egui state, dialogs, selection UI, plot interaction,
  display formatting and application orchestration;
- `main.rs`: startup, CLI/check route and native entry point;
- `fonts.rs`: Lite interface fonts;
- `image_export.rs`: screenshot/egui image export;
- `updater.rs`: Windows update flow;
- `edit_history.rs`: Lite editing command history.

`edit_history` consumes core and processing APIs but Studio V1 does not require
Lite's destructive-edit history. Keeping it in the Lite app avoids expanding
the shared contract prematurely.

## 4. Existing public surface to preserve

The initial extraction must preserve names, arguments, return values, error
codes and numerical behavior for these externally consumed groups:

- core: all `DataSet` fields and its six current methods, plus
  `DataSetKind::metadata_value`;
- I/O: `read_data_file`, the non-test export entry points, format/summary types
  and fit-export view;
- processing: `apply_to_dataset`, five direct numerical operations and the
  processing operation/result/error types;
- fitting: four expression entry points, `fit_values` and the fit types;
- history: the existing Lite-facing `EditHistory` methods and `HistoryEffect`.

The current structs expose public fields and therefore permit invalid
`row_count`/column/`alive` combinations. B0 must not silently redesign this API.
Invariant-enforcing constructors are a later, separately tested change.

## 5. Verification anchors

The 115 tests divide into the following migration anchors:

- core data behavior: 4 tests;
- import and data export: 42 tests;
- processing: 13 tests;
- fitting and expressions: 12 tests;
- Lite edit history: 6 tests;
- app, fonts, PNG export, updater and entry point: 38 tests.

After moving a module, move its unit tests with it and also run the complete
Lite suite. Cross-module round-trip tests belong in `instplot-io`; Lite UI and
orchestration tests stay in the app.

Required commands for every code-moving increment:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
target/release/instplot-lite --check tests/fixtures/smoke.csv
```

Record the stripped release executable size, mandatory runtime assets and the
agreed smoke-process memory measurement before and after each extraction.

## 6. Migration sequence

1. **B0.2 preflight and core extraction** — refresh the 0.3.7 size/memory
   baseline, add only `instplot-core`, move the data model/identity code and its
   four tests, then make Lite consume the path dependency.
2. **B0.3 I/O extraction** — add `instplot-io`, move importer/exporter code and
   all 42 I/O tests, then prove text and XLS/XLSX round trips.
3. **B0.4 processing pass one** — add `instplot-processing`, move independent
   numerical algorithms and types, while the formula dispatcher remains a thin
   Lite adapter.
4. **B0.5 fitting extraction** — add `instplot-fitting`, move fitting and the
   shared expression implementation with all 12 tests.
5. **B0.6 processing completion** — reconnect the formula operation to the one
   expression implementation; document the one-way
   `instplot-processing -> instplot-fitting` edge unless an ADR selects a small
   expression crate instead.
6. **B0.7 application/workspace closeout** — move the Lite binary under the
   application boundary, create the workspace root, run all release and
   dependency audits, and prove the Studio prototype can consume
   `instplot-core` without copying it.

Each item is its own reviewable commit. Do not combine a file move with schema,
error-message, algorithm, dependency-version or GUI changes.

## 7. Risks that remain explicit

- `data.rs` currently mixes the core model and I/O across one file; the split
  must preserve private parser helpers and identity bytes exactly.
- processing-to-fitting expression reuse conflicts with a perfectly flat
  sibling-crate diagram. The proposed one-way edge is acyclic and preserves one
  implementation, but its final form must be recorded before B0.6.
- fit identity/orchestration (`FitOverlay`, `FitTarget`, filtering and batch
  selection) remains mixed into `app.rs`; it is not part of the first shared
  API and must not be mistaken for the fitting algorithm itself.
- core error types do not yet exist; current import, processing and fitting
  errors have stable codes but contain Chinese presentation text. Initial moves
  preserve them. Error unification/localization is a later explicit change.
- Studio typography, layout, PDF and raster dependencies must never enter the
  Lite dependency graph during these extractions.

## 8. B0.1 conclusion

The first code-moving task is unblocked. Start B0.2 with a fresh 0.3.7 release
baseline, then extract only `instplot-core`. Do not create the Studio app, move
I/O, or change public data semantics in that commit.
