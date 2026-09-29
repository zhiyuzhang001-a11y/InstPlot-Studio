# InstPlot Studio post-feature consolidation — Phase 2–5 evidence

> Status: `COMPLETED`  
> Date: 2026-09-28  
> Scope: structural consolidation only; no product defaults, schema, public API, GitHub state or release state changed.

## Phase 2 — UI and window boundaries

- `app_ui.rs` is now a small module facade.
- `app_ui/shell.rs` assembles application chrome and tool windows.
- `app_ui/canvas_view.rs` owns the central canvas, zoom/scroll placement, preview painting and canvas overlays.
- `app_ui/canvas_interaction.rs` contains pure interaction decisions.
- `app_ui/axis_placeholders.rs` contains X2/Y2 placeholder geometry.
- Existing stable viewport IDs and object `window_key` values were not changed.
- Palette, publication, axis visibility, manual data and object editors continue to use `instplot_ui::ToolWindowPolicy` and `ToolWindowSpec`; system dialogs remain independent.

Targeted evidence:

- 5 `app_ui::tests` passed;
- 3 `instplot-ui::window::tests` passed;
- normal/maximized/fullscreen policy remains covered by the existing window tests.

## Phase 3 — domain rules

The remaining pure axis/object visibility rules were moved from the Studio UI controller into `document/axes.rs`:

- project ID → axis identity;
- reference-line value visibility on its bound axis;
- measurement endpoint visibility on its bound X/Y axes;
- finite, range and Log10-domain validation used by those decisions.

Direct tests cover axis IDs and label IDs, dual-Y binding, endpoint boundaries and invalid non-positive values on a logarithmic secondary axis. UI-only wording and editor widgets remain in the controller.

## Phase 4 — layout and export resolver

`instplot-layout/src/layout.rs` remains the public orchestration facade and public type location. Cohesive implementation is now in:

- `layout/axes.rs`;
- `layout/legend.rs`;
- `layout/series.rs`;
- `layout/objects.rs`.

`instplot-export/src/resolve.rs` remains the public resolver facade and public type location. Tight visible-ink bounds and text shaping are now in `resolve/bounds.rs` and `resolve/text.rs`.

No backend gained layout decisions. PDF, SVG and raster still consume the same resolved display list. The complete layout suite (including 24 publication-layout and 9 scale tests) and the complete export crate suite passed after the move.

## Phase 5 — tests and public API

Large test files were grouped without deleting tests:

- Studio application tests: `data_workflows`, `visual_contracts`, `interaction_contracts`;
- document tests: `axes_and_data`, `legend_and_export`, `series`, `objects`.

The Studio library test count is 182 (181 passed, one explicitly ignored visual artifact generator); the Studio binary test count is 87 passed. The four Phase 0 `lib.rs` public API snapshots are byte-for-byte unchanged, and `cargo check --locked -p instplot-demo` passes.

The extension guide now identifies the authoritative change location for workflows, UI, document rules, layout geometry and export resolution.

## Remaining gate

Phase 6 still requires the complete workspace checks, repository validators, independent functional QA, release build, Spotlight replacement and real ordinary/maximized/fullscreen UI verification.
