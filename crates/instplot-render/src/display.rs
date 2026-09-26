use core::fmt::Write;
use instplot_text::Label;

use crate::{NodeId, Pt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub color: Color,
    pub width: Pt,
    pub cap: LineCap,
    pub join: LineJoin,
    pub dash: Vec<Pt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fill {
    pub color: Color,
    pub rule: FillRule,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathVerb {
    MoveTo(Pt, Pt),
    LineTo(Pt, Pt),
    CurveTo(Pt, Pt, Pt, Pt, Pt, Pt),
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub verbs: Vec<PathVerb>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub source: NodeId,
    pub label: Label,
    pub x: Pt,
    pub y: Pt,
    pub size: Pt,
    pub color: Color,
    pub rotation_degrees: f64,
    pub anchor: TextAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub source: NodeId,
    pub resource_id: String,
    pub x: Pt,
    pub y: Pt,
    pub width: Pt,
    pub height: Pt,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DisplayItem {
    Path {
        source: NodeId,
        path: Path,
        fill: Option<Fill>,
        stroke: Option<Stroke>,
    },
    GlyphRun(GlyphRun),
    Image(Image),
    ClipPush {
        source: NodeId,
        x: Pt,
        y: Pt,
        width: Pt,
        height: Pt,
    },
    ClipPop {
        source: NodeId,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DisplayList {
    pub width: Pt,
    pub height: Pt,
    pub items: Vec<DisplayItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SvgOutput {
    pub svg: String,
    pub warnings: Vec<String>,
}

impl DisplayList {
    pub fn debug_snapshot(&self) -> String {
        let mut output = String::new();
        writeln!(
            output,
            "DISPLAY_LIST {:.3} {:.3}",
            self.width.get(),
            self.height.get()
        )
        .unwrap();
        for (index, item) in self.items.iter().enumerate() {
            write!(output, "{index:03} ").unwrap();
            snapshot_item(&mut output, item);
            output.push('\n');
        }
        output
    }

    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut clip_depth = 0_u32;
        for (index, item) in self.items.iter().enumerate() {
            if !item_is_finite(item) {
                errors.push(format!("item {index} contains non-finite coordinates"));
            }
            match item {
                DisplayItem::ClipPush { .. } => clip_depth += 1,
                DisplayItem::ClipPop { .. } if clip_depth == 0 => {
                    errors.push(format!("item {index} pops an empty clip stack"));
                }
                DisplayItem::ClipPop { .. } => clip_depth -= 1,
                _ => {}
            }
        }
        if clip_depth != 0 {
            errors.push(format!("{clip_depth} clip scope(s) remain open"));
        }
        errors
    }
}

pub fn to_svg(list: &DisplayList) -> SvgOutput {
    let mut warnings = list.validation_errors();
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.5}pt\" height=\"{:.5}pt\" viewBox=\"0 0 {:.5} {:.5}\">\n",
        list.width.get(),
        list.height.get(),
        list.width.get(),
        list.height.get()
    );
    let mut clip_serial = 0_u32;
    let mut open_clips = 0_u32;
    for (index, item) in list.items.iter().enumerate() {
        if !item_is_finite(item) {
            warnings.push(format!("SVG skipped invalid item {index}"));
            continue;
        }
        match item {
            DisplayItem::Path {
                source,
                path,
                fill,
                stroke,
            } => {
                let d = svg_path(path);
                writeln!(
                    svg,
                    "  <path data-node=\"{}\" d=\"{}\"{}{} />",
                    source.0,
                    d,
                    svg_fill(*fill),
                    svg_stroke(stroke.as_ref())
                )
                .unwrap();
            }
            DisplayItem::GlyphRun(run) => {
                writeln!(svg, "  <text data-node=\"{}\" x=\"{:.3}\" y=\"{:.3}\" font-size=\"{:.3}\" fill=\"{}\" text-anchor=\"{}\" transform=\"rotate({:.3} {:.3} {:.3})\">{}</text>", run.source.0, run.x.get(), run.y.get(), run.size.get(), hex(run.color), match run.anchor { TextAnchor::Start => "start", TextAnchor::Middle => "middle", TextAnchor::End => "end" }, run.rotation_degrees, run.x.get(), run.y.get(), escape(&run.label.normalized_text())).unwrap();
            }
            DisplayItem::Image(image) => {
                writeln!(svg, "  <image data-node=\"{}\" data-resource=\"{}\" x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" />", image.source.0, escape(&image.resource_id), image.x.get(), image.y.get(), image.width.get(), image.height.get()).unwrap();
            }
            DisplayItem::ClipPush {
                source,
                x,
                y,
                width,
                height,
            } => {
                let clip_id = format!("clip-{clip_serial}");
                clip_serial += 1;
                writeln!(svg, "  <defs><clipPath id=\"{clip_id}\"><rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" /></clipPath></defs>", x.get(), y.get(), width.get(), height.get()).unwrap();
                writeln!(
                    svg,
                    "  <g data-node=\"{}\" clip-path=\"url(#{clip_id})\">",
                    source.0
                )
                .unwrap();
                open_clips += 1;
            }
            DisplayItem::ClipPop { .. } if open_clips > 0 => {
                svg.push_str("  </g>\n");
                open_clips -= 1;
            }
            DisplayItem::ClipPop { .. } => {
                warnings.push(format!("SVG skipped unmatched clip pop at item {index}"))
            }
        }
    }
    while open_clips > 0 {
        svg.push_str("  </g>\n");
        open_clips -= 1;
    }
    svg.push_str("</svg>\n");
    SvgOutput { svg, warnings }
}

fn snapshot_item(output: &mut String, item: &DisplayItem) {
    match item {
        DisplayItem::Path {
            source,
            path,
            fill,
            stroke,
        } => {
            write!(
                output,
                "PATH node={} verbs={} fill={} stroke={}",
                source.0,
                snapshot_path(path),
                fill.map(|value| hex(value.color))
                    .unwrap_or_else(|| "none".into()),
                stroke
                    .as_ref()
                    .map(snapshot_stroke)
                    .unwrap_or_else(|| "none".into())
            )
            .unwrap();
        }
        DisplayItem::GlyphRun(run) => write!(
            output,
            "GLYPH node={} at={:.3},{:.3} size={:.3} rotate={:.3} anchor={:?} text={:?}",
            run.source.0,
            run.x.get(),
            run.y.get(),
            run.size.get(),
            run.rotation_degrees,
            run.anchor,
            run.label.normalized_text()
        )
        .unwrap(),
        DisplayItem::Image(image) => write!(
            output,
            "IMAGE node={} resource={:?} rect={:.3},{:.3},{:.3},{:.3}",
            image.source.0,
            image.resource_id,
            image.x.get(),
            image.y.get(),
            image.width.get(),
            image.height.get()
        )
        .unwrap(),
        DisplayItem::ClipPush {
            source,
            x,
            y,
            width,
            height,
        } => write!(
            output,
            "CLIP_PUSH node={} rect={:.3},{:.3},{:.3},{:.3}",
            source.0,
            x.get(),
            y.get(),
            width.get(),
            height.get()
        )
        .unwrap(),
        DisplayItem::ClipPop { source } => write!(output, "CLIP_POP node={}", source.0).unwrap(),
    }
}

fn snapshot_path(path: &Path) -> String {
    path.verbs
        .iter()
        .map(|verb| match verb {
            PathVerb::MoveTo(x, y) => format!("M{:.3},{:.3}", x.get(), y.get()),
            PathVerb::LineTo(x, y) => format!("L{:.3},{:.3}", x.get(), y.get()),
            PathVerb::CurveTo(x1, y1, x2, y2, x3, y3) => format!(
                "C{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}",
                x1.get(),
                y1.get(),
                x2.get(),
                y2.get(),
                x3.get(),
                y3.get()
            ),
            PathVerb::Close => "Z".into(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn snapshot_stroke(stroke: &Stroke) -> String {
    let dash = stroke
        .dash
        .iter()
        .map(|value| format!("{:.3}", value.get()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}@{:.3}/{:?}/{:?}/[{dash}]",
        hex(stroke.color),
        stroke.width.get(),
        stroke.cap,
        stroke.join
    )
}

fn item_is_finite(item: &DisplayItem) -> bool {
    let pt = |value: Pt| value.get().is_finite();
    match item {
        DisplayItem::Path { path, stroke, .. } => {
            path.verbs.iter().all(|verb| match verb {
                PathVerb::MoveTo(x, y) | PathVerb::LineTo(x, y) => pt(*x) && pt(*y),
                PathVerb::CurveTo(x1, y1, x2, y2, x3, y3) => {
                    [x1, y1, x2, y2, x3, y3].iter().all(|value| pt(**value))
                }
                PathVerb::Close => true,
            }) && stroke
                .as_ref()
                .is_none_or(|value| pt(value.width) && value.dash.iter().all(|dash| pt(*dash)))
        }
        DisplayItem::GlyphRun(run) => {
            [run.x, run.y, run.size].into_iter().all(pt) && run.rotation_degrees.is_finite()
        }
        DisplayItem::Image(image) => [image.x, image.y, image.width, image.height]
            .into_iter()
            .all(pt),
        DisplayItem::ClipPush {
            x,
            y,
            width,
            height,
            ..
        } => [*x, *y, *width, *height].into_iter().all(pt),
        DisplayItem::ClipPop { .. } => true,
    }
}

fn svg_path(path: &Path) -> String {
    snapshot_path(path)
}

fn svg_fill(fill: Option<Fill>) -> String {
    match fill {
        Some(fill) => format!(
            " fill=\"{}\" fill-rule=\"{}\"",
            hex(fill.color),
            match fill.rule {
                FillRule::NonZero => "nonzero",
                FillRule::EvenOdd => "evenodd",
            }
        ),
        None => " fill=\"none\"".into(),
    }
}

fn svg_stroke(stroke: Option<&Stroke>) -> String {
    let Some(stroke) = stroke else {
        return String::new();
    };
    let dash = if stroke.dash.is_empty() {
        String::new()
    } else {
        format!(
            " stroke-dasharray=\"{}\"",
            stroke
                .dash
                .iter()
                .map(|value| format!("{:.3}", value.get()))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    format!(
        " stroke=\"{}\" stroke-width=\"{:.3}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"{dash}",
        hex(stroke.color),
        stroke.width.get(),
        match stroke.cap {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        },
        match stroke.join {
            LineJoin::Miter => "miter",
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
        }
    )
}

fn hex(color: Color) -> String {
    if color.3 == 255 {
        format!("#{:02X}{:02X}{:02X}", color.0, color.1, color.2)
    } else {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.0, color.1, color.2, color.3
        )
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
