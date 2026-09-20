# ADR-014: HiDPI transform and window-independent export

Status: Accepted for Part A prototype; native scale matrix pending A8

Date: 2026-09-20

## Decision

Keep publication coordinates in physical points. Preview applies exactly one
view transform:

`framebuffer_px = figure_pt × canvas_zoom × native_pixels_per_point`

`canvas_zoom` is document-independent view state. The OS/native scale affects
only logical-point-to-framebuffer conversion and never recompiles publication
layout.

Headless export branches before eframe configuration or event-loop creation and
calls the display-list export backend directly. Export must not read window size,
theme, canvas zoom, or screen scale.

## Consequences

- Moving a window between 1× and 2× displays changes framebuffer density, not
  figure geometry.
- UI zoom and publication physical size remain separate concepts.
- The transform is unit tested at 1× and 2×; physical Retina, Windows, and Linux
  runs remain mandatory in A8.
- Both exact SVG and full A4 PDF exports have been exercised in a process that
  creates no visible window.
