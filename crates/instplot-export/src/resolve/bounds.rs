use super::*;

pub(super) fn visible_ink_bounds(
    items: &[ResolvedItem],
    resources: &BTreeMap<String, RasterAsset>,
) -> Option<ResolvedBounds> {
    let mut union = None::<ResolvedBounds>;
    let mut clips = Vec::<ResolvedBounds>::new();
    for item in items {
        match item {
            ResolvedItem::Graphics(DisplayItem::ClipPush {
                x,
                y,
                width,
                height,
                ..
            }) => clips.push(ResolvedBounds {
                min_x: x.get() as f32,
                min_y: y.get() as f32,
                max_x: (x.get() + width.get()) as f32,
                max_y: (y.get() + height.get()) as f32,
            }),
            ResolvedItem::Graphics(DisplayItem::ClipPop { .. }) => {
                clips.pop();
            }
            _ => {
                let Some(mut bounds) = item_bounds(item, resources) else {
                    continue;
                };
                for clip in &clips {
                    let Some(clipped) = bounds.intersect(*clip) else {
                        bounds = ResolvedBounds::default();
                        break;
                    };
                    bounds = clipped;
                }
                if bounds.width() > 0.0 || bounds.height() > 0.0 {
                    union = Some(union.map_or(bounds, |current| current.union(bounds)));
                }
            }
        }
    }
    union
}

fn item_bounds(
    item: &ResolvedItem,
    resources: &BTreeMap<String, RasterAsset>,
) -> Option<ResolvedBounds> {
    match item {
        ResolvedItem::Graphics(DisplayItem::Path {
            path, fill, stroke, ..
        }) => {
            let visible_fill = fill.is_some_and(|value| value.color.3 > 0);
            let visible_stroke = stroke.as_ref().is_some_and(|value| value.color.3 > 0);
            if !visible_fill && !visible_stroke {
                return None;
            }
            let fill_bounds = visible_fill.then(|| path_geometric_bounds(path)).flatten();
            let stroke_bounds = stroke
                .as_ref()
                .filter(|_| visible_stroke)
                .and_then(|stroke| stroked_path_bounds(path, stroke));
            match (fill_bounds, stroke_bounds) {
                (Some(fill), Some(stroke)) => Some(fill.union(stroke)),
                (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
                (None, None) => None,
            }
        }
        ResolvedItem::Graphics(DisplayItem::Image(image))
            if resources
                .get(&image.resource_id)
                .is_some_and(raster_asset_has_visible_pixels) =>
        {
            Some(ResolvedBounds {
                min_x: image.x.get() as f32,
                min_y: image.y.get() as f32,
                max_x: (image.x.get() + image.width.get()) as f32,
                max_y: (image.y.get() + image.height.get()) as f32,
            })
        }
        ResolvedItem::Graphics(DisplayItem::Image(_)) => None,
        ResolvedItem::Text(text) if text.color.3 == 0 => None,
        ResolvedItem::Text(text) => text.ink_bounds.or_else(|| {
            let mut local = None::<ResolvedBounds>;
            for run in &text.runs {
                let mut cursor = run.start_x;
                for glyph in &run.glyphs {
                    let left = cursor + glyph.x_offset;
                    let baseline = run.baseline_shift - glyph.y_offset;
                    let glyph_bounds = ResolvedBounds {
                        min_x: left,
                        min_y: baseline - run.font_size,
                        max_x: left + glyph.advance.max(run.font_size * 0.2),
                        max_y: baseline + run.font_size * 0.3,
                    };
                    local = Some(local.map_or(glyph_bounds, |bounds| bounds.union(glyph_bounds)));
                    cursor += glyph.advance;
                }
            }
            let local = local.unwrap_or(ResolvedBounds {
                min_x: 0.0,
                min_y: -text.size,
                max_x: text.size * 0.5,
                max_y: text.size * 0.3,
            });
            let corners = [
                (local.min_x, local.min_y),
                (local.max_x, local.min_y),
                (local.max_x, local.max_y),
                (local.min_x, local.max_y),
            ];
            let radians = text.rotation_degrees.to_radians();
            let (sin, cos) = radians.sin_cos();
            let mut bounds = ResolvedBounds {
                min_x: f32::INFINITY,
                min_y: f32::INFINITY,
                max_x: f32::NEG_INFINITY,
                max_y: f32::NEG_INFINITY,
            };
            for (x, y) in corners {
                let px = text.x + x * cos - y * sin;
                let py = text.y + x * sin + y * cos;
                bounds.min_x = bounds.min_x.min(px);
                bounds.min_y = bounds.min_y.min(py);
                bounds.max_x = bounds.max_x.max(px);
                bounds.max_y = bounds.max_y.max(py);
            }
            Some(bounds)
        }),
        ResolvedItem::Graphics(DisplayItem::GlyphRun(_))
        | ResolvedItem::Graphics(DisplayItem::ClipPush { .. })
        | ResolvedItem::Graphics(DisplayItem::ClipPop { .. }) => None,
    }
}

fn raster_asset_has_visible_pixels(asset: &RasterAsset) -> bool {
    let Some(pixel_count) = asset.width.checked_mul(asset.height) else {
        return false;
    };
    let Some(expected_len) = pixel_count.checked_mul(4) else {
        return false;
    };
    asset.rgba.len() == expected_len as usize
        && asset
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] != 0)
}

fn path_geometric_bounds(path: &instplot_render::Path) -> Option<ResolvedBounds> {
    let mut bounds = None;
    let mut cursor = None::<(f32, f32)>;
    let mut subpath_start = None::<(f32, f32)>;
    let mut add = |point: (f32, f32)| {
        let point_bounds = ResolvedBounds {
            min_x: point.0,
            min_y: point.1,
            max_x: point.0,
            max_y: point.1,
        };
        bounds = Some(bounds.map_or(point_bounds, |current: ResolvedBounds| {
            current.union(point_bounds)
        }));
    };
    for verb in &path.verbs {
        match *verb {
            PathVerb::MoveTo(x, y) => {
                let point = (x.get() as f32, y.get() as f32);
                add(point);
                cursor = Some(point);
                subpath_start = Some(point);
            }
            PathVerb::LineTo(x, y) => {
                let point = (x.get() as f32, y.get() as f32);
                add(point);
                cursor = Some(point);
            }
            PathVerb::CurveTo(x1, y1, x2, y2, x3, y3) => {
                let p0 = cursor?;
                let p1 = (x1.get() as f32, y1.get() as f32);
                let p2 = (x2.get() as f32, y2.get() as f32);
                let p3 = (x3.get() as f32, y3.get() as f32);
                add(p3);
                for t in cubic_extrema(p0.0, p1.0, p2.0, p3.0)
                    .into_iter()
                    .chain(cubic_extrema(p0.1, p1.1, p2.1, p3.1))
                {
                    add((
                        cubic_value(p0.0, p1.0, p2.0, p3.0, t),
                        cubic_value(p0.1, p1.1, p2.1, p3.1, t),
                    ));
                }
                cursor = Some(p3);
            }
            PathVerb::Close => {
                if let Some(start) = subpath_start {
                    add(start);
                    cursor = Some(start);
                }
            }
        }
    }
    bounds
}

fn stroked_path_bounds(
    path: &instplot_render::Path,
    stroke: &instplot_render::Stroke,
) -> Option<ResolvedBounds> {
    let mut builder = SkPathBuilder::new();
    for verb in &path.verbs {
        match *verb {
            PathVerb::MoveTo(x, y) => builder.move_to(x.get() as f32, y.get() as f32),
            PathVerb::LineTo(x, y) => builder.line_to(x.get() as f32, y.get() as f32),
            PathVerb::CurveTo(x1, y1, x2, y2, x3, y3) => builder.cubic_to(
                x1.get() as f32,
                y1.get() as f32,
                x2.get() as f32,
                y2.get() as f32,
                x3.get() as f32,
                y3.get() as f32,
            ),
            PathVerb::Close => builder.close(),
        }
    }
    let path = builder.finish()?;
    let outline = path.stroke(
        &SkStroke {
            width: stroke.width.get() as f32,
            miter_limit: 10.0,
            line_cap: match stroke.cap {
                instplot_render::LineCap::Butt => SkLineCap::Butt,
                instplot_render::LineCap::Round => SkLineCap::Round,
                instplot_render::LineCap::Square => SkLineCap::Square,
            },
            line_join: match stroke.join {
                instplot_render::LineJoin::Miter => SkLineJoin::Miter,
                instplot_render::LineJoin::Round => SkLineJoin::Round,
                instplot_render::LineJoin::Bevel => SkLineJoin::Bevel,
            },
            dash: (!stroke.dash.is_empty()).then(|| {
                StrokeDash::new(
                    stroke.dash.iter().map(|value| value.get() as f32).collect(),
                    0.0,
                )
                .expect("validated dash")
            }),
        },
        1.0,
    )?;
    let bounds = outline
        .compute_tight_bounds()
        .unwrap_or_else(|| outline.bounds());
    Some(ResolvedBounds {
        min_x: bounds.left(),
        min_y: bounds.top(),
        max_x: bounds.right(),
        max_y: bounds.bottom(),
    })
}

fn cubic_value(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let one_minus = 1.0 - t;
    one_minus.powi(3) * p0
        + 3.0 * one_minus.powi(2) * t * p1
        + 3.0 * one_minus * t.powi(2) * p2
        + t.powi(3) * p3
}

fn cubic_extrema(p0: f32, p1: f32, p2: f32, p3: f32) -> Vec<f32> {
    let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
    let b = 3.0 * p0 - 6.0 * p1 + 3.0 * p2;
    let c = -3.0 * p0 + 3.0 * p1;
    let qa = 3.0 * a;
    let qb = 2.0 * b;
    if qa.abs() < 1.0e-7 {
        if qb.abs() < 1.0e-7 {
            return Vec::new();
        }
        let t = -c / qb;
        return (0.0 < t && t < 1.0).then_some(t).into_iter().collect();
    }
    let discriminant = qb * qb - 4.0 * qa * c;
    if discriminant < 0.0 {
        return Vec::new();
    }
    let root = discriminant.sqrt();
    [(-qb + root) / (2.0 * qa), (-qb - root) / (2.0 * qa)]
        .into_iter()
        .filter(|t| 0.0 < *t && *t < 1.0)
        .collect()
}
