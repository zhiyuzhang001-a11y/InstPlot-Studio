# ADR-019: Preview overlay and document-edit boundary

Status: Accepted for Part A

Date: 2026-09-20

## Decision

The layout compiler emits a hit map beside the Display List. Every selectable
entry records stable node ID, bounds, optional path-proximity points, z-order,
role, data index, and tooltip mapping. Preview hit testing consumes this map and
must not infer selection geometry by reparsing display commands.

Hover, selection, handles, and zoom are view overlays. A drag becomes a document
edit only after an interaction command is committed against a stable node ID;
the document is then recompiled to a new Display List and hit map.

## Consequences

- Preview interaction does not become another source of publication geometry.
- Z-order and path proximity make overlapping selections deterministic.
- Backends ignore the hit map, while GUI overlays ignore export serialization.
