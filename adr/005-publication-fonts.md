# ADR-005: Deterministic publication fonts and CJK fallback

Status: Accepted for Part A

Date: 2026-09-20

## Decision

Bundle the static OTF Regular, Italic, and Bold faces of Adobe Source Sans 3
v3.052 for deterministic Latin/Greek publication text. Register them under an
internal family alias so an installed same-named font cannot shadow the pinned
bytes. Use system fonts only as fallback for unsupported scripts, principally
CJK.

The three font files plus OFL text total 922,053 bytes. The exact checksums,
PostScript names, tables, and embedding rights are recorded in the A3 report.

## Consequences

- Latin/Greek glyph metrics originate from repository-pinned font bytes on every
  platform.
- CJK output may differ by host and must pass export-time embedding checks.
- V1 does not bundle a multi-megabyte CJK family merely to claim identical CJK
  metrics.
- Replacing any font file requires a versioned checksum change, metric snapshot
  review, and an ADR amendment.

## Rejected alternatives

- System Source Sans 3: same family/version names can refer to different files
  or glyph order and silently break deterministic snapshots.
- Bundled CJK by default: unnecessary size cost for V1 and incompatible with the
  current lightweight target.
- Text outlines: destroys search/copy behavior and is prohibited as the default.
