# InstPlot Studio post-feature consolidation — final report

> Status: `PASS`  
> Date: 2026-09-28  
> Branch: `codex/dual-axes`  
> Baseline HEAD: `6aa97c3`  
> Remote action: none; no commit, push, tag or Release was created.

## Outcome

The post-feature consolidation plan is complete locally. Existing product behavior remains the acceptance baseline; this work reorganized responsibilities and tests without changing project schema, exported library API or product defaults.

Final module boundaries:

- `app_controller/`: lifecycle, data, export, axes, artists, tools, selection and interaction coordination;
- `app_ui/`: shell, central canvas view, pure canvas interaction decisions and secondary-axis placeholders;
- `document/axes.rs`: authoritative axis identity and bound-object visibility rules;
- `instplot-layout/layout/`: axes, legend, series and object geometry behind the unchanged public layout facade;
- `instplot-export/resolve/`: visible-ink bounds and text shaping behind the unchanged public resolver facade;
- Studio and document tests grouped by workflow/domain without dropping baseline tests.

## Automated gates

Passed:

- `cargo fmt --all -- --check`;
- workspace check with all targets/features;
- workspace test with all targets/features;
- workspace doc tests;
- strict `cargo clippy --locked --all-targets -- -D warnings`;
- `cargo check --locked -p instplot-demo`;
- `git diff --check`;
- repository hygiene;
- B5 Studio-side validation: 19/19 PASS, including release build, handoff, import, project, publication, compatibility and source-integrity checks.

Public API verification: all four Phase 0 `lib.rs` snapshots are byte-for-byte unchanged. The independent QA also compared baseline/current function bodies and found no missing baseline Studio tests: 267 baseline tests remain, plus two new domain tests.

## Independent functional QA

Independent agent result:

- P0: 0;
- P1: 0;
- P2 defects: none identified;
- verdict: PASS.

It confirmed unchanged transaction/state behavior, unchanged stable viewport/object window identities, one resolved geometry pipeline for PDF/SVG/PNG, no lost baseline tests and no public API drift.

## Installed application and real UI

Installed and verified the unique Spotlight application:

- path: `$HOME/Applications/InstPlot Studio.app`;
- version: `0.1.0`;
- build: `202609281706`;
- bundle ID: `com.instplot.studio`;
- architecture: arm64;
- ad-hoc signature: valid;
- Spotlight matches: exactly one.

Current-build real UI checks passed:

- ordinary-window startup and CSV import with automatic X/Y plotting;
- maximized layout and embedded palette window;
- repeated palette command keeps the window open and raises it;
- fullscreen layout and embedded publication-check window;
- dual-Y empty-axis placeholder;
- binding the imported curve to Y2 updates the real secondary axis;
- returning to single-axis mode hides the dormant Y2-bound curve and restores symmetric single-axis canvas geometry.

The temporary UI project was intentionally not saved and the test process was closed after verification.

## Recovery and remaining decision

The external recovery package remains at:

`$HOME/Downloads/InstPlot-Consolidation-Recovery-20260928-162441`

It contains the recorded baseline, verified tracked patch, untracked archive/hashes, public API snapshots, serde contracts and window identities. The current work remains uncommitted so the user can inspect the installed application before deciding how to commit or push it.
