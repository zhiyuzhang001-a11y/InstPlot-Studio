# ADR-010: Raster backend and DPI rounding

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Use a direct tiny-skia adapter over the resolved Display List for PNG and the
future TIFF RGBA buffer. DPI changes only the final point-to-pixel transform:

```text
px = round-half-away-from-zero(pt × dpi / 72)
```

The adapter draws the selected glyph IDs as outlines for raster output; it never
shapes source text. White and transparent backgrounds are explicit options.
PNG stores RGBA, DPI text metadata, and physical pixels-per-metre.

## Evidence and alternatives

300/600/1200 dpi dimensions, alpha, clip, paint, paths, text, images, and PNG
metadata pass locally. SVG → usvg/resvg produces a close comparison rendering
and remains a regression oracle/fallback, but is not selected as the primary
route because it adds serialization/parsing and can invoke an SVG text shaper.

TIFF will encode the same RGBA buffer; it must not create another renderer.
