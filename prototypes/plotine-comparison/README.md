# A5 Plotine comparison

This independent crate renders the A1 publication fixture with Plotine 0.5.2.
It is evidence for ADR-012, not a production dependency.

Upstream references checked on 2026-09-20:

- [Plotine repository](https://github.com/AIGO3fz/plotine)
- [Plotine 0.5.2 API documentation](https://docs.rs/plotine/0.5.2/plotine/)
- [Plotine PDF backend documentation](https://docs.rs/plotine-backend-pdf/0.5.2/plotine_backend_pdf/)

## Reproduce

```sh
cargo fmt --check
cargo test --locked
cargo run --release --locked
cargo tree --locked --edges normal
```

Generated PDF, SVG, and 300 dpi PNG files are written to the ignored
`artifacts/` directory. The lockfile preserves the dependency evidence.

## Results

| Check | Observed result | SciPlot consequence |
| --- | --- | --- |
| Version and license | Plotine 0.5.2, MIT, Rust 1.85 | Acceptable in isolation; pinning would require an exact version and lockfile. |
| 89 mm × 65 mm | `Figure::size` accepts inches, but every backend first rounds to integer pixels. At 72 dpi the PDF/SVG are 252 × 184 pt instead of 252.28346 × 184.25197 pt. | Both errors exceed the A1 0.01 pt tolerance. |
| Raster size | A separately constructed 300 dpi figure produces 1051 × 768 px. | Pixel dimensions are correct, but changing DPI rebuilds and relays out the figure. |
| Shared layout | `render_svg`, `render_pdf`, and `render_png` each create a renderer and call `Figure::draw`; vector and 300 dpi raster therefore use different pixel coordinate systems. | Does not satisfy the single resolved, point-space Display List contract. |
| Axis/tick control | Explicit ranges, major tick locations/formatters, minor ticks, scale, grid, and spines are exposed. | Useful API and test ideas, but not a reason to replace the Figure IR. |
| Marker/error bar/clipping | Circle/square markers, error bars, cap size, line styles, and axes clip paths work. | Functionally useful; the A1 plot was straightforward to express. |
| Text shaping | Plotine uses cosmic-text 0.14.2 and an embedded DejaVu family. SVG keeps real text and may use viewer fallback. | Conflicts with A3's selected Parley/fontique/HarfRust/skrifa route and pinned TeX Gyre Heros metrics. |
| Italic/upright and mathtext | Built-in mathtext renders italic `μ`/`H`, upright `DL`, and a subscript as separate text runs. | Capable, but it is a second semantic/text-layout system beside SciPlot's Label AST. |
| PDF font embedding | PDF contains a subset `MIOATK+DejaVuSans`, `/FontFile2`, and `/ToUnicode`. | Embedding exists, but the selected V1 font identity and metrics are not exposed or preserved. |
| PDF text extraction | Text is technically extractable, but `Experiment A` becomes `E x p e r i m e n t   A`. | Fails the A1 search/copy contract. |
| SVG/PDF/PNG parity | Math xlabel placement is below the 184-unit canvas and is clipped; the legend covers data at this compact size. | Publication layout needs InstPlot Studio-owned constraints and one backend-neutral layout. |
| Customization | High-level axes and artist APIs are broad. Font identity, positioned glyphs, physical vector page size, and resolved display-list reuse are not exposed at the required boundary. | An adapter would either leak Plotine's model or duplicate layout/shaping work. |
| Dependency surface | 221 normal dependency-tree lines, 142 unique lines in this locked comparison; duplicate fontdb, rustybuzz, skrifa, font-types/read-fonts, kurbo, GIF, and PNG generations are present. | Material overlap and version skew with the selected A3/A4 stack. |
| Binary and runtime | Release executable: 10,587,840 bytes; stripped: 9,174,680 bytes. Clean release build: 31.29 s. Cached generation: 0.04 s. Peak resident set: about 25.5 MB. | Runtime is fine, but the stripped comparison is about 3.72 MB larger than the A4 spike and leaves little room under the 10 MiB target. |

The PDF was also rasterized through macOS Quick Look for visual inspection. The
the clipped x label is visible in that independent viewer.

## Decision summary

ADR-012 selects option 3: study or independently reimplement useful algorithms
and tests, but do not add Plotine or its low-level crates as production
dependencies. The useful material is primarily tick policy, mathtext test cases,
legend-placement tests, and chart recipe coverage. Any adopted idea must enter
through SciPlot's own Figure IR, Label AST, point-space layout, and Display List.
