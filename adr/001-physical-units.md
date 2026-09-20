# ADR-001: Physical units and conversion boundaries

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Document-facing figure sizes use `Mm`, layout and Display List values use `Pt`,
and raster targets use `Px`. The three units are distinct Rust newtypes. A core
layout API must not accept a bare number when its unit would be ambiguous.

The exact conversions are:

```text
pt = mm × 72 / 25.4
px = round-half-away-from-zero(pt × dpi / 72)
```

Figure-space uses a top-left origin, positive X to the right, and positive Y
downward. Coordinates may be negative when geometry lies outside a clip;
dimensions must be finite and non-negative.

## Consequences

- The A1 89 mm × 65 mm fixture maps to 252.28346 pt × 184.25197 pt.
- DPI affects only raster density, never layout.
- Backends receive point coordinates and cannot reinterpret physical size.
- A later affine screen transform may map points to logical pixels without
  modifying the Figure Document.

## Rejected alternatives

- Bare `f32` throughout: unit mistakes are invisible to the type system.
- Pixel-based layout: preview scale and export DPI would change geometry.
- Bottom-left core coordinates: common in PDF, but would make screen backends
  and top-down layout add repeated inversions. PDF must adapt at its boundary.
