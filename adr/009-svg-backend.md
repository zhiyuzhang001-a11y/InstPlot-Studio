# ADR-009: SVG backend

Status: Superseded as a V1 requirement by ADR-021; prototype retained

Date: 2026-09-20

> The direct-SVG backend decision remains active. Its Source Sans/CJK font
> details are superseded by ADR-020 and require new typography fixtures.

> ADR-021 removes SVG from the Gate A and V1 required export set after native
> Illustrator testing showed that embedded OTF `@font-face` data URLs do not
> provide portable editable text. The implementation remains useful research,
> but its compatibility no longer blocks the product.

## Decision

Serialize SVG directly from the resolved Display List. SciPlot owns this small
adapter because its primitives map directly to SVG and doing so preserves point
size, source node IDs, font provenance, and text policy without another layout
abstraction.

The selected bundled TeX Gyre Heros face is embedded as a data-URI font. Text
remains Unicode text; each shaped cluster receives its resolved origin and glyph
ID metadata. Unsupported V1 scripts fail before serialization and cannot select
a system font. Paths, clipPath, paint, dash/cap/join, rotation, metadata, and
RGBA images are self-contained.

## Evidence and consequences

roxmltree and usvg parse the fixed artifact, resvg rasterizes it without crop,
and no file or network URL is present. The output is larger than PDF because an
OTF is embedded whole in this spike; production may subset web fonts if that can
be done without changing metrics or text semantics.

An SVG library that reshapes/re-lays out content and path-only SVG as the default
are rejected. usvg remains a parser/verification dependency, not the serializer.
