use eframe::egui::{self, Color32, Pos2, Rect, Stroke as EguiStroke, Vec2};
#[cfg(feature = "publication-stack")]
use export_backend_spike::{ResolvedDisplayList, ResolvedItem, ResolvedRun, ResolvedText};
use studio_render_spike::{Color, DisplayItem, DisplayList, PathVerb, Pt};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenTransform {
    pub origin: Pos2,
    pub zoom: f32,
    pub pixels_per_point: f32,
}

impl ScreenTransform {
    pub fn new(origin: Pos2, zoom: f32, pixels_per_point: f32) -> Self {
        Self {
            origin,
            zoom,
            pixels_per_point,
        }
    }

    pub fn position(self, x: Pt, y: Pt) -> Pos2 {
        self.origin + Vec2::new(x.get() as f32, y.get() as f32) * self.zoom
    }

    pub fn logical_size(self, width: Pt, height: Pt) -> Vec2 {
        Vec2::new(width.get() as f32, height.get() as f32) * self.zoom
    }

    pub fn framebuffer_size(self, width: Pt, height: Pt) -> [u32; 2] {
        let logical = self.logical_size(width, height);
        [
            (logical.x * self.pixels_per_point).round() as u32,
            (logical.y * self.pixels_per_point).round() as u32,
        ]
    }
}

pub fn paint_display_list(painter: &egui::Painter, list: &DisplayList, transform: ScreenTransform) {
    let mut current = painter.clone();
    let mut clips = Vec::new();
    for item in &list.items {
        match item {
            DisplayItem::Path {
                path, fill, stroke, ..
            } => {
                let mut subpaths: Vec<Vec<Pos2>> = Vec::new();
                let mut points = Vec::new();
                let mut cursor = None;
                for verb in &path.verbs {
                    match *verb {
                        PathVerb::MoveTo(x, y) => {
                            if !points.is_empty() {
                                subpaths.push(std::mem::take(&mut points));
                            }
                            let point = transform.position(x, y);
                            points.push(point);
                            cursor = Some(point);
                        }
                        PathVerb::LineTo(x, y) => {
                            let point = transform.position(x, y);
                            points.push(point);
                            cursor = Some(point);
                        }
                        PathVerb::CurveTo(x1, y1, x2, y2, x3, y3) => {
                            if let Some(start) = cursor {
                                let c1 = transform.position(x1, y1);
                                let c2 = transform.position(x2, y2);
                                let end = transform.position(x3, y3);
                                for step in 1..=12 {
                                    let t = step as f32 / 12.0;
                                    points.push(cubic(start, c1, c2, end, t));
                                }
                                cursor = Some(end);
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
                let stroke = stroke
                    .as_ref()
                    .map(|value| {
                        EguiStroke::new(
                            value.width.get() as f32 * transform.zoom,
                            color32(value.color),
                        )
                    })
                    .unwrap_or(EguiStroke::NONE);
                for points in subpaths.into_iter().filter(|points| points.len() >= 2) {
                    if let Some(fill) = fill {
                        current.add(egui::epaint::PathShape::convex_polygon(
                            points,
                            color32(fill.color),
                            stroke,
                        ));
                    } else {
                        current.add(egui::epaint::PathShape::line(points, stroke));
                    }
                }
            }
            DisplayItem::GlyphRun(run) => {
                let text = run.label.normalized_text();
                let anchor = match run.anchor {
                    studio_render_spike::TextAnchor::Start => egui::Align2::LEFT_BOTTOM,
                    studio_render_spike::TextAnchor::Middle => egui::Align2::CENTER_BOTTOM,
                    studio_render_spike::TextAnchor::End => egui::Align2::RIGHT_BOTTOM,
                };
                current.text(
                    transform.position(run.x, run.y),
                    anchor,
                    text,
                    egui::FontId::proportional(run.size.get() as f32 * transform.zoom),
                    color32(run.color),
                );
            }
            DisplayItem::Image(image) => {
                let min = transform.position(image.x, image.y);
                current.rect_stroke(
                    Rect::from_min_size(min, transform.logical_size(image.width, image.height)),
                    0.0,
                    EguiStroke::new(1.0, Color32::LIGHT_GRAY),
                    egui::StrokeKind::Inside,
                );
            }
            DisplayItem::ClipPush {
                x,
                y,
                width,
                height,
                ..
            } => {
                clips.push(current.clip_rect());
                current = current.with_clip_rect(Rect::from_min_size(
                    transform.position(*x, *y),
                    transform.logical_size(*width, *height),
                ));
            }
            DisplayItem::ClipPop { .. } => {
                if let Some(rect) = clips.pop() {
                    current = current.with_clip_rect(rect);
                }
            }
        }
    }
}

#[cfg(feature = "publication-stack")]
pub fn install_publication_fonts(context: &egui::Context) {
    use std::sync::Arc;

    let mut definitions = egui::FontDefinitions::default();
    for face in export_backend_spike::bundled_font_faces() {
        let name = face.postscript_name.to_owned();
        definitions.font_data.insert(
            name.clone(),
            Arc::new(egui::FontData::from_static(face.data)),
        );
        definitions.families.insert(
            egui::FontFamily::Name(Arc::from(face.postscript_name)),
            vec![name],
        );
    }
    // UI localization uses an installed system font as a runtime fallback. It is deliberately
    // not bundled or used by the named publication families, so exported figure typography and
    // application size remain unchanged.
    if let Some((name, bytes)) = system_cjk_ui_font() {
        definitions
            .font_data
            .insert(name.clone(), Arc::new(egui::FontData::from_owned(bytes)));
        definitions
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push(name);
    }
    context.set_fonts(definitions);
}

#[cfg(feature = "publication-stack")]
fn system_cjk_ui_font() -> Option<(String, Vec<u8>)> {
    #[cfg(target_os = "macos")]
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    ];
    #[cfg(target_os = "windows")]
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];
    #[cfg(target_os = "linux")]
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    ];
    CANDIDATES.iter().find_map(|path| {
        std::fs::read(path)
            .ok()
            .map(|bytes| ("instplot-ui-cjk".to_owned(), bytes))
    })
}

#[cfg(feature = "publication-stack")]
pub fn paint_resolved_display_list(
    painter: &egui::Painter,
    list: &ResolvedDisplayList,
    transform: ScreenTransform,
) {
    let mut current = painter.clone();
    let mut clips = Vec::new();
    for item in &list.items {
        match item {
            ResolvedItem::Graphics(DisplayItem::Path {
                path, fill, stroke, ..
            }) => {
                let mut subpaths: Vec<Vec<Pos2>> = Vec::new();
                let mut points = Vec::new();
                let mut cursor = None;
                for verb in &path.verbs {
                    match *verb {
                        PathVerb::MoveTo(x, y) => {
                            if !points.is_empty() {
                                subpaths.push(std::mem::take(&mut points));
                            }
                            let point = transform.position(x, y);
                            points.push(point);
                            cursor = Some(point);
                        }
                        PathVerb::LineTo(x, y) => {
                            let point = transform.position(x, y);
                            points.push(point);
                            cursor = Some(point);
                        }
                        PathVerb::CurveTo(x1, y1, x2, y2, x3, y3) => {
                            if let Some(start) = cursor {
                                let c1 = transform.position(x1, y1);
                                let c2 = transform.position(x2, y2);
                                let end = transform.position(x3, y3);
                                for step in 1..=12 {
                                    let t = step as f32 / 12.0;
                                    points.push(cubic(start, c1, c2, end, t));
                                }
                                cursor = Some(end);
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
                let stroke = stroke
                    .as_ref()
                    .map(|value| {
                        EguiStroke::new(
                            value.width.get() as f32 * transform.zoom,
                            color32(value.color),
                        )
                    })
                    .unwrap_or(EguiStroke::NONE);
                for points in subpaths.into_iter().filter(|points| points.len() >= 2) {
                    if let Some(fill) = fill {
                        current.add(egui::epaint::PathShape::convex_polygon(
                            points,
                            color32(fill.color),
                            stroke,
                        ));
                    } else {
                        current.add(egui::epaint::PathShape::line(points, stroke));
                    }
                }
            }
            ResolvedItem::Text(text) => paint_resolved_text(&current, text, transform),
            ResolvedItem::Graphics(DisplayItem::Image(image)) => {
                let min = transform.position(image.x, image.y);
                current.rect_stroke(
                    Rect::from_min_size(min, transform.logical_size(image.width, image.height)),
                    0.0,
                    EguiStroke::new(1.0, Color32::LIGHT_GRAY),
                    egui::StrokeKind::Inside,
                );
            }
            ResolvedItem::Graphics(DisplayItem::ClipPush {
                x,
                y,
                width,
                height,
                ..
            }) => {
                clips.push(current.clip_rect());
                current = current.with_clip_rect(Rect::from_min_size(
                    transform.position(*x, *y),
                    transform.logical_size(*width, *height),
                ));
            }
            ResolvedItem::Graphics(DisplayItem::ClipPop { .. }) => {
                if let Some(rect) = clips.pop() {
                    current = current.with_clip_rect(rect);
                }
            }
            ResolvedItem::Graphics(DisplayItem::GlyphRun(_)) => {
                unreachable!("text is resolved before preview dispatch")
            }
        }
    }
}

#[cfg(feature = "publication-stack")]
fn paint_resolved_text(painter: &egui::Painter, text: &ResolvedText, transform: ScreenTransform) {
    use std::sync::Arc;

    let color = color32(text.color);
    for run in &text.runs {
        let Some(value) = resolved_run_text(text, run) else {
            continue;
        };
        let family = egui::FontFamily::Name(Arc::from(run.font.postscript_name.as_str()));
        let font = egui::FontId::new(run.font_size * transform.zoom, family);
        let galley = painter.layout_no_wrap(value.to_owned(), font, color);
        let local = Pos2::new(
            text.x + run.start_x,
            text.y + run.baseline_shift - galley.size().y / transform.zoom,
        );
        let rotated = rotate_about(
            local,
            Pos2::new(text.x, text.y),
            text.rotation_degrees.to_radians(),
        );
        let position = transform.origin + rotated.to_vec2() * transform.zoom;
        painter.add(
            egui::epaint::TextShape::new(position, galley, color)
                .with_angle(text.rotation_degrees.to_radians()),
        );
    }
}

#[cfg(feature = "publication-stack")]
fn resolved_run_text<'a>(text: &'a ResolvedText, run: &ResolvedRun) -> Option<&'a str> {
    let start = run
        .glyphs
        .iter()
        .map(|glyph| glyph.text_range.start)
        .min()?;
    let end = run.glyphs.iter().map(|glyph| glyph.text_range.end).max()?;
    text.text.get(start..end)
}

#[cfg(feature = "publication-stack")]
fn rotate_about(point: Pos2, pivot: Pos2, angle: f32) -> Pos2 {
    let offset = point - pivot;
    let (sin, cos) = angle.sin_cos();
    pivot
        + Vec2::new(
            offset.x * cos - offset.y * sin,
            offset.x * sin + offset.y * cos,
        )
}

fn color32(Color(red, green, blue, alpha): Color) -> Color32 {
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn cubic(start: Pos2, c1: Pos2, c2: Pos2, end: Pos2, t: f32) -> Pos2 {
    let mt = 1.0 - t;
    let weights = [mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t];
    Pos2::new(
        start.x * weights[0] + c1.x * weights[1] + c2.x * weights[2] + end.x * weights[3],
        start.y * weights[0] + c1.y * weights[1] + c2.y * weights[2] + end.y * weights[3],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidpi_changes_framebuffer_not_figure_layout() {
        let width = Pt::new(252.0).unwrap();
        let height = Pt::new(184.0).unwrap();
        let normal = ScreenTransform::new(Pos2::ZERO, 1.25, 1.0);
        let retina = ScreenTransform::new(Pos2::ZERO, 1.25, 2.0);
        assert_eq!(
            normal.logical_size(width, height),
            retina.logical_size(width, height)
        );
        assert_eq!(normal.framebuffer_size(width, height), [315, 230]);
        assert_eq!(retina.framebuffer_size(width, height), [630, 460]);
        assert_eq!(
            normal.position(width, height),
            retina.position(width, height)
        );
    }

    #[cfg(feature = "publication-stack")]
    #[test]
    fn resolved_preview_retains_publication_faces_and_rotation() {
        use studio_render_spike::{NodeId, compile, fixed_figure};

        let display = compile(&fixed_figure()).unwrap();
        let resolved = export_backend_spike::resolve(&display);
        let x = resolved
            .items
            .iter()
            .find_map(|item| match item {
                ResolvedItem::Text(text) if text.source == NodeId(3) => Some(text),
                _ => None,
            })
            .unwrap();
        let y = resolved
            .items
            .iter()
            .find_map(|item| match item {
                ResolvedItem::Text(text) if text.source == NodeId(4) => Some(text),
                _ => None,
            })
            .unwrap();

        assert_eq!(x.rotation_degrees, 0.0);
        assert_eq!(y.rotation_degrees, -90.0);
        assert!(x.runs.iter().all(|run| {
            run.font.postscript_name.starts_with("TeXGyreHeros-")
                && resolved_run_text(x, run).is_some_and(|value| !value.is_empty())
        }));
        assert!(
            x.runs
                .iter()
                .any(|run| run.font.postscript_name == "TeXGyreHeros-Italic")
        );
        assert!(x.runs.iter().any(|run| run.baseline_shift != 0.0));
    }

    #[cfg(feature = "publication-stack")]
    #[test]
    fn preview_rotation_uses_the_display_list_pivot() {
        let point = Pos2::new(12.0, 10.0);
        let pivot = Pos2::new(10.0, 10.0);
        let rotated = rotate_about(point, pivot, 90.0_f32.to_radians());
        assert!((rotated.x - 10.0).abs() < 1e-6);
        assert!((rotated.y - 12.0).abs() < 1e-6);
    }

    #[cfg(feature = "publication-stack")]
    #[test]
    fn resolved_preview_emits_a_rotated_egui_text_shape() {
        let context = egui::Context::default();
        install_publication_fonts(&context);
        let display = export_backend_spike::resolve(
            &studio_render_spike::compile(&studio_render_spike::fixed_figure()).unwrap(),
        );
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            paint_resolved_display_list(
                ui.painter(),
                &display,
                ScreenTransform::new(Pos2::ZERO, 1.0, 1.0),
            );
        });
        let has_rotated_text = output.shapes.iter().any(|clipped| {
            matches!(
                &clipped.shape,
                egui::Shape::Text(shape)
                    if (shape.angle + core::f32::consts::FRAC_PI_2).abs() < 1e-6
            )
        });
        output.textures_delta.clear();
        assert!(has_rotated_text);
    }
}
