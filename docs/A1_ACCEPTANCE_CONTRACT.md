# A1 publication fixture acceptance contract

Status: **frozen; export scope revised by ADR-021**

Frozen: 2026-09-20

The canonical input is `fixtures/publication-v1/manifest.toml`. Relative paths
are resolved from the manifest directory. A candidate implementation must consume
the fixture without silently replacing labels, colors, dimensions, series order,
or rounding rules.

## Required outputs

- one preview produced from the same resolved layout as every export backend;
- PDF at the declared point dimensions, with searchable text and vector paths;
- PNG at 300, 600, and 1200 dpi with the exact declared pixel dimensions;
- normal-color, grayscale, and deuteranopia validation views.

TIFF is deliberately outside Gate A but remains required for the V1 release.
SVG is an optional experimental export. Its presence, text behavior, and editor
compatibility are not Gate A or V1 release requirements.

## Determinism and tolerances

All layout coordinates use points. Millimetres are converted using exactly
`72 / 25.4`. Raster dimensions use round-half-away-from-zero. Page size, glyph
position, path coordinates, raster comparison, anti-aliasing edge allowance, and
cross-platform font-metric tolerances are declared in the manifest and are part
of the versioned fixture.

Exceeding a declared tolerance is a failure. Anti-aliasing differences are
ignored only inside the declared one-pixel edge band; they do not excuse changed
geometry, missing content, or incorrect colors.

## Semantic requirements

- Experiment and its Fit share semantic identity and color.
- Experiment is encoded as markers; Fit as a solid line.
- Theory is a dashed neutral line; Reference is a dotted lighter-neutral line.
- The output must remain interpretable without color through marker fill/shape
  and dash pattern.
- Missing glyphs must produce an explicit warning. Silent substitution is a
  failure.

## Change control

The validator checks the contract itself before renderer work begins:

```sh
python3 scripts/validate_a1.py
```

Any intentional incompatible change requires either `publication-v2` or an ADR
that records why the A1 comparison baseline changed. ADR-021 records the removal
of SVG from the required export set; geometry, data, typography, palette, PDF,
and raster expectations remain unchanged.
