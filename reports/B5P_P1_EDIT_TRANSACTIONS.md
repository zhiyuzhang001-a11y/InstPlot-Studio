# B5P P1 Edit Transactions Report

Date: 2026-09-22  
Status: PASS

## Implemented

- Added typed `EditCommand` and `EditGroup` APIs for Figure Document property edits.
- Every edit now runs against a cloned candidate, then passes project validation and formal figure
  layout before commit.
- Added bounded Undo/Redo history with descriptions and coalescing for continuous numeric edits.
- Added a saved snapshot, dirty detection, save-point updates and history reset/rebase behavior.
- Wired the existing axes-range controls to the transaction layer; invalid values leave both the
  current document and history unchanged.
- Added Command/Ctrl+Z, Command/Ctrl+Shift+Z and Command/Ctrl+S handling plus enabled/disabled
  Undo and Redo controls.
- Added unsaved markers in the application heading and native window title.
- Added Save / Discard / Cancel protection before replacing the current project from Lite, opening
  another project or closing the application.
- Data import is an atomic project-and-session lifecycle operation. It rebases property history so
  Undo can never restore only half of the UI state.

## Schema decision

Undo history and save points are runtime workspace state, not publication facts. P1 therefore does
not add stored project fields and does not increment schema version 1. Existing schema-0 migration,
schema-1 compatibility, unknown-field rejection, backup recovery and resolved-figure round trips all
remain covered by the existing regression suite.

## Verification

- `cargo test --workspace --all-targets`: PASS
- `cargo clippy --locked --all-targets -- -D warnings`: PASS
- Editing regression tests: 5 PASS
- Manual macOS launch: unsaved title marker, disabled empty Undo/Redo controls and close-protection
  dialog visually confirmed.

## UI language disposition

The current interface text is English because the prototype placed literal strings directly in UI
code and its publication-font setup is not a localization system. P2 will introduce a centralized UI
text layer and make an explicit Chinese/English interface decision. This is independent from the
rule that CJK is unsupported inside V1 publication-figure labels.
