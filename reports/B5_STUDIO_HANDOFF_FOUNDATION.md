# B5 — Studio handoff foundation

Date: 2026-09-22  
Status: **Studio side DONE; Lite application integration pending**

## Completed in this repository

- Lite partitioned TXT and XLSX imports preserve source/fit kind, stable IDs,
  Parent-ID, Source-X, Source-Y, precise equation and display equation.
- The versioned `.instplot-handoff` envelope carries complete numeric columns,
  column identity, alive state and fit links with a payload checksum.
- Studio creates a new embedded Figure Document and source/fit artists from the
  package without filename inference.
- Source and fit share object colour while marker/line channels remain
  redundant; imported data passes formal layout and Publication Check.
- `--open-handoff` and **Open from Lite…** consume a validated temporary package.
  Exact-file cleanup occurs only after successful document creation and layout;
  unsafe paths and failed imports are retained.
- A formally saved project reopens after the temporary package has been
  deleted, and Studio style edits leave the Lite source byte-for-byte unchanged.

## Compatibility

The project stays at schema version 1. Older schema-v1 embedded payloads with no
`alive` member retain their legacy all-alive interpretation and checksum.
Missing fit display equations decode as `None`. New handoff-created projects
store both fields explicitly.

## Remaining external integration

The actual Lite menu action that writes this envelope and launches Studio must
be implemented and released from the separate `InstPlot-Lite` repository. That
repository is not present in this workspace, so this report does not claim B5
is fully closed.

Protocol details: [`../docs/INSTPLOT_HANDOFF_PROTOCOL.md`](../docs/INSTPLOT_HANDOFF_PROTOCOL.md).
