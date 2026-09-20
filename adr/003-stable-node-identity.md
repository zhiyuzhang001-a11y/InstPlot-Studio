# ADR-003: Stable node identity

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Every Figure IR node owns an opaque `NodeId`. Compilation copies that identity
onto every Display List item derived from the node. A node may produce multiple
items, but an item has exactly one source node.

IDs are assigned by the document layer and are not derived from vector position,
display order, labels, or backend object numbers. Reordering artists therefore
does not silently change identity. Persistence and ID generation policy will be
defined with the formal Figure Document in Part B; A2 validates propagation only.

## Consequences

- Hit testing, warnings, inspection, and incremental invalidation can point back
  to semantic document nodes.
- Snapshot output exposes node mapping directly.
- Backends may retain node IDs as debug metadata but must not replace them.

## Rejected alternatives

- Array index identity: changes under insertion and reordering.
- Label-derived identity: labels are editable and not necessarily unique.
- Backend-generated identity: prevents cross-backend diagnostics and comparison.
