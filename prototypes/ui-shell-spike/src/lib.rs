use eframe::egui::{self, Color32, Pos2, Rect, Stroke as EguiStroke, Vec2};
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
}
