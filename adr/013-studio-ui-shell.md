# ADR-013: Studio UI shell and GUI framework

Status: Accepted for Part A prototype; three-platform evidence pending A8

Date: 2026-09-20

## Decision

Use eframe/egui for the SciPlot Studio application shell, pinned at eframe
0.36.1 for the current baseline. Disable default features and enable only the
bundled UI font, glow renderer, Wayland, and X11 support. Use rfd 0.17.2 for the
native file dialog, with the XDG portal backend selected on Linux.

Do not use `egui_plot` for publication layout. The shell edits the Figure
Document or transient view overlays and consumes the backend-neutral Display
List through a replaceable preview adapter.

## Consequences

- The shell stays aligned with the proven Lite framework and interaction model.
- There is no webview, JavaScript runtime, or first-start runtime download.
- File dialogs and window state remain application-shell concerns.
- The preview adapter may be replaced without changing Figure IR, layout, or
  export backends.
- macOS interaction is locally validated; Windows and Linux remain A8 evidence.
