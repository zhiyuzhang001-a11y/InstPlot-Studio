use std::collections::{BTreeMap, BTreeSet};

use instplot_export::ResolvedItem;
use instplot_layout::SelectableRole;
use instplot_render::DisplayItem;
use serde::{Deserialize, Serialize};

use crate::palette::{PaletteKind, builtin_palette, registry_matches_metadata};
use crate::semantic::color_id;
use crate::{ArtistProperties, FigureDocument, ProjectDocument, ResolvedFigure};

pub const PUBLICATION_RULES_VERSION: &str = "instplot-publication-rules-v1";
pub const CVD_SIMULATION_VERSION: &str = "machado-2009-deuteranopia-severity-1-linear-srgb-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckSeverity {
    Error,
    Warning,
    Information,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationFinding {
    pub rule_id: String,
    pub severity: CheckSeverity,
    pub node_id: Option<String>,
    pub message: String,
    pub impact: String,
    pub remediation: String,
    pub overridden: bool,
    pub override_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationReport {
    pub rules_version: String,
    pub raster_dpi: u32,
    pub findings: Vec<PublicationFinding>,
}

impl PublicationReport {
    pub fn error_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == CheckSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == CheckSeverity::Warning)
            .count()
    }

    pub fn information_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == CheckSeverity::Information)
            .count()
    }
}

pub fn check_publication(
    document: &FigureDocument,
    resolved: &ResolvedFigure,
    raster_dpi: u32,
) -> PublicationReport {
    let project = document.project();
    let mut findings = vec![
        physical_size(project),
        font_size(resolved),
        stroke_width(resolved),
        font_embedding(resolved),
        clipping(resolved),
        legend_overlap(resolved),
        color_only_encoding(project),
        palette_relationship(project),
        grayscale_distinguishability(project),
        cvd_risk(project),
        raster_dimensions(project, resolved, raster_dpi),
        transparency(project),
        provenance(project),
    ];
    for finding in &mut findings {
        apply_override(project, finding);
    }
    PublicationReport {
        rules_version: PUBLICATION_RULES_VERSION.to_owned(),
        raster_dpi,
        findings,
    }
}

fn finding(
    rule_id: &str,
    severity: CheckSeverity,
    node_id: Option<String>,
    message: impl Into<String>,
) -> PublicationFinding {
    let (impact, remediation) = finding_guidance(rule_id);
    PublicationFinding {
        rule_id: rule_id.to_owned(),
        severity,
        node_id,
        message: message.into(),
        impact: impact.to_owned(),
        remediation: remediation.to_owned(),
        overridden: false,
        override_reason: None,
    }
}

fn finding_guidance(rule_id: &str) -> (&'static str, &'static str) {
    match rule_id {
        "physical_size" => (
            "The final physical dimensions control readability and journal placement.",
            "Choose an allowed figure width and height in Figure Size.",
        ),
        "font_size" => (
            "Text that is too small may be unreadable after publication scaling.",
            "Shorten labels or increase the final figure size; do not rely on preview zoom.",
        ),
        "stroke_width" => (
            "Thin strokes can disappear in print or raster conversion.",
            "Select the reported artist and increase its line or error-bar width.",
        ),
        "font_embedding" => (
            "Missing embedded fonts can change scientific symbols on another computer.",
            "Keep the bundled TeX Gyre Heros profile and export again as PDF.",
        ),
        "clipping" => (
            "Clipped labels or marks make the exported figure incomplete.",
            "Inspect the reported object, spacing, axis range, and final figure size.",
        ),
        "legend_overlap" => (
            "A legend covering data can conceal evidence or make curves ambiguous.",
            "Move the legend or reduce/reorder its visible entries.",
        ),
        "color_only_encoding" | "grayscale_distinguishability" | "cvd_risk" => (
            "Readers may not be able to distinguish objects from colour alone.",
            "Use distinct marker shapes or dash patterns in addition to colour.",
        ),
        "palette_data_relationship" => (
            "Untracked colour relationships can break source/fit identity.",
            "Use the project palette and keep related source and fit colours aligned.",
        ),
        "raster_dpi_pixels" => (
            "Insufficient raster dimensions reduce detail and may fail submission checks.",
            "Choose 300 dpi or higher and verify the shown pixel dimensions.",
        ),
        "transparency" => (
            "Transparency may be flattened differently by journal production systems.",
            "Use an opaque white background unless the target journal explicitly allows alpha.",
        ),
        "provenance_completeness" => (
            "Incomplete provenance makes styling and data relationships harder to audit.",
            "Keep palette, font, data, and explicit style records in the Figure Document.",
        ),
        _ => (
            "This check affects publication reliability.",
            "Inspect the reported object and adjust its applicable Inspector settings.",
        ),
    }
}

fn physical_size(project: &ProjectDocument) -> PublicationFinding {
    let width = project.figure.width_mm;
    let height = project.figure.height_mm;
    let severity = if width < 30.0 || height < 30.0 || width > 250.0 || height > 250.0 {
        CheckSeverity::Warning
    } else {
        CheckSeverity::Information
    };
    finding(
        "physical_size",
        severity,
        Some(project.figure.id.clone()),
        format!("figure physical size is {width:.2} × {height:.2} mm"),
    )
}

fn font_size(resolved: &ResolvedFigure) -> PublicationFinding {
    let smallest = resolved
        .display
        .items
        .iter()
        .filter_map(|item| match item {
            ResolvedItem::Text(text) => Some((text.source, text.size as f64)),
            ResolvedItem::Graphics(_) => None,
        })
        .min_by(|left, right| left.1.total_cmp(&right.1));
    let Some((node, size)) = smallest else {
        return finding(
            "font_size",
            CheckSeverity::Error,
            None,
            "resolved figure contains no text",
        );
    };
    let severity = if size < 5.0 {
        CheckSeverity::Error
    } else if size < 7.0 {
        CheckSeverity::Warning
    } else {
        CheckSeverity::Information
    };
    finding(
        "font_size",
        severity,
        project_id(resolved, node),
        format!("minimum resolved font size is {size:.2} pt"),
    )
}

fn stroke_width(resolved: &ResolvedFigure) -> PublicationFinding {
    let smallest = resolved
        .display
        .items
        .iter()
        .filter_map(|item| match item {
            ResolvedItem::Graphics(DisplayItem::Path {
                source,
                stroke: Some(stroke),
                ..
            }) => Some((*source, stroke.width.get())),
            _ => None,
        })
        .min_by(|left, right| left.1.total_cmp(&right.1));
    let Some((node, width)) = smallest else {
        return finding(
            "stroke_width",
            CheckSeverity::Error,
            None,
            "resolved figure contains no stroked paths",
        );
    };
    let severity = if width < 0.2 {
        CheckSeverity::Error
    } else if width < 0.35 {
        CheckSeverity::Warning
    } else {
        CheckSeverity::Information
    };
    finding(
        "stroke_width",
        severity,
        project_id(resolved, node),
        format!("minimum resolved stroke width is {width:.2} pt"),
    )
}

fn font_embedding(resolved: &ResolvedFigure) -> PublicationFinding {
    let diagnostics = resolved.display.font_diagnostics();
    if let Some(problem) = diagnostics.iter().find(|diagnostic| {
        diagnostic.missing_glyph
            || !diagnostic.embedding_allowed
            || !diagnostic.subsetting_allowed
            || !matches!(
                diagnostic.origin,
                instplot_export::FontOrigin::BundledPrimary
                    | instplot_export::FontOrigin::BundledSymbol
            )
    }) {
        return finding(
            "font_embedding",
            CheckSeverity::Error,
            project_id(resolved, problem.source),
            format!(
                "font {} is not a complete embeddable bundled run",
                problem.postscript_name
            ),
        );
    }
    match instplot_export::to_pdf(&resolved.display) {
        Ok(pdf) if pdf.windows(9).any(|window| window == b"/FontFile") => finding(
            "font_embedding",
            CheckSeverity::Information,
            None,
            format!(
                "{} resolved font runs are bundled and the PDF contains embedded font streams",
                diagnostics.len()
            ),
        ),
        Ok(_) => finding(
            "font_embedding",
            CheckSeverity::Error,
            None,
            "PDF does not contain an embedded font stream",
        ),
        Err(error) => finding(
            "font_embedding",
            CheckSeverity::Error,
            None,
            format!("PDF font verification failed: {error}"),
        ),
    }
}

fn clipping(resolved: &ResolvedFigure) -> PublicationFinding {
    let pushes = resolved
        .display
        .items
        .iter()
        .filter(|item| matches!(item, ResolvedItem::Graphics(DisplayItem::ClipPush { .. })))
        .count();
    let pops = resolved
        .display
        .items
        .iter()
        .filter(|item| matches!(item, ResolvedItem::Graphics(DisplayItem::ClipPop { .. })))
        .count();
    let valid = pushes > 0 && pushes == pops;
    finding(
        "clipping",
        if valid {
            CheckSeverity::Information
        } else {
            CheckSeverity::Error
        },
        None,
        format!("data clipping stack contains {pushes} push and {pops} pop operations"),
    )
}

fn legend_overlap(resolved: &ResolvedFigure) -> PublicationFinding {
    let Some(legend) = resolved
        .layout
        .result
        .hit_map
        .items
        .iter()
        .find(|item| item.role == SelectableRole::Legend)
    else {
        return finding(
            "legend_overlap",
            CheckSeverity::Information,
            None,
            "figure has no legend",
        );
    };
    let overlaps = resolved.layout.result.hit_map.items.iter().any(|item| {
        if item.node == legend.node {
            return false;
        }
        match item.role {
            SelectableRole::Series => item
                .path_proximity
                .windows(2)
                .any(|segment| segment_intersects_bounds(segment[0], segment[1], legend.bounds)),
            SelectableRole::DataPoint | SelectableRole::ErrorBar => {
                legend.bounds.intersection_area(item.bounds) > 0.5
            }
            _ => false,
        }
    });
    finding(
        "legend_overlap",
        if overlaps {
            CheckSeverity::Warning
        } else {
            CheckSeverity::Information
        },
        project_id(resolved, legend.node),
        if overlaps {
            "legend bounds overlap at least one plotted artist"
        } else {
            "legend bounds do not overlap plotted artists"
        },
    )
}

fn segment_intersects_bounds(
    start: (f64, f64),
    end: (f64, f64),
    bounds: instplot_layout::Bounds,
) -> bool {
    let inside = |(x, y): (f64, f64)| {
        x >= bounds.x && x <= bounds.right() && y >= bounds.y && y <= bounds.bottom()
    };
    if inside(start) || inside(end) {
        return true;
    }
    let corners = [
        (bounds.x, bounds.y),
        (bounds.right(), bounds.y),
        (bounds.right(), bounds.bottom()),
        (bounds.x, bounds.bottom()),
    ];
    corners
        .iter()
        .copied()
        .zip(corners.iter().copied().cycle().skip(1))
        .take(4)
        .any(|(left, right)| segments_intersect(start, end, left, right))
}

fn segments_intersect(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    fn cross(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    }
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    ab_c * ab_d <= 0.0 && cd_a * cd_b <= 0.0
}

fn color_only_encoding(project: &ProjectDocument) -> PublicationFinding {
    let risky = risky_color_pair(project, |left, right| left != right);
    match risky {
        Some((left, right)) => finding(
            "color_only_encoding",
            CheckSeverity::Warning,
            Some(left.clone()),
            format!("artists {left} and {right} differ only by colour"),
        ),
        None => finding(
            "color_only_encoding",
            CheckSeverity::Information,
            None,
            "scientific roles retain marker, dash, or position encoding in addition to colour",
        ),
    }
}

fn palette_relationship(project: &ProjectDocument) -> PublicationFinding {
    let metadata = builtin_palette(&project.palette.id);
    let valid = metadata
        .is_some_and(|entry| entry.recommended && registry_matches_metadata(&project.palette));
    let valid_message =
        metadata.map_or(
            "active palette is registered and recommended",
            |entry| match entry.kind {
                PaletteKind::Qualitative => {
                    "active palette is recommended for independent categorical series"
                }
                PaletteKind::Sequential => {
                    "active palette is recommended for series ordered from low to high"
                }
                PaletteKind::Diverging => {
                    "active palette is recommended for series ordered around a meaningful midpoint"
                }
                PaletteKind::Neutral => {
                    "active palette is recommended for theory, reference, or supporting series"
                }
            },
        );
    finding(
        "palette_data_relationship",
        if valid {
            CheckSeverity::Information
        } else {
            CheckSeverity::Warning
        },
        Some(project.figure.id.clone()),
        if valid {
            valid_message
        } else {
            "active palette metadata is missing or does not match the registered palette"
        },
    )
}

fn grayscale_distinguishability(project: &ProjectDocument) -> PublicationFinding {
    let risky = risky_color_pair(project, |left, right| {
        (relative_luminance(left) - relative_luminance(right)).abs() < 0.08
    });
    match risky {
        Some((left, right)) => finding(
            "grayscale_distinguishability",
            CheckSeverity::Warning,
            Some(left.clone()),
            format!(
                "artists {left} and {right} become similar in grayscale without redundant style"
            ),
        ),
        None => finding(
            "grayscale_distinguishability",
            CheckSeverity::Information,
            None,
            "grayscale-similar colours retain distinct non-colour encodings",
        ),
    }
}

fn cvd_risk(project: &ProjectDocument) -> PublicationFinding {
    let risky = risky_color_pair(project, |left, right| {
        let left = simulate_deuteranopia(left);
        let right = simulate_deuteranopia(right);
        color_distance(left, right) < 36.0
    });
    match risky {
        Some((left, right)) => finding(
            "cvd_risk",
            CheckSeverity::Warning,
            Some(left.clone()),
            format!(
                "artists {left} and {right} are difficult to distinguish under deuteranopia simulation"
            ),
        ),
        None => finding(
            "cvd_risk",
            CheckSeverity::Information,
            None,
            "deuteranopia-similar colours retain distinct non-colour encodings",
        ),
    }
}

fn raster_dimensions(
    project: &ProjectDocument,
    resolved: &ResolvedFigure,
    dpi: u32,
) -> PublicationFinding {
    let listed = project.export_preferences.raster_dpi.contains(&dpi);
    let dimensions = instplot_export::checked_raster_dimensions(
        f64::from(resolved.display.width),
        f64::from(resolved.display.height),
        dpi,
    );
    let Ok((width, height)) = dimensions else {
        return finding(
            "raster_dpi_pixels",
            CheckSeverity::Error,
            Some(project.figure.id.clone()),
            format!(
                "PNG export cannot use {dpi} dpi at the final canvas size: {}",
                dimensions.unwrap_err()
            ),
        );
    };
    let severity = if dpi < 150 {
        CheckSeverity::Error
    } else if dpi < 300 || !listed {
        CheckSeverity::Warning
    } else {
        CheckSeverity::Information
    };
    finding(
        "raster_dpi_pixels",
        severity,
        Some(project.figure.id.clone()),
        format!("{dpi} dpi export resolves to {width} × {height} pixels"),
    )
}

fn transparency(project: &ProjectDocument) -> PublicationFinding {
    let transparent_color = project
        .palette
        .colors
        .iter()
        .find(|color| color.rgba[3] != 255);
    let risky = project.export_preferences.transparent_background || transparent_color.is_some();
    let node_id = transparent_color
        .and_then(|color| {
            project
                .figure
                .artists
                .iter()
                .find(|artist| color_id(artist) == Some(color.id.as_str()))
                .map(|artist| artist.id.clone())
        })
        .or_else(|| Some(project.figure.id.clone()));
    finding(
        "transparency",
        if risky {
            CheckSeverity::Warning
        } else {
            CheckSeverity::Information
        },
        node_id,
        if risky {
            "transparent background or palette alpha requires journal-specific verification"
        } else {
            "export background and palette colours are fully opaque"
        },
    )
}

fn provenance(project: &ProjectDocument) -> PublicationFinding {
    let palette_complete = registry_matches_metadata(&project.palette);
    let document_complete = !project.provenance.is_empty()
        && project
            .provenance
            .iter()
            .all(|record| !record.id.is_empty() && !record.operation.is_empty());
    finding(
        "provenance_completeness",
        if palette_complete && document_complete {
            CheckSeverity::Information
        } else {
            CheckSeverity::Error
        },
        Some(project.figure.id.clone()),
        if palette_complete && document_complete {
            "palette and document provenance are complete and version-pinned"
        } else {
            "palette or document provenance is incomplete"
        },
    )
}

fn project_id(resolved: &ResolvedFigure, node: instplot_render::NodeId) -> Option<String> {
    resolved.layout.project_ids.get(&node).cloned()
}

#[derive(Clone, Debug)]
struct VisualSeriesEncoding {
    id: String,
    color: [u8; 4],
    non_color_signature: String,
}

#[derive(Clone, Debug)]
struct VisualSeriesBuilder {
    id: String,
    color: [u8; 4],
    representative_rank: usize,
    line_signatures: BTreeSet<String>,
    marker_signatures: BTreeSet<String>,
}

fn visual_series_encodings(project: &ProjectDocument) -> Vec<VisualSeriesEncoding> {
    let colors = project
        .palette
        .colors
        .iter()
        .map(|color| (color.id.as_str(), color.rgba))
        .collect::<BTreeMap<_, _>>();
    let legend_ranks = project
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Legend { entries, .. }
                if project.artist_effectively_visible(artist) =>
            {
                Some(entries)
            }
            _ => None,
        })
        .flat_map(|entries| {
            entries.iter().filter(|entry| {
                entry.visible && project.artist_id_effectively_visible(&entry.artist_id)
            })
        })
        .enumerate()
        .map(|(rank, entry)| (entry.artist_id.clone(), rank))
        .collect::<BTreeMap<_, _>>();
    let mut builders = BTreeMap::<String, VisualSeriesBuilder>::new();
    for artist in project
        .figure
        .artists
        .iter()
        .filter(|artist| project.artist_effectively_visible(artist))
    {
        let Some(color) = color_id(artist).and_then(|id| colors.get(id).copied()) else {
            continue;
        };
        let key = match &artist.properties {
            ArtistProperties::Line { .. }
            | ArtistProperties::Scatter { .. }
            | ArtistProperties::ErrorBar { .. } => {
                project.series_group_for_artist(&artist.id).map_or_else(
                    || format!("ungrouped:{}", artist.id),
                    |group| group.id.clone(),
                )
            }
            ArtistProperties::ReferenceLine { .. } => format!("reference:{}", artist.id),
            ArtistProperties::Annotation { .. } | ArtistProperties::Legend { .. } => continue,
        };
        let rank = legend_ranks.get(&artist.id).copied().unwrap_or(usize::MAX);
        let builder = builders.entry(key).or_insert_with(|| VisualSeriesBuilder {
            id: artist.id.clone(),
            color,
            representative_rank: rank,
            line_signatures: BTreeSet::new(),
            marker_signatures: BTreeSet::new(),
        });
        if rank < builder.representative_rank {
            builder.id.clone_from(&artist.id);
            builder.color = color;
            builder.representative_rank = rank;
        }
        match &artist.properties {
            ArtistProperties::Line { stroke, .. }
            | ArtistProperties::ReferenceLine { stroke, .. } => {
                builder
                    .line_signatures
                    .insert(format!("{:?}", stroke.dash_pt));
            }
            ArtistProperties::Scatter { marker, .. } => {
                builder
                    .marker_signatures
                    .insert(format!("{:?}:{}", marker.shape, marker.filled));
            }
            ArtistProperties::ErrorBar { .. }
            | ArtistProperties::Annotation { .. }
            | ArtistProperties::Legend { .. } => {}
        }
    }
    builders
        .into_values()
        .map(|builder| VisualSeriesEncoding {
            id: builder.id,
            color: builder.color,
            non_color_signature: format!(
                "line={:?};marker={:?}",
                builder.line_signatures, builder.marker_signatures
            ),
        })
        .collect()
}

fn risky_color_pair(
    project: &ProjectDocument,
    color_is_risky: impl Fn([u8; 4], [u8; 4]) -> bool,
) -> Option<(String, String)> {
    let series = visual_series_encodings(project);
    for (index, left) in series.iter().enumerate() {
        for right in series.iter().skip(index + 1) {
            if left.color != right.color
                && color_is_risky(left.color, right.color)
                && left.non_color_signature == right.non_color_signature
            {
                return Some((left.id.clone(), right.id.clone()));
            }
        }
    }
    None
}

fn relative_luminance(color: [u8; 4]) -> f64 {
    fn channel(value: u8) -> f64 {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color[0]) + 0.7152 * channel(color[1]) + 0.0722 * channel(color[2])
}

fn simulate_deuteranopia(color: [u8; 4]) -> [f64; 3] {
    let linear = [
        srgb_channel_to_linear(color[0]),
        srgb_channel_to_linear(color[1]),
        srgb_channel_to_linear(color[2]),
    ];
    // Machado, Oliveira and Fernandes (2009), severity 1.0 deuteranopia.
    let transformed = [
        0.367_322 * linear[0] + 0.860_646 * linear[1] - 0.227_968 * linear[2],
        0.280_085 * linear[0] + 0.672_501 * linear[1] + 0.047_413 * linear[2],
        -0.011_820 * linear[0] + 0.042_940 * linear[1] + 0.968_881 * linear[2],
    ];
    transformed.map(linear_channel_to_srgb)
}

fn srgb_channel_to_linear(value: u8) -> f64 {
    let value = f64::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_channel_to_srgb(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    encoded * 255.0
}

fn color_distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    ((left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2) + (left[2] - right[2]).powi(2))
        .sqrt()
}

fn apply_override(project: &ProjectDocument, finding: &mut PublicationFinding) {
    let property = format!("publication_check:{}", finding.rule_id);
    let Some(record) = project.overrides.iter().find(|record| {
        record.property == property
            && finding.node_id.as_deref().is_none_or(|node| {
                record.target_id == node || record.target_id == project.figure.id
            })
    }) else {
        return;
    };
    let Some(reason) = record.value.get("reason").and_then(|value| value.as_str()) else {
        return;
    };
    if reason.trim().is_empty() {
        return;
    }
    finding.severity = CheckSeverity::Information;
    finding.overridden = true;
    finding.override_reason = Some(reason.to_owned());
}

#[cfg(test)]
mod tests {
    use instplot_render::{DisplayItem, Pt};
    use serde_json::json;

    use super::*;
    use crate::{
        ArtistKind, ArtistProperties, FigureDocument, MarkerShape, OverrideRecord, PaletteColor,
        resolve_document,
    };

    fn report(document: &FigureDocument, dpi: u32) -> PublicationReport {
        let resolved = resolve_document(document).unwrap();
        check_publication(document, &resolved, dpi)
    }

    #[test]
    fn fixed_fixture_runs_every_versioned_rule() {
        let report = report(&FigureDocument::fixed(), 300);
        assert_eq!(report.rules_version, PUBLICATION_RULES_VERSION);
        assert_eq!(
            CVD_SIMULATION_VERSION,
            "machado-2009-deuteranopia-severity-1-linear-srgb-v1"
        );
        assert_eq!(report.findings.len(), 13);
        assert_eq!(report.error_count(), 0);
        assert!(report.warning_count() <= 1);
        assert!(report.findings.iter().all(|finding| {
            finding.severity == CheckSeverity::Information || finding.node_id.is_some()
        }));
        for rule in [
            "physical_size",
            "font_size",
            "stroke_width",
            "font_embedding",
            "clipping",
            "legend_overlap",
            "color_only_encoding",
            "palette_data_relationship",
            "grayscale_distinguishability",
            "cvd_risk",
            "raster_dpi_pixels",
            "transparency",
            "provenance_completeness",
        ] {
            assert!(
                report
                    .findings
                    .iter()
                    .any(|finding| finding.rule_id == rule)
            );
        }
    }

    #[test]
    fn raster_finding_uses_final_canvas_with_outside_legend() {
        let mut document = FigureDocument::showcase();
        let mut legend = document.artist_record("node-15").unwrap();
        legend.visible = true;
        if let ArtistProperties::Legend { placement, .. } = &mut legend.properties {
            *placement = crate::LegendPlacement::Right;
        }
        document.set_artist_record(legend).unwrap();
        let resolved = resolve_document(&document).unwrap();
        let (width, height) = instplot_export::checked_raster_dimensions(
            f64::from(resolved.display.width),
            f64::from(resolved.display.height),
            300,
        )
        .unwrap();
        assert!(width > (85.0_f64 / 25.4 * 300.0).round() as u32);
        let report = check_publication(&document, &resolved, 300);
        let finding = report
            .findings
            .iter()
            .find(|finding| finding.rule_id == "raster_dpi_pixels")
            .unwrap();
        assert!(finding.message.contains(&format!("{width} × {height}")));
    }

    #[test]
    fn oversized_png_is_reported_before_export() {
        let mut document = FigureDocument::fixed();
        document.set_figure_size_mm(500.0, 500.0).unwrap();
        let report = report(&document, 1200);
        let finding = report
            .findings
            .iter()
            .find(|finding| finding.rule_id == "raster_dpi_pixels")
            .unwrap();
        assert_eq!(finding.severity, CheckSeverity::Error);
        assert!(finding.message.contains("safety limit"));
    }

    #[test]
    fn export_parameters_and_document_changes_update_findings() {
        let mut project = ProjectDocument::fixed_fixture();
        project.figure.width_mm = 20.0;
        project.export_preferences.transparent_background = true;
        let document = FigureDocument::from_project(project).unwrap();
        let report = report(&document, 120);
        assert!(report.findings.iter().any(|finding| {
            finding.rule_id == "physical_size" && finding.severity == CheckSeverity::Warning
        }));
        assert!(report.findings.iter().any(|finding| {
            finding.rule_id == "raster_dpi_pixels" && finding.severity == CheckSeverity::Error
        }));
        assert!(report.findings.iter().any(|finding| {
            finding.rule_id == "transparency" && finding.severity == CheckSeverity::Warning
        }));
    }

    #[test]
    fn transparent_palette_finding_targets_the_first_affected_artist() {
        let mut project = ProjectDocument::fixed_fixture();
        let blue = project
            .palette
            .colors
            .iter_mut()
            .find(|color| color.id == "blue")
            .unwrap();
        blue.rgba[3] = 128;
        let expected_artist_id = project
            .figure
            .artists
            .iter()
            .find(|artist| color_id(artist) == Some("blue"))
            .unwrap()
            .id
            .clone();

        let document = FigureDocument::from_project(project).unwrap();
        let report = report(&document, 300);
        let finding = report
            .findings
            .iter()
            .find(|finding| finding.rule_id == "transparency")
            .unwrap();
        assert_eq!(finding.severity, CheckSeverity::Warning);
        assert_eq!(
            finding.node_id.as_deref(),
            Some(expected_artist_id.as_str())
        );
    }

    #[test]
    fn override_requires_and_records_a_reason() {
        let mut project = ProjectDocument::fixed_fixture();
        project.figure.width_mm = 20.0;
        project.overrides.push(OverrideRecord {
            target_id: project.figure.id.clone(),
            property: "publication_check:physical_size".to_owned(),
            value: json!({"reason": "journal requests a narrow inset"}),
        });
        let document = FigureDocument::from_project(project).unwrap();
        let override_report = report(&document, 300);
        let finding = override_report
            .findings
            .iter()
            .find(|finding| finding.rule_id == "physical_size")
            .unwrap();
        assert!(finding.overridden);
        assert_eq!(finding.severity, CheckSeverity::Information);
        assert_eq!(
            finding.override_reason.as_deref(),
            Some("journal requests a narrow inset")
        );

        let mut invalid = ProjectDocument::fixed_fixture();
        invalid.overrides.push(OverrideRecord {
            target_id: invalid.figure.id.clone(),
            property: "publication_check:physical_size".to_owned(),
            value: json!({"reason": ""}),
        });
        assert!(FigureDocument::from_project(invalid).is_err());
    }

    #[test]
    fn structural_rule_fixtures_detect_font_stroke_and_clip_failures() {
        let document = FigureDocument::fixed();

        let mut font = resolve_document(&document).unwrap();
        let text = font
            .display
            .items
            .iter_mut()
            .find_map(|item| match item {
                ResolvedItem::Text(text) => Some(text),
                ResolvedItem::Graphics(_) => None,
            })
            .unwrap();
        text.size = 4.0;
        text.runs[0].font.embedding_allowed = false;
        let report = check_publication(&document, &font, 300);
        assert!(report.findings.iter().any(|finding| {
            finding.rule_id == "font_size" && finding.severity == CheckSeverity::Error
        }));
        assert!(report.findings.iter().any(|finding| {
            finding.rule_id == "font_embedding" && finding.severity == CheckSeverity::Error
        }));

        let mut graphics = resolve_document(&document).unwrap();
        let stroke = graphics
            .display
            .items
            .iter_mut()
            .find_map(|item| match item {
                ResolvedItem::Graphics(DisplayItem::Path {
                    stroke: Some(stroke),
                    ..
                }) => Some(stroke),
                _ => None,
            })
            .unwrap();
        stroke.width = Pt::new(0.1).unwrap();
        graphics.display.items.retain(|item| {
            !matches!(
                item,
                ResolvedItem::Graphics(DisplayItem::ClipPush { .. } | DisplayItem::ClipPop { .. })
            )
        });
        let report = check_publication(&document, &graphics, 300);
        assert!(report.findings.iter().any(|finding| {
            finding.rule_id == "stroke_width" && finding.severity == CheckSeverity::Error
        }));
        assert!(report.findings.iter().any(|finding| {
            finding.rule_id == "clipping" && finding.severity == CheckSeverity::Error
        }));
    }

    #[test]
    fn color_rule_accepts_either_marker_or_dash_as_redundant_encoding() {
        let mut project = FigureDocument::showcase().project().clone();
        let families = project
            .figure
            .artists
            .iter()
            .filter_map(|artist| match &artist.properties {
                ArtistProperties::Scatter { binding, .. } => Some(binding.data_source_id.clone()),
                _ => None,
            })
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(families.len(), 2);
        let family_by_source = project
            .data_sources
            .iter()
            .map(|source| {
                (
                    source.id.clone(),
                    source.fit.as_ref().map_or_else(
                        || source.id.clone(),
                        |fit| fit.parent_data_source_id.clone(),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let palette_ids = project
            .palette
            .colors
            .iter()
            .take(2)
            .map(|color| color.id.clone())
            .collect::<Vec<_>>();
        assert_eq!(palette_ids.len(), 2);
        for artist in &mut project.figure.artists {
            let binding = match &artist.properties {
                ArtistProperties::Line { binding, .. }
                | ArtistProperties::Scatter { binding, .. }
                | ArtistProperties::ErrorBar { binding, .. } => Some(binding),
                _ => None,
            };
            let family_index = binding.and_then(|binding| {
                family_by_source
                    .get(&binding.data_source_id)
                    .and_then(|family| families.iter().position(|candidate| candidate == family))
            });
            artist.visible = family_index.is_some();
            let Some(index) = family_index else {
                continue;
            };
            match &mut artist.properties {
                ArtistProperties::Line { stroke, .. } => {
                    stroke.color_id.clone_from(&palette_ids[index]);
                    stroke.dash_pt.clear();
                }
                ArtistProperties::Scatter { marker, .. } => {
                    marker.color_id.clone_from(&palette_ids[index]);
                    marker.shape = if index == 0 {
                        MarkerShape::Circle
                    } else {
                        MarkerShape::Square
                    };
                }
                ArtistProperties::ErrorBar { stroke, .. } => {
                    stroke.color_id.clone_from(&palette_ids[index]);
                }
                _ => {}
            }
        }

        assert_eq!(
            color_only_encoding(&project).severity,
            CheckSeverity::Information,
            "different markers must be sufficient even when line styles match"
        );

        for artist in &mut project.figure.artists {
            match &mut artist.properties {
                ArtistProperties::Scatter { marker, .. } if artist.visible => {
                    marker.shape = MarkerShape::Circle;
                }
                ArtistProperties::Line { binding, stroke } if artist.visible => {
                    let family = &family_by_source[&binding.data_source_id];
                    let index = families
                        .iter()
                        .position(|candidate| candidate == family)
                        .unwrap();
                    stroke.dash_pt = if index == 0 {
                        Vec::new()
                    } else {
                        vec![4.0, 2.0]
                    };
                }
                _ => {}
            }
        }
        assert_eq!(
            color_only_encoding(&project).severity,
            CheckSeverity::Information,
            "different dash patterns must be sufficient even when markers match"
        );

        for artist in &mut project.figure.artists {
            if artist.visible
                && let ArtistProperties::Line { stroke, .. } = &mut artist.properties
            {
                stroke.dash_pt.clear();
            }
        }
        assert_eq!(
            color_only_encoding(&project).severity,
            CheckSeverity::Warning,
            "matching markers and dash patterns leave colour as the only distinction"
        );
    }

    #[test]
    fn semantic_and_palette_rule_fixtures_detect_risks() {
        let mut project = ProjectDocument::fixed_fixture();
        project.palette.colors.push(PaletteColor {
            id: "near-blue".to_owned(),
            rgba: [69, 120, 171, 255],
        });
        let mut duplicate_source = project.data_sources[0].clone();
        duplicate_source.id = "fixture-risk-copy".to_owned();
        duplicate_source.label = "Colour-only risk copy".to_owned();
        project.data_sources.push(duplicate_source);
        let mut duplicate = project.figure.artists[1].clone();
        duplicate.id = "node-16".to_owned();
        duplicate.kind = ArtistKind::Line;
        let ArtistProperties::Line { binding, stroke } = &mut duplicate.properties else {
            panic!("fixed node-11 must remain a line");
        };
        binding.data_source_id = "fixture-risk-copy".to_owned();
        stroke.color_id = "near-blue".to_owned();
        project.figure.axes[0].artist_ids.push(duplicate.id.clone());
        project.figure.axes[0]
            .series_groups
            .push(crate::SeriesGroupRecord {
                id: "series-group-node-16".to_owned(),
                artist_ids: vec![duplicate.id.clone()],
                axes: crate::AxisBinding::PRIMARY,
            });
        project.figure.artists.push(duplicate);
        let document = FigureDocument::from_project(project).unwrap();
        let risk_report = report(&document, 300);
        for rule in [
            "color_only_encoding",
            "grayscale_distinguishability",
            "cvd_risk",
        ] {
            assert!(risk_report.findings.iter().any(|finding| {
                finding.rule_id == rule && finding.severity == CheckSeverity::Warning
            }));
        }

        let mut project = ProjectDocument::fixed_fixture();
        project.palette.id = "unregistered-palette".to_owned();
        project.provenance.clear();
        let document = FigureDocument::from_project(project).unwrap();
        let incomplete_report = report(&document, 300);
        assert!(incomplete_report.findings.iter().any(|finding| {
            finding.rule_id == "palette_data_relationship"
                && finding.severity == CheckSeverity::Warning
        }));
        assert!(incomplete_report.findings.iter().any(|finding| {
            finding.rule_id == "provenance_completeness" && finding.severity == CheckSeverity::Error
        }));
    }
}
