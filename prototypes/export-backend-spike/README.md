# A4 export backend spike

This independent crate resolves the A2 Display List text through the A3 Parley
route once, then feeds the same positioned glyph and geometry contract to:

- Krilla PDF;
- a direct, physical-size SVG serializer;
- direct tiny-skia raster output;
- an SVG → usvg/resvg comparison raster route.

Run the structural and visual-contract tests:

```sh
cargo fmt --check
cargo test --locked --all-features --tests
```

Generate local inspection artifacts:

```sh
cargo run --release --locked --bin generate_fixture -- artifacts
```

The generated directory is ignored. It contains PDF, SVG, white-background PNG
at 300/600/1200 dpi, and a transparent 300 dpi PNG.

Run the repository visual regression:

```sh
cargo run --release --locked --bin visual_regression
```

Expected PNGs live in `tests/visual-baselines/`; actual and amplified diff PNGs
plus machine-readable metrics are written to `../../target/visual-regression/`.
The matrix covers normal output at 300/600/1200 dpi plus grayscale and
deuteranopia references at 300 dpi. Baselines are updated only by an intentional
reviewed run with `--update`.
