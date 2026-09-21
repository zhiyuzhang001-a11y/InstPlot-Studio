# ADR-004: Default text shaping engine

Status: Accepted for Part A

Date: 2026-09-20

> The shaping-engine decision remains active. Its Source Sans/CJK corpus was
> historical Part A evidence and must be rerun against ADR-020.

## Decision

Use Parley + fontique + HarfRust + skrifa as InstPlot Studio's only default text shaping
route. The layout compiler shapes source Unicode once into resolved glyph runs.
Preview, PDF, SVG, and raster backends consume the same glyph IDs, positions,
font identity, and source text and must not shape independently.

## Evidence

Parley 0.11.1 passed the original comparison corpus and remains selected. The
ADR-020 rerun now covers the four bundled TeX Gyre Heros faces, semantic style
selection, U+00B5 normalization, explicit missing glyphs, and pre-shaping
rejection of unsupported CJK without system fallback.

The stripped comparison probe was 2,648,824 bytes, only 26,688 bytes larger than
the cosmic-text probe. The selected stack is MIT-compatible: Parley, fontique,
and skrifa are MIT/Apache-2.0, while HarfRust is MIT.

## Consequences

- Display List GlyphRun must carry resolved glyphs and source Unicode.
- Backend adapters may transform coordinates but cannot redo line breaking,
  fallback, or shaping.
- A4 must prove real PDF text extraction before this decision clears Gate A.

## Rejected alternative and exit strategy

cosmic-text 0.19.0 also passed the local corpus and was slightly smaller/faster,
but its route is less aligned with the selected fontique-based fallback and uses
an older HarfRust/skrifa line in this comparison. The retained probe is the exit
path: if Parley breaks a required contract, rerun the same AST corpus and replace
the shaping adapter without changing Figure IR or backend contracts.
