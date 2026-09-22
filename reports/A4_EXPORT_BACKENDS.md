# A4 — PDF and raster backend validation with optional SVG research

Status: **backend architecture and ADR-020 typography closure locally complete**

Updated: 2026-09-21

## Reusable decision

One resolved Display List remains the only backend input. PDF uses Krilla and
raster uses direct tiny-skia mapping. The optional SVG research serializer also
consumes that Display List but is not a V1 or Gate A requirement under ADR-021.
Backends must consume the same source Unicode, selected font, glyph IDs, and
positions without independently shaping or laying out text.

The existing physical-page, path, clip, paint, metadata, image, DPI, and direct
raster architecture remains valid. The earlier structural evidence that PDF is
not a whole-page image remains required; SVG self-containment remains research evidence.

## Typography migration status

The backend prototype now embeds the four bundled TeX Gyre Heros faces and no
longer loads system fonts for V1 publication labels. Positive fixture text was
migrated to the V1-supported script set; CJK is retained only as an explicit
unsupported-script input fixture.

The Figure model and Display List now carry the semantic Label AST. A4 shapes
each span once, selecting its real upright/italic/bold/bold-italic face, scale,
and baseline shift. U+2080/U+207B presentation characters are no longer required.
The U+202F `UnitSeparator` uses deterministic 0.2 em positioning while retaining
its source range for searchable/copyable output.

The bundled-font diagnostic, PDF embedding/subsetting and extraction checks,
direct raster comparison, and missing-glyph diagnostic all pass locally. SVG
structural parsing also passes as a non-blocking experiment. No successful
publication run resolves through a system font.

## Reusable raster contract

The exact A1 round-half-away dimensions remain unchanged:

- 300 dpi: 1051 × 768 px;
- 600 dpi: 2102 × 1535 px;
- 1200 dpi: 4205 × 3071 px.

Normal, transparent, grayscale, and deuteranopia routes pass against the newly
reviewed TeX Gyre Heros baselines.

## Gate evidence disposition

- Native Windows automated and manual checks, including Edge PDF viewing, are
  complete.
- Linux is deferred by the product owner and is not represented as passed.

## Verification

```sh
cargo fmt --manifest-path prototypes/export-backend-spike/Cargo.toml --check
cargo test --manifest-path prototypes/export-backend-spike/Cargo.toml \
  --locked --all-features --tests
```

## Sources

- Krilla documentation: <https://docs.rs/krilla/0.8.2/krilla/>
- Krilla repository: <https://github.com/LaurenzV/krilla>
- usvg documentation: <https://docs.rs/usvg/0.48.1/usvg/>
- resvg rendering API: <https://docs.rs/resvg/0.48.1/resvg/>
