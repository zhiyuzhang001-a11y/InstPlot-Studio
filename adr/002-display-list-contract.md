# ADR-002: Display List coordinate and primitive contract

Status: Accepted for Part A

Date: 2026-09-20

## Decision

The layout compiler emits one ordered, backend-neutral Display List in
figure-space points. Its minimum primitives are Path, Stroke, Fill, GlyphRun
placeholder, Image, ClipPush, and ClipPop. Every drawable item keeps its source
node ID.

Paths explicitly contain their verbs and fill rule. Strokes explicitly contain
color, width, cap, join, and dash sequence. Clips are scoped and paired. All
coordinates and widths must be finite.

Backends only map these resolved primitives into a target format. They do not
recalculate axes, scales, series geometry, labels, or legend positions. Invalid
items are reported and skipped rather than causing a panic.

## Consequences

- Preview, PDF, SVG, and raster backends can share one layout result.
- Deterministic debug serialization can compare geometry, ordering, clips, and
  node mapping without pixel rendering.
- The A2 SVG path backend is deliberately small and is validation evidence, not
  the selected production SVG implementation.

## Rejected alternatives

- Backend-specific layout: produces drift between preview and export.
- A screenshot or raster buffer as the interchange format: loses searchable
  text, editable paths, physical coordinates, and semantic provenance.
- Generic untyped command maps: defer invariant checking until each backend.
