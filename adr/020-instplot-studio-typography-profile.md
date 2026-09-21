# ADR-020: InstPlot Studio V1 typography profile

Status: Accepted; local implementation evidence complete, cross-platform evidence pending

Date: 2026-09-21

## Decision

The product name is **InstPlot Studio**. V1 uses one deterministic bundled
publication family, TeX Gyre Heros, for Latin text, Greek text, numbers, units,
and the V1 Scientific Symbol Core. The required real faces are Regular, Italic,
Bold, and Bold Italic. V1 does not introduce a separate Symbol font and does not
synthesize bold or italic faces.

Semantic styling remains independent of character identity: variables are
italic, units and ordinary text are upright, descriptive subscripts are
upright, and variable subscripts are italic. Greek variables use the real TeX
Gyre Heros Italic face; Greek units or ordinary Greek text use Regular.

Modern Unicode output uses U+03BC GREEK SMALL LETTER MU for both the micro unit
prefix and Greek mu. Their semantic nodes determine upright versus italic
styling. U+00B5 MICRO SIGN is accepted only as legacy input and is normalized to
U+03BC unless a project explicitly requests preservation of the original code
point.

CJK and other unsupported scripts are outside the V1 product scope. They must
produce an explicit unsupported-script diagnostic; V1 must not silently use a
system fallback, convert the text to outlines, or rasterize it and report a
successful publication export.

The detailed acceptance contract is
`docs/INSTPLOT_STUDIO_TYPOGRAPHY_SPEC.md`, version V1.0 dated 2026-09-21.

## Consequences

- ADR-005's Source Sans 3 decision is superseded.
- ADR-006's system-CJK fallback policy is superseded for V1.
- The A3/A4 Source Sans and CJK results remain historical prototype evidence,
  but they no longer satisfy the current typography acceptance contract.
- A3 pins the exact TeX Gyre Heros release, hashes and license files, verifies
  all four faces and both V1 Core sets, and implements the semantic Label AST.
- A4 and A8 now pass locally with semantic runs and reviewed Heros baselines;
  native Windows/Linux evidence remains part of Gate A.
- The repository and package rename from SciPlot to InstPlot Studio is a
  separate mechanical migration; user-facing documentation should use the new
  product name immediately without renaming paths or package identifiers
  prematurely.

## Rejected alternatives

- Source Sans 3 as the publication family: superseded by the new product
  typography specification.
- A separate Symbol font for Greek: rejected for V1 in favor of one visually
  consistent family with verified Unicode coverage.
- System CJK fallback: rejected because CJK is not a V1 product requirement and
  host-dependent output conflicts with the deterministic publication target.
- U+00B5 as the default micro-prefix output: retained only for explicit legacy
  compatibility; U+03BC is the modern Unicode default.
