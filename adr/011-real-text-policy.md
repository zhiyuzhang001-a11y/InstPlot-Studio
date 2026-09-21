# ADR-011: Real text and optional outline policy

Status: Accepted for PDF; SVG portion superseded by ADR-021

Date: 2026-09-20

## Decision

PDF preserves real Unicode text by default, embeds/subsets the actual font, and
carries cluster-to-Unicode mapping. Raster formats naturally render glyph
outlines into pixels but do not change the Figure Document or vector-export
policy. ADR-021 makes SVG optional and removes its live-text behavior from V1
acceptance requirements.

An explicit future compatibility export may outline selected text only when the
user requests it and receives a search/editability warning. It must not become a
silent fallback for missing glyphs or forbidden font embedding, and source text
must remain in document/project metadata.

## Consequences

- PDF search/copy is an acceptance test, not optional polish.
- A font that cannot legally or technically embed blocks vector export with a
  clear remedy.
- Backend convenience text APIs that reshape strings are prohibited.
