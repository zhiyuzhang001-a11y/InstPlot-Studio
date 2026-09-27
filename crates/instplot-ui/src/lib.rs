mod design;
mod shell;
mod window;

pub use design::{
    InterfaceMetrics, card_frame, close_button_sized, configure_interface_style, dialog_heading,
    file_card_frame, label_format_button, side_frame, surface, symbol_button,
};
pub use shell::{AppServices, Branding, FeatureSet, ShellEvent};
pub use window::{ToolWindowMode, ToolWindowPolicy, ToolWindowSpec, viewport_close_requested};

use eframe::egui::{self, Color32, Pos2, Rect, Stroke as EguiStroke, Vec2};
#[cfg(feature = "publication-stack")]
use instplot_export::{ResolvedDisplayList, ResolvedItem, ResolvedRun, ResolvedText};
use instplot_render::{Color, DisplayItem, DisplayList, PathVerb, Pt};

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
                let dash = stroke
                    .as_ref()
                    .map(|value| {
                        value
                            .dash
                            .iter()
                            .map(|length| length.get() as f32 * transform.zoom)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
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
                    } else if !dash.is_empty() {
                        let dashes = dash.iter().step_by(2).copied().collect::<Vec<_>>();
                        let gaps = dash.iter().skip(1).step_by(2).copied().collect::<Vec<_>>();
                        for shape in egui::Shape::dashed_line_with_offset(
                            &points, stroke, &dashes, &gaps, 0.0,
                        ) {
                            current.add(shape);
                        }
                    } else {
                        current.add(egui::epaint::PathShape::line(points, stroke));
                    }
                }
            }
            DisplayItem::GlyphRun(run) => {
                let text = run.label.normalized_text();
                let anchor = match run.anchor {
                    instplot_render::TextAnchor::Start => egui::Align2::LEFT_BOTTOM,
                    instplot_render::TextAnchor::Middle => egui::Align2::CENTER_BOTTOM,
                    instplot_render::TextAnchor::End => egui::Align2::RIGHT_BOTTOM,
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
                clips.push(current.clone());
                current = current.with_clip_rect(Rect::from_min_size(
                    transform.position(*x, *y),
                    transform.logical_size(*width, *height),
                ));
            }
            DisplayItem::ClipPop { .. } => {
                if let Some(previous) = clips.pop() {
                    current = previous;
                }
            }
        }
    }
}

#[cfg(feature = "publication-stack")]
pub fn install_publication_fonts(context: &egui::Context) {
    use std::sync::Arc;

    let mut definitions = egui::FontDefinitions::default();
    for face in instplot_export::bundled_font_faces() {
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
    let relation = instplot_export::bundled_relation_face();
    definitions.font_data.insert(
        relation.postscript_name.to_owned(),
        Arc::new(egui::FontData::from_static(relation.data)),
    );
    definitions.families.insert(
        egui::FontFamily::Name(Arc::from(relation.postscript_name)),
        vec![relation.postscript_name.to_owned()],
    );
    // Use one system UI face for both CJK and Latin text. Mixing egui's default Latin face with
    // a CJK-only fallback gives the glyphs different vertical metrics within the same control.
    // Publication faces remain named and isolated, so figure exports and package size are unchanged.
    if let Some((name, bytes, y_offset_factor)) = system_cjk_ui_font() {
        definitions.font_data.insert(
            name.clone(),
            Arc::new(egui::FontData::from_owned(bytes).tweak(egui::FontTweak {
                y_offset_factor,
                ..Default::default()
            })),
        );
        definitions
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, name);
    }
    // Keep the native UI face for ordinary controls, but make every publication-core symbol
    // visible in text fields even when the system UI font lacks that glyph.
    definitions
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("TeXGyreHeros-Regular".to_owned());
    context.set_fonts(definitions);
}

#[cfg(feature = "publication-stack")]
fn system_cjk_ui_font() -> Option<(String, Vec<u8>, f32)> {
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
        std::fs::read(path).ok().map(|bytes| {
            // On macOS, the installed Hiragino face has 24 pt lines at 16 pt text,
            // but its visible ink is centered around 8 pt rather than 12 pt.
            // Keep unmeasured system fallbacks untweaked.
            let y_offset_factor = if path.ends_with("Hiragino Sans GB.ttc") {
                0.25
            } else {
                0.0
            };
            ("instplot-ui-cjk".to_owned(), bytes, y_offset_factor)
        })
    })
}

#[cfg(feature = "publication-stack")]
pub fn paint_resolved_display_list(
    painter: &egui::Painter,
    list: &ResolvedDisplayList,
    transform: ScreenTransform,
) {
    paint_resolved_display_list_impl(painter, list, transform, true);
}

#[cfg(feature = "publication-stack")]
pub fn paint_resolved_graphics_only(
    painter: &egui::Painter,
    list: &ResolvedDisplayList,
    transform: ScreenTransform,
) {
    paint_resolved_display_list_impl(painter, list, transform, false);
}

#[cfg(feature = "publication-stack")]
fn paint_resolved_display_list_impl(
    painter: &egui::Painter,
    list: &ResolvedDisplayList,
    transform: ScreenTransform,
    paint_text: bool,
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
                let dash = stroke
                    .as_ref()
                    .map(|value| {
                        value
                            .dash
                            .iter()
                            .map(|length| length.get() as f32 * transform.zoom)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
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
                    } else if !dash.is_empty() {
                        let dashes = dash.iter().step_by(2).copied().collect::<Vec<_>>();
                        let gaps = dash.iter().skip(1).step_by(2).copied().collect::<Vec<_>>();
                        for shape in egui::Shape::dashed_line_with_offset(
                            &points, stroke, &dashes, &gaps, 0.0,
                        ) {
                            current.add(shape);
                        }
                    } else {
                        current.add(egui::epaint::PathShape::line(points, stroke));
                    }
                }
            }
            ResolvedItem::Text(text) if paint_text => {
                paint_resolved_text(&current, text, transform);
            }
            ResolvedItem::Text(_) => {}
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
                clips.push(current.clone());
                current = current.with_clip_rect(Rect::from_min_size(
                    transform.position(*x, *y),
                    transform.logical_size(*width, *height),
                ));
            }
            ResolvedItem::Graphics(DisplayItem::ClipPop { .. }) => {
                if let Some(previous) = clips.pop() {
                    current = previous;
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
    fn graphics_after_axis_clip_are_visible_for_outside_legends() {
        use instplot_render::{Fill, FillRule, NodeId, Path};

        let pt = |value| Pt::new(value).unwrap();
        let display = DisplayList {
            width: pt(100.0),
            height: pt(100.0),
            items: vec![
                DisplayItem::ClipPush {
                    source: NodeId(1),
                    x: pt(10.0),
                    y: pt(10.0),
                    width: pt(20.0),
                    height: pt(20.0),
                },
                DisplayItem::ClipPop { source: NodeId(1) },
                DisplayItem::Path {
                    source: NodeId(2),
                    path: Path {
                        verbs: vec![
                            PathVerb::MoveTo(pt(70.0), pt(70.0)),
                            PathVerb::LineTo(pt(75.0), pt(70.0)),
                            PathVerb::LineTo(pt(75.0), pt(75.0)),
                            PathVerb::Close,
                        ],
                    },
                    fill: Some(Fill {
                        color: Color(255, 0, 0, 255),
                        rule: FillRule::NonZero,
                    }),
                    stroke: None,
                },
            ],
        };
        let resolved = instplot_export::resolve(&display);
        let context = egui::Context::default();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            paint_resolved_graphics_only(
                ui.painter(),
                &resolved,
                ScreenTransform::new(Pos2::ZERO, 1.0, 1.0),
            );
        });
        assert!(output.shapes.iter().any(|clipped| {
            matches!(&clipped.shape, egui::Shape::Path(path)
                if path.points.iter().any(|point| point.x == 70.0 && point.y == 70.0)
                    && clipped.clip_rect.contains(Pos2::new(72.0, 72.0)))
        }));
        output.textures_delta.clear();
        let mut plain = context.run_ui(egui::RawInput::default(), |ui| {
            paint_display_list(
                ui.painter(),
                &display,
                ScreenTransform::new(Pos2::ZERO, 1.0, 1.0),
            );
        });
        assert!(plain.shapes.iter().any(|clipped| {
            matches!(&clipped.shape, egui::Shape::Path(path)
                if path.points.iter().any(|point| point.x == 70.0 && point.y == 70.0)
                    && clipped.clip_rect.contains(Pos2::new(72.0, 72.0)))
        }));
        plain.textures_delta.clear();
    }

    #[cfg(feature = "publication-stack")]
    #[test]
    fn resolved_preview_retains_publication_faces_and_rotation() {
        use instplot_render::{NodeId, compile, fixed_figure};

        let display = compile(&fixed_figure()).unwrap();
        let resolved = instplot_export::resolve(&display);
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
    fn ui_font_keeps_latin_and_hanzi_on_one_face() {
        let Some((_, _, y_offset_factor)) = system_cjk_ui_font() else {
            return;
        };
        let context = egui::Context::default();
        install_publication_fonts(&context);
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            for size in [14.0, 15.0, 16.0, 22.0] {
                let galley = ui.ctx().fonts_mut(|fonts| {
                    fonts.layout_no_wrap(
                        "图形尺寸 Width (mm) 85.0".to_owned(),
                        egui::FontId::proportional(size),
                        egui::Color32::WHITE,
                    )
                });
                let glyphs = &galley.rows[0].glyphs;
                let hanzi = glyphs.iter().find(|glyph| glyph.chr == '图').unwrap();
                let latin = glyphs.iter().find(|glyph| glyph.chr == 'W').unwrap();
                assert!((hanzi.font_face_ascent - latin.font_face_ascent).abs() < 0.01);
                assert!((hanzi.font_face_height - latin.font_face_height).abs() < 0.01);
                if y_offset_factor > 0.0 {
                    for glyph in [hanzi, latin] {
                        let ink_center =
                            glyph.pos.y + glyph.uv_rect.offset.y + glyph.uv_rect.size.y / 2.0;
                        assert!((ink_center - glyph.line_height / 2.0).abs() <= 1.0);
                    }
                }
            }
        });
        output.textures_delta.clear();
    }

    #[cfg(feature = "publication-stack")]
    #[test]
    fn resolved_preview_emits_a_rotated_egui_text_shape() {
        let context = egui::Context::default();
        install_publication_fonts(&context);
        let display = instplot_export::resolve(
            &instplot_render::compile(&instplot_render::fixed_figure()).unwrap(),
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
