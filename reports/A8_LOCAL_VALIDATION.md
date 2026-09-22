# A8 local validation baseline

Status: **ADR-020 local typography and visual baseline passes**

Updated: 2026-09-21

## Reproduce

```sh
python3 scripts/validate_part_a.py
```

The runner writes `target/part-a-validation/summary.json` and one log per
command. These generated files are caches/evidence for the current checkout,
not committed acceptance records.

## What remains valid from the 2026-09-20 run

- the pinned Rust 1.98.0 toolchain and Clippy discovery procedure;
- the repeatable format/test/Clippy harness;
- A0's InstPlot Lite baseline;
- A1 geometry, data, palette, size, and export acceptance contract;
- the A2/A4 backend-neutral architecture and structural snapshots as design
  evidence, subject to the text-run migration;
- A5's Plotine reuse boundary and A6's UI/HiDPI shell evidence;
- the visual comparison method and its thresholds.

## Invalidated evidence

The former Source Sans 3 font measurements, PingFang/system-CJK fallback
diagnostics, font-specific layout snapshot, generated PDF/SVG/PNG files, and
reviewed visual PNG baselines do not prove the accepted ADR-020 contract. They
must not be labeled current simply because an old copy remains in history or a
local build/output directory. The active Source Sans visual PNGs were moved out
of the repository rather than relabeled as Heros evidence.

The replacement diagnostics are:

- all four TeX Gyre Heros faces pass the two V1 Core coverage sets;
- U+00B5 input normalizes to U+03BC;
- unsupported CJK produces a typed error and no shaped/exported fallback run;
- every successful publication run has bundled origin and an allowed real face.

## Current local result

The fresh complete run recorded in `target/part-a-validation/summary.json`
passes all 21 local checks across the six Part A prototypes. Semantic script
layout reaches A2/A4, every successful font diagnostic has bundled origin, the
A7 Heros metric snapshot passes, and the new normal 300/600/1200 dpi, grayscale,
and deuteranopia baselines were visually reviewed and reproduce with zero
changed pixels.

## Completed A8 evidence and deferred platforms

- Windows automated validation passed all 21 checks in GitHub Actions at commit
  `7d89c666f964d96e49cd72f67fba480bbe0532f8`. The product owner completed the
  Windows 10 manual review on 2026-09-22: 100%/125%/150%/200% scaling and a live
  scale change, Unicode keyboard/clipboard input, the native Open dialog, and
  Microsoft Edge PDF rendering/search/copy all passed. Structured evidence is
  recorded in `reports/A8_WINDOWS_MANUAL_EVIDENCE.json`. Linux is deferred by
  the current product-owner decision and is not recorded as passed.
- The attached T2752Q reports 2560×1440 at 1.0 pixels per point and Quartz Debug
  is not installed. By product-owner decision on 2026-09-22, physical Retina
  evidence is deferred and is not a Gate A requirement. The 1×/2× transform
  arithmetic remains unit tested; suitable Retina hardware should be checked
  before a release that claims support for it.

The macOS Preview check for the generated PDF passed on 2026-09-21: the page,
vector geometry, clipping, labels, Greek quantity symbol, and scripted text
rendered correctly. A fresh accessibility-assisted inspection also confirmed
one search result for `μ0HDL` and exact semantic select/copy output. Structured
manual evidence is recorded in `reports/A8_MACOS_MANUAL_EVIDENCE.json`. The
unified audit is `scripts/audit_part_a.py`; its exit codes distinguish failures
(`1`) from incomplete or blocked evidence (`2`).

The same fresh 1× inspection confirmed that the A6 shell opens at
`pixels_per_point=1.000`, reports a 378×276 px framebuffer at 1.50× canvas zoom,
and routes Command-O to the native macOS Open dialog. It also reproduced the
documented A6 placeholder limitation: the y-axis source string is neither
rotated nor fully visible in the shell preview. The publication PDF is correct,
but this disposable shell must not be cited as a publication-faithful preview;
the future Studio preview must consume the resolved A3/A7 runs or outlines.

The Illustrator 2026 SVG import failure discovered during this review is kept
as evidence for the optional SVG experiment. ADR-021 removes SVG from the V1
required export set, so the limitation is no longer a Gate A failure and must
not be represented as a passed editor-compatibility result.

After applying ADR-021 and installing the pinned `cargo-audit` 0.22.2 tool, the
last complete local audit reported 16 passes, zero failures, one now-retired
Retina blocker, and two informational warnings. Applying the 2026-09-22 manual
gate policy to the structured evidence returns the required macOS PDF item as
passed with no remaining manual blocker. The Windows automated scope is defined
in `.github/workflows/part-a-windows-audit.yml` and its manual evidence also
passes all three required items.

Gate A / Part A is closed. Physical Retina and Linux HiDPI checks are deferred;
they must not be represented as passed.
