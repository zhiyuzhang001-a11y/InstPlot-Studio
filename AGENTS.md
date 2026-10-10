# SciPlot agent notes

## Risk-based validation and CI efficiency

- Before choosing tests or changing CI, read `docs/CI_VALIDATION_POLICY.md`.
- Classify the actual behavioral risk and affected platforms, not the diff size.
  Use targeted validation for low-risk edits; retain the relevant security and
  recovery matrix for updater behavior changes and full validation for releases.
- Batch related fixes and reuse only explicitly proven exact-source evidence;
  do not rerun expensive packaging/recovery tests for every intermediate edit.
- This policy does not override current required checks. Do not bypass checks,
  weaken security gates, or claim the optimized workflow exists before it does.
- Separate queue, build/test, packaging, upload and public verification timings;
  do not describe OSS upload delays as CI test time. Poll only at the agreed
  interval and stay quiet when nothing actionable has changed.

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

