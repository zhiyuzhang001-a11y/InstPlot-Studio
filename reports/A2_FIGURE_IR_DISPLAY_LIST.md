# A2 — minimal Figure IR and Display List spike

Status: **implemented; local verification complete**

Date: 2026-09-20

## Scope

The independent `prototypes/studio-render-spike` crate implements:

- `Mm`, `Pt`, and `Px` newtypes with the A1 conversion and rounding rules;
- Figure, Axes, Axis, Line, Scatter, ErrorBar, ReferenceLine, Text, and Legend;
- opaque stable node IDs propagated into output items;
- Path, Stroke, Fill, GlyphRun placeholder, Image, ClipPush, and ClipPop;
- a deterministic structural snapshot;
- a minimal SVG path backend that maps resolved items without recomputing layout;
- validation that reports malformed clip scopes/non-finite items without panic.

The crate has no dependency on egui, Krilla, a file format parser, or the Lite
checkout. It currently has no third-party dependencies.

## Evidence

```sh
cd prototypes/studio-render-spike
cargo fmt --check
cargo test --locked
```

The tests cover A1 physical conversion, repeat compilation, exact Display List
snapshot comparison, source-node mapping, clipping, Unicode text preservation,
and SVG mapping of the fixed axes and curve.

## ADRs

- ADR-001: physical units and conversion boundaries;
- ADR-002: Display List coordinate and primitive contract;
- ADR-003: stable node identity.

## Known limitations

- GlyphRun is a placeholder and contains source Unicode rather than shaped glyph
  IDs and positions; A3 owns that decision.
- Axis tick location/formatting and automatic margin calculation are not part of
  this minimal spike; A7 owns publication layout policy.
- The SVG mapper is validation-only and does not establish the production SVG
  dependency or text policy.
- Part A cross-platform evidence is resolved under the current gate policy:
  Windows validation is complete and Linux is deferred. Shared-core extraction
  is now governed by B0 of the Part B short plan.

## Next dependency

A3 must select and validate one shaping/font route shared by preview, PDF, and
SVG. If a backend later needs to recompute axes, ticks, or text positions, A2 must
be reopened because the Display List boundary would be invalid.
