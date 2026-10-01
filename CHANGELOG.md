# Changelog

All notable changes to InstPlot Studio are recorded here.

## [Unreleased]

No changes yet.

## [0.1.2-rc.2] - 2026-10-01

### Fixed

- Use Studio's existing dark interface on Windows, macOS and Linux regardless of the operating
  system theme. Panels, controls and text use the same dark palette; plot canvases remain white.

## [0.1.2-rc.1] - 2026-09-29

### Added

- A manually triggered in-app update window backed by the fixed InstPlot Studio OSS channel.
- Ed25519 verification with current and next embedded trust keys, strict update URL boundaries,
  signed manifest validation, platform-specific package selection and downgrade protection.
- Background package downloads with progress, cancellation, bounded size, SHA-256 verification,
  no-clobber saving and explicit manual-install handoff.
- GitHub OIDC authentication for short-lived Alibaba Cloud STS credentials and least-privilege
  publication under the dedicated `instplot-studio/` OSS prefix.
- Reproducible Windows, macOS and Linux technical-prerelease packaging and installation checks.

### Security and release infrastructure

- Public-root and update-key configuration now has one checked-in build-time source, verified
  against protected GitHub Environment variables before OSS publication.
- Signed metadata rejects duplicate JSON keys, invalid validity windows, untrusted release-note
  links, cross-origin redirects, stale sequences and mismatched key identifiers.
- Immutable OSS objects receive explicit content types and long-lived immutable caching; channel
  pointers use revalidation caching and are activated only after public verification.

### Compatibility and limitations

- This is an unsigned technical prerelease. The macOS package uses ad-hoc signing and is not
  notarized; the Windows installer has no Authenticode signature and may trigger SmartScreen.
- The application downloads and verifies an installer but never launches it or replaces the
  installed application automatically.

## [0.1.1] - 2026-09-29

### Added

- Dual X and dual Y axis workflows with explicit per-series bindings, independent autoscale,
  secondary-axis placeholders and per-axis visibility controls.
- Publication reference lines and measurement arrows with editable geometry, labels and styles.
- Automatic and manually incorporated shared `×10ⁿ` axis factors, including scale-aware range and
  tick-interval editing for primary and secondary axes.
- Additional publication palettes, dash patterns and compact object-editing workflows.
- A clickable product/version footer that opens the latest GitHub Release for update checks.

### Changed

- Autoscale now responds consistently to imports, manual-data edits, column changes, error bars,
  data removal and series movement between enabled axes.
- Export uses tight content geometry without changing the normal editing canvas.
- Data files, manual data, line/point/error components and legends retain one logical-series
  identity across editing, saving and reopening.

### Fixed

- Rebinding a series that was suspended on a disabled secondary axis now restores autoscale on its
  newly enabled axis.
- Multiple curves sharing an axis now use the combined range for one shared display exponent, while
  X1/X2 and Y1/Y2 determine their exponents independently.
- Removed the internal timestamp build identifier from the visible application footer.

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

[Unreleased]: https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/compare/v0.1.2-rc.1...HEAD
[0.1.2-rc.1]: https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/compare/v0.1.1...v0.1.2-rc.1
[0.1.1]: https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/tag/v0.1.0
