# B5P P0 — Real workflow baseline

Date: 2026-09-22  
Status: **DONE**  
Scope: repeatable evidence and prioritized gaps only; no product behavior was changed

## Outcome

P0 now has four reproducible valid cases and one deliberate rejection case:

- one ordinary source;
- one source with an explicitly linked fit and both equation forms;
- two independent sources in one partitioned file;
- one source with a disabled outlier row;
- one source containing a missing numeric value.

The baseline generator writes an embedded `.instplot` project, a versioned handoff package, a
font-embedded vector PDF and a 300 dpi PNG for every valid case. Generated evidence lives below
`target/b5p-p0-validation/` and is reproducible with:

```sh
python3 scripts/validate_b5p_p0.py
```

The validation does not treat file existence as success. It checks dataset/source/artist/row counts,
the alive bitmap, disabled-row exclusion from autoscale, embedded project storage, canonical handoff
checksums, project reopening and layout, Publication Check, PDF signatures and bundled font records,
PNG dimensions, source immutability, workspace tests and Clippy.

## Current workflow findings

### Blocking normal use

1. **Open Data does not construct the imported figure.** The GUI imports datasets and calls
   `sync_external_datasets`, which only updates data-source records; the pre-existing fixture artists
   remain. An ordinary user therefore cannot turn a newly opened CSV into the displayed plot.
   Evidence: `apps/instplot-studio/src/main.rs` in `open_data`, and
   `apps/instplot-studio/src/document.rs` in `sync_external_datasets`.
2. **There is no X/Y binding or series-creation UI.** The Inspector exposes only four axis-range
   values. Line/scatter/error-bar structures exist in the document but cannot be created or bound
   through the product UI. Evidence: `StudioApp::inspector` and `StudioApp::series_tree`.
3. **There is no edit transaction, Undo/Redo, dirty state or unsaved-change guard.** Any additional
   editor controls would be unsafe to scale until P1 provides this foundation.
4. **Missing numeric values cannot enter a Figure Document.** Import accepts the blank cell as a
   non-finite value, but figure creation rejects the whole dataset with the generic message
   “inconsistent columns or alive state”. The message does not identify the row, column or actual
   non-finite cause.

### High-friction workflow problems

1. Opening a project replaces the document but does not replace or clear `StudioSession` datasets,
   so the Data panel can be empty or can describe data from the previous workspace.
2. Partitioned source and fit datasets currently show the same filename in the legend instead of the
   section names. Multi-source datasets are therefore visually ambiguous even though stable IDs are
   correct.
3. Legend placement is fixed. Long labels in the source+fit, multi-source and disabled-row baselines
   extend beyond the figure and are clipped in both rendered PDF and extracted text.
4. The formal hit map is not connected to canvas pointer input. Selection works only from the series
   tree, with no on-canvas selection feedback, fit-to-window or overflow scrolling.
5. The top bar is a flat row of buttons. The only implemented document shortcut is Command/Ctrl+O,
   and it always means Open Data rather than distinguishing data from projects.
6. PNG export is fixed at 300 dpi in the GUI. Figure size, background and raster choices are stored
   in the project model but do not have a complete product workflow.

### Visible polish issues

1. Series labels expose generic kind plus stable IDs rather than concise user-facing names.
2. The tree heading still says “Fixed publication figure” for imported figures.
3. The preview displays framebuffer diagnostics as permanent canvas text.
4. Publication findings and runtime warnings are shown in separate places and cannot select the
   affected object or navigate to a corrective control.

## Baseline facts

- All four valid cases create schema-v1 projects with embedded data and reopen successfully.
- The disabled outlier remains in the stored values with `alive=false`, is not drawn and does not
  influence the automatic Y range.
- Every PNG is 1051 × 768 px, matching 89 mm × 65 mm at 300 dpi after rounding.
- Every PDF is one page at approximately 252.3 × 184.3 pt and embeds subsetted
  TeX Gyre Heros Regular and Italic fonts.
- The source+fit case preserves Parent-ID, Source-X/Y, precise equation and display equation.
- Visual rendering confirms the axes and plotted points are sharp and deterministic; it also confirms
  the long-legend clipping described above.
- The missing-value rejection is deterministic but not yet actionable enough for a user.

## P0 decision

The first implementation target remains **P1: typed document edit transactions, Undo/Redo, dirty
state, save points and schema evolution support**. P3 will resolve the ordinary Open Data plotting
blocker, but it must be built on P1 so failed or partial edits cannot corrupt the document or evade
unsaved-change protection.

Lite producer/launcher work and B6 packaging remain deferred. P0 found no reason to change the
validated typography, layout compiler, single Display List or export backend architecture.
