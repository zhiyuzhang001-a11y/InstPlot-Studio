# ADR-018: Legend auto-placement

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Score four deterministic in-Axes corner candidates using coarse point, error-bar,
and annotation occupancy, a penalty for covering data extrema, and an edge term.
Accept the lowest candidate only below the fixed safety threshold. Otherwise,
reserve a right margin and place the legend outside the Axes with an explicit
warning.

Legend measurement and placement participate in the same bounded layout loop as
ticks and labels. Entry order follows semantic series order.

## Consequences

- A legend never silently covers a critical region merely to remain inside the
  Axes.
- The compact A1 fixture exercises and snapshots the outside fallback.
- Future draggable legend overrides are document intent; transient hover or
  selection state cannot rewrite automatic placement.
