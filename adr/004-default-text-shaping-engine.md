# ADR-004: Default text shaping engine

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Use Parley + fontique + HarfRust + skrifa as SciPlot's only default text shaping
route. The layout compiler shapes source Unicode once into resolved glyph runs.
Preview, PDF, SVG, and raster backends consume the same glyph IDs, positions,
font identity, and source text and must not shape independently.

## Evidence

Parley 0.11.1 passed the A3 corpus for styled Latin, Greek, semantic subscript and
superscript spans, system CJK fallback, and explicit missing glyph reporting. A
repeat run is identical on the current host. With a pinned bundled face, Parley
and cosmic-text returned identical glyph IDs, X positions, and advances.

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
