# ADR-015: UI size, startup, and idle-memory baseline

Status: Accepted as local baseline; three-platform evidence pending A8

Date: 2026-09-20

## Decision

Keep the A6 release profile at size optimization, LTO, one codegen unit, aborting
panic, and stripped symbols. Retain the 10 MiB architecture target, 12 MiB soft
ceiling, and mandatory review above 15 MiB.

The local macOS baseline is:

- stripped UI shell: 3,745,344 bytes;
- UI shell plus selected A3/A4 publication stack: 5,719,984 bytes;
- integrated dependency delta: 1,974,640 bytes;
- warm process-to-first-canvas: 111 ms;
- idle resident memory after about 51 seconds: 115,680 KiB;
- mandatory runtime assets outside the executable: none.

## Consequences

- The integrated prototype is below the architecture target and does not trigger
  a size review.
- Startup and memory numbers are baselines, not budgets inferred from one host.
- A8 must repeat the same executable/assets boundary and measurement procedure on
  macOS, Windows, and Linux before Gate A.
- Any new GUI renderer or runtime asset must report its incremental cost against
  this baseline.
