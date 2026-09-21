# A0 — InstPlot Lite baseline

Status: **local baseline captured; cross-platform evidence remains open**  
Captured: 2026-09-20, macOS arm64

## Identity

- Product: InstPlot Lite 0.3.4
- Branch: `main`
- Commit: `4e95a75f602daf77b9f0c8fb4a1ea0b56353da4a`
- Rust: `rustc 1.98.0 (88d9e12ae 2026-08-18)`
- Cargo: `cargo 1.98.0 (797e8a9bc 2026-08-05)`
- Clippy: `clippy 0.1.98 (88d9e12ae1 2026-08-18)`, installed in the
  `1.98.0-aarch64-apple-darwin` rustup toolchain; it is not installed in the
  machine's default `stable-aarch64-apple-darwin` toolchain
- Host: `aarch64-apple-darwin`
- Declared CI platforms: Ubuntu 22.04, macOS 15, Windows latest

The Lite checkout contained unrelated local documentation/release-script changes
and an untracked fitted-curve fixture. They were preserved. No application source,
manifest, asset, or test behavior was changed while capturing this baseline.

## Reproducible local commands

```sh
cargo fmt --check
cargo test --locked
cargo build --release --locked
target/release/instplot-lite --check tests/fixtures/smoke.csv
/usr/bin/time -l target/release/instplot-lite --check tests/fixtures/smoke.csv
```

## Results

- Formatting: pass
- Unit tests: 113 passed, 0 failed, 0 ignored
- Smoke import: pass; 7 rows, 2 columns, UTF-8 comma-delimited input
- Smoke `--check` maximum resident set size: 8,044,544 bytes
- Release executable, macOS arm64: 6,929,280 bytes (6.608 MiB)
- Mandatory external runtime assets: none; current fonts are embedded
- Embedded Latin font: 119,444 bytes
- Embedded CJK subset: 1,063,072 bytes
- Latest locally available macOS arm64 DMG (0.3.4): 4,045,163 bytes
- Locally available Windows x64 installer (0.3.3): 6,108,135 bytes

Recorded SHA-256 values:

- release executable: `9505099222b5de2f8d3f31d3bff02d66719b9faef568621ab864cdf419ba4185`
- Latin font: `25e940e1e5275125303bf31c427feb94b98eb563ebe0451a3b2a65b31fe8da18`
- CJK subset: `c88ac8726709ec9e4e94e0d5577b6b125fe20b3e6041f79e58b7962f94d83081`
- smoke fixture: `cdf639a30cb353afc2c387c7e104c94f3897c709cafc4a33a3b8bbf8be0acd79`

Installer sizes are recorded for context only. The architecture size boundary is
the stripped per-architecture executable plus mandatory runtime assets.

## Direct production dependency baseline

- `calamine 0.36.1`
- `chardetng 1.0.0`
- `csv 1.4.0`
- `eframe 0.36.1`, default features disabled; `default_fonts`, `glow`, `wayland`, `x11`
- `egui_plot 0.37.0`
- `encoding_rs 0.8.35`
- `fasteval2 2.1.1`
- `nalgebra 0.33.2`, default features disabled; `std`
- `png 0.18.0`
- `rfd 0.17.2`
- `rust_xlsxwriter 0.99.0`
- Windows-only updater dependencies: `ed25519-dalek`, `semver`, `serde`,
  `serde_json`, `sha2`, and `ureq` with a restricted feature set

Release profile: one codegen unit, LTO enabled, size optimization, aborting panic,
and symbol stripping.

## Covered regression contracts

The passing test suite provides evidence for:

- delimited text import, encoding detection, headers, comments and sections;
- XLS and XLSX import;
- source/fit identity and association;
- deletion, restoration, undo and redo;
- processing operations;
- built-in and custom fitting;
- CSV, text and XLSX export/round trip;
- PNG plot-rectangle export;
- peak-preserving large-data decimation;
- bundled font presence and fallback order;
- update manifest/signature validation.

## Evidence still required before A0 is complete

- record GUI cold-start and idle-memory baseline using one agreed procedure;
- archive one representative Lite PNG baseline;
- record current Windows, macOS and Linux CI run identities and results;
- record current Windows and Linux release executable sizes using the same scope;
- confirm fixed representative fixtures for every critical import/export route;
- document any warnings produced by clean release builds on all three platforms.

Until those items are captured, A0 is in progress and A1 may define contracts, but
no Lite shared-core extraction is permitted.
