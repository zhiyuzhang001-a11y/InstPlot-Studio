# A4 — PDF, SVG, and raster backend validation

Status: **backend routes selected; local structural and renderer evidence complete**

Date: 2026-09-20

## Decision

One resolved Display List is the only backend input. Plain A2 text placeholders
are resolved once by the A3 Parley route into font identity, source Unicode,
cluster byte ranges, glyph IDs, offsets, and advances. No backend computes axes,
ticks, legends, fallback, or line layout.

- PDF: Krilla 0.8.2 low-level positioned glyph API.
- SVG: SciPlot-owned serializer over resolved items, keeping real text and exact
  cluster origins while recording glyph IDs and fallback identity.
- Raster: direct tiny-skia mapping, including glyph outlines from the already
  selected glyph IDs. SVG → usvg/resvg remains a comparison oracle and fallback.

## PDF evidence

The PDF adapter maps page size, path fill/stroke, dash/cap/join, rectangular clip
scopes, rotation, positioned glyph runs, metadata, RGBA images, and transparency.
Krilla subsets and embeds the actual fonts; text is not converted to outlines.

The fixed artifact is 8,040 bytes. `pdfinfo` reports one PDF 1.7 page at
252.283 × 184.252 pt, matching 89 × 65 mm. lopdf structural tests find multiple
font objects and embedded font streams and find no whole-page image. Pure-Rust
text extraction preserves Latin, Greek, subscript/superscript Unicode, Chinese,
and legend text. Extraction tools may insert whitespace at fallback-run
boundaries; the characters remain searchable and copyable.

Poppler and macOS Quick Look both rendered the PDF correctly without cropping.
Illustrator/Inkscape import sampling remains a manual A8 matrix item because
neither application is installed on this host.

## SVG evidence

The serializer emits point-valued physical width and height, a matching point
viewBox, paths, fill/stroke properties, dash/cap/join, clipPath scopes, metadata,
rotation, and data-URI RGBA images. It embeds the pinned Source Sans OTF as a
data URI and records the resolved system CJK face/version without a temporary
external file.

Each text cluster has its resolved absolute origin and glyph ID metadata while
the element content remains the original Unicode. The fixed SVG is 450,344
bytes, dominated by the embedded deterministic font. roxmltree and usvg 0.48.1
both parse it, and resvg rasterizes it without clipping.

## Raster evidence

The direct tiny-skia path maps Display List geometry, clip scopes, alpha, dash,
cap/join, rotated text, resolved glyph outlines, and RGBA image resources. It
does not perform shaping. The comparison route serializes the same list to SVG
and rasterizes with usvg/resvg; a 300 dpi channel-difference guard keeps the two
implementations within the declared prototype tolerance.

Both white and transparent backgrounds pass. PNG output is RGBA, contains
software and DPI text metadata plus physical pixels-per-metre, and has the exact
A1 round-half-away dimensions:

- 300 dpi: 1051 × 768 px;
- 600 dpi: 2102 × 1535 px;
- 1200 dpi: 4205 × 3071 px.

The same `RasterImage { width, height, rgba, dpi }` buffer can feed a future TIFF
encoder without changing layout or rasterization.

## Size and runtime

On the current Apple Silicon macOS host:

- clean release build observed during the spike: 29.62 s;
- cached generation of PDF, SVG, four PNGs: 0.14 s;
- generator binary before strip: 6,093,424 bytes;
- stripped generator binary: 5,455,736 bytes;
- mandatory bundled font/license assets remain 922,053 bytes and are already
  represented in this standalone binary measurement.

The full A3+A4 validation executable remains below the 10 MiB architecture
target. Test-only PDF parsing/extraction crates are dev-dependencies and are not
part of the release binary.

## Verification

```sh
cargo fmt --manifest-path prototypes/studio-render-spike/Cargo.toml --check
cargo test --manifest-path prototypes/studio-render-spike/Cargo.toml --locked
cargo fmt --manifest-path prototypes/export-backend-spike/Cargo.toml --check
cargo test --manifest-path prototypes/export-backend-spike/Cargo.toml --locked --all-features --tests
cargo run --release --locked --manifest-path prototypes/export-backend-spike/Cargo.toml \
  --bin generate_fixture -- prototypes/export-backend-spike/artifacts
pdfinfo prototypes/export-backend-spike/artifacts/a4-fixture.pdf
```

## Remaining gate evidence

A4 is locally complete. A8 still owns three-platform execution, additional PDF
viewer/editor sampling, and committed regression thresholds. If any backend
needs to reshape text or recalculate layout, ADR-002 and ADR-004 must be reopened.

## Sources

- Krilla documentation: <https://docs.rs/krilla/0.8.2/krilla/>
- Krilla repository and examples: <https://github.com/LaurenzV/krilla>
- usvg documentation: <https://docs.rs/usvg/0.48.1/usvg/>
- resvg rendering API: <https://docs.rs/resvg/0.48.1/resvg/>
