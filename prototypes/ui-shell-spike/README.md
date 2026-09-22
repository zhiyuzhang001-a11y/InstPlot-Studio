# A6 UI shell spike

This disposable native shell validates eframe/egui around the real A2 Display
List. It contains a toolbar, white publication canvas, inspector placeholder,
canvas zoom, editable text, keyboard shortcut, and an rfd native file dialog.
No `egui_plot`, webview, JavaScript runtime, or first-run download is used.

Upstream references checked on 2026-09-20:

- [eframe 0.36.1](https://docs.rs/eframe/0.36.1/eframe/)
- [egui coordinate and scaling model](https://docs.rs/egui/0.36.2/egui/)
- [rfd 0.17.2](https://docs.rs/rfd/0.17.2/rfd/)

## Reproduce

```sh
cargo fmt --check
cargo test --locked --all-features
cargo build --release --locked
target/release/ui-shell-spike
target/release/ui-shell-spike --headless-svg /tmp/sciplot.svg
cargo build --release --locked --features publication-stack
target/release/ui-shell-spike --publication-pdf /tmp/sciplot.pdf
```

The optional `publication-stack` feature exists only to measure the integrated
A3/A4 dependency delta and prove that publication PDF export exits before the
window/event-loop path.

## Architecture evidence

- The shell depends on the A2 crate and paints its backend-neutral Display List.
- `ScreenTransform` maps figure points to egui logical points with an independent
  canvas zoom. Native `pixels_per_point` affects only framebuffer dimensions.
- The publication canvas is always white; surrounding controls follow the OS
  theme through eframe defaults.
- Inspector state changes only a view zoom and placeholder text. It cannot mutate
  display-list geometry or export state.
- `--headless-svg` and feature-gated `--publication-pdf` complete without
  constructing `NativeOptions`, a window, a GPU context, or an event loop.
- eframe is pinned to 0.36.1 with default features disabled and only
  `default_fonts`, `glow`, `wayland`, and `x11`, matching the Lite baseline.
- rfd is pinned to 0.17.2; Linux selects its XDG portal backend, while macOS and
  Windows use their native implementations.

## Local measurements

Machine/date: local macOS host, 2026-09-20. These are reproducible engineering
measurements, not three-platform Gate A evidence.

| Measurement | Result |
| --- | --- |
| Stripped shell release executable | 3,745,344 bytes; no mandatory external assets |
| Stripped shell + A3/A4 publication stack | 5,719,984 bytes |
| Integrated publication dependency delta | 1,974,640 bytes |
| Clean shell release build | 30.08 s |
| Warm process-to-first-canvas | 111 ms |
| Idle resident memory after about 51 s | 115,680 KiB |
| Shell dependency tree | 251 lines, 146 unique lines |
| Headless SVG | 0.41 s; exact 252.28346 × 184.25197 pt |
| Headless PDF with publication stack | 0.60 s; exact 252.283 × 184.252 pt reported by `pdfinfo` |
| Observed local native scale | 1.0 pixels/logical point |

The shell-only and integrated sizes are both below the 10 MiB architecture
target. The integrated result is more representative than adding standalone A4
and A6 binaries because the linker shares and removes code.

## Interactive verification

On the local macOS host the real window was opened and visually inspected:

- toolbar, central white canvas, right inspector, and dark OS chrome rendered;
- the canvas zoom slider changed 1.50× to 2.80× and framebuffer reporting changed
  from 378 × 276 px to 706 × 516 px without changing the Display List;
- the editable field accepted keyboard text;
- Open and Command-O route to the native macOS file dialog;
- the file dialog was cancelled without changing document state.

## Open evidence and prototype limitations

- Windows scaling passed at 100%/125%/150%/200%. Native Retina and Linux HiDPI
  runs are deferred by the product owner and do not block Gate A; the pure
  transform test covers 1× versus 2× arithmetic. Retina hardware remains a
  release-time check when such hardware is available.
- The A2 placeholder `GlyphRun` contains source strings rather than A3 positioned
  glyphs. This egui preview can therefore use UI-font metrics and ignores text
  rotation. A7 must connect the resolved A3 runs or outlines before the preview
  can be publication-faithful.
- The local shell was inspected under a dark OS theme. Automatic OS-theme routing
  is provided by eframe, but light-mode and all three OS combinations remain A8
  evidence.
- Publication-label CJK is outside the V1 scope and must be rejected before
  shaping. Localized UI text is a separate UI-font concern and does not authorize
  publication export through a system fallback.
