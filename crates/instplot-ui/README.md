# instplot-ui

Reusable egui UI primitives and application-shell contracts for InstPlot products.

## State ownership

- `Branding` owns product name, version and optional build identity only.
- `FeatureSet` declares which workflows a product exposes; it contains no document state.
- `ToolWindowSpec` owns window sizing and platform presentation policy, not whether a tool is open.
- The consuming application owns open/closed state, selections, drafts, documents and undo history.
- Preview functions consume immutable display lists and never mutate the document.

## Events and services

Reusable shells emit `ShellEvent`. A product implements `AppServices::dispatch` to translate those events into its own application transactions. Shared components must not reach into a product's model directly.

## Dimensions

`InterfaceMetrics::STUDIO` is the single source for control height, text sizes and row/column gaps. `ToolWindowSpec` carries default and minimum sizes for each tool. Fullscreen and maximized hosts use embedded windows; ordinary hosts use movable, resizable native windows with the same content and size contract.

## Rendering boundary

The preview consumes `instplot_export::ResolvedDisplayList`. Layout, label parsing, autoscale and export decisions belong to lower-level crates and must not be repeated in UI widgets.
