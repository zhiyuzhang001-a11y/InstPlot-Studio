# Changelog

All notable changes to InstPlot Studio are recorded here.

## [Unreleased]

No changes yet.

## [0.1.0] - 2026-09-27

### Added

- Publication-oriented figure editing with a fixed 85 × 65 mm main canvas.
- CSV, TSV, TXT, DAT, XLSX and XLS import through the shared InstPlot data layer.
- Automatic X/Y selection, multiple data sources, manual data entry, repeated measurements,
  sample standard deviation and standard error error bars.
- Independent XY cards for manual data entry, with X/Y repeated measurements, SD/SEM error bars,
  explicit edit mode and one synchronized sidebar source per plotted group.
- Managed manual-data persistence in CSV, TSV, TXT, DAT or XLSX alongside the embedded project
  recipe, plus a separate data-export command and external-file conflict choices.
- Scientific labels with Latin and Greek variables, mathematical symbols, scripts, units and
  consistent preview/PDF/PNG/SVG typography.
- Line, scatter and line-plus-point series; seven publication colors, point shapes and line styles;
  filled and hollow points; per-series and batch styling.
- Editable legends inside, above or to the right of the main plot, with draggable placement and
  row/column layout.
- Multiline annotations and configurable connector arrows.
- Versioned `.instplot` projects with embedded normalized data, atomic saves, backup recovery and
  schema migration.
- PDF, transparent or white-background PNG, and SVG export from one resolved display list.
- Publication checks, undo/redo, independent tool windows and macOS Spotlight installation.

### Changed

- Imported and manually entered curves share one ordering, palette, marker, legend, autoscale,
  deletion and project lifecycle while retaining explicit source identity.
- Managed data files use fingerprint checks and atomic replacement; imported source files remain
  read-only and deleting a project card does not delete its disk file.

### Architecture

- Split application transactions, data import, document editing, project persistence, scientific
  text, layout, rendering, export and reusable UI into explicit modules and workspace crates.
- Added `instplot-demo` as a second product proving reuse without copying Studio UI code.
- Promoted validated prototype implementations into formal `instplot-*` crates and retired their
  migrated spike directories; the layout spike remains only as a compatibility fixture.
- Added cross-platform CI, domain contracts, render/export contracts and realistic workflow QA.

### Compatibility and limitations

- Projects retain imported normalized data and reopen without their original source files.
- Historical project schemas are migrated and validated before commit.
- V1 does not support CJK scientific-label shaping, dual Y axes or inset axes.
- The local macOS installer uses ad-hoc signing; public distribution signing and notarization are
  separate release work.

[Unreleased]: https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/tag/v0.1.0
