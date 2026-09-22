# InstPlot Lite → Studio handoff protocol

Status: **Studio consumer implemented; Lite application integration pending**  
Protocol version: **1**  
File extension: **`.instplot-handoff`**

## Purpose

The handoff package transfers selected Lite datasets into a new Studio Figure
Document without filename inference or a dependency on shared process memory.
It is a temporary transport, not a Studio project file.

## Envelope

The UTF-8 JSON envelope contains:

- `schema_version`;
- SHA-256 of the canonical serialized payload;
- producer name and version;
- a complete ordered dataset list.

Each dataset records its stable dataset ID, label, original source identity,
source/fit kind, ordered column IDs and all `f64` values, row count, complete
`alive` bitmap and optional fit link. A fit link contains exact Parent-ID,
Source-X, Source-Y, full-precision equation and display equation.

Unknown fields, unsupported versions, checksum changes, inconsistent lengths,
non-finite values, duplicate IDs, missing parent datasets and missing source
columns are rejected. Fit association never uses a filename or label guess.

## Studio behavior

Studio validates the complete package before creating a Figure Document. The
document embeds the data and alive bitmap, maps source datasets to marker
artists and fits to same-object-colour line artists, and records the imported
dataset IDs in provenance. Once this succeeds, the document no longer depends
on the temporary package or original Lite source file.

The `--open-handoff` path and **Open from Lite…** UI consume an exact regular
`.instplot-handoff` file and delete only that file after successful validation,
document creation and layout. Symlinks, wrong extensions and failed imports are
never deleted. Normal `--import-handoff` keeps the package and writes a formal
`.instplot` project for automated workflows.

Studio style edits and project saves never modify Lite source TXT/XLSX files.

## Commands

The Studio executable exposes protocol test and integration entry points:

```text
instplot-studio --create-handoff LITE_EXPORT PACKAGE.instplot-handoff
instplot-studio --import-handoff PACKAGE.instplot-handoff FIGURE.instplot
instplot-studio --open-handoff PACKAGE.instplot-handoff
```

`--create-handoff` is a compatibility producer used for validation until the
same protocol writer is integrated into the Lite application repository. The
Lite “Open in InstPlot Studio” action should write the package to a private
temporary file, launch Studio with `--open-handoff`, and relinquish ownership
only after process launch succeeds.

## Compatibility policy

- Any incompatible envelope change requires a new handoff schema version.
- Studio rejects future versions explicitly.
- Existing Lite partitioned TXT and XLSX remain supported independently of the
  handoff protocol.
- The project schema remains version 1; newly embedded `alive` and fit display
  equation fields use backward-compatible defaults for older schema-v1 files.
