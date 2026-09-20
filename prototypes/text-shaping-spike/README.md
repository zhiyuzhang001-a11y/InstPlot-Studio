# A3 text shaping spike

This independent crate compares the two A3 candidates against the same semantic
Label AST and pinned Source Sans 3 font files.

Run the selected Parley route:

```sh
cargo run --release --locked --features parley-candidate --bin parley-probe
```

Run the cosmic-text comparison route:

```sh
cargo run --release --locked --features cosmic-candidate --bin cosmic-probe
```

Run all local tests:

```sh
cargo fmt --check
cargo test --all-features --locked
```

The probe emits source Unicode, resolved font identity, face index, embedding
permissions, font size, glyph IDs, positions, advances, and missing-glyph
warnings. Bundled fonts are always registered ahead of same-named system fonts.
System CJK fallback remains available and its actual identity is recorded.

The source fonts are Adobe Source Sans 3 v3.052, licensed under the SIL Open
Font License 1.1. The exact files and checksums are recorded in the A3 report.
