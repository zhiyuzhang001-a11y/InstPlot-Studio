# Building another InstPlot product

This guide assembles a new product without copying Studio source.

## 1. Select dependencies

Use the narrowest formal crates needed:

- `instplot-ui`: branding, feature flags, design metrics, window policy and preview painting;
- `instplot-studio` library: current high-level importer, figure document, project and export services;
- `instplot-text`: semantic labels when building a specialized editor;
- `instplot-layout`, `instplot-render`, `instplot-export`: lower-level rendering products.

## 2. Define product identity and capabilities

```rust
use instplot_ui::{Branding, FeatureSet};

let branding = Branding::new("My Plot Tool", "0.1.0");
let features = FeatureSet {
    data_import: true,
    manual_data: false,
    project_files: false,
    annotations: false,
    publication_check: false,
    export_pdf: false,
    export_png: true,
    export_svg: true,
};
```

The product owns open windows, selections, drafts, documents and undo history. Shared widgets must receive values and emit events instead of reaching into that state.

## 3. Dispatch shell events

Implement `AppServices` and translate `ShellEvent` values into product-specific transactions. Mutating operations should prepare and validate a complete outcome before replacing live state.

## 4. Use the shared scientific pipeline

For a minimal import/export flow:

1. `DataImporter::read_file`;
2. `FigureDocument::from_datasets`;
3. `FigureDocument::rebind_series` for selected X/Y;
4. `FigureDocument::set_axis_label`;
5. `FigureDocument::refresh_autoscale`;
6. `save_figure_pdf`, `save_figure_png_with_background` or `save_figure_svg`.

`apps/instplot-demo` is an executable example of this exact flow with a different brand and feature combination.

## 5. Where to extend Studio itself

The Studio binary keeps product coordination separate from reusable scientific rules:

- `app_controller/lifecycle.rs`, `data_workflow.rs`, and `export_workflow.rs` coordinate complete user workflows;
- `app_controller/axis_editor.rs` and `artist_editor.rs` edit drafts and submit document commands;
- `app_controller/tool_windows.rs` owns tool-window lifecycle, while `instplot-ui::ToolWindowPolicy` owns the common normal/maximized/fullscreen policy;
- `app_ui/shell.rs` assembles the application chrome and tool windows;
- `app_ui/canvas_view.rs` owns the central canvas composition, and `app_ui/canvas_interaction.rs` contains pure pointer-event decisions;
- `document/axes.rs`, `series.rs`, `objects.rs`, and `datasets.rs` are the authoritative domain rules;
- `instplot-layout::layout/{axes,legend,series,objects}.rs` owns physical geometry;
- `instplot-export::resolve/bounds.rs` owns tight visible-ink bounds and `resolve/text.rs` owns text shaping.

Add a rule at the lowest applicable layer. UI modules should not duplicate autoscale, axis binding, object visibility, legend sample, or export-bound calculations.

## 6. Required verification

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets --no-fail-fast
cargo test --locked --workspace --doc
```

Do not introduce a product-to-product source dependency, duplicate label parsing in a UI, or render directly from a mutable document.
