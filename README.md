# SciPlot

SciPlot is an independent Rust application for publication-quality scientific figures.
It is designed to cooperate with InstPlot Lite without becoming a mode, crate, or
workspace member inside the Lite repository.

## Repository boundary

- This repository owns the SciPlot application, figure document, layout compiler,
  display list, publication backends, visual benchmarks, and project format.
- InstPlot Lite remains an independently buildable and releasable application.
- During Part A, SciPlot must not use path dependencies into the Lite checkout.
- Shared data code is extracted only after Gate A, with an explicit compatibility
  contract and regression tests for Lite.
- Generated artifacts and machine-local paths are never committed as dependencies.

## Current phase

Part A:

1. A0 — local InstPlot Lite baseline captured; cross-platform evidence remains open;
2. A1 — fixed publication fixture and acceptance contract frozen and validated;
3. A2 — the next implementation phase: a point-based Figure IR and
   backend-neutral Display List spike.

Validate the A1 contract with:

```sh
python3 scripts/validate_a1.py
```

The authoritative plan is in
[`docs/SCIPLOT_EXECUTION_PLAN.md`](docs/SCIPLOT_EXECUTION_PLAN.md).
