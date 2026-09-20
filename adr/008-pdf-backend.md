# ADR-008: PDF backend

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Use Krilla as the PDF backend. Feed `Surface::draw_glyphs` the exact glyph IDs,
cluster ranges, offsets, advances, font bytes, and source Unicode produced by the
single layout pass. Use Krilla paths, clips, paint, images, metadata, font
embedding, and subsetting; do not use its convenience shaping path.

## Evidence

The A4 artifact has the exact point MediaBox, vector geometry, clip/dash/stroke
operators, embedded font subsets, RGBA image support, and searchable mixed text.
Poppler and macOS Quick Look render it correctly. Krilla is MIT/Apache-2.0.

## Consequences and exit strategy

Krilla owns PDF serialization, not layout. A narrow adapter contains its API.
If it cannot satisfy a later structural requirement, another PDF writer can
consume the same resolved Display List without changing Figure IR or shaping.

Using a second PDF-specific shaper, whole-page raster PDF, or default text
outlines is rejected.
