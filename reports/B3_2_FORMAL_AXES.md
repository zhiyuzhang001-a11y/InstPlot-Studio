# B3.2 — Formal axes decorations and clipping

Date: 2026-09-22  
Status: **DONE locally; cross-platform CI pending implementation push**

## Boundary established

The versioned B2 Figure Document now has a direct adapter to the production
`instplot-layout` compiler. The adapter transfers physical figure dimensions,
linear/log10 scales, ranges, automatic/fixed locators, automatic/decimal/
scientific formatters and the complete semantic label AST. Layout remains in
point units and does not read framebuffer or operating-system display scaling.

The formal axis model now carries an explicit major/minor grid policy. B3.2
enables major grid lines for project-document axes, draws them below the axes
frame, ticks and labels, and preserves the A7 fixture by leaving its grid policy
off. The axes rectangle is the authoritative data clipping boundary. Existing
artist drawing opens exactly one matching clip scope and closes it; the B3.2
document adapter exposes that rectangle as `data_clip` for B3.3 consumers.

Project string IDs are mapped deterministically to renderer `NodeId` values and
retained in a reverse map. Conventional `node-N` identities preserve `N`;
arbitrary IDs use a deterministic FNV-1a value, and a collision fails explicitly
instead of silently aliasing two project nodes.

B3.2 deliberately does not switch the current preview to an axes-only Display
List. B3.3 will connect resolved glyph runs/outlines and then replace the
temporary preview without dropping existing artists during an intermediate
commit.

## Verification

`python3 scripts/validate_b3_2.py` is the repeatable phase audit. It runs format,
all workspace tests, Clippy with warnings denied, the complete promoted A7
compatibility suite and a release build. It also checks the adapter coverage,
semantic-label conversion, grid/clipping contract, reversible IDs, schema
stability and the 12 MiB release-size ceiling.

The focused B3.2 tests prove:

- repeated project-to-layout compilation produces an identical snapshot;
- project dimensions, explicit ranges, fixed ticks and explicit formatting are
  honored;
- the major-grid path count matches the resolved X/Y major ticks;
- X and rotated Y semantic labels reach the Display List;
- layout warnings and Display List structural errors are absent;
- numeric and arbitrary project IDs remain recoverable;
- A7 artist clipping equals the resolved axes rectangle and is balanced.

## Acceptance result

- axes rectangle and four spines: **PASS**;
- major ticks, minor ticks and labels: **PASS**;
- deterministic major grid below decorations: **PASS**;
- semantic X label and rotated Y label: **PASS**;
- explicit data clipping rectangle and balanced artist clip: **PASS**;
- project scale/locator/formatter conversion: **PASS**;
- stable identity reverse mapping: **PASS**;
- project schema remains version 1: **PASS**.

## Next dependency

Proceed to **B3.3**: make preview consume the resolved glyph runs or outlines,
then remove the temporary preview layout path while preserving the single
Display List boundary.
