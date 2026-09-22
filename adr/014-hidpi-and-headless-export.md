# ADR-014: HiDPI transform and window-independent export

Status: Accepted; Gate A scale evidence complete

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
- The transform is unit tested at 1× and 2×, and Windows was inspected at
  100%/125%/150%/200% including a live scale change. By product-owner decision
  on 2026-09-22, physical Retina and Linux HiDPI runs are deferred and do not
  block Gate A. Retina hardware should be checked before a release that claims
  support for such hardware, when a suitable display is available.
- Both exact SVG and full A4 PDF exports have been exercised in a process that
  creates no visible window.
