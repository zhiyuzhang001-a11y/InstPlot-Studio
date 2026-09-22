# B1.1 — Independent Studio application bootstrap

Date: 2026-09-22  
Status: **DONE locally; cross-platform CI started by the implementation push**

## Boundary established

- The Studio repository now has its own Cargo workspace.
- `apps/instplot-studio` is an independent package and binary named
  `instplot-studio`; its visible product name is **InstPlot Studio**.
- The application consumes `instplot-core` from the Lite Git repository pinned
  to exact revision `80ad374044d91dd3c306a384fc7f07ba82cac429`.
- `StudioSession` owns shared-core `DataSet` values without copying or redefining
  the data model.
- The minimal eframe window uses the accepted A6 framework and release profile.
  It does not copy Lite `app.rs` and contains no Figure Document schema, data
  importer, publishing backend or Lite UI.
- Part A prototype packages remain explicitly outside the production workspace,
  so their frozen manifests and lockfiles remain independently reproducible.

## Verification

The following commands pass on the local macOS host:

```text
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
cargo build --release --locked --package instplot-studio
target/release/instplot-studio --product-info
```

Results:

- 2 unit tests passed;
- Clippy passed with warnings denied;
- product output is exactly
  `InstPlot Studio<TAB>instplot-studio<TAB>0.1.0` without creating a window;
- the release GUI process started successfully in a bounded local smoke run;
- stripped release executable: **3,661,888 bytes**;
- no mandatory runtime asset was added;
- the shared dependency is a non-local, exact Git revision;
- `cargo audit` found no known vulnerability in 298 locked dependencies;
- every locked package declares license metadata, and `otool -L` reports only
  the expected macOS system frameworks and libraries;
- the complete existing 21-check Part A local validation still passes, proving
  that the new root workspace does not disturb the frozen prototype packages.

The first compile exposed an eframe API mismatch in the initial shell draft:
eframe 0.36 uses `App::ui(&mut Ui, ...)`, matching the accepted A6 prototype.
The implementation was corrected to that already validated interface before the
passing verification run.

## Next dependency

Proceed to **B1.2**: add the shared `instplot-io` dependency and a small import
service that can open one Lite-supported data file into `StudioSession`. Keep
native file-dialog state in the application shell and do not start B2's formal
project format.
