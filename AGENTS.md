# SciPlot agent notes

## Rust toolchain and Clippy

- This repository pins Rust 1.98.0 through `rust-toolchain.toml`.
- Clippy is installed in the `1.98.0-aarch64-apple-darwin` rustup toolchain as
  `clippy 0.1.98`; do not conclude that Clippy is unavailable merely because it
  is absent from the machine's default `stable-aarch64-apple-darwin` toolchain.
- From the repository or a prototype directory, verify it with
  `cargo clippy --version` and run it with
  `cargo clippy --locked --all-targets -- -D warnings`.
- If PATH or Cargo shim behavior is unclear, use
  `rustup run 1.98.0-aarch64-apple-darwin cargo clippy --version` and inspect
  `rustup component list --toolchain 1.98.0-aarch64-apple-darwin --installed`.

