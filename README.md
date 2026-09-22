# InstPlot Studio

InstPlot Studio is an independent Rust application for publication-quality scientific figures.
It is designed to cooperate with InstPlot Lite without becoming a mode, crate, or
workspace member inside the Lite repository.

The repository directory and existing prototype package identifiers still use
the historical `SciPlot` name until a separate mechanical rename is completed.

## Repository boundary

- This repository owns the InstPlot Studio application, figure document, layout compiler,
  display list, publication backends, visual benchmarks, and project format.
- InstPlot Lite remains an independently buildable and releasable application.
- During Part A, InstPlot Studio must not use path dependencies into the Lite checkout.
- Shared data code is extracted only after Gate A, with an explicit compatibility
  contract and regression tests for Lite.
- Generated artifacts and machine-local paths are never committed as dependencies.

## Current phase

Part A and Gate A are complete. The local 21-check validation, typography and
visual matrix, dependency audit, macOS PDF review, Windows automated audit and
Windows 10 manual scaling/input/PDF review pass. Linux and physical Retina
checks are explicitly deferred to a release that claims those targets; they are
not represented as passed.

Part B is active. B0 is complete: Lite now consumes the shared core, I/O,
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
clip with reversible stable identities. B3.3 is next: move preview text and
geometry onto that resolved Display List. See
[`docs/INSTPLOT_STUDIO_PART_B_SHORT_PLAN.md`](docs/INSTPLOT_STUDIO_PART_B_SHORT_PLAN.md).

Build and check the production Studio shell with:

```sh
cargo test --workspace --locked
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
cargo run --release --locked --package instplot-studio -- --product-info
```

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
