# ADR-022: Versioned InstPlot Studio project container

Status: Accepted for V1

Date: 2026-09-22

## Decision

InstPlot Studio V1 stores a project as one UTF-8 JSON document with the
`.instplot` extension. The root object carries an integer `schema_version` and
the producer version. Schema 1 records stable identities, external or embedded
data-source policy, axes and artists, the semantic label registry, typography
and palette provenance, user overrides, export preferences and operation
provenance.

V1 JSON is deliberately strict. Unknown fields in a supported schema are an
error instead of being silently discarded. A schema newer than the reader is
rejected explicitly. Older schemas are admitted only through a named migration
that returns a current document and appends a migration provenance record.

Saving uses a same-directory atomic replacement. Before replacing an existing
valid project, Studio atomically writes its previous bytes to `<name>.bak`.
Studio refuses to overwrite an existing file that does not decode and validate
as a project. Opening falls back to the backup only when the primary cannot be
read or validated, and reports that recovery visibly.

External data records store their stable source identity, Unicode path, byte
length and SHA-256 digest. Opening compares the current bytes with that stored
fingerprint. Missing or changed sources produce warnings and never update the
project or replace stored identities automatically. Embedded numeric columns
remain valid schema content for self-contained projects.

## Rationale and evidence

A single JSON document is inspectable, deterministic and sufficient for the V1
single-axes data sizes. It avoids ZIP and archive dependencies before the
product has evidence that large embedded assets require them. The application
tests cover create/save/open round trips, exact resolved-output preservation,
strict unknown-field handling, schema-0 migration, stable source and fit
identity, changed/missing external sources, backup creation and recovery from a
corrupt primary.

## Consequences

- B3-B5 extend the versioned schema through explicit migrations, not ad-hoc
  optional fields with undocumented meaning.
- A future archive container may wrap the same manifest, but requires a new ADR
  and a schema/container migration.
- Non-Unicode external paths are rejected rather than serialized lossily.
- Recovery never claims that a backup is the primary project.
- JSON formatting is not semantic; decoded schema values and the resolved
  figure are the round-trip contract.

## Rejected alternatives

- ZIP plus JSON plus binary payloads: useful for large assets, but premature for
  V1 and adds format and recovery complexity without current evidence.
- Unversioned or permissive JSON: risks silently losing newer information when
  an older Studio rewrites the project.
- Automatic source refresh: can change a publication figure without an explicit
  user decision.
- UI-widget serialization: makes the GUI the only source of truth and breaks
  headless export.
