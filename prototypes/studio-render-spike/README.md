# Studio render spike

This independent crate is the A2 validation prototype. It has no third-party
dependencies and is not a production application crate.

It proves the following boundary:

```text
Figure IR (mm + scientific data)
  → layout compiler (pt)
  → ordered Display List (pt + source node IDs)
  → deterministic snapshot / minimal SVG path backend
```

Run it with:

```sh
cargo fmt --check
cargo test --locked
```

The SVG backend is evidence that fixed axes and curves can be mapped without
backend layout decisions. It is intentionally not a production backend choice.
