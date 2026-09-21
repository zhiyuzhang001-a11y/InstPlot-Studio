# ADR-021: PDF-first V1 export scope

Status: Accepted

Date: 2026-09-21

## Decision

PDF is the only required vector publication format for InstPlot Studio V1 and
Gate A. It must preserve vector geometry, searchable/copyable semantic text,
embedded or subset TeX Gyre Heros faces, exact physical dimensions, and Unicode
extraction. PNG remains required at 300, 600, and 1200 dpi for Gate A; TIFF
remains required for the V1 release.

SVG is optional experimental output. The existing direct serializer and tests
may remain as research and regression evidence, but SVG availability, live text,
font portability, and editor compatibility do not block Gate A or V1 release.
The product must not advertise optional SVG as Illustrator-compatible without a
separate validated contract.

## Evidence

The generated SVG is self-contained for browsers and resvg because it embeds
TeX Gyre Heros OTF data through CSS `@font-face`. Adobe Illustrator 2026 does not
honor those embedded font data URLs during import: it reports the Regular and
Italic faces as missing and imports the fixture as a blank canvas. Requiring
portable live SVG text would therefore add a second font-distribution/export
policy without improving the primary journal workflow, which accepts vector PDF.

The generated PDF opened correctly in macOS Preview with vector geometry,
semantic text, Greek symbols, and scripted layout intact.

## Consequences

- The A1 required output set is PDF and PNG; SVG is explicitly optional.
- Gate A manual viewer evidence applies to PDF, not SVG editors.
- A4 may retain the SVG spike, but its result is not a product readiness gate.
- V1 documentation and UI must present PDF as the publication vector format.
- Reintroducing required SVG needs a new ADR defining its font portability,
  text editability, target editors, file-size budget, and acceptance fixtures.

## Rejected alternatives

- Requiring users to install bundled fonts merely to edit SVG: host-dependent
  and inconsistent with self-contained publication output.
- Making outlined SVG the default: portable visually, but loses live semantic
  text and duplicates guarantees already provided by PDF.
- Blocking V1 on Illustrator SVG import: disproportionate to the validated PDF
  workflow and the stated product requirement.
