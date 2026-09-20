# A3 — text, font, and semantic label validation

Status: **default route selected; local evidence complete**

Date: 2026-09-20

## Decision

SciPlot will use Parley + fontique + HarfRust + skrifa as its sole default text
shaping route. Layout produces backend-neutral positioned glyph runs once; the
preview, PDF, and SVG backends must consume those runs without shaping again.

The deterministic Latin/Greek family is bundled Source Sans 3 v3.052 in static
Regular, Italic, and Bold OTF faces. CJK uses a system fallback whose resolved
font identity, version, face index, embedding permission, and subsetting status
must be recorded.

## Candidate evidence

Both Parley 0.11.1 and cosmic-text 0.19.0 passed the local label corpus. After
preventing a same-named system font from shadowing the bundled face, both engines
returned identical glyph IDs, X positions, and advances for the deterministic
Source Sans runs. Their Y values differ because they expose different baseline
coordinate conventions, not because they shape different glyphs.

Parley is selected because its current stack directly joins font discovery and
fallback (fontique), shaping (HarfRust), metrics/outlines (skrifa), and styled
layout. It also uses the newer HarfRust/skrifa line in this comparison. The
cosmic-text probe remains as an exit-path comparison, but is not a production
dependency decision.

Measured on the current Apple Silicon macOS host with a clean Cargo target:

- Parley candidate: 13.06 s clean release build; 2,648,824-byte stripped probe.
- cosmic-text candidate: 10.58 s clean release build; 2,622,136-byte stripped probe.
- Parley delta in this probe: 26,688 bytes.
- Mandatory Source Sans assets plus OFL text: 922,053 bytes.

The probes include the same three fonts, test model, metadata parser, and system
font discovery, so these figures compare route cost rather than final app size.
Both remain compatible with the 10 MiB architecture target at this stage.

## Font audit

The pinned family is Adobe Source Sans 3 v3.052 under OFL-1.1. All three static
faces expose BASE, CFF, GDEF, GPOS, GSUB, OS/2, cmap, head, hhea, hmtx, maxp,
name, and post tables. The tested coverage includes Latin, Greek, the Unicode
minus sign, superscript digits, and the publication labels in the fixture. It
does not include CJK.

- Regular: `Source Sans 3`, PDF/PostScript name `SourceSans3-Regular`, 334,924
  bytes, SHA-256 `08df266400933d3178d081a45f94a08814c3e55b4b7dd2e0ff69cb1329f13ab6`.
- Italic: `Source Sans 3 Italic`, PDF/PostScript name `SourceSans3-It`, 239,048
  bytes, SHA-256 `430b9f0eb1170be0981706d14b6cf87122efba9d86c373bbdf69daf276421462`.
- Bold: `Source Sans 3 Bold`, PDF/PostScript name `SourceSans3-Bold`, 343,596
  bytes, SHA-256 `7776ddb9f3eb58683e59f28d558d8896b768c2d0c80799fb3f1c56c54dfd98c9`.
- License text: 4,485 bytes, SHA-256
  `f9e57d28452ab6162c7fc0d248b5a5e81bf64676a6950d56b2e137c1253f79e8`.

The OS/2 embedding flag is installable for the bundled faces and subsetting is
allowed. The local mixed Chinese/Latin test resolved Chinese to PingFang SC
Regular, PostScript name `PingFangSC-Regular`, version `21.0d1e1`, collection
face index 3, preview-and-print embedding, with subsetting allowed. This is host
evidence, not a cross-platform fallback promise.

## Label and failure-path evidence

The semantic Label AST supports Text, Variable, Upright, Greek, Subscript,
Superscript, Unit, Operator, and Group. It emits independent styled spans before
shaping. The corpus covers plain Latin, italic variables, upright descriptive
subscripts, italic mathematical subscripts, upright units and numbers, Greek,
negative superscript exponents, mixed Chinese/Latin, a missing scalar, rotated
Y-axis label content, and legend content.

The selected route preserves source Unicode alongside glyph IDs and positions.
The invalid scalar resolves to glyph ID zero and produces an explicit warning.
The local CJK fallback records the actual face and embedding rights. Repeating
the selected probe produces byte-identical snapshots on this host.

## Verification

```sh
cd prototypes/text-shaping-spike
cargo fmt --check
cargo test --all-features --locked
cargo run --release --locked --features parley-candidate --bin parley-probe
cargo run --release --locked --features cosmic-candidate --bin cosmic-probe
```

## Remaining gate evidence

A3's architecture decision is complete, but two acceptance claims deliberately
remain owned by later planned stages:

- A4 must prove that PDF and SVG consume the selected positioned glyph runs,
  embed/subset the resolved fonts, and preserve searchable/copyable Unicode.
- A8 must run the pinned Latin/Greek metric snapshot on macOS, Windows, and Linux.

Until those pass, A3 is not a claim that Gate A is complete. Any backend that
reshapes text or requires outline-only PDF text reopens ADR-004.

## Sources

- Parley documentation: <https://docs.rs/parley/latest/parley/>
- Parley repository and license: <https://github.com/linebender/parley>
- cosmic-text documentation: <https://docs.rs/cosmic-text/latest/cosmic_text/>
- cosmic-text repository: <https://github.com/pop-os/cosmic-text>
- Source Sans repository and OFL: <https://github.com/adobe-fonts/source-sans>
- Source Sans releases: <https://github.com/adobe-fonts/source-sans/releases>
