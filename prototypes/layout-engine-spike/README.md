# A7 single-Axes layout spike

This crate turns a semantic single-Axes chart into the A2 point-space Display
List. It owns scale, tick, layout, marks, legend placement, warnings, and the hit
map; GUI and export backends do not make layout decisions.

## Reproduce

```sh
cargo fmt --check
cargo test --locked
cargo run --locked --example generate -- artifacts
```

The ignored `artifacts/` directory contains the deterministic layout and Display
List snapshots plus PDF, SVG, and 300 dpi PNG rendered through the A4 backends.
It also contains a marker gallery that visually exercises every marker, every
dash style, line+marker composition, and two-sided X/Y error bars.

## Implemented contract

- linear and log10 transforms;
- automatic and fixed major locators plus linear/log minor locators;
- scalar formatting, shared powers of ten, precision from step, and negative-zero
  suppression;
- deterministic horizontal tick-label collision stride;
- a maximum four-pass layout loop: ticks, A3-compatible Parley measurement,
  decoration margins, axes update, stable result or a node-locatable warning;
- line, scatter, line+marker, open/filled circle, square, triangle-up/down,
  diamond, plus, cross, X/Y error bars, reference lines, annotations, and legend
  keys;
- solid, dashed, dotted, and dash-dot strokes;
- four deterministic in-Axes legend candidates scored against coarse data/error
  occupancy, annotations, extrema, and edges, with an outside-Axes fallback;
- hit entries containing node ID, bounds, path proximity, z-order, selectable
  role, data index, and tooltip mapping; `HitMap::hit_test` selects without
  reparsing Display List geometry.

## A1 result

- exact page: 252.28346 × 184.25197 pt;
- stable after two layout iterations;
- Axes: x=39.398, y=12.000, width=115.213, height=129.710 pt;
- x and y major ticks: −2, 0, 2; eight minor ticks on each axis;
- the five-entry legend had no safe inside location and moved outside the Axes;
- 124 Display List items and 88 hit-map entries;
- no non-convergence, text-outside-page, or tick-overlap failure;
- PDF remains vector/searchable, SVG remains vector text, and PNG is 1051 × 768
  at 300 dpi through the already selected A4 backends.

The wide outside legend intentionally reduces plot width in this compact fixture.
That is a deterministic conservative fallback, not silent data occlusion. A later
product policy may choose a smaller legend font, multiple columns, or additional
figure width, but must do so through the same layout contract.

## Known Part A follow-ups

- The layout measurer uses the selected Parley engine and pinned Source Sans 3
  regular face. A7 source labels remain strings in the A2 placeholder GlyphRun;
  full Label AST span styles and positioned A3 glyph runs must replace that
  placeholder during production consolidation.
- CJK measurement/export still emits the previously recorded ICU4X complex-script
  model warning on this host. The system fallback renders correctly locally, but
  A8 must normalize the diagnostic and record all three platform font identities.
- A8 must add visual-diff thresholds and cross-platform normalized snapshots.
