# ADR-006: Font fallback and project reproducibility

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Each shaped run records source Unicode, resolved full and PostScript names,
version string, collection face index, embedding permission, subsetting support,
font size, glyph IDs, positions, and advances. A project using system fallback
also records that resolved identity in its validation metadata.

Opening or exporting on another host re-resolves fallback and compares the
result with the recorded identity. A change produces a visible reproducibility
warning and a fresh layout; a missing glyph is always a warning. If the resolved
font cannot be embedded, vector export must stop with a clear remedy instead of
silently rasterizing or outlining text.

## Consequences

- Bundled Latin/Greek remains deterministic; system CJK is transparent but not
  falsely claimed to be identical across machines.
- Users can install or explicitly provide an embeddable CJK font to stabilize a
  project.
- Font bytes and machine-local paths are not persisted in Figure IR by default.

## Rejected alternatives

- Silent fallback: output can change without an auditable cause.
- Persist only a family name: it does not identify a face or its rights.
- Store CJK outlines in the document: loses text semantics and bloats projects.
