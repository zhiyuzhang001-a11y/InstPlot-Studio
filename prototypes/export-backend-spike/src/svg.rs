use std::collections::BTreeMap;
use std::fmt::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use studio_render_spike::{
    Color, DisplayItem, Fill, FillRule, LineCap, LineJoin, Path, PathVerb, Stroke,
};

use crate::raster::encode_asset_png;
use crate::{ResolvedDisplayList, ResolvedItem, ResolvedRun, ResolvedText};

pub fn to_svg(list: &ResolvedDisplayList) -> String {
    let mut fonts = BTreeMap::<String, Vec<u8>>::new();
    for item in &list.items {
        if let ResolvedItem::Text(text) = item {
            for run in &text.runs {
                if run.font.postscript_name.starts_with("TeXGyreHeros-") {
                    fonts
                        .entry(run.font.postscript_name.clone())
                        .or_insert_with(|| run.font_data.as_ref().clone());
                }
            }
        }
    }

    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.5}pt\" height=\"{:.5}pt\" viewBox=\"0 0 {:.5} {:.5}\">\n",
        list.width, list.height, list.width, list.height
    );
    svg.push_str(
        "  <metadata>InstPlot Studio A4 resolved Display List</metadata>\n  <defs>\n    <style>\n",
    );
    for (postscript_name, data) in &fonts {
        writeln!(
            svg,
            "      @font-face {{ font-family: '{}'; src: url(data:font/otf;base64,{}) format('opentype'); }}",
            css(postscript_name),
            STANDARD.encode(data)
        )
        .unwrap();
    }
    svg.push_str("    </style>\n  </defs>\n");

    let mut clip_serial = 0_u32;
    let mut open_clips = 0_u32;
    for item in &list.items {
        match item {
            ResolvedItem::Text(text) => write_text(&mut svg, text),
            ResolvedItem::Graphics(DisplayItem::Path {
                source,
                path,
                fill,
                stroke,
            }) => {
                writeln!(
                    svg,
                    "  <path data-node=\"{}\" d=\"{}\"{}{} />",
                    source.0,
                    svg_path(path),
                    svg_fill(*fill),
                    svg_stroke(stroke.as_ref())
                )
                .unwrap();
            }
            ResolvedItem::Graphics(DisplayItem::ClipPush {
                source,
                x,
                y,
                width,
                height,
            }) => {
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
            ResolvedItem::Graphics(DisplayItem::ClipPop { .. }) if open_clips > 0 => {
                svg.push_str("  </g>\n");
                open_clips -= 1;
            }
            ResolvedItem::Graphics(DisplayItem::Image(image)) => {
                if let Some(asset) = list.resources.get(&image.resource_id)
                    && let Ok(png) = encode_asset_png(asset)
                {
                    writeln!(svg, "  <image data-node=\"{}\" x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" href=\"data:image/png;base64,{}\" />", image.source.0, image.x.get(), image.y.get(), image.width.get(), image.height.get(), STANDARD.encode(png)).unwrap();
                }
            }
            ResolvedItem::Graphics(DisplayItem::GlyphRun(_)) => {
                unreachable!("text is resolved before backend dispatch")
            }
            ResolvedItem::Graphics(DisplayItem::ClipPop { .. }) => {}
        }
    }
    while open_clips > 0 {
        svg.push_str("  </g>\n");
        open_clips -= 1;
    }
    svg.push_str("</svg>\n");
    svg
}

fn write_text(svg: &mut String, text: &ResolvedText) {
    write!(
        svg,
        "  <text data-node=\"{}\" data-source=\"{}\" x=\"{:.3}\" y=\"{:.3}\" font-size=\"{:.3}\" fill=\"{}\" transform=\"rotate({:.3} {:.3} {:.3})\">",
        text.source.0,
        xml(&text.text),
        text.x,
        text.y,
        text.size,
        hex(text.color),
        text.rotation_degrees,
        text.x,
        text.y
    )
    .unwrap();
    for run in &text.runs {
        write_run_tspans(svg, text, run);
    }
    svg.push_str("</text>\n");
}

fn write_run_tspans(svg: &mut String, text: &ResolvedText, run: &ResolvedRun) {
    let mut cursor = 0.0_f32;
    let mut index = 0;
    while index < run.glyphs.len() {
        let first = &run.glyphs[index];
        let range = first.text_range.clone();
        let mut end = index + 1;
        let mut advance = first.advance;
        let mut ids = vec![first.id.to_string()];
        while end < run.glyphs.len() && run.glyphs[end].text_range == range {
            advance += run.glyphs[end].advance;
            ids.push(run.glyphs[end].id.to_string());
            end += 1;
        }
        write!(
            svg,
            "<tspan x=\"{:.3}\" y=\"{:.3}\" font-family=\"{}\" font-size=\"{:.3}\" data-font-version=\"{}\" data-glyph-ids=\"{}\">{}</tspan>",
            text.x + run.start_x + cursor + first.x_offset,
            text.y + run.baseline_shift - first.y_offset,
            xml(&run.font.postscript_name),
            run.font_size,
            xml(&run.font.version),
            ids.join(","),
            xml(&text.text[range])
        )
        .unwrap();
        cursor += advance;
        index = end;
    }
}

fn svg_path(path: &Path) -> String {
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

fn svg_fill(fill: Option<Fill>) -> String {
    match fill {
        Some(fill) => format!(
            " fill=\"{}\" fill-opacity=\"{:.6}\" fill-rule=\"{}\"",
            hex(fill.color),
            fill.color.3 as f32 / 255.0,
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
        " stroke=\"{}\" stroke-opacity=\"{:.6}\" stroke-width=\"{:.3}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"{dash}",
        hex(stroke.color),
        stroke.color.3 as f32 / 255.0,
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
        },
    )
}

fn hex(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.0, color.1, color.2)
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn css(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}
