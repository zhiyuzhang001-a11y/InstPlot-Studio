# ADR 023: Modular product boundaries

Status: Accepted
Date: 2026-09-25

## Context

InstPlot Studio accumulated application state, import orchestration, project persistence, text semantics, layout, rendering, export and native-window behavior in a small number of large modules. Reusing the working frontend or importer in another product would have required copying Studio code and its implicit state transitions.

## Decision

Use a directed workspace architecture:

```text
product app → application/document services → data + text
            → instplot-ui → resolved display list
            → instplot-export → instplot-render + instplot-text
            → instplot-layout → instplot-render + instplot-text
```

- Product binaries own branding, feature selection and live UI state.
- `ApplicationController` owns cross-layer transactions and commits complete outcomes only.
- `FigureDocument` is the authoritative editable graph; session caches are derived.
- Import and manual entry share `DataImporter` normalization.
- Preview and all export backends consume one immutable resolved display list.
- `instplot-ui` owns reusable design/window policy but never document or undo state.
- A new crate is justified only by a real consumer or an enforceable dependency boundary.

## Consequences

- InstPlot Quick serves as the second consumer proving these are real boundaries.
- Project schema and legacy public APIs remain compatible through narrow wrappers.
- Fullscreen/maximized tool-window behavior is centralized and testable.
- Workspace-wide formatting, Clippy and tests can block dependency or behavior regressions.
