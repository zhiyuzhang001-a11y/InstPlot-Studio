# ADR-012: Plotine reuse boundary

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Choose A5 option 3: only borrow or independently reimplement algorithms and test
ideas from Plotine; do not make Plotine or its low-level crates a SciPlot
production dependency.

The comparison is pinned to Plotine 0.5.2 in the independent A5 crate. It shows
valuable coverage for axes, ticks, markers, error bars, legends, chart recipes,
and built-in mathtext. Those ideas may inform SciPlot tests and policy code only
after license/provenance review. No source is to be copied merely because the
algorithm was evaluated.

## Reasons

- Plotine converts physical inches and DPI to integer pixel dimensions before
  every backend. Its 72 dpi PDF/SVG page is 252 × 184 pt rather than the exact
  89 mm × 65 mm page required by A1.
- Each backend invokes high-level figure drawing in its own pixel coordinate
  system. A 300 dpi export therefore recomputes layout instead of consuming the
  same resolved point-space Display List as PDF and SVG.
- Its cosmic-text/DejaVu path conflicts with A3's selected
  Parley/fontique/HarfRust/skrifa and Source Sans 3 route.
- The PDF embeds a subset font and has a Unicode map, but extraction inserts
  spaces between Latin glyphs and its embedded font misses the CJK probe.
- An adapter would duplicate or bypass SciPlot's semantic Figure IR, Label AST,
  stable node identity, layout constraints, and export diagnostics.
- The dependency graph duplicates several font and rendering generations. The
  stripped comparison executable is 9,174,680 bytes before the rest of the
  application is added.

## Benefits retained

- Use Plotine behavior as external comparison evidence for tick locators,
  formatters, legends, mathtext, and recipe coverage.
- Derive black-box test cases and expected policy behavior where appropriate.
- Re-evaluate individual algorithms only when SciPlot has a concrete gap and a
  clean integration boundary.

## Exit cost and version policy

There is no production removal cost because no Plotine dependency crosses into
SciPlot. The A5 prototype remains an isolated, locked comparison fixture and may
be deleted without changing application code or project files.

If a future ADR proposes a Plotine crate, it must name the exact crate and
version, demonstrate compatibility with the selected text and Display List
contracts, quantify binary impact in the full application, and define an
adapter-owned exit path. A semver range alone is insufficient evidence.

## Evidence

See `prototypes/plotine-comparison/README.md`, its locked dependency graph,
contract test, and generated local artifacts.
