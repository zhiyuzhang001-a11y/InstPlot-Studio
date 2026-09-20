# ADR-011: Real text and optional outline policy

Status: Accepted for Part A

Date: 2026-09-20

## Decision

PDF and SVG preserve real Unicode text by default. PDF embeds/subsets the actual
font and carries cluster-to-Unicode mapping. SVG contains Unicode text, pinned
font data for deterministic Latin/Greek, and resolved cluster origins. Raster
formats naturally render glyph outlines into pixels but do not change the Figure
Document or vector-export policy.

An explicit future compatibility export may outline selected text only when the
user requests it and receives a search/editability warning. It must not become a
silent fallback for missing glyphs or forbidden font embedding, and source text
must remain in document/project metadata.

## Consequences

- Search/copy and editability are acceptance tests, not optional polish.
- A font that cannot legally or technically embed blocks vector export with a
  clear remedy.
- Backend convenience text APIs that reshape strings are prohibited.
