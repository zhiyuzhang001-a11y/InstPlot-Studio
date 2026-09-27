# instplot-render

Backend-neutral figure IR and deterministic display-list compiler.

```text
Figure IR (mm + scientific data)
  → layout/compiler decisions in points
  → ordered DisplayList with stable source node IDs
```

Backends may paint the display list but must not redo labels, autoscale, legend placement or margins.

```sh
cargo test --locked -p instplot-render
```
