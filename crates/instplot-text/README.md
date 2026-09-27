# instplot-text

Semantic scientific-label model and bundled-font metadata for InstPlot products.

The crate distinguishes ordinary text, variables, Greek variables, upright descriptive subscripts, numerical scripts, units, operators and emphasis before layout. It owns no GUI and performs no figure layout.

Consumers should construct or parse one semantic label tree and pass that tree through layout, preview and export. Do not infer italic/upright meaning from string length in a backend.

```sh
cargo test --locked -p instplot-text
```
