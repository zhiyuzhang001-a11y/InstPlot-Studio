# instplot-export

Production PDF, SVG and raster backends for a resolved InstPlot display list.

All backends consume the same positioned glyphs and geometry. PDF uses embedded publication fonts, SVG preserves physical point dimensions, and PNG uses direct rasterization with explicit DPI and background policy.

```sh
cargo test --locked -p instplot-export --all-targets
```

The crate does not parse labels, compute data ranges or place legends. Those decisions must already be present in the resolved scene.
