use super::*;

#[derive(Default)]
pub(super) struct FlowCursor {
    cursor: f32,
    script_group: Option<u32>,
    subscript_advance: f32,
    superscript_advance: f32,
}

impl FlowCursor {
    pub(super) fn place(&mut self, flow: SpanFlow, advance: f32) -> f32 {
        match flow {
            SpanFlow::Inline => {
                self.finish_script_group();
                let start = self.cursor;
                self.cursor += advance;
                start
            }
            SpanFlow::Subscript(group) => {
                self.start_script_group(group);
                let start = self.cursor + self.subscript_advance;
                self.subscript_advance += advance;
                start
            }
            SpanFlow::Superscript(group) => {
                self.start_script_group(group);
                let start = self.cursor + self.superscript_advance;
                self.superscript_advance += advance;
                start
            }
        }
    }

    fn start_script_group(&mut self, group: u32) {
        if self.script_group != Some(group) {
            self.finish_script_group();
            self.script_group = Some(group);
        }
    }

    fn finish_script_group(&mut self) {
        if self.script_group.take().is_some() {
            self.cursor += self.subscript_advance.max(self.superscript_advance);
            self.subscript_advance = 0.0;
            self.superscript_advance = 0.0;
        }
    }

    pub(super) fn finish(mut self) -> f32 {
        self.finish_script_group();
        self.cursor
    }
}

pub(super) fn shape_text(
    source: &instplot_render::GlyphRun,
    font_context: &mut FontContext,
    layout_context: &mut LayoutContext<[u8; 4]>,
) -> ResolvedText {
    let size = source.size.get() as f32;
    let text = source.label.normalized_text();
    let mut runs = Vec::new();
    let mut flow_cursor = FlowCursor::default();
    let mut byte_offset = 0_usize;
    for span in source.label.spans() {
        let font_size = size * span.scale;
        let is_unit_separator = span.is_unit_separator;
        if is_unit_separator {
            let face = ttf_parser::Face::parse(REGULAR, 0).expect("valid bundled regular face");
            let glyph_id = face.glyph_index(' ').expect("bundled space glyph").0.into();
            let advance = size * 0.2;
            let start_x = flow_cursor.place(span.flow, advance);
            let data = Arc::new(REGULAR.to_vec());
            runs.push(ResolvedRun {
                font_data: data.clone(),
                font_index: 0,
                font: font_metadata(data.as_slice(), 0),
                start_x,
                baseline_shift: span.baseline_shift_em * size,
                font_size,
                glyphs: vec![ResolvedGlyph {
                    id: glyph_id,
                    text_range: byte_offset..byte_offset + span.text.len(),
                    advance,
                    x_offset: 0.0,
                    y_offset: 0.0,
                }],
            });
            byte_offset += span.text.len();
            continue;
        }
        let shaping_text = span.text.as_str();
        let mut builder = layout_context.ranged_builder(font_context, shaping_text, 1.0, false);
        let family = if matches!(span.text.as_str(), "≤" | "≥") {
            "'InstPlot Studio STIX Two Math'"
        } else {
            "'InstPlot Studio TeX Gyre Heros'"
        };
        builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
            Cow::Borrowed(family),
        )));
        builder.push_default(StyleProperty::FontSize(font_size));
        match span.style {
            Style::Upright => {}
            Style::Italic => builder.push_default(StyleProperty::FontStyle(FontStyle::Italic)),
            Style::Bold => builder.push_default(StyleProperty::FontWeight(FontWeight::BOLD)),
            Style::BoldItalic => {
                builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
                builder.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
            }
        }
        let mut layout = builder.build(shaping_text);
        layout.break_all_lines(None);

        for line in layout.lines() {
            for run in line.runs() {
                let font = run.font().clone();
                let data = Arc::new(font.data.as_ref().to_vec());
                let mut glyphs: Vec<ResolvedGlyph> = Vec::new();
                for cluster in run.visual_clusters() {
                    if cluster.is_ligature_continuation() {
                        if let Some(glyph) = glyphs.last_mut() {
                            glyph.text_range.end = byte_offset + cluster.text_range().end;
                        }
                        continue;
                    }
                    for glyph in cluster.glyphs() {
                        glyphs.push(ResolvedGlyph {
                            id: glyph.id,
                            text_range: byte_offset + cluster.text_range().start
                                ..byte_offset + cluster.text_range().end,
                            advance: glyph.advance,
                            x_offset: glyph.x,
                            y_offset: glyph.y,
                        });
                    }
                }
                let advance = glyphs.iter().map(|glyph| glyph.advance).sum::<f32>();
                let start_x = flow_cursor.place(span.flow, advance);
                runs.push(ResolvedRun {
                    font_data: data.clone(),
                    font_index: font.index,
                    font: font_metadata(data.as_slice(), font.index),
                    start_x,
                    baseline_shift: span.baseline_shift_em * size,
                    font_size,
                    glyphs,
                });
            }
        }
        byte_offset += span.text.len();
    }
    let cursor_x = flow_cursor.finish();
    let anchor_offset = match source.anchor {
        TextAnchor::Start => 0.0,
        TextAnchor::Middle => -cursor_x / 2.0,
        TextAnchor::End => -cursor_x,
    };
    for run in &mut runs {
        run.start_x += anchor_offset;
    }

    let mut resolved = ResolvedText {
        source: source.source,
        text,
        x: source.x.get() as f32,
        y: source.y.get() as f32,
        size,
        color: source.color,
        rotation_degrees: source.rotation_degrees as f32,
        runs,
        ink_bounds: None,
    };
    resolved.ink_bounds = glyph_outline_bounds(&resolved);
    resolved
}

fn glyph_outline_bounds(text: &ResolvedText) -> Option<ResolvedBounds> {
    let mut result = None;
    let radians = text.rotation_degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    for run in &text.runs {
        let face = Face::parse(run.font_data.as_slice(), run.font_index).ok()?;
        let font_scale = run.font_size / face.units_per_em() as f32;
        let mut cursor_x = text.x + run.start_x;
        for glyph in &run.glyphs {
            let mut collector = GlyphInkCollector {
                bounds: None,
                origin_x: cursor_x + glyph.x_offset,
                baseline_y: text.y + run.baseline_shift - glyph.y_offset,
                font_scale,
                pivot_x: text.x,
                pivot_y: text.y,
                sin,
                cos,
            };
            if face
                .outline_glyph(GlyphId(glyph.id as u16), &mut collector)
                .is_some()
                && let Some(bounds) = collector.bounds
            {
                result =
                    Some(result.map_or(bounds, |current: ResolvedBounds| current.union(bounds)));
            }
            cursor_x += glyph.advance;
        }
    }
    result
}

struct GlyphInkCollector {
    bounds: Option<ResolvedBounds>,
    origin_x: f32,
    baseline_y: f32,
    font_scale: f32,
    pivot_x: f32,
    pivot_y: f32,
    sin: f32,
    cos: f32,
}

impl GlyphInkCollector {
    fn add(&mut self, x: f32, y: f32) {
        let x = self.origin_x + x * self.font_scale;
        let y = self.baseline_y - y * self.font_scale;
        let dx = x - self.pivot_x;
        let dy = y - self.pivot_y;
        let x = self.pivot_x + dx * self.cos - dy * self.sin;
        let y = self.pivot_y + dx * self.sin + dy * self.cos;
        let point = ResolvedBounds {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        };
        self.bounds = Some(self.bounds.map_or(point, |bounds| bounds.union(point)));
    }
}

impl OutlineBuilder for GlyphInkCollector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.add(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.add(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.add(x1, y1);
        self.add(x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.add(x1, y1);
        self.add(x2, y2);
        self.add(x, y);
    }

    fn close(&mut self) {}
}
