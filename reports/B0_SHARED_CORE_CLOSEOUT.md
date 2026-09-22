# B0 — Shared-core closeout

Date: 2026-09-22  
Status: **DONE**

## Scope delivered

InstPlot Lite is now the application consumer of four shared crates in one Cargo
workspace:

- `instplot-core`: data model and stable identity;
- `instplot-io`: text, XLS/XLSX import and data export;
- `instplot-fitting`: fitting algorithms and the sole expression implementation;
- `instplot-processing`: processing algorithms and the formula adapter, with the
  one-way acyclic edge `instplot-processing -> instplot-fitting`.

The Lite root package remains the application package. Cargo supports a package
and workspace root in the same manifest, so moving it physically into
`apps/instplot-lite` was intentionally avoided: that would rewrite established
asset, installer and release paths without improving isolation. The workspace
has exactly five members and one root `Cargo.lock`.

The Studio repository contains `prototypes/shared-core-consumer-spike`, which
consumes `instplot-core` from the Lite Git repository pinned to exact revision
`80ad374044d91dd3c306a384fc7f07ba82cac429`. It does not use a machine-local path
and does not copy the model.

## Reviewable implementation commits

- `f0644cf` — extract shared data core;
- `fdcaf08` — extract shared data I/O;
- `7e0239d` — extract independent processing algorithms;
- `a96a674` — extract fitting and expressions;
- `892e8ad` — complete shared processing integration;
- `80ad374` — establish the five-member workspace;
- Studio `7e20cf4` — verify external shared-core consumption.

## Verification evidence

The final Lite workspace state passed:

- `cargo test --workspace --locked`: **116 tests passed** across the application
  and four shared crates;
- `cargo clippy --workspace --locked --all-targets --all-features -- -D warnings`:
  **zero warnings**;
- release build and import smoke test: **passed**;
- release executable: **7,011,856 bytes**, unchanged from the extraction
  baseline;
- mandatory Lite fonts: **1,182,516 bytes** total (`InstPlotSans-Latin.ttf`
  119,444; the Lite UI's existing `InstPlotSansSC-Level1.otf` 1,063,072),
  unchanged; this does not add or select Studio typography;
- smoke-process maximum RSS: **7,946,240 bytes** in the final local run, with no
  material regression;
- `cargo metadata`: exactly five workspace members, no package lacking license
  metadata, and all declared expressions provide a permissive distribution
  choice compatible with the current release;
- `cargo tree --duplicates`: no duplicated InstPlot implementation crate; only
  pre-existing GUI/platform ecosystem version families remain;
- `otool -L`: only expected macOS system frameworks and libraries; no Studio
  rendering, typography, PDF or raster runtime entered the Lite executable;
- Rust 1.98.0, Cargo 1.98.0 and Clippy 0.1.98 match the pinned toolchain;
- `cargo audit`: **no known vulnerability** in 369 locked dependencies. It emits
  one allowed maintenance warning, `RUSTSEC-2024-0436`, for indirect
  `paste 1.0.15` through `nalgebra -> simba`; this is not a vulnerability and is
  retained as an explicit dependency-watch item.

The Studio-side Git consumer passed its test and Clippy with warnings denied,
including stable `plot_id` and point-content assertions.

GitHub Actions run
[`35691281679`](https://github.com/zhiyuzhang001-a11y/InstPlot-Lite/actions/runs/35691281679)
for the final Lite workspace commit passed on macOS 15, Windows and Ubuntu 22.04,
including workspace formatting/tests, release builds and the platform-appropriate
release importer smoke check. Packaging and publishing jobs were correctly skipped
because this was not a release event.

## Compatibility and boundary result

- Existing Lite behavior and all regression tests are preserved.
- Data, I/O, processing, fitting and formula evaluation each have one production
  implementation, not forked Lite/Studio copies.
- Lite remains independently buildable and releasable.
- Studio-only GUI, typography and publishing dependencies remain outside the
  Lite dependency graph.
- B1 can start without changing B0 public semantics.

## Next dependency

Proceed to **B1.1**: define the independent `instplot-studio` application
package/binary and bootstrap the smallest product shell. Do not copy Lite UI or
start the B2 project schema in that step.
