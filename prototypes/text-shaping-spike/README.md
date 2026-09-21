# A3 text shaping spike

This independent crate compares the two A3 candidates against the same semantic
Label AST and pinned TeX Gyre Heros 2.004 font files.

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
cargo test --locked --features parley-candidate
cargo clippy --locked --all-targets --features parley-candidate -- -D warnings
```

The probe emits source Unicode, resolved font identity, face index, embedding
permissions, font size, glyph IDs, positions, advances, and missing-glyph
warnings. Only the four bundled publication faces are available to the probe;
CJK and other unsupported scripts produce an explicit diagnostic before shaping
and never resolve through system fonts.

The source fonts are TeX Gyre Heros 2.004 under the GUST Font License. The exact
files, license, manifest, source README, and checksums are recorded with A3.
