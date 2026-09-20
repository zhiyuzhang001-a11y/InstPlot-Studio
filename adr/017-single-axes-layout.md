# ADR-017: Deterministic single-Axes layout

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Use a bounded layout loop with at most four passes:

1. propose the Axes rectangle;
2. locate and format ticks;
3. measure labels with the selected Parley/font route;
4. calculate decoration margins;
5. update the Axes rectangle;
6. stop when all margins differ by less than 0.05 pt.

Failure to stabilize returns the conservative final pass plus a
`NonConvergent` warning. Invalid axes or data return errors carrying the owning
node ID. All geometry is expressed in points and emitted once as the A2 Display
List.

## Consequences

- Backends cannot independently change ticks, margins, labels, marks, or clips.
- Text and tick bounds are structural test outputs.
- The A1 fixture stabilizes after two passes with no text clipping or tick-label
  collision.
- Further multi-Axes, constrained-layout, and shared-axis work must extend this
  compiler rather than add backend-specific layout.
