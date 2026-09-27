use super::*;

pub(super) fn publication_object_name(
    document: &FigureDocument,
    node: &str,
    language: UiLanguage,
) -> String {
    if let Some(series) = document
        .series()
        .into_iter()
        .find(|series| series.id == node)
    {
        return series.label;
    }
    let axes = &document.project().figure.axes[0];
    if node == axes.x.id {
        language.text(Text::XAxis).to_owned()
    } else if node == axes.y.id {
        language.text(Text::YAxis).to_owned()
    } else if node == axes.id || node == document.project().figure.id {
        language.text(Text::FixedFigure).to_owned()
    } else {
        "相关对象".to_owned()
    }
}

pub(super) fn localized_publication_finding(
    language: UiLanguage,
    finding: &instplot_studio::PublicationFinding,
) -> (&str, &str, &str, &str) {
    if language == UiLanguage::English {
        return (
            &finding.rule_id,
            &finding.message,
            &finding.impact,
            &finding.remediation,
        );
    }
    match finding.rule_id.as_str() {
        "physical_size" => (
            "图形尺寸",
            "画布尺寸超出常用出版范围",
            "可能影响版面适配和可读性",
            "在图形尺寸中调整最终宽度和高度",
        ),
        "font_size" => (
            "字号",
            "图中存在过小文字",
            "缩放或印刷后可能难以阅读",
            "缩短标签或增大最终画布，勿用预览缩放代替",
        ),
        "stroke_width" => (
            "线宽",
            "图中存在过细线条",
            "印刷或栅格化后可能消失",
            "选中对应曲线或误差棒并适当增大线宽",
        ),
        "font_embedding" => (
            "字体嵌入",
            "导出字体或字形不完整",
            "换一台电脑可能改变科学符号",
            "保持内置字体方案并重新导出 PDF",
        ),
        "clipping" => (
            "内容裁切",
            "有文字或图形超出导出范围",
            "导出的图可能缺少内容",
            "检查对象位置、间距、坐标范围和画布尺寸",
        ),
        "legend_overlap" => (
            "图例遮挡",
            "图例与数据区域发生重叠",
            "可能遮住数据或混淆曲线",
            "移动图例或调整可见条目和行列",
        ),
        "color_only_encoding" => (
            "仅用颜色区分",
            "系列之间只依靠颜色区分",
            "灰度或色觉差异下可能无法辨认",
            "同时使用不同 marker 或线型",
        ),
        "palette_data_relationship" => (
            "颜色关系",
            "数据与拟合的颜色关系不完整",
            "来源和拟合关系可能变得不清楚",
            "使用项目色板并让关联系列保持一致",
        ),
        "grayscale_distinguishability" => (
            "灰度可辨识性",
            "部分系列转为灰度后不易区分",
            "黑白打印时可能混淆",
            "增加不同 marker 或线型编码",
        ),
        "cvd_risk" => (
            "色觉可辨识性",
            "部分颜色在常见色觉差异下过于接近",
            "部分读者可能无法区分系列",
            "更换颜色并配合 marker 或线型",
        ),
        "raster_dpi_pixels" => (
            "位图分辨率",
            "导出像素或分辨率不足",
            "细节可能模糊且不满足投稿要求",
            "使用 300 dpi 或更高并核对像素尺寸",
        ),
        "transparency" => (
            "透明度",
            "图中使用了透明内容",
            "期刊处理时可能出现不一致",
            "除非期刊允许，否则使用不透明白色背景",
        ),
        "provenance_completeness" => (
            "来源记录",
            "图形来源或样式记录不完整",
            "后续复核和重现会更困难",
            "保留数据、色板、字体和显式样式记录",
        ),
        _ => (
            "出版规范",
            "检测到可能影响出版的问题",
            "可能降低图形可靠性",
            "检查对应对象并调整相关设置",
        ),
    }
}

pub(super) fn insert_label_snippet(
    text: &mut String,
    output: &mut egui::text_edit::TextEditOutput,
    snippet: &str,
    cursor_inside: bool,
    context: &egui::Context,
) {
    let range = output
        .state
        .cursor
        .char_range()
        .or(output.cursor_range)
        .map(|range| {
            let range = range.as_sorted_char_range();
            range.start.0..range.end.0
        })
        .unwrap_or_else(|| text.chars().count()..text.chars().count());
    let (updated, position) = apply_label_snippet(text, range, snippet, cursor_inside);
    *text = updated;
    output
        .state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(position),
        )));
    output.state.clone().store(context, output.response.id);
    context.memory_mut(|memory| memory.request_focus(output.response.id));
}

pub(super) fn apply_label_snippet(
    text: &str,
    range: std::ops::Range<usize>,
    snippet: &str,
    cursor_inside: bool,
) -> (String, usize) {
    let start = text
        .char_indices()
        .nth(range.start)
        .map_or(text.len(), |(index, _)| index);
    let end = text
        .char_indices()
        .nth(range.end)
        .map_or(text.len(), |(index, _)| index);
    let selection = &text[start..end];
    let (replacement, position) = if cursor_inside {
        let (prefix, suffix) = match snippet {
            "$$" => ("$", "$"),
            "_{}" => ("_{", "}"),
            "^{}" => ("^{", "}"),
            "\\mathrm{}" => ("\\mathrm{", "}"),
            _ => (snippet, ""),
        };
        let replacement = format!("{prefix}{selection}{suffix}");
        let position = range.start + prefix.chars().count() + selection.chars().count();
        (replacement, position)
    } else {
        (snippet.to_owned(), range.start + snippet.chars().count())
    };
    let mut updated = text.to_owned();
    updated.replace_range(start..end, &replacement);
    (updated, position)
}

pub(super) const ANNOTATION_CONNECTOR_DEFAULT_COLOR_ID: &str = "object-black";
pub(super) const ANNOTATION_CONNECTOR_DEFAULT_WIDTH_PT: f64 = 0.9;

pub(super) fn normalize_label_editor_line_breaks(
    text: &mut String,
    allow_hard_line_breaks: bool,
) -> bool {
    let normalized = if allow_hard_line_breaks {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.replace(['\r', '\n'], " ")
    };
    if *text == normalized {
        return false;
    }
    *text = normalized;
    true
}

pub(super) fn label_preview_job(nodes: &[LabelNode]) -> egui::text::LayoutJob {
    fn font_id(face: &str, script: bool) -> egui::FontId {
        egui::FontId::new(
            if script { 12.0 } else { 17.0 },
            egui::FontFamily::Name(std::sync::Arc::from(face)),
        )
    }
    fn append(nodes: &[LabelNode], job: &mut egui::text::LayoutJob, script: Option<egui::Align>) {
        for node in nodes {
            let (value, face) = match node {
                LabelNode::Text(value)
                | LabelNode::Upright(value)
                | LabelNode::Number(value)
                | LabelNode::Unit(value) => (value.as_str(), "TeXGyreHeros-Regular"),
                LabelNode::Operator(value) if matches!(value.as_str(), "≤" | "≥") => {
                    (value.as_str(), "STIXTwoMath-Regular")
                }
                LabelNode::Operator(value) => (value.as_str(), "TeXGyreHeros-Regular"),
                LabelNode::Emphasis(value) => (value.as_str(), "TeXGyreHeros-Bold"),
                LabelNode::Variable(value) => (value.as_str(), "TeXGyreHeros-Italic"),
                LabelNode::BoldVariable(value) => (value.as_str(), "TeXGyreHeros-BoldItalic"),
                LabelNode::GreekVariable(value) => {
                    let value = value.to_string();
                    job.append(
                        &value,
                        0.0,
                        egui::TextFormat {
                            font_id: font_id("TeXGyreHeros-Italic", script.is_some()),
                            valign: script.unwrap_or(egui::Align::Center),
                            ..Default::default()
                        },
                    );
                    continue;
                }
                LabelNode::DescriptiveSubscript(inner) | LabelNode::VariableSubscript(inner) => {
                    append(inner, job, Some(egui::Align::BOTTOM));
                    continue;
                }
                LabelNode::Superscript(inner) => {
                    append(inner, job, Some(egui::Align::TOP));
                    continue;
                }
                LabelNode::UnitSeparator => ("\u{202f}", "TeXGyreHeros-Regular"),
            };
            job.append(
                value,
                0.0,
                egui::TextFormat {
                    font_id: font_id(face, script.is_some()),
                    valign: script.unwrap_or(egui::Align::Center),
                    ..Default::default()
                },
            );
        }
    }
    let mut job = egui::text::LayoutJob::default();
    append(nodes, &mut job, None);
    job
}

pub(super) fn role_editor(ui: &mut egui::Ui, language: UiLanguage, role: &mut ArtistRole) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(language.text(Text::Role));
        egui::ComboBox::from_id_salt("artist-role")
            .selected_text(artist_role_name(language, *role))
            .show_ui(ui, |ui| {
                for candidate in [
                    ArtistRole::Data,
                    ArtistRole::Fit,
                    ArtistRole::Theory,
                    ArtistRole::Reference,
                    ArtistRole::Baseline,
                ] {
                    changed |= ui
                        .selectable_value(role, candidate, artist_role_name(language, candidate))
                        .changed();
                }
            });
    });
    changed
}

pub(super) fn color_editor(
    ui: &mut egui::Ui,
    language: UiLanguage,
    artist_id: &str,
    color_id: &mut String,
    palette: &[instplot_studio::PaletteColor],
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(language.text(Text::Color));
        if let Some(selected) = palette.iter().find(|color| color.id == *color_id) {
            let [r, g, b, a] = selected.rgba;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 3.0, egui::Color32::from_rgba_unmultiplied(r, g, b, a));
        }
        egui::ComboBox::from_id_salt(("artist-color", artist_id))
            .selected_text(palette_color_name(language, color_id))
            .show_ui(ui, |ui| {
                for color in palette {
                    ui.horizontal(|ui| {
                        let [r, g, b, a] = color.rgba;
                        let swatch = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, swatch);
                        changed |= ui
                            .selectable_value(
                                color_id,
                                color.id.clone(),
                                palette_color_name(language, &color.id),
                            )
                            .changed();
                    });
                }
            });
    });
    changed
}

pub(super) fn palette_color_name(language: UiLanguage, id: &str) -> &str {
    match (language, id) {
        (UiLanguage::Chinese, "blue") => "蓝色",
        (UiLanguage::Chinese, "gray") => "灰色",
        (UiLanguage::Chinese, "object-black") => "黑色",
        (UiLanguage::Chinese, "object-red") => "红色",
        (UiLanguage::Chinese, "object-green") => "绿色",
        (UiLanguage::Chinese, "object-yellow") => "黄色",
        (UiLanguage::Chinese, "object-cyan") => "青色",
        (UiLanguage::Chinese, "object-purple") => "紫色",
        (UiLanguage::Chinese, "object-light-gray") => "浅灰色",
        (UiLanguage::Chinese, "bright-blue") => "明亮蓝",
        (UiLanguage::Chinese, "bright-red") => "明亮红",
        (UiLanguage::Chinese, "bright-green") => "明亮绿",
        (UiLanguage::Chinese, "bright-yellow") => "明亮黄",
        (UiLanguage::Chinese, "bright-cyan") => "明亮青",
        (UiLanguage::Chinese, "bright-purple") => "明亮紫",
        (UiLanguage::Chinese, "bright-gray") => "明亮灰",
        (UiLanguage::Chinese, "high-blue") => "高对比蓝",
        (UiLanguage::Chinese, "high-yellow") => "高对比黄",
        (UiLanguage::Chinese, "high-red") => "高对比红",
        (UiLanguage::Chinese, "okabe-orange") => "橙色",
        (UiLanguage::Chinese, "okabe-sky-blue") => "天蓝色",
        (UiLanguage::Chinese, "okabe-bluish-green") => "蓝绿色",
        (UiLanguage::Chinese, "okabe-yellow") => "黄色",
        (UiLanguage::Chinese, "okabe-blue") => "蓝色",
        (UiLanguage::Chinese, "okabe-vermillion") => "朱红色",
        (UiLanguage::Chinese, "okabe-reddish-purple") => "紫红色",
        (UiLanguage::Chinese, "okabe-black") => "黑色",
        (UiLanguage::Chinese, "batlow-1") => "Batlow 1（低值）",
        (UiLanguage::Chinese, "batlow-2") => "Batlow 2",
        (UiLanguage::Chinese, "batlow-3") => "Batlow 3",
        (UiLanguage::Chinese, "batlow-4") => "Batlow 4",
        (UiLanguage::Chinese, "batlow-5") => "Batlow 5",
        (UiLanguage::Chinese, "batlow-6") => "Batlow 6",
        (UiLanguage::Chinese, "batlow-7") => "Batlow 7（高值）",
        (UiLanguage::Chinese, "viridis-1") => "Viridis 1（低值）",
        (UiLanguage::Chinese, "viridis-2") => "Viridis 2",
        (UiLanguage::Chinese, "viridis-3") => "Viridis 3",
        (UiLanguage::Chinese, "viridis-4") => "Viridis 4",
        (UiLanguage::Chinese, "viridis-5") => "Viridis 5",
        (UiLanguage::Chinese, "viridis-6") => "Viridis 6",
        (UiLanguage::Chinese, "viridis-7") => "Viridis 7（高值）",
        (UiLanguage::Chinese, "cividis-1") => "Cividis 1（低值）",
        (UiLanguage::Chinese, "cividis-2") => "Cividis 2",
        (UiLanguage::Chinese, "cividis-3") => "Cividis 3",
        (UiLanguage::Chinese, "cividis-4") => "Cividis 4",
        (UiLanguage::Chinese, "cividis-5") => "Cividis 5",
        (UiLanguage::Chinese, "cividis-6") => "Cividis 6",
        (UiLanguage::Chinese, "cividis-7") => "Cividis 7（高值）",
        (UiLanguage::Chinese, "neutral-primary") => "中性深灰",
        (UiLanguage::Chinese, "neutral-secondary") => "中性浅灰",
        (UiLanguage::Chinese, "burd-blue-4") => "发散深蓝",
        (UiLanguage::Chinese, "burd-blue-3") => "发散蓝",
        (UiLanguage::Chinese, "burd-blue-2") => "发散浅蓝",
        (UiLanguage::Chinese, "burd-blue-1") => "发散极浅蓝",
        (UiLanguage::Chinese, "burd-center") => "发散中点",
        (UiLanguage::Chinese, "burd-red-1") => "发散极浅红",
        (UiLanguage::Chinese, "burd-red-2") => "发散浅红",
        (UiLanguage::Chinese, "burd-red-3") => "发散红",
        (UiLanguage::Chinese, "burd-red-4") => "发散深红",
        (_, "object-red") => "Red",
        (_, "object-green") => "Green",
        (_, "object-yellow") => "Yellow",
        (_, "object-cyan") => "Cyan",
        (_, "object-purple") => "Purple",
        (_, "object-light-gray") => "Light gray",
        (_, "object-black") => "Black",
        (_, "blue") => "Blue",
        (_, "gray") => "Gray",
        (_, "bright-blue") => "Bright blue",
        (_, "bright-red") => "Bright red",
        (_, "bright-green") => "Bright green",
        (_, "bright-yellow") => "Bright yellow",
        (_, "bright-cyan") => "Bright cyan",
        (_, "bright-purple") => "Bright purple",
        (_, "bright-gray") => "Bright gray",
        (_, "high-blue") => "High-contrast blue",
        (_, "high-yellow") => "High-contrast yellow",
        (_, "high-red") => "High-contrast red",
        (_, "okabe-orange") => "Orange",
        (_, "okabe-sky-blue") => "Sky blue",
        (_, "okabe-bluish-green") => "Bluish green",
        (_, "okabe-yellow") => "Yellow",
        (_, "okabe-blue") => "Blue",
        (_, "okabe-vermillion") => "Vermillion",
        (_, "okabe-reddish-purple") => "Reddish purple",
        (_, "okabe-black") => "Black",
        (_, "batlow-1") => "Batlow 1 (low)",
        (_, "batlow-2") => "Batlow 2",
        (_, "batlow-3") => "Batlow 3",
        (_, "batlow-4") => "Batlow 4",
        (_, "batlow-5") => "Batlow 5",
        (_, "batlow-6") => "Batlow 6",
        (_, "batlow-7") => "Batlow 7 (high)",
        (_, "viridis-1") => "Viridis 1 (low)",
        (_, "viridis-2") => "Viridis 2",
        (_, "viridis-3") => "Viridis 3",
        (_, "viridis-4") => "Viridis 4",
        (_, "viridis-5") => "Viridis 5",
        (_, "viridis-6") => "Viridis 6",
        (_, "viridis-7") => "Viridis 7 (high)",
        (_, "cividis-1") => "Cividis 1 (low)",
        (_, "cividis-2") => "Cividis 2",
        (_, "cividis-3") => "Cividis 3",
        (_, "cividis-4") => "Cividis 4",
        (_, "cividis-5") => "Cividis 5",
        (_, "cividis-6") => "Cividis 6",
        (_, "cividis-7") => "Cividis 7 (high)",
        (_, "neutral-primary") => "Neutral dark",
        (_, "neutral-secondary") => "Neutral light",
        (_, "burd-blue-4") => "Diverging dark blue",
        (_, "burd-blue-3") => "Diverging blue",
        (_, "burd-blue-2") => "Diverging light blue",
        (_, "burd-blue-1") => "Diverging pale blue",
        (_, "burd-center") => "Diverging center",
        (_, "burd-red-1") => "Diverging pale red",
        (_, "burd-red-2") => "Diverging light red",
        (_, "burd-red-3") => "Diverging red",
        (_, "burd-red-4") => "Diverging dark red",
        _ => id,
    }
}

pub(super) fn palette_scheme_name(language: UiLanguage, id: &str) -> &'static str {
    match (language, id) {
        (UiLanguage::Chinese, "tol-bright-v1") => "明亮分类 · Paul Tol（默认）",
        (UiLanguage::Chinese, "tol-high-contrast-v1") => "高对比 · Paul Tol",
        (UiLanguage::Chinese, "okabe-ito-v1") => "色觉友好 · Okabe–Ito",
        (UiLanguage::Chinese, "batlow-v1") => "科学渐变 · Batlow",
        (UiLanguage::Chinese, "viridis-v1") => "清晰渐变 · Viridis",
        (UiLanguage::Chinese, "cividis-v1") => "无障碍渐变 · Cividis",
        (UiLanguage::Chinese, "tol-burd-v1") => "蓝红发散 · Paul Tol",
        (UiLanguage::Chinese, "sciplot-neutral-v1") => "中性灰阶",
        (_, "tol-bright-v1") => "Bright categories · Paul Tol (default)",
        (_, "tol-high-contrast-v1") => "High contrast · Paul Tol",
        (_, "okabe-ito-v1") => "Colour-safe · Okabe–Ito",
        (_, "batlow-v1") => "Scientific gradient · Batlow",
        (_, "viridis-v1") => "Clear gradient · Viridis",
        (_, "cividis-v1") => "Accessible gradient · Cividis",
        (_, "tol-burd-v1") => "Blue–red diverging · Paul Tol",
        (_, "sciplot-neutral-v1") => "Neutral grayscale",
        _ => "Custom",
    }
}

pub(super) fn palette_group_name(language: UiLanguage, kind: PaletteKind) -> &'static str {
    match (language, kind) {
        (UiLanguage::Chinese, PaletteKind::Qualitative) => "分类",
        (UiLanguage::Chinese, PaletteKind::Sequential) => "有序",
        (UiLanguage::Chinese, PaletteKind::Diverging) => "发散",
        (UiLanguage::Chinese, PaletteKind::Neutral) => "辅助",
        (_, PaletteKind::Qualitative) => "Categorical",
        (_, PaletteKind::Sequential) => "Ordered",
        (_, PaletteKind::Diverging) => "Diverging",
        (_, PaletteKind::Neutral) => "Supporting",
    }
}

pub(super) fn stroke_editor(
    ui: &mut egui::Ui,
    language: UiLanguage,
    artist_id: &str,
    stroke: &mut StrokeStyle,
    palette: &[instplot_studio::PaletteColor],
) -> UiEdit {
    let mut edit = UiEdit {
        changed: color_editor(ui, language, artist_id, &mut stroke.color_id, palette),
        ..UiEdit::default()
    };
    let selected = dash_name(language, &stroke.dash_pt);
    ui.horizontal_wrapped(|ui| {
        let response = ui.add(
            egui::DragValue::new(&mut stroke.width_pt)
                .range(0.1..=72.0)
                .prefix(format!("{}: ", language.text(Text::LineWidth))),
        );
        edit.merge(continuous_edit(&response));
        ui.label(language.text(Text::Dash));
        egui::ComboBox::from_id_salt(("artist-dash", artist_id))
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (name, pattern) in [
                    (dash_name(language, &[]), Vec::new()),
                    (dash_name(language, &[4.0, 2.4]), vec![4.0, 2.4]),
                    (dash_name(language, &[0.8, 1.8]), vec![0.8, 1.8]),
                    (
                        dash_name(language, &[4.0, 2.0, 0.8, 2.0]),
                        vec![4.0, 2.0, 0.8, 2.0],
                    ),
                    (dash_name(language, &[8.0, 3.0]), vec![8.0, 3.0]),
                    (
                        dash_name(language, &[8.0, 2.0, 2.0, 2.0]),
                        vec![8.0, 2.0, 2.0, 2.0],
                    ),
                    (
                        dash_name(language, &[6.0, 2.0, 0.8, 2.0, 0.8, 2.0]),
                        vec![6.0, 2.0, 0.8, 2.0, 0.8, 2.0],
                    ),
                ] {
                    let response = ui.selectable_value(
                        &mut stroke.dash_pt,
                        pattern.clone(),
                        format!("            {name}"),
                    );
                    paint_dash_preview(ui.painter(), response.rect, &pattern);
                    edit.changed |= response.changed();
                }
            });
    });
    edit
}

pub(super) fn paint_dash_preview(painter: &egui::Painter, row: egui::Rect, pattern: &[f64]) {
    let start = row.left() + 8.0;
    let end = start + 52.0;
    let y = row.center().y;
    let stroke = egui::Stroke::new(1.8, egui::Color32::from_rgb(68, 119, 170));
    if pattern.is_empty() {
        painter.line_segment([egui::pos2(start, y), egui::pos2(end, y)], stroke);
        return;
    }
    let mut x = start;
    let mut index = 0;
    while x < end {
        let length = pattern[index % pattern.len()].max(0.5) as f32 * 2.0;
        let next = (x + length).min(end);
        if index % 2 == 0 {
            painter.line_segment([egui::pos2(x, y), egui::pos2(next, y)], stroke);
        }
        x = next;
        index += 1;
    }
}

pub(super) const PRODUCT_MARKER_SHAPES: [MarkerShape; 7] = [
    MarkerShape::Circle,
    MarkerShape::Square,
    MarkerShape::Triangle,
    MarkerShape::TriangleDown,
    MarkerShape::Diamond,
    MarkerShape::Pentagon,
    MarkerShape::Star,
];

pub(super) fn paint_marker_preview(
    painter: &egui::Painter,
    row: egui::Rect,
    shape: MarkerShape,
    filled: bool,
    color: egui::Color32,
) {
    let center = egui::pos2(row.left() + 17.0, row.center().y);
    let radius = 5.0;
    let outline = egui::Stroke::new(1.8, color);
    match shape {
        MarkerShape::Circle => {
            if filled {
                painter.circle_filled(center, radius, color);
            } else {
                painter.circle_stroke(center, radius, outline);
            }
        }
        MarkerShape::Square => {
            let bounds = egui::Rect::from_center_size(center, egui::vec2(10.0, 10.0));
            if filled {
                painter.rect_filled(bounds, 0.0, color);
            } else {
                painter.rect_stroke(bounds, 0.0, outline, egui::StrokeKind::Middle);
            }
        }
        MarkerShape::Triangle => {
            paint_marker_polygon(
                painter,
                vec![
                    center + egui::vec2(0.0, -radius),
                    center + egui::vec2(radius, radius),
                    center + egui::vec2(-radius, radius),
                ],
                filled,
                color,
            );
        }
        MarkerShape::TriangleDown => {
            paint_marker_polygon(
                painter,
                vec![
                    center + egui::vec2(-radius, -radius),
                    center + egui::vec2(radius, -radius),
                    center + egui::vec2(0.0, radius),
                ],
                filled,
                color,
            );
        }
        MarkerShape::Diamond => {
            paint_marker_polygon(
                painter,
                vec![
                    center + egui::vec2(0.0, -radius),
                    center + egui::vec2(radius, 0.0),
                    center + egui::vec2(0.0, radius),
                    center + egui::vec2(-radius, 0.0),
                ],
                filled,
                color,
            );
        }
        MarkerShape::Pentagon => {
            paint_marker_polygon(
                painter,
                marker_preview_points(center, radius, 5, None),
                filled,
                color,
            );
        }
        MarkerShape::Star => {
            let points = marker_preview_points(center, radius, 5, Some(0.46));
            if filled {
                for index in 0..points.len() {
                    painter.add(egui::Shape::convex_polygon(
                        vec![center, points[index], points[(index + 1) % points.len()]],
                        color,
                        egui::Stroke::NONE,
                    ));
                }
            } else {
                painter.add(egui::Shape::closed_line(points, outline));
            }
        }
        MarkerShape::Plus => {
            let stroke = egui::Stroke::new(1.8, color);
            painter.line_segment(
                [
                    center + egui::vec2(-radius, 0.0),
                    center + egui::vec2(radius, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(0.0, -radius),
                    center + egui::vec2(0.0, radius),
                ],
                stroke,
            );
        }
        MarkerShape::Cross => {
            let stroke = egui::Stroke::new(1.8, color);
            painter.line_segment(
                [
                    center + egui::vec2(-radius, -radius),
                    center + egui::vec2(radius, radius),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(-radius, radius),
                    center + egui::vec2(radius, -radius),
                ],
                stroke,
            );
        }
    }
}

pub(super) fn paint_marker_polygon(
    painter: &egui::Painter,
    points: Vec<egui::Pos2>,
    filled: bool,
    color: egui::Color32,
) {
    if filled {
        painter.add(egui::Shape::convex_polygon(
            points,
            color,
            egui::Stroke::NONE,
        ));
    } else {
        painter.add(egui::Shape::closed_line(
            points,
            egui::Stroke::new(1.8, color),
        ));
    }
}

pub(super) fn marker_preview_points(
    center: egui::Pos2,
    radius: f32,
    corners: usize,
    inner_ratio: Option<f32>,
) -> Vec<egui::Pos2> {
    let vertices = corners * if inner_ratio.is_some() { 2 } else { 1 };
    (0..vertices)
        .map(|index| {
            let angle = -std::f32::consts::FRAC_PI_2
                + 2.0 * std::f32::consts::PI * index as f32 / vertices as f32;
            let current_radius = if index % 2 == 1 {
                inner_ratio.map_or(radius, |ratio| radius * ratio)
            } else {
                radius
            };
            center + egui::vec2(current_radius * angle.cos(), current_radius * angle.sin())
        })
        .collect()
}

pub(super) fn move_legend_entry<T>(entries: &mut Vec<T>, from: usize, to: usize) {
    if from != to && from < entries.len() && to < entries.len() {
        let entry = entries.remove(from);
        entries.insert(to, entry);
    }
}

pub(super) fn position_editor(
    ui: &mut egui::Ui,
    language: UiLanguage,
    x_pt: &mut f64,
    y_pt: &mut f64,
    canvas_width_mm: f64,
    canvas_height_mm: f64,
) -> UiEdit {
    const POINTS_PER_MM: f64 = 72.0 / 25.4;
    let mut x_mm = *x_pt / POINTS_PER_MM;
    let mut y_mm = *y_pt / POINTS_PER_MM;
    let (x, y) = ui
        .horizontal(|ui| {
            let x = ui.add(
                egui::DragValue::new(&mut x_mm)
                    .range(0.0..=canvas_width_mm)
                    .speed(0.1)
                    .prefix(format!("{}: ", language.text(Text::XPosition))),
            );
            let y = ui.add(
                egui::DragValue::new(&mut y_mm)
                    .range(0.0..=canvas_height_mm)
                    .speed(0.1)
                    .prefix(format!("{}: ", language.text(Text::YPosition))),
            );
            (x, y)
        })
        .inner;
    if x.changed() {
        *x_pt = x_mm * POINTS_PER_MM;
    }
    if y.changed() {
        *y_pt = y_mm * POINTS_PER_MM;
    }
    let mut edit = continuous_edit(&x);
    edit.merge(continuous_edit(&y));
    edit
}

pub(super) fn manual_measurement_inputs(
    ui: &mut egui::Ui,
    axis_name: &str,
    axis_index: usize,
    measurements: &mut Vec<String>,
) {
    let mut remove = None;
    let count = measurements.len();
    for (measurement_index, text) in measurements.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("测量 {}", measurement_index + 1));
            ui.add_sized(
                [
                    ui.available_width() - if count > 1 { 36.0 } else { 0.0 },
                    72.0,
                ],
                egui::TextEdit::multiline(text)
                    .id_salt((
                        "manual-measurement",
                        axis_name,
                        axis_index,
                        measurement_index,
                    ))
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            if count > 1 && studio_close_button_sized(ui, "删除本次测量", 28.0).clicked() {
                remove = Some(measurement_index);
            }
        });
    }
    if let Some(index) = remove {
        measurements.remove(index);
    }
    if ui.button("＋ 重复测量（用于误差棒）").clicked() {
        measurements.push(String::new());
    }
}

pub(super) fn manual_group_input_card(
    ui: &mut egui::Ui,
    index: usize,
    group: &mut ManualDataGroupInput,
    saved: bool,
    editable: bool,
    can_remove: bool,
) -> (bool, bool) {
    let mut edit = false;
    let mut remove = false;
    studio_file_card_frame(ui.ctx().theme() == egui::Theme::Dark).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.strong(format!("数据组 {}", index + 1));
            if editable {
                ui.add_sized(
                    [220.0, 30.0],
                    egui::TextEdit::singleline(&mut group.source_name),
                );
            } else {
                ui.label(&group.source_name);
            }
            if saved && !editable && ui.button("编辑").clicked() {
                edit = true;
            }
            if can_remove && studio_close_button_sized(ui, "删除数据组", 28.0).clicked() {
                remove = true;
            }
        });
        ui.add_enabled_ui(editable, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("图形");
                egui::ComboBox::from_id_salt(("manual-style", index))
                    .selected_text(match group.plot_style {
                        ManualPlotStyle::Line => "曲线",
                        ManualPlotStyle::Scatter => "散点",
                        ManualPlotStyle::LineAndMarker => "曲线 + 点",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut group.plot_style,
                            ManualPlotStyle::LineAndMarker,
                            "曲线 + 点",
                        );
                        ui.selectable_value(
                            &mut group.plot_style,
                            ManualPlotStyle::Scatter,
                            "散点",
                        );
                        ui.selectable_value(&mut group.plot_style, ManualPlotStyle::Line, "曲线");
                    });
                ui.label("重复测量误差");
                egui::ComboBox::from_id_salt(("manual-statistic", index))
                    .selected_text(match group.error_statistic {
                        ErrorStatistic::StandardDeviation => "样本标准差 (SD)",
                        ErrorStatistic::StandardError => "标准误 (SEM)",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut group.error_statistic,
                            ErrorStatistic::StandardDeviation,
                            "样本标准差 (SD)",
                        );
                        ui.selectable_value(
                            &mut group.error_statistic,
                            ErrorStatistic::StandardError,
                            "标准误 (SEM)",
                        );
                    });
            });
            ui.columns(2, |columns| {
                columns[0].horizontal(|ui| {
                    ui.strong("X");
                    ui.add_sized([160.0, 30.0], egui::TextEdit::singleline(&mut group.x.name));
                });
                manual_measurement_inputs(&mut columns[0], "X", index, &mut group.x.measurements);
                columns[1].horizontal(|ui| {
                    ui.strong("Y");
                    ui.add_sized([160.0, 30.0], egui::TextEdit::singleline(&mut group.y.name));
                });
                manual_measurement_inputs(&mut columns[1], "Y", index, &mut group.y.measurements);
            });
        });
    });
    ui.add_space(6.0);
    (edit, remove)
}

pub(super) fn minor_interval_editor(
    ui: &mut egui::Ui,
    language: UiLanguage,
    identity: AxisIdentity,
    record: &mut AxisRecord,
    numeric_inputs: &mut BTreeMap<String, DeferredNumericInput>,
) -> UiEdit {
    if record.scale != AxisScale::Linear {
        return UiEdit::default();
    }
    let mut edit = UiEdit::default();
    ui.horizontal_wrapped(|ui| {
        ui.label(language.text(Text::MinorTickInterval));
        egui::ComboBox::from_id_salt(("minor-interval-mode", identity))
            .selected_text(language.text(if record.minor_interval.is_some() {
                Text::Interval
            } else {
                Text::Auto
            }))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(record.minor_interval.is_none(), language.text(Text::Auto))
                    .clicked()
                    && record.minor_interval.is_some()
                {
                    record.minor_interval = None;
                    edit.changed = true;
                }
                if ui
                    .selectable_label(
                        record.minor_interval.is_some(),
                        language.text(Text::Interval),
                    )
                    .clicked()
                    && record.minor_interval.is_none()
                {
                    record.minor_interval = Some(suggested_minor_interval(record));
                    edit.changed = true;
                }
            });
        if let Some(step) = &mut record.minor_interval {
            edit.merge(deferred_f64_editor(
                ui,
                numeric_inputs,
                format!("axis-{identity:?}-minor-interval"),
                step,
                f64::MIN_POSITIVE..=f64::INFINITY,
                112.0,
            ));
        }
    });
    edit
}

pub(super) fn suggested_minor_interval(record: &AxisRecord) -> f64 {
    let range = record.maximum - record.minimum;
    let candidate = match &record.locator {
        LocatorSpec::Interval { step } => step / 5.0,
        LocatorSpec::Fixed { values } if values.len() > 1 => (values[1] - values[0]) / 5.0,
        LocatorSpec::Auto { target_count } => range / (5.0 * f64::from(*target_count)),
        LocatorSpec::Fixed { .. } => range / 25.0,
    };
    let minimum = range / 100.0;
    if candidate.is_finite() && candidate > 0.0 {
        candidate.max(minimum)
    } else if minimum.is_finite() && minimum > 0.0 {
        minimum
    } else {
        1.0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct UiEdit {
    pub(super) changed: bool,
    pub(super) continuous: bool,
    pub(super) finish: bool,
}

impl UiEdit {
    pub(super) fn merge(&mut self, other: Self) {
        self.changed |= other.changed;
        self.continuous |= other.continuous;
        self.finish |= other.finish;
    }
}

pub(super) fn continuous_edit(response: &egui::Response) -> UiEdit {
    UiEdit {
        changed: response.changed(),
        continuous: response.changed(),
        finish: response.drag_stopped() || response.lost_focus(),
    }
}

pub(super) fn deferred_f64_editor(
    ui: &mut egui::Ui,
    inputs: &mut BTreeMap<String, DeferredNumericInput>,
    key: impl Into<String>,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    width: f32,
) -> UiEdit {
    let key = key.into();
    let id = egui::Id::new(("deferred-number", &key));
    let was_focused = ui.memory(|memory| memory.has_focus(id));
    let state = inputs.entry(key).or_insert_with(|| DeferredNumericInput {
        source_value: *value,
        text: value.to_string(),
        error: None,
    });
    if !was_focused && state.source_value.to_bits() != value.to_bits() {
        state.source_value = *value;
        state.text = value.to_string();
        state.error = None;
    }
    let response = ui.add_sized(
        [width, 30.0],
        egui::TextEdit::singleline(&mut state.text).id(id),
    );
    let escape = response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape));
    let enter = response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    if escape {
        state.text = value.to_string();
        state.source_value = *value;
        state.error = None;
        ui.memory_mut(|memory| memory.surrender_focus(id));
        return UiEdit {
            finish: true,
            ..UiEdit::default()
        };
    }
    let commit = enter || response.lost_focus();
    let mut edit = UiEdit {
        finish: commit,
        ..UiEdit::default()
    };
    if commit {
        match state.text.trim().parse::<f64>() {
            Ok(parsed) if parsed.is_finite() && range.contains(&parsed) => {
                edit.changed = parsed.to_bits() != value.to_bits();
                *value = parsed;
                state.source_value = parsed;
                state.text = parsed.to_string();
                state.error = None;
            }
            _ => {
                state.error = Some("请输入有效数值后按 Enter".to_owned());
            }
        }
        if enter {
            ui.memory_mut(|memory| memory.surrender_focus(id));
        }
    }
    if let Some(error) = &state.error {
        ui.painter().rect_stroke(
            response.rect,
            4.0,
            egui::Stroke::new(1.0, egui::Color32::LIGHT_RED),
            egui::StrokeKind::Inside,
        );
        response.on_hover_text(error);
    }
    edit
}

pub(super) fn artist_role_name(language: UiLanguage, role: ArtistRole) -> &'static str {
    language.text(match role {
        ArtistRole::Data => Text::DataRole,
        ArtistRole::Fit => Text::FitRole,
        ArtistRole::Theory => Text::TheoryRole,
        ArtistRole::Reference => Text::ReferenceRole,
        ArtistRole::Baseline => Text::BaselineRole,
        ArtistRole::Annotation => Text::Annotation,
        ArtistRole::Legend => Text::Legend,
    })
}

pub(super) fn series_style_name(language: UiLanguage, style: SeriesCreationStyle) -> &'static str {
    match (language, style) {
        (UiLanguage::Chinese, SeriesCreationStyle::Scatter) => "散点",
        (UiLanguage::Chinese, SeriesCreationStyle::Line) => "曲线",
        (UiLanguage::Chinese, SeriesCreationStyle::LineAndMarker) => "曲线＋点",
        (UiLanguage::English, SeriesCreationStyle::Scatter) => "Scatter",
        (UiLanguage::English, SeriesCreationStyle::Line) => "Line",
        (UiLanguage::English, SeriesCreationStyle::LineAndMarker) => "Line + markers",
    }
}

pub(super) fn marker_shape_name(language: UiLanguage, shape: MarkerShape) -> &'static str {
    language.text(match shape {
        MarkerShape::Circle => Text::Circle,
        MarkerShape::Square => Text::Square,
        MarkerShape::Triangle => Text::Triangle,
        MarkerShape::TriangleDown => Text::TriangleDown,
        MarkerShape::Diamond => Text::Diamond,
        MarkerShape::Pentagon => Text::Pentagon,
        MarkerShape::Star => Text::Star,
        MarkerShape::Plus => Text::Plus,
        MarkerShape::Cross => Text::Cross,
    })
}

pub(super) fn reference_orientation_name(
    language: UiLanguage,
    orientation: ReferenceOrientation,
) -> &'static str {
    language.text(match orientation {
        ReferenceOrientation::Horizontal => Text::Horizontal,
        ReferenceOrientation::Vertical => Text::Vertical,
    })
}

pub(super) fn dash_name(language: UiLanguage, pattern: &[f64]) -> &'static str {
    if pattern.is_empty() {
        language.text(Text::Solid)
    } else if pattern == [4.0, 2.4] {
        language.text(Text::Dashed)
    } else if pattern == [0.8, 1.8] {
        language.text(Text::Dotted)
    } else if pattern == [8.0, 3.0] {
        match language {
            UiLanguage::Chinese => "长虚线",
            UiLanguage::English => "Long dash",
        }
    } else if pattern == [8.0, 2.0, 2.0, 2.0] {
        match language {
            UiLanguage::Chinese => "长短划线",
            UiLanguage::English => "Long-short dash",
        }
    } else if pattern == [6.0, 2.0, 0.8, 2.0, 0.8, 2.0] {
        match language {
            UiLanguage::Chinese => "双点划线",
            UiLanguage::English => "Dash-dot-dot",
        }
    } else {
        language.text(Text::DashDot)
    }
}
