# InstPlot Studio Part A — Windows manual checklist

Use this checklist on Windows 10 or Windows 11. The expected review time is
20–30 minutes. Do not install Rust or rebuild the program; use the executable
and PDF distributed in this test kit.

## Test environment

- Windows version/build:
- Display resolution:
- PDF viewer and version:
- Reviewer:
- Date and time:

## 1. Display scaling

For each scale, set **Settings → System → Display → Scale**, launch or return to
`InstPlot-Studio-Part-A.exe`, and record the observed `UI pixels/point` and
`preview framebuffer` values. Also change the scale once while the program is
already running.

- [ ] 100%: expected about `1.00` and `378 × 276 px`; result/notes:
- [ ] 125%: expected about `1.25` and `473 × 345 px`; result/notes:
- [ ] 150%: expected about `1.50` and `568 × 415 px`; result/notes:
- [ ] 200%: expected about `2.00` and `757 × 553 px`; result/notes:
- [ ] No toolbar, inspector, canvas or text overlaps or clipping caused by scale.
- [ ] Text and lines remain crisp rather than bitmap-blurred.
- [ ] Buttons, text fields and the zoom slider respond at their visible position.
- [ ] A live scale change redraws the window without a crash or stale hit areas.

Small rounding differences of one pixel are acceptable. Record larger
differences and attach a screenshot.

## 2. Native input and file dialog

- [ ] Type `Keyboard test 123` in the text field.
- [ ] Paste `μ0 α β ≤ m−2`; the characters remain visible and are not boxes.
- [ ] Press `Ctrl+O`; a native Windows Open dialog appears.
- [ ] Cancel the dialog; the app remains responsive and the canvas is unchanged.

## 3. PDF viewer

Open `InstPlot-Studio-Part-A.pdf` in the named target viewer.

- [ ] At 400% zoom, the curve and axes remain sharp and are not a page-sized bitmap.
- [ ] The page has no clipped labels, missing glyphs or overlapping text.
- [ ] Search for `μ0HDL`; exactly one match is found.
- [ ] Search for `Experiment`; exactly one match is found.
- [ ] Select all text, copy it into Notepad, and confirm these semantic strings:
  - `Current density Je (A m−2)`
  - `T ≤ 300 K`
  - `Experiment`
  - `Fit`
  - `μ0HDL (mT)`
- [ ] If the viewer exposes font properties, TeX Gyre Heros is reported as
  embedded or embedded subset.

## Result to return

- Overall: PASS / FAIL
- Failed checklist items:
- Observed scale values:
- Copied PDF text:
- Screenshots for failures only:
- Additional notes:
