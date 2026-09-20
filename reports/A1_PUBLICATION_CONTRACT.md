# A1 — fixed publication fixture and acceptance contract

Status: **DONE**

Frozen: 2026-09-20

## Inputs

- `fixtures/publication-v1/data.csv`
- `fixtures/publication-v1/manifest.toml`
- `fixtures/publication-v1/palettes.toml`
- `fixtures/publication-v1/expected.toml`
- `benchmarks/publication-corpus-v1/manifest.toml`
- publisher figure guides and cited publication pages recorded in the corpus

## Outputs

- one frozen, synthetic single-axes publication fixture;
- explicit experiment, fit, theory, and reference semantics;
- a versioned Paul Tol Bright, High Contrast, and BuRd color contract;
- SciPlot-owned neutral semantic colors and redundant non-color encodings;
- exact PDF and 300/600/1200 dpi PNG dimensions;
- explicit point, glyph, path, raster, anti-aliasing, and cross-platform tolerances;
- normal, grayscale, and deuteranopia reference expectations;
- a 12-entry citation-only Publication Visual Benchmark Corpus;
- a standard-library-only validator and acceptance contract.

## Acceptance evidence

Command:

```sh
python3 scripts/validate_a1.py
```

Result:

```text
A1 validation: PASS
fixture=publication-v1-single-axes series=5 corpus=12 outputs=PDF,SVG,PNG
```

The validator confirms that:

- all manifest-relative files resolve;
- millimetre, point, and pixel dimensions agree under the frozen rounding rule;
- the data contains dense, multiscale, clipping, positive, negative, and zero probes;
- every series references an existing column;
- semantic roles and experiment/fit identity are complete;
- palette values, grayscale ordering, role mappings, and non-color encodings are complete;
- the benchmark corpus contains 10–20 entries and covers the required publisher families.

## Known limitations

- Renderer output does not exist yet, so PDF/SVG structure and raster perceptual
  tolerances are contracts rather than executed backend results.
- Corpus observations are citation-only and deliberately do not redistribute
  publisher images.
- The corpus comparison ranges are A1 calibration hypotheses. A7 must replace
  estimates with measurements from the implemented renderer before product
  defaults freeze.
- A0 still lacks Windows/Linux release-size evidence, current three-platform CI
  run identities, and an agreed GUI cold-start/idle-memory measurement.

## Next dependency

A2 may now build the independent `studio-render-spike` and test deterministic
Figure IR → Display List snapshots against this fixture. Lite shared-core
extraction remains prohibited until Gate A and the remaining A0 evidence are
complete.
