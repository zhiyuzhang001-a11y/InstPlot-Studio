use super::*;

pub(super) fn resolved_preview(
    document: &FigureDocument,
) -> Result<ResolvedFigure, instplot_studio::DocumentLayoutError> {
    resolve_document(document)
}

pub(super) fn data_coordinates_at(
    resolved: &ResolvedFigure,
    document: &FigureDocument,
    point: egui::Pos2,
    origin: egui::Pos2,
    zoom: f32,
) -> Option<(f64, f64)> {
    if zoom <= 0.0 {
        return None;
    }
    let local = (point - origin) / zoom;
    let axes = resolved.layout.result.axes;
    let x = f64::from(local.x);
    let y = f64::from(local.y);
    let edge_tolerance = 0.01;
    if x < axes.x - edge_tolerance
        || x > axes.right() + edge_tolerance
        || y < axes.y - edge_tolerance
        || y > axes.bottom() + edge_tolerance
    {
        return None;
    }
    data_coordinates_from_local(document, axes, x, y)
}

pub(super) fn active_data_coordinates_at(
    resolved: &ResolvedFigure,
    document: &FigureDocument,
    point: egui::Pos2,
    origin: egui::Pos2,
    zoom: f32,
) -> Option<HoverDataCoordinates> {
    let (x1, y1) = data_coordinates_at(resolved, document, point, origin, zoom)?;
    let local = (point - origin) / zoom;
    let axes = resolved.layout.result.axes;
    let x_fraction = ((f64::from(local.x) - axes.x) / axes.width).clamp(0.0, 1.0);
    let y_fraction = (1.0 - (f64::from(local.y) - axes.y) / axes.height).clamp(0.0, 1.0);
    let x2 = (document.axis_mode() == AxisMode::DualX)
        .then(|| document.axis_record_by_identity(AxisIdentity::X2))
        .flatten()
        .and_then(|axis| axis_value_at_fraction(&axis, x_fraction));
    let y2 = (document.axis_mode() == AxisMode::DualY)
        .then(|| document.axis_record_by_identity(AxisIdentity::Y2))
        .flatten()
        .and_then(|axis| axis_value_at_fraction(&axis, y_fraction));
    Some(HoverDataCoordinates { x1, y1, x2, y2 })
}

pub(super) fn data_coordinates_from_local(
    document: &FigureDocument,
    axes: instplot_layout::Bounds,
    x: f64,
    y: f64,
) -> Option<(f64, f64)> {
    let x_fraction = ((x - axes.x) / axes.width).clamp(0.0, 1.0);
    let y_fraction = (1.0 - (y - axes.y) / axes.height).clamp(0.0, 1.0);
    Some((
        axis_value_at_fraction(&document.axis_record(AxisDimension::X), x_fraction)?,
        axis_value_at_fraction(&document.axis_record(AxisDimension::Y), y_fraction)?,
    ))
}

pub(super) fn axis_value_at_fraction(axis: &AxisRecord, fraction: f64) -> Option<f64> {
    match axis.scale {
        AxisScale::Linear => Some(axis.minimum + fraction * (axis.maximum - axis.minimum)),
        AxisScale::Log10 if axis.minimum > 0.0 && axis.maximum > 0.0 => {
            Some(10.0_f64.powf(
                axis.minimum.log10() + fraction * (axis.maximum.log10() - axis.minimum.log10()),
            ))
        }
        AxisScale::Log10 => None,
    }
}

pub(super) fn format_data_coordinate(value: f64) -> String {
    let absolute = value.abs();
    let text = if absolute >= 10_000.0 || (absolute > 0.0 && absolute < 0.001) {
        format!("{value:.4e}")
    } else {
        let mut text = format!("{value:.6}");
        while text.contains('.') && text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        text
    };
    text.replace('-', "−")
}

#[cfg(test)]
pub(super) fn hit_project_id(
    resolved: &ResolvedFigure,
    point: egui::Pos2,
    origin: egui::Pos2,
    zoom: f32,
) -> Option<String> {
    hit_project(resolved, point, origin, zoom).map(|hit| hit.project_id)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CanvasHit {
    pub(super) project_id: String,
    pub(super) role: SelectableRole,
    pub(super) data_index: Option<usize>,
}

pub(super) fn hit_project(
    resolved: &ResolvedFigure,
    point: egui::Pos2,
    origin: egui::Pos2,
    zoom: f32,
) -> Option<CanvasHit> {
    hit_project_candidates(resolved, point, origin, zoom)
        .into_iter()
        .next()
}

pub(super) fn legend_resize_handle_at(
    pointer: egui::Pos2,
    bounds: (f64, f64, f64, f64),
    origin: egui::Pos2,
    zoom: f32,
) -> Option<ArtistDragMode> {
    let rect = egui::Rect::from_min_max(
        origin + egui::vec2(bounds.0 as f32, bounds.1 as f32) * zoom,
        origin + egui::vec2(bounds.2 as f32, bounds.3 as f32) * zoom,
    )
    .expand(4.0);
    let radius = 11.0;
    if pointer.distance(rect.right_center()) <= radius {
        Some(ArtistDragMode::ResizeColumns)
    } else if pointer.distance(rect.center_bottom()) <= radius {
        Some(ArtistDragMode::ResizeRows)
    } else {
        None
    }
}

pub(super) fn same_frame_primary_drag(events: &[egui::Event]) -> Option<(egui::Pos2, egui::Pos2)> {
    let mut press = None;
    for event in events {
        if let egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            ..
        } = event
        {
            if *pressed {
                press = Some(*pos);
            } else if let Some(start) = press
                && start.distance(*pos) > 6.0
            {
                return Some((start, *pos));
            }
        }
    }
    None
}

pub(super) fn hit_project_candidates(
    resolved: &ResolvedFigure,
    point: egui::Pos2,
    origin: egui::Pos2,
    zoom: f32,
) -> Vec<CanvasHit> {
    let local = (point - origin) / zoom;
    let mut candidates = Vec::new();
    let mut background = None;
    for hit in resolved.layout.result.hit_map.hit_candidates(
        f64::from(local.x),
        f64::from(local.y),
        f64::from(6.0 / zoom),
    ) {
        let Some(project_id) = resolved.layout.project_ids.get(&hit.node) else {
            continue;
        };
        let candidate = CanvasHit {
            project_id: project_id.clone(),
            role: hit.role,
            data_index: hit.data_index,
        };
        if hit.role == SelectableRole::Axes {
            background = Some(candidate);
        } else if !candidates
            .iter()
            .any(|existing: &CanvasHit| existing.project_id == candidate.project_id)
        {
            candidates.push(candidate);
        }
    }
    // Outside legends sit close to the top or right spine. Both hit targets
    // intentionally have a small safety expansion, so their rectangles can
    // overlap even though the rendered objects do not. In that narrow overlap
    // a precise axis/tick/label hit must win over the legend's coarse bounding
    // box; otherwise moving the legend outside makes nearby tick editors appear
    // to stop working.
    let legend_is_outside = resolved.layout.result.legend.is_some_and(|legend| {
        let axes = resolved.layout.result.axes;
        legend.bottom() <= axes.y || legend.x >= axes.right()
    });
    if legend_is_outside
        && candidates.iter().any(|candidate| {
            matches!(
                candidate.role,
                SelectableRole::Axis | SelectableRole::AxisLabel | SelectableRole::Tick
            )
        })
    {
        candidates.retain(|candidate| candidate.role != SelectableRole::Legend);
    }
    if candidates.is_empty() {
        candidates.extend(background);
    }
    candidates
}

#[cfg(test)]
pub(super) fn selected_hit_bounds(
    resolved: &ResolvedFigure,
    project_id: &str,
) -> Option<(f64, f64, f64, f64)> {
    selected_hit_bounds_for_role(resolved, project_id, None)
}

pub(super) fn selected_hit_bounds_for_role(
    resolved: &ResolvedFigure,
    project_id: &str,
    role: Option<SelectableRole>,
) -> Option<(f64, f64, f64, f64)> {
    let mut bounds = resolved
        .layout
        .result
        .hit_map
        .items
        .iter()
        .filter(|item| {
            role.is_none_or(|role| item.role == role)
                && resolved
                    .layout
                    .project_ids
                    .get(&item.node)
                    .map(String::as_str)
                    == Some(project_id)
        })
        .map(|item| item.bounds);
    let first = bounds.next()?;
    let hit_bounds = bounds.fold(
        (first.x, first.y, first.right(), first.bottom()),
        |(left, top, right, bottom), next| {
            (
                left.min(next.x),
                top.min(next.y),
                right.max(next.right()),
                bottom.max(next.bottom()),
            )
        },
    );
    if matches!(
        role,
        Some(SelectableRole::Annotation | SelectableRole::AxisLabel | SelectableRole::Tick)
    ) {
        let mut ink: Option<(f64, f64, f64, f64)> = None;
        for item in &resolved.display.items {
            let ResolvedItem::Text(text) = item else {
                continue;
            };
            if resolved
                .layout
                .project_ids
                .get(&text.source)
                .map(String::as_str)
                != Some(project_id)
            {
                continue;
            }
            let Some(text_bounds) = resolved_text_ink_bounds(text) else {
                continue;
            };
            let overlaps_hit = text_bounds.0 <= hit_bounds.2 + 2.0
                && text_bounds.2 >= hit_bounds.0 - 2.0
                && text_bounds.1 <= hit_bounds.3 + 2.0
                && text_bounds.3 >= hit_bounds.1 - 2.0;
            if overlaps_hit {
                ink = Some(match ink {
                    Some(current) => (
                        current.0.min(text_bounds.0),
                        current.1.min(text_bounds.1),
                        current.2.max(text_bounds.2),
                        current.3.max(text_bounds.3),
                    ),
                    None => text_bounds,
                });
            }
        }
        if let Some(ink) = ink {
            return Some(ink);
        }
    }
    Some(hit_bounds)
}

pub(super) fn resolved_text_ink_bounds(text: &ResolvedText) -> Option<(f64, f64, f64, f64)> {
    let angle = f64::from(text.rotation_degrees).to_radians();
    let (sin, cos) = angle.sin_cos();
    let pivot = (f64::from(text.x), f64::from(text.y));
    let mut ink: Option<(f64, f64, f64, f64)> = None;
    for run in &text.runs {
        let Ok(face) = ttf_parser::Face::parse(run.font_data.as_slice(), run.font_index) else {
            continue;
        };
        let scale = f64::from(run.font_size) / f64::from(face.units_per_em());
        let mut cursor_x = f64::from(text.x + run.start_x);
        for glyph in &run.glyphs {
            if let Some(bounds) = face.glyph_bounding_box(ttf_parser::GlyphId(glyph.id as u16)) {
                let left = cursor_x + f64::from(glyph.x_offset) + f64::from(bounds.x_min) * scale;
                let right = cursor_x + f64::from(glyph.x_offset) + f64::from(bounds.x_max) * scale;
                let baseline = f64::from(text.y + run.baseline_shift - glyph.y_offset);
                let top = baseline - f64::from(bounds.y_max) * scale;
                let bottom = baseline - f64::from(bounds.y_min) * scale;
                for (x, y) in [(left, top), (right, top), (left, bottom), (right, bottom)] {
                    let dx = x - pivot.0;
                    let dy = y - pivot.1;
                    let rx = pivot.0 + dx * cos - dy * sin;
                    let ry = pivot.1 + dx * sin + dy * cos;
                    ink = Some(match ink {
                        Some(current) => (
                            current.0.min(rx),
                            current.1.min(ry),
                            current.2.max(rx),
                            current.3.max(ry),
                        ),
                        None => (rx, ry, rx, ry),
                    });
                }
            }
            cursor_x += f64::from(glyph.advance);
        }
    }
    ink
}

pub(super) fn paint_object_path_overlay(
    painter: &egui::Painter,
    resolved: &ResolvedFigure,
    project_id: &str,
    role: Option<SelectableRole>,
    origin: egui::Pos2,
    zoom: f32,
    selected: bool,
) -> bool {
    if !matches!(
        role,
        Some(SelectableRole::Series | SelectableRole::DataPoint | SelectableRole::ErrorBar)
    ) {
        return false;
    }
    let mut painted = false;
    let figure_clip = egui::Rect::from_min_size(
        origin,
        egui::vec2(resolved.display.width, resolved.display.height) * zoom,
    );
    let mut current_clip = figure_clip;
    let mut clip_stack = Vec::new();
    let transform = |x: instplot_render::Pt, y: instplot_render::Pt| {
        origin + egui::vec2(x.get() as f32, y.get() as f32) * zoom
    };
    for item in &resolved.display.items {
        let (source, path, stroke) = match item {
            ResolvedItem::Graphics(DisplayItem::ClipPush {
                x,
                y,
                width,
                height,
                ..
            }) => {
                clip_stack.push(current_clip);
                current_clip = current_clip.intersect(egui::Rect::from_min_size(
                    transform(*x, *y),
                    egui::vec2(width.get() as f32, height.get() as f32) * zoom,
                ));
                continue;
            }
            ResolvedItem::Graphics(DisplayItem::ClipPop { .. }) => {
                current_clip = clip_stack.pop().unwrap_or(figure_clip);
                continue;
            }
            ResolvedItem::Graphics(DisplayItem::Path {
                source,
                path,
                stroke,
                ..
            }) => (source, path, stroke),
            _ => continue,
        };
        if resolved.layout.project_ids.get(source).map(String::as_str) != Some(project_id) {
            continue;
        }
        let plot_painter = painter.with_clip_rect(current_clip);
        let mut subpaths = Vec::new();
        let mut points = Vec::new();
        for verb in &path.verbs {
            match *verb {
                PathVerb::MoveTo(x, y) => {
                    if !points.is_empty() {
                        subpaths.push(std::mem::take(&mut points));
                    }
                    points.push(transform(x, y));
                }
                PathVerb::LineTo(x, y) => points.push(transform(x, y)),
                PathVerb::CurveTo(x1, y1, x2, y2, x3, y3) => {
                    if let Some(start) = points.last().copied() {
                        let c1 = transform(x1, y1);
                        let c2 = transform(x2, y2);
                        let end = transform(x3, y3);
                        for step in 1..=12 {
                            points.push(cubic_overlay_point(
                                start,
                                c1,
                                c2,
                                end,
                                step as f32 / 12.0,
                            ));
                        }
                    }
                }
                PathVerb::Close => {
                    if let Some(first) = points.first().copied() {
                        points.push(first);
                    }
                }
            }
        }
        if !points.is_empty() {
            subpaths.push(points);
        }
        for points in subpaths {
            if points.len() < 2 {
                continue;
            }
            let glow = if selected { 8.0 } else { 4.0 };
            let alpha = if selected { 105 } else { 28 };
            let dash = stroke
                .as_ref()
                .map(|stroke| {
                    stroke
                        .dash
                        .iter()
                        .map(|length| length.get() as f32 * zoom)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let paint_overlay = |overlay_stroke| {
                if dash.is_empty() {
                    plot_painter.add(egui::Shape::line(points.clone(), overlay_stroke));
                } else {
                    let dashes = dash.iter().step_by(2).copied().collect::<Vec<_>>();
                    let gaps = dash.iter().skip(1).step_by(2).copied().collect::<Vec<_>>();
                    for shape in egui::Shape::dashed_line_with_offset(
                        &points,
                        overlay_stroke,
                        &dashes,
                        &gaps,
                        0.0,
                    ) {
                        plot_painter.add(shape);
                    }
                }
            };
            paint_overlay(egui::Stroke::new(
                glow,
                egui::Color32::from_rgba_unmultiplied(56, 145, 225, alpha),
            ));
            painted = true;
        }
    }
    painted
}

pub(super) fn cubic_overlay_point(
    start: egui::Pos2,
    c1: egui::Pos2,
    c2: egui::Pos2,
    end: egui::Pos2,
    t: f32,
) -> egui::Pos2 {
    let u = 1.0 - t;
    let point = start.to_vec2() * (u * u * u)
        + c1.to_vec2() * (3.0 * u * u * t)
        + c2.to_vec2() * (3.0 * u * t * t)
        + end.to_vec2() * (t * t * t);
    egui::pos2(point.x, point.y)
}

pub(super) fn zoom_about_pointer(
    scroll: egui::Vec2,
    pointer: egui::Pos2,
    origin: egui::Pos2,
    old_zoom: f32,
    wheel: f32,
) -> (egui::Vec2, f32) {
    let factor = (wheel * 0.002).exp();
    let next_zoom = (old_zoom * factor).clamp(0.1, 8.0);
    let document_point = (pointer - origin) / old_zoom;
    (scroll + document_point * (next_zoom - old_zoom), next_zoom)
}

/// Trackpad scrolling has a start/end phase on the native backend. A plain
/// mouse wheel normally only emits Move. Keep the gesture state across frames
/// so two-finger motion pans instead of masquerading as wheel zoom.
pub(super) fn trackpad_scroll_state(events: &[egui::Event], was_active: bool) -> (bool, bool) {
    let mut active = was_active;
    let mut trackpad_this_frame = was_active;
    for event in events {
        if let egui::Event::MouseWheel { phase, .. } = event {
            match phase {
                egui::TouchPhase::Start => {
                    active = true;
                    trackpad_this_frame = true;
                }
                egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                    trackpad_this_frame |= active;
                    active = false;
                }
                egui::TouchPhase::Move => {
                    trackpad_this_frame |= active;
                }
            }
        }
    }
    (trackpad_this_frame, active)
}

pub(super) fn series_kind_name(language: UiLanguage, kind: SeriesKind) -> &'static str {
    match kind {
        SeriesKind::Line => language.text(Text::Line),
        SeriesKind::Scatter => language.text(Text::Scatter),
        SeriesKind::ErrorBar => language.text(Text::ErrorBar),
        SeriesKind::ReferenceLine => language.text(Text::ReferenceLine),
        SeriesKind::Annotation => language.text(Text::Annotation),
        SeriesKind::Legend => language.text(Text::Legend),
    }
}

pub(super) fn canvas_series_title(language: UiLanguage, series: &SeriesDescriptor) -> String {
    let kind = series_kind_name(language, series.kind);
    if matches!(series.kind, SeriesKind::Legend | SeriesKind::Annotation) {
        kind.to_owned()
    } else {
        format!("{kind} · {}", series.label)
    }
}

pub(super) fn canvas_logical_series_title(
    language: UiLanguage,
    document: &FigureDocument,
    series: &SeriesDescriptor,
) -> String {
    if matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter)
        && let Some(style) = document.series_style(&series.id)
    {
        return format!("{} · {}", series_style_name(language, style), series.label);
    }
    canvas_series_title(language, series)
}

pub(super) fn fixed_ticks_text(record: &AxisRecord) -> String {
    match &record.locator {
        LocatorSpec::Fixed { values } => values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        LocatorSpec::Auto { .. } | LocatorSpec::Interval { .. } => String::new(),
    }
}

pub(super) fn parse_fixed_ticks(value: &str) -> Result<Vec<f64>, String> {
    let parsed = value
        .split([',', ';', ' ', '\t'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>()
                .map_err(|_| format!("invalid fixed tick value: {part}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parsed.is_empty() || parsed.iter().any(|value| !value.is_finite()) {
        return Err("fixed ticks require one or more finite values".to_owned());
    }
    Ok(parsed)
}

pub(super) fn column_combo(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    selected: &mut String,
    dataset: &DataSet,
) {
    egui::ComboBox::from_id_salt(id)
        .width((ui.available_width() - 8.0).max(54.0))
        .selected_text(format!("{label} · {selected}"))
        .show_ui(ui, |ui| {
            for column in &dataset.columns {
                ui.selectable_value(selected, column.name.clone(), &column.name);
            }
        });
}

pub(super) fn optional_column_combo(
    ui: &mut egui::Ui,
    id: &str,
    selected: &mut Option<String>,
    dataset: &DataSet,
    excluded: [&str; 2],
) {
    egui::ComboBox::from_id_salt(id)
        .width((ui.available_width() - 8.0).max(54.0))
        .selected_text(selected.as_deref().unwrap_or("无"))
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "无");
            for column in &dataset.columns {
                if excluded.contains(&column.name.as_str()) {
                    continue;
                }
                ui.selectable_value(selected, Some(column.name.clone()), &column.name);
            }
        });
}

pub(super) struct DataNavigationItem {
    pub(super) id: String,
    pub(super) kind: DataSetKind,
    pub(super) rows: usize,
    pub(super) columns: usize,
}

pub(super) struct DataNavigationGroup {
    pub(super) key: String,
    pub(super) title: String,
    pub(super) path: String,
    pub(super) items: Vec<DataNavigationItem>,
}

pub(super) fn data_navigation_groups(
    datasets: &[DataSet],
    language: UiLanguage,
) -> Vec<DataNavigationGroup> {
    let mut groups: Vec<DataNavigationGroup> = Vec::new();
    let mut group_index: BTreeMap<String, usize> = BTreeMap::new();
    for dataset in datasets {
        let path = dataset.source.to_string_lossy().into_owned();
        let embedded = path.starts_with("embedded://");
        let key = if embedded {
            "embedded://".to_owned()
        } else {
            path.clone()
        };
        let file_name = dataset
            .source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let group_title = if embedded {
            language.text(Text::EmbeddedData).to_owned()
        } else {
            file_name.to_owned()
        };
        let item = DataNavigationItem {
            id: dataset.plot_id.clone(),
            kind: dataset.kind,
            rows: dataset.row_count,
            columns: dataset.columns.len(),
        };
        if let Some(&index) = group_index.get(&key) {
            groups[index].items.push(item);
        } else {
            group_index.insert(key.clone(), groups.len());
            groups.push(DataNavigationGroup {
                key,
                title: group_title,
                path: if embedded {
                    language.text(Text::EmbeddedData).to_owned()
                } else {
                    path
                },
                items: vec![item],
            });
        }
    }
    // Equal basenames are common when importing runs from separate folders.
    // Keep the short filename normally, adding context only when it is needed.
    let titles = groups
        .iter()
        .map(|group| group.title.clone())
        .collect::<Vec<_>>();
    for group in &mut groups {
        if group.key == "embedded://"
            || titles.iter().filter(|title| **title == group.title).count() < 2
        {
            continue;
        }
        let parent = Path::new(&group.path)
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        group.title = format!("{} · {parent}", group.title);
    }
    let disambiguated = groups
        .iter()
        .map(|group| group.title.clone())
        .collect::<Vec<_>>();
    for group in &mut groups {
        if disambiguated
            .iter()
            .filter(|title| **title == group.title)
            .count()
            > 1
        {
            group.title = group.path.clone();
        }
    }
    groups
}
