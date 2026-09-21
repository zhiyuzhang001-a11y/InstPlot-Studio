# A3 — text, font, and semantic label validation

Status: **typography migration and backend integration locally complete**

Updated: 2026-09-21

## Decision

InstPlot Studio uses Parley + fontique + HarfRust + skrifa as its sole default
text-shaping route. V1 bundles TeX Gyre Heros 2.004 in four real OTF faces:
Regular, Italic, Bold, and Bold Italic. No system font discovery is used for
publication labels, and no separate Symbol font is required.

The semantic Label AST selects the face independently from character identity:
variables and variable subscripts are italic; ordinary text, numbers, units,
and descriptive subscripts are upright. U+00B5 legacy input is normalized to
U+03BC. Unsupported scripts, including CJK in V1, produce an explicit error
instead of fallback, outlines, or rasterized text.

## Bundled font audit

The files were copied from the locally supplied TeX Gyre distribution together
with its GUST Font License, upstream manifest, and README. The four fonts total
543,276 bytes; the complete bundled font directory including checksums is
579,168 bytes.

- Regular: `TeXGyreHeros-Regular`, 133,600 bytes, SHA-256
  `6ae1a09d5a940367b7aaaa91ee8bd8a2c333bfe193e7096e23f931357d62081f`.
- Italic: `TeXGyreHeros-Italic`, 139,208 bytes, SHA-256
  `6473df7fa107b3fb4be38973710afe22b0640c2ac076d5337cf126bed9aa108c`.
- Bold: `TeXGyreHeros-Bold`, 135,204 bytes, SHA-256
  `b170162835f4efc288886dd4231406dc47e19b614cf4416836635599d44a7d60`.
- Bold Italic: `TeXGyreHeros-BoldItalic`, 135,264 bytes, SHA-256
  `166fc6d068d9c9974281555cb3d730365537a9b676ab269bb5163f5a75496505`.
- GUST Font License: SHA-256
  `2bd69affc3da00715116f713f57eab9707e96daf3562ad0215987b15b9c16f73`.
- Upstream manifest: SHA-256
  `3263a067e409258be34027de883e618cc2c76c70135897835f65f3c569dec5d1`.
- Upstream README: SHA-256
  `cb41cbe4091a67a7a7b39df62553a7fc05f03e9014d7d1e148687967f3250188`.

All four faces pass the accepted Greek Core and Scientific Symbol Core coverage
checks and allow embedding/subsetting. The family does not map U+2080 SUBSCRIPT
ZERO or U+207B SUPERSCRIPT MINUS. That is not a Core coverage failure: the V1
contract requires semantic subscript/superscript layout from ordinary base
characters, rather than dependence on Unicode presentation glyphs.

## Local evidence

The migrated Parley prototype loads only the four bundled faces. Its positive
corpus covers upright/italic/bold/bold-italic selection, Greek variables,
upright units, semantic subscripts and superscripts, U+00B5 normalization, and
missing-glyph reporting. A negative `温度 T (K)` fixture proves that unsupported
CJK is rejected before shaping and produces no fallback run.

The selected Parley tests currently pass locally. The cosmic-text probe remains
an exit-path comparison, not a production dependency decision.

## Remaining gate evidence

A2/A4 now carry semantic labels end-to-end, use ordinary base characters for
script layout, and prove that every successful PDF/SVG/raster run uses a bundled
Heros face. A7's metric snapshot and A8's five local visual baselines have been
regenerated and reviewed. The remaining A3/A8 gate evidence is the native
Windows/Linux deterministic metric and export matrix.

The historical Source Sans 3 and system-CJK measurements remain available in
Git history, but are not current acceptance evidence.

## Verification

```sh
cargo fmt --manifest-path prototypes/text-shaping-spike/Cargo.toml --check
cargo test --manifest-path prototypes/text-shaping-spike/Cargo.toml --locked --all-features
cargo clippy --manifest-path prototypes/text-shaping-spike/Cargo.toml \
  --locked --all-targets --all-features -- -D warnings
```

## Sources

- TeX Gyre Heros project: <https://www.gust.org.pl/projects/e-foundry/tex-gyre>
- Parley documentation: <https://docs.rs/parley/latest/parley/>
- Parley repository and license: <https://github.com/linebender/parley>
- cosmic-text documentation: <https://docs.rs/cosmic-text/latest/cosmic_text/>
