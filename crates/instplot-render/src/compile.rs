use core::fmt;

use crate::{
    Artist, Axes, Color, DisplayItem, DisplayList, Fill, FillRule, GlyphRun, LineCap, LineJoin,
    NodeId, Path, PathVerb, Pt, Stroke, TextAnchor,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    InvalidFigureSize,
    InvalidAxesRect(NodeId),
    InvalidAxisRange(NodeId),
    NonFiniteData(NodeId),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFigureSize => {
                formatter.write_str("figure size must be finite and positive")
            }
            Self::InvalidAxesRect(id) => {
                write!(formatter, "axes {} has an invalid rectangle", id.0)
            }
            Self::InvalidAxisRange(id) => write!(formatter, "axis {} has an invalid range", id.0),
            Self::NonFiniteData(id) => write!(formatter, "node {} contains non-finite data", id.0),
        }
    }
}

impl std::error::Error for CompileError {}

pub fn compile(figure: &crate::Figure) -> Result<DisplayList, CompileError> {
    let width = figure.width.to_pt();
    let height = figure.height.to_pt();
    if width.get() <= 0.0 || height.get() <= 0.0 {
        return Err(CompileError::InvalidFigureSize);
    }
    let mut list = DisplayList {
        width,
        height,
        items: Vec::new(),
    };
    for axes in &figure.axes {
        compile_axes(axes, &mut list)?;
    }
    Ok(list)
}

fn compile_axes(axes: &Axes, list: &mut DisplayList) -> Result<(), CompileError> {
    if axes.width.get() <= 0.0 || axes.height.get() <= 0.0 {
        return Err(CompileError::InvalidAxesRect(axes.id));
    }
    validate_axis(axes.x.id, axes.x.minimum, axes.x.maximum)?;
    validate_axis(axes.y.id, axes.y.minimum, axes.y.maximum)?;

    list.items.push(DisplayItem::ClipPush {
        source: axes.id,
        x: axes.left,
        y: axes.top,
        width: axes.width,
        height: axes.height,
    });
    for artist in &axes.artists {
        compile_artist(axes, artist, list)?;
    }
    list.items.push(DisplayItem::ClipPop { source: axes.id });

    list.items.push(DisplayItem::Path {
        source: axes.id,
        path: rectangle(axes.left, axes.top, axes.width, axes.height),
        fill: None,
        stroke: Some(default_axes_stroke()),
    });

    let label_color = Color(0, 0, 0, 255);
    list.items.push(DisplayItem::GlyphRun(GlyphRun {
        source: axes.x.id,
        label: axes.x.label.clone(),
        x: pt(axes.left.get() + axes.width.get() / 2.0)?,
        y: pt(axes.top.get() + axes.height.get() + 18.0)?,
        size: pt(9.0)?,
        color: label_color,
        rotation_degrees: 0.0,
        anchor: TextAnchor::Middle,
    }));
    list.items.push(DisplayItem::GlyphRun(GlyphRun {
        source: axes.y.id,
        label: axes.y.label.clone(),
        x: pt(axes.left.get() - 24.0)?,
        y: pt(axes.top.get() + axes.height.get() / 2.0)?,
        size: pt(9.0)?,
        color: label_color,
        rotation_degrees: -90.0,
        anchor: TextAnchor::Middle,
    }));
    Ok(())
}

fn compile_artist(
    axes: &Axes,
    artist: &Artist,
    list: &mut DisplayList,
) -> Result<(), CompileError> {
    match artist {
        Artist::Line(line) => {
            list.items.push(DisplayItem::Path {
                source: line.id,
                path: data_path(axes, line.id, &line.points)?,
                fill: None,
                stroke: Some(line.stroke.clone()),
            });
        }
        Artist::Scatter(scatter) => {
            validate_points(scatter.id, &scatter.points)?;
            for &(x, y) in &scatter.points {
                let (x, y) = transform(axes, x, y)?;
                list.items.push(DisplayItem::Path {
                    source: scatter.id,
                    path: circle(x, y, scatter.radius)?,
                    fill: Some(Fill {
                        color: scatter.color,
                        rule: FillRule::NonZero,
                    }),
                    stroke: None,
                });
            }
        }
        Artist::ErrorBar(error_bar) => {
            for &(x, y, error) in &error_bar.points {
                if !x.is_finite() || !y.is_finite() || !error.is_finite() || error < 0.0 {
                    return Err(CompileError::NonFiniteData(error_bar.id));
                }
                let (center_x, high_y) = transform(axes, x, y + error)?;
                let (_, low_y) = transform(axes, x, y - error)?;
                let half_cap = error_bar.cap_width.get() / 2.0;
                let path = Path {
                    verbs: vec![
                        PathVerb::MoveTo(center_x, high_y),
                        PathVerb::LineTo(center_x, low_y),
                        PathVerb::MoveTo(pt(center_x.get() - half_cap)?, high_y),
                        PathVerb::LineTo(pt(center_x.get() + half_cap)?, high_y),
                        PathVerb::MoveTo(pt(center_x.get() - half_cap)?, low_y),
                        PathVerb::LineTo(pt(center_x.get() + half_cap)?, low_y),
                    ],
                };
                list.items.push(DisplayItem::Path {
                    source: error_bar.id,
                    path,
                    fill: None,
                    stroke: Some(error_bar.stroke.clone()),
                });
            }
        }
        Artist::ReferenceLine(reference) => {
            let (left, y) = transform(axes, axes.x.minimum, reference.y)?;
            let (right, _) = transform(axes, axes.x.maximum, reference.y)?;
            list.items.push(DisplayItem::Path {
                source: reference.id,
                path: Path {
                    verbs: vec![PathVerb::MoveTo(left, y), PathVerb::LineTo(right, y)],
                },
                fill: None,
                stroke: Some(reference.stroke.clone()),
            });
        }
        Artist::Text(text) => list.items.push(DisplayItem::GlyphRun(GlyphRun {
            source: text.id,
            label: text.value.clone(),
            x: text.x,
            y: text.y,
            size: text.size,
            color: text.color,
            rotation_degrees: 0.0,
            anchor: TextAnchor::Start,
        })),
        Artist::Legend(legend) => {
            for (index, entry) in legend.entries.iter().enumerate() {
                list.items.push(DisplayItem::GlyphRun(GlyphRun {
                    source: legend.id,
                    label: entry.clone(),
                    x: legend.x,
                    y: pt(legend.y.get() + index as f64 * legend.size.get() * 1.35)?,
                    size: legend.size,
                    color: legend.color,
                    rotation_degrees: 0.0,
                    anchor: TextAnchor::Start,
                }));
            }
        }
    }
    Ok(())
}

fn validate_axis(id: NodeId, minimum: f64, maximum: f64) -> Result<(), CompileError> {
    if minimum.is_finite() && maximum.is_finite() && minimum < maximum {
        Ok(())
    } else {
        Err(CompileError::InvalidAxisRange(id))
    }
}

fn validate_points(id: NodeId, points: &[(f64, f64)]) -> Result<(), CompileError> {
    if points.iter().all(|(x, y)| x.is_finite() && y.is_finite()) {
        Ok(())
    } else {
        Err(CompileError::NonFiniteData(id))
    }
}

fn data_path(axes: &Axes, id: NodeId, points: &[(f64, f64)]) -> Result<Path, CompileError> {
    validate_points(id, points)?;
    let mut verbs = Vec::with_capacity(points.len());
    for (index, &(x, y)) in points.iter().enumerate() {
        let (x, y) = transform(axes, x, y)?;
        verbs.push(if index == 0 {
            PathVerb::MoveTo(x, y)
        } else {
            PathVerb::LineTo(x, y)
        });
    }
    Ok(Path { verbs })
}

fn transform(axes: &Axes, x: f64, y: f64) -> Result<(Pt, Pt), CompileError> {
    if !x.is_finite() || !y.is_finite() {
        return Err(CompileError::NonFiniteData(axes.id));
    }
    let x_fraction = (x - axes.x.minimum) / (axes.x.maximum - axes.x.minimum);
    let y_fraction = (y - axes.y.minimum) / (axes.y.maximum - axes.y.minimum);
    Ok((
        pt(axes.left.get() + x_fraction * axes.width.get())?,
        pt(axes.top.get() + (1.0 - y_fraction) * axes.height.get())?,
    ))
}

fn rectangle(x: Pt, y: Pt, width: Pt, height: Pt) -> Path {
    Path {
        verbs: vec![
            PathVerb::MoveTo(x, y),
            PathVerb::LineTo(Pt::new(x.get() + width.get()).unwrap(), y),
            PathVerb::LineTo(
                Pt::new(x.get() + width.get()).unwrap(),
                Pt::new(y.get() + height.get()).unwrap(),
            ),
            PathVerb::LineTo(x, Pt::new(y.get() + height.get()).unwrap()),
            PathVerb::Close,
        ],
    }
}

fn circle(x: Pt, y: Pt, radius: Pt) -> Result<Path, CompileError> {
    let r = radius.get();
    if !r.is_finite() || r < 0.0 {
        return Err(CompileError::NonFiniteData(NodeId(0)));
    }
    let k = r * 0.552_284_749_830_793_6;
    Ok(Path {
        verbs: vec![
            PathVerb::MoveTo(pt(x.get() + r)?, y),
            PathVerb::CurveTo(
                pt(x.get() + r)?,
                pt(y.get() + k)?,
                pt(x.get() + k)?,
                pt(y.get() + r)?,
                x,
                pt(y.get() + r)?,
            ),
            PathVerb::CurveTo(
                pt(x.get() - k)?,
                pt(y.get() + r)?,
                pt(x.get() - r)?,
                pt(y.get() + k)?,
                pt(x.get() - r)?,
                y,
            ),
            PathVerb::CurveTo(
                pt(x.get() - r)?,
                pt(y.get() - k)?,
                pt(x.get() - k)?,
                pt(y.get() - r)?,
                x,
                pt(y.get() - r)?,
            ),
            PathVerb::CurveTo(
                pt(x.get() + k)?,
                pt(y.get() - r)?,
                pt(x.get() + r)?,
                pt(y.get() - k)?,
                pt(x.get() + r)?,
                y,
            ),
            PathVerb::Close,
        ],
    })
}

fn default_axes_stroke() -> Stroke {
    Stroke {
        color: Color(0, 0, 0, 255),
        width: Pt::new(0.6).unwrap(),
        cap: LineCap::Butt,
        join: LineJoin::Miter,
        dash: Vec::new(),
    }
}

fn pt(value: f64) -> Result<Pt, CompileError> {
    Pt::new(value).map_err(|_| CompileError::NonFiniteData(NodeId(0)))
}
