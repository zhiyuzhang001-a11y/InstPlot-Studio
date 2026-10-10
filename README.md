# InstPlot Studio

InstPlot Studio is an independent Rust application for publication-quality scientific figures.
It is designed to cooperate with InstPlot Lite without becoming a mode, crate, or
workspace member inside the Lite repository.

The repository directory retains the historical `SciPlot` checkout name. Production
packages use formal `instplot-*` identities under `apps/` and `crates/`; only explicitly
historical comparison fixtures remain under `prototypes/`.

## Repository boundary

- This repository owns the InstPlot Studio application, figure document, layout compiler,
  display list, publication backends, visual benchmarks, and project format.
- InstPlot Lite remains an independently buildable and releasable application.
- During Part A, InstPlot Studio must not use path dependencies into the Lite checkout.
- Shared data code is extracted only after Gate A, with an explicit compatibility
  contract and regression tests for Lite.
- Generated artifacts and machine-local paths are never committed as dependencies.

## Current architecture

The staged modularization plan is complete through the second-product proof. See
[`docs/INSTPLOT_STUDIO_MODULARIZATION_PLAN.md`](docs/INSTPLOT_STUDIO_MODULARIZATION_PLAN.md),
[`adr/023-modular-product-boundaries.md`](adr/023-modular-product-boundaries.md), and
[`docs/INSTPLOT_EXTENSION_GUIDE.md`](docs/INSTPLOT_EXTENSION_GUIDE.md).
The repository finalization procedure is recorded in
[`docs/INSTPLOT_STUDIO_CODEBASE_FINALIZATION_PLAN.md`](docs/INSTPLOT_STUDIO_CODEBASE_FINALIZATION_PLAN.md),
and release changes are summarized in [`CHANGELOG.md`](CHANGELOG.md).

The Windows in-place updater's implementation, failure analysis, reuse boundaries,
and exact GUI/public verification evidence are documented in
[`docs/WINDOWS_UPDATER_ENGINEERING_PLAYBOOK.md`](docs/WINDOWS_UPDATER_ENGINEERING_PLAYBOOK.md).
This is an isolated QA delivery record, not an announcement of production updater enablement.

`apps/instplot-studio` is the full product. `apps/instplot-demo` is the independent
InstPlot Quick consumer that imports data, selects XY columns, edits labels and exports
SVG through shared services. Formal reusable crates are `instplot-text`,
`instplot-render`, `instplot-layout`, `instplot-export`, and `instplot-ui`.

## Historical delivery record

Part A and Gate A are complete. The local 21-check validation, typography and
visual matrix, dependency audit, macOS PDF review, Windows automated audit and
Windows 10 manual scaling/input/PDF review pass. Linux and physical Retina
checks are explicitly deferred to a release that claims those targets; they are
not represented as passed.

Part B and the modularization program are complete. B0 established that Lite consumes the shared core, I/O,
processing, fitting, and expression crates from one Cargo workspace, while an
independent Studio-side consumer proves that the shared data model can be used
through a pinned Git revision. B1 is complete: the independent
`instplot-studio` application reads shared data, previews one Display List,
exposes the minimum editing panels and exports the fixed PDF without creating a
window. B2 is complete: the formal versioned Figure Document, strict `.instplot`
JSON project format, atomic save, backup recovery, migration and external-source
change diagnostics are implemented. B3.1 is complete: the validated A7 scale,
locator, formatter and bounded single-axes layout now live in the formal
`instplot-layout` crate while the historical spike remains a compatibility test
facade. B3.2 is complete: the versioned Figure Document now resolves formal
axes geometry, spines, ticks, major grid, semantic labels and an explicit data
clip with reversible stable identities. B3.3 is complete: the GUI preview now
starts from the formal layout and paints resolved bundled-font runs, including
scientific baseline shifts and the rotated Y label. B3.4 is complete: embedded
linear line, scatter and composed line-plus-marker artists now flow through the
same formal layout and preview. B3.5 is complete: log axes, error bars,
reference/baseline lines, semantic annotations and the explicit legend now use
the formal layout. B3.6 is complete: Preview, PDF and PNG now consume one
formal layout result and resolved Display List. B4 is complete: versioned
palette and semantic registries now drive a node-specific Publication Check in
both the live Inspector and headless JSON report. The Studio side of the B5
handoff is implemented; Lite producer/launcher integration is deliberately
deferred while B5P improves the actual editing workflow and product details. See
[`docs/INSTPLOT_STUDIO_PART_B_SHORT_PLAN.md`](docs/INSTPLOT_STUDIO_PART_B_SHORT_PLAN.md).
The detailed polish sequence is in
[`docs/INSTPLOT_STUDIO_PRODUCT_POLISH_SHORT_PLAN.md`](docs/INSTPLOT_STUDIO_PRODUCT_POLISH_SHORT_PLAN.md).
The reproducible P0 workflow baseline and prioritized usability gaps are in
[`reports/B5P_P0_WORKFLOW_BASELINE.md`](reports/B5P_P0_WORKFLOW_BASELINE.md).

Build and check the production Studio shell with:

```sh
cargo test --workspace --locked
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
cargo run --release --locked --package instplot-studio -- --product-info
```

For a single Spotlight-searchable local macOS app, run
`python3 scripts/install_studio_macos.py`. It builds the release binary and
installs `~/Applications/InstPlot Studio.app` with a stable bundle identifier.
Run the same command after later changes: it replaces that same app rather
than creating a version-suffixed copy. Close the installed app before updating.

Studio opens TXT, CSV, DAT, TSV, XLSX and XLS data through the same shared
parser as Lite, including multiple sections or worksheets. Use File → Open Data,
the data drawer, or drag files into the window. The initial seven-curve figure
is a disposable example: the first successful import replaces it, even if the
example was edited. Later imports add to the current figure; a failed file
does not discard other successful files in the same batch. A fit file selected
before its source is retried after the source imports. Lite exports with an
aggregate fit marked `Parent-ID: *` do not identify the fit's individual source
datasets: Studio imports the source data and explicitly warns that it skipped
that fit, rather than inventing a scientific association.
The data drawer groups datasets by file, keeps X/Y and plot creation controls
with the selected dataset, and links directly to curves already using it. A
curve's compact editor also exposes its data binding, so changing the plotted
file or X/Y columns does not require opening the full inspector.
Repeated basenames are distinguished by their parent folder, and section names
are shown without repeating the filename or exposing embedded internal IDs.
Each file group can be removed with its linked plot objects in one undoable
action; multi-section files also allow removing an individual data section.
The File menu clears all imported data in one undoable action.
These operations change only the current project, never the source files on disk.
Project schema 7 preserves imported-file provenance and distinguishes each manually entered XY
group. Manual groups retain their original X/Y repetitions and SD/SEM recipe in the project; the
first project save can additionally create one managed CSV, TSV, TXT, DAT or XLSX file per group.
Later saves update those files atomically after fingerprint checks, while File → Export → Export
Data writes independent copies. Older project schemas migrate automatically, although legacy
manual data cannot recover repetitions that were never stored.
The data drawer remains focused on files and XY/error bindings; raw numeric processing remains in
Lite rather than crowding the Studio drawing interface.
Reimporting the same path refreshes data only while its existing dataset IDs
remain present; if a section disappears or changes identity, Studio rejects the
refresh and keeps the current figure unchanged to avoid stale curves. Start a
new figure to load the changed set of sections.

Run the complete repeatable B1 validation with:

```sh
python3 scripts/validate_b1.py
```

Run the complete repeatable B2 project-format validation with:

```sh
python3 scripts/validate_b2.py
```

Validate the formal B3.1 layout promotion with:

```sh
python3 scripts/validate_b3_1.py
```

Validate the B3.2 formal axes and clipping contract with:

```sh
python3 scripts/validate_b3_2.py
```

Validate the B3.3 resolved preview path with:

```sh
python3 scripts/validate_b3_3.py
```

Validate the B3.4 line/scatter artist slice with:

```sh
python3 scripts/validate_b3_4.py
```

Validate the B3.5 complete artist framework with:

```sh
python3 scripts/validate_b3_5.py
```

Validate the B3.6 single Display List pipeline with:

```sh
python3 scripts/validate_b3_6.py
```

Validate the B4 registries and Publication Check with:

```sh
python3 scripts/validate_b4.py
```

Validate the Studio side of the B5 Lite handoff with:

```sh
python3 scripts/validate_b5_studio.py
```

The versioned exchange contract is documented in
[`docs/INSTPLOT_HANDOFF_PROTOCOL.md`](docs/INSTPLOT_HANDOFF_PROTOCOL.md).

The release binary also exposes window-independent project checks:

```sh
target/release/instplot-studio --create-project figure.instplot
target/release/instplot-studio --check-project figure.instplot
```

Validate the A1 contract with:

```sh
python3 scripts/validate_a1.py
```

Run the complete repeatable local Part A validation with:

```sh
python3 scripts/validate_part_a.py
```

The command validates the frozen A1 contract and runs formatting, tests, and
Clippy with warnings denied for every prototype. It writes a machine-readable
summary and per-command logs under `target/part-a-validation/`.
The captured macOS baseline is documented in
[`reports/A8_LOCAL_VALIDATION.md`](reports/A8_LOCAL_VALIDATION.md).

Run the evidence-based release audit with:

```sh
python3 scripts/audit_part_a.py
```

On native Windows, run `scripts/audit_part_a.ps1`. The audit reruns all 21
validation checks, verifies font assets and dependency graphs, inventories
licenses/MSRV/native links, queries direct-dependency release activity, runs
RustSec when `cargo-audit` is installed, and requires structured evidence for
PDF-viewer, native-input, and display-scaling checks. It exits `0` only for a complete
pass, `1` for a detected failure, and `2` when required evidence or tooling is
still blocked. Exact findings and remediation hints are written under
`target/part-a-audit/`.

CI may use `--automated-only`; exit `0` then means only that the full automated
scope passed. It never creates or substitutes the native GUI evidence required
by the complete audit.

## Local Rust toolchain

This repository pins Rust 1.98.0 in `rust-toolchain.toml`. The installed
`1.98.0-aarch64-apple-darwin` rustup toolchain includes Clippy 0.1.98 and
rustfmt. The machine's default `stable-aarch64-apple-darwin` toolchain does not
include Clippy, so checking only `rustup component list --installed` against the
default toolchain gives a false impression that Clippy is unavailable.

From this repository or any prototype directory, use:

```sh
cargo clippy --version
cargo clippy --locked --all-targets -- -D warnings
```

For an explicit toolchain-independent diagnostic, use:

```sh
rustup run 1.98.0-aarch64-apple-darwin cargo clippy --version
rustup component list --toolchain 1.98.0-aarch64-apple-darwin --installed
```

The authoritative plan is in
[`docs/SCIPLOT_EXECUTION_PLAN.md`](docs/SCIPLOT_EXECUTION_PLAN.md).
The accepted V1 typography contract is in
[`docs/INSTPLOT_STUDIO_TYPOGRAPHY_SPEC.md`](docs/INSTPLOT_STUDIO_TYPOGRAPHY_SPEC.md).
