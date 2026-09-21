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

Part A:

1. A0 — local InstPlot Lite baseline captured; cross-platform evidence remains open;
2. A1 — fixed publication fixture and acceptance contract frozen and validated;
3. A2 — point-based Figure IR and backend-neutral Display List spike implemented
   and locally verified;
4. A3 — Parley/fontique/HarfRust/skrifa retained; bundled TeX Gyre Heros and the
   revised semantic/Unicode contract now pass the local shaping prototype;
5. A4 — semantic labels now reach Krilla PDF and direct tiny-skia raster with
   only bundled Heros faces; local embedding/extraction checks pass. The direct
   SVG prototype is retained as optional, non-blocking research under ADR-021;
6. A5 — Plotine 0.5.2 compared against the same fixture; ADR-012 keeps it out of
   production dependencies and retains only algorithm/test ideas;
7. A6 — eframe/egui shell, native dialog, keyboard input, view zoom, HiDPI
   transform, and window-independent export locally validated; three-platform
   scaling/theme evidence remains an A8 check;
8. A7 — deterministic single-Axes layout, ticks, publication marks, legend
   placement, warnings, hit map, and the Heros metric snapshot pass locally;
9. A8 — the local Heros typography and five-image visual-regression matrix pass;
   native Windows/Linux and remaining final-audit evidence remain.

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
