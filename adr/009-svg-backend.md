# ADR-009: SVG backend

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Serialize SVG directly from the resolved Display List. SciPlot owns this small
adapter because its primitives map directly to SVG and doing so preserves point
size, source node IDs, font provenance, and text policy without another layout
abstraction.

Pinned Source Sans is embedded as a data-URI font. Text remains Unicode text;
each shaped cluster receives its resolved origin and glyph ID metadata. CJK uses
the recorded system fallback identity. Paths, clipPath, paint, dash/cap/join,
rotation, metadata, and RGBA images are self-contained.

## Evidence and consequences

roxmltree and usvg parse the fixed artifact, resvg rasterizes it without crop,
and no file or network URL is present. The output is larger than PDF because an
OTF is embedded whole in this spike; production may subset web fonts if that can
be done without changing metrics or text semantics.

An SVG library that reshapes/re-lays out content and path-only SVG as the default
are rejected. usvg remains a parser/verification dependency, not the serializer.
