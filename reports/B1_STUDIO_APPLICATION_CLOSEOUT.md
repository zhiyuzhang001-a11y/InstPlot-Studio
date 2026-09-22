# B1 — Independent Studio application closeout

Date: 2026-09-22  
Status: **DONE**

## Scope delivered

InstPlot Studio is now an independent application package and binary with the
visible product identity **InstPlot Studio**. B1 delivers the complete minimum
application shell without copying the Lite application:

- `StudioSession` consumes `instplot-core` and `instplot-io` from exact Lite Git
  revision `80ad374044d91dd3c306a384fc7f07ba82cac429`;
- the native Open action supports the shared TXT, CSV, DAT, TSV, XLSX and XLS
  importer and replaces an already loaded dataset by stable `plot_id` instead
  of silently duplicating it;
- `FigureDocument` is a deliberately non-persistent B1 in-memory editing model;
  schema versioning, serialization, migration and provenance remain reserved
  for B2;
- the UI compiles that document into the validated Part A Display List and
  paints it through the replaceable `PreviewAdapter` trait;
- the application exposes a series tree, dataset list, axis-range inspector,
  view-only canvas zoom, warning panel and status feedback;
- axis edits mutate the non-UI Figure Document and recompile the preview;
- fixed publication PDF export is available from the native save dialog and
  from `--export-fixed-pdf PATH` before any window or GPU context is created;
- the headless `--product-info` path provides deterministic product identity for
  packaging and CI.

Part A render/export crates are intentionally used as transitional B1 backends,
as required by the plan. They remain separate from the B2 formal document and
will be promoted or replaced in the B2/B3 production boundaries rather than
forked into a second implementation.

## Automated verification

`python3 scripts/validate_b1.py` writes structured evidence under
`target/b1-validation/` and passes all **11** checks:

1. workspace formatting;
2. workspace tests;
3. Clippy for all targets/features with warnings denied;
4. stripped release build;
5. exact product identity;
6. headless fixed-PDF export;
7. 12 MiB release-size ceiling;
8. exact workspace membership;
9. exact shared-core/shared-I/O Git revision;
10. complete Cargo license metadata;
11. pinned Rust 1.98.0 toolchain.

The application has **9 passing tests**: product identity, empty-session state,
shared CSV import, stable-ID replacement, fixed-document compilation, valid and
invalid axis edits, PDF generation and headless command routing.

The unchanged Part A validation also passes all **21** checks after the B1
workspace and PDF metadata changes.

## Output, performance and dependency evidence

- stripped release executable: **5,887,792 bytes** (5.62 MiB);
- increase from B1.1 minimal shell: **2,225,904 bytes**;
- mandatory external runtime assets: **none**; the selected font resources are
  embedded in the executable/PDF subsets;
- local process-to-first-canvas smoke result: **131 ms** at 1.0 pixel/point;
- fixed PDF: **8,275 bytes**, one page, PDF 1.7;
- PDF page: **252.283 × 184.252 pt**, matching the 89 × 65 mm fixture;
- PDF title: `InstPlot Studio fixed publication figure`;
- PDF creator: `InstPlot Studio`;
- extracted text includes the axis labels, `μ`, subscript/superscript content,
  annotation and legend;
- rendered inspection shows no clipping, overlap, missing glyph or broken
  rotation;
- native links on macOS are the expected operating-system GUI/framework
  libraries; no external runtime executable or first-launch download is used;
- all locked packages declare license metadata and provide a distribution path
  compatible with the project's MIT boundary; the text-shaping package records
  `MIT AND GUST-Font-License-1.0`, and the bundled font license/manifest remain
  present for later B6 packaging notices;
- `cargo audit` reports **no known vulnerability** in 384 locked packages.

RustSec emits one allowed maintenance warning:
`RUSTSEC-2026-0192` for `ttf-parser 0.25.1`. It enters through the already
validated A3 text/export stack, is not a vulnerability, and remains an explicit
B2/B3 dependency-watch item. Replacing the font parser inside B1 would change a
validated typography path and is therefore not done silently.

## Manual and cross-platform evidence

- the local release GUI reached its first canvas and remained responsive during
  the bounded smoke run;
- the raw development executable is not yet a macOS `.app` bundle, so macOS UI
  automation could not bind it by application identity; packaging is a B6 task
  and this limitation is not represented as a screenshot pass;
- B1.1 cross-platform run `35693342002` passed on macOS, Windows and Ubuntu;
- final B1 run
  [`35695073147`](https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/actions/runs/35695073147)
  passed on macOS 15, Windows and Ubuntu 22.04. It builds the full
  shared-I/O/preview/PDF shell and runs tests, Clippy, release, product identity
  and headless PDF export on all three systems.

## B1 acceptance result

- Studio and Lite are independent binaries and repositories: **PASS**.
- Lite remains independently buildable and was not modified: **PASS**.
- Studio reads Lite-supported data through the one shared implementation:
  **PASS**.
- UI state edits a non-UI Figure Document and consumes a replaceable preview
  adapter: **PASS**.
- headless export creates no window or GPU context: **PASS**.
- B2 schema/persistence work was not pulled forward: **PASS**.

## Next dependency

Proceed to **B2.1**: freeze the formal Figure Document schema boundary and write
the container/versioning ADR before adding persistence. The B1 in-memory wrapper
is migration input, not the final project format.
