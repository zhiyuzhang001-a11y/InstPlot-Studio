# ADR-007: V1 semantic Label AST and advanced math scope

Status: Accepted for Part A

Date: 2026-09-20

## Decision

V1 labels use a small backend-independent semantic AST with Text, Variable,
Upright, Greek, Subscript, Superscript, Unit, Operator, and Group nodes. The AST
compiles to styled Unicode spans and then to positioned glyph runs before any
backend is invoked.

Variables default to italic. Units, operators, ordinary text, and descriptive
subscripts are upright. Mathematical subscripts inherit mathematical italic.
Subscript/superscript scale and baseline shifts are layout policy, not Unicode
character substitution or backend behavior.

## Consequences

- Publication labels cover the required scientific notation without embedding a
  full TeX engine.
- Source Unicode remains searchable and can be validated before export.
- Advanced equations, arbitrary LaTeX, multiline alignment, and general math
  layout are explicitly outside V1 and Gate A.

## Rejected alternatives and exit strategy

- Raw markup interpreted separately by each backend: inconsistent and difficult
  to validate.
- Full LaTeX/Typst engine in V1: disproportionate dependency, size, and security
  surface for axis and legend labels.

The AST is extensible. A future math engine must compile into the same shaped-run
contract and must not create a second preview/export layout path.
