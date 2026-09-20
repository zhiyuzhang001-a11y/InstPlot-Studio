# ADR-016: Scale, locator, and formatter contract

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Keep scale transforms, tick location, and formatting as separate deterministic
policies owned by the layout compiler.

V1 starts with linear and log10 scales, automatic and fixed major locators,
linear/log minor locators, scalar formatting, optional shared powers of ten,
precision derived from the major step, and negative-zero suppression. Horizontal
label collision is resolved by a stable stride after shaping, never by an export
backend.

## Consequences

- The same numeric values and labels feed preview, PDF, SVG, and raster output.
- Invalid log domains fail with the owning axis node ID.
- Locale-specific formatting and date/category scales require later explicit
  policies; they are not hidden inside the baseline formatter.
