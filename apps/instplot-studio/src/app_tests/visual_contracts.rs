use super::*;

#[test]
fn palette_internal_ids_are_not_shown_as_color_names() {
    let showcase = FigureDocument::showcase();
    for color in showcase.palette_colors() {
        let chinese = palette_color_name(UiLanguage::Chinese, &color.id);
        let english = palette_color_name(UiLanguage::English, &color.id);
        assert!(!chinese.contains("object-"), "{}", color.id);
        assert!(!english.contains("object-"), "{}", color.id);
        assert_ne!(chinese, color.id);
    }
    for palette_id in USER_PALETTE_IDS {
        let registry = builtin_palette_registry(palette_id).unwrap();
        for color in registry.colors {
            let chinese = palette_color_name(UiLanguage::Chinese, &color.id);
            let english = palette_color_name(UiLanguage::English, &color.id);
            assert_ne!(chinese, color.id);
            assert_ne!(english, color.id);
        }
    }
}

#[test]
fn palette_window_names_cover_four_groups_without_internal_ids() {
    let groups = [
        (PaletteKind::Qualitative, "分类", "Categorical"),
        (PaletteKind::Sequential, "有序", "Ordered"),
        (PaletteKind::Diverging, "发散", "Diverging"),
        (PaletteKind::Neutral, "辅助", "Supporting"),
    ];
    for (kind, chinese, english) in groups {
        assert_eq!(palette_group_name(UiLanguage::Chinese, kind), chinese);
        assert_eq!(palette_group_name(UiLanguage::English, kind), english);
    }
    for palette_id in USER_PALETTE_IDS {
        assert_ne!(
            palette_scheme_name(UiLanguage::Chinese, palette_id),
            "Custom"
        );
        assert_ne!(
            palette_scheme_name(UiLanguage::English, palette_id),
            "Custom"
        );
        assert!(builtin_palette(palette_id).is_some());
    }
}

#[test]
fn showcase_keeps_fixed_fixture_separate_and_resolves() {
    let fixed = FigureDocument::fixed();
    let showcase = FigureDocument::showcase();
    assert_eq!(fixed.palette_colors().len(), 3);
    assert_eq!(showcase.palette_colors().len(), 9);
    assert!(showcase.project().figure.artists.len() > fixed.project().figure.artists.len());
    let lines = showcase
        .project()
        .figure
        .artists
        .iter()
        .filter(|artist| matches!(artist.properties, ArtistProperties::Line { .. }))
        .collect::<Vec<_>>();
    let markers = showcase
        .project()
        .figure
        .artists
        .iter()
        .filter_map(|artist| match &artist.properties {
            ArtistProperties::Scatter { marker, .. } => Some(marker),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 7);
    assert_eq!(markers.len(), 7);
    assert!(
        markers
            .iter()
            .all(|marker| PRODUCT_MARKER_SHAPES.contains(&marker.shape))
    );
    assert_eq!(
        markers
            .iter()
            .map(|marker| marker.color_id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        7
    );
    assert!(markers.iter().enumerate().all(|(index, marker)| {
        markers[..index]
            .iter()
            .all(|earlier| earlier.shape != marker.shape)
    }));
    for source in showcase
        .project()
        .data_sources
        .iter()
        .filter(|source| source.id.starts_with("showcase-line-"))
    {
        let instplot_studio::DataSourcePayload::Embedded {
            columns, row_count, ..
        } = &source.payload
        else {
            panic!("showcase curve is not embedded")
        };
        assert_eq!(*row_count, 57);
        let y = &columns
            .iter()
            .find(|column| column.name == "y")
            .unwrap()
            .values;
        assert!(
            y.windows(3)
                .any(|values| (values[0] - 2.0 * values[1] + values[2]).abs() > 0.001),
            "{} is not visibly curved",
            source.id
        );
    }
    assert!(
        showcase
            .project()
            .figure
            .artists
            .iter()
            .all(|artist| { !matches!(artist.properties, ArtistProperties::ReferenceLine { .. }) })
    );
    let resolved = resolved_preview(&showcase).unwrap();
    let report = check_publication(&showcase, &resolved, 300);
    assert_eq!(report.error_count(), 0, "{:#?}", report.findings);
    assert_eq!(report.warning_count(), 0, "{:#?}", report.findings);
    let pdf = instplot_studio::figure_pdf(&showcase).unwrap();
    let png = instplot_studio::figure_png(&showcase, 300).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn all_product_marker_shapes_validate_and_resolve() {
    for shape in [
        MarkerShape::Circle,
        MarkerShape::Square,
        MarkerShape::Triangle,
        MarkerShape::TriangleDown,
        MarkerShape::Diamond,
        MarkerShape::Pentagon,
        MarkerShape::Star,
        MarkerShape::Plus,
        MarkerShape::Cross,
    ] {
        let mut project = FigureDocument::showcase().project().clone();
        let artist = project
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.id == "node-17")
            .unwrap();
        let ArtistProperties::Scatter { marker, .. } = &mut artist.properties else {
            panic!("showcase marker missing");
        };
        marker.shape = shape;
        project.validate().unwrap();
        let encoded = serde_json::to_vec(&project).unwrap();
        let decoded = serde_json::from_slice(&encoded).unwrap();
        let document = FigureDocument::from_project(decoded).unwrap();
        resolved_preview(&document).unwrap();
    }
}

#[test]
fn hollow_marker_choices_round_trip_and_reach_the_export_display_list() {
    for shape in PRODUCT_MARKER_SHAPES {
        let mut document = FigureDocument::showcase();
        let mut artist = document.artist_record("node-17").unwrap();
        let ArtistProperties::Scatter { marker, .. } = &mut artist.properties else {
            panic!("showcase marker missing")
        };
        marker.shape = shape;
        marker.filled = false;
        document.set_artist_record(artist).unwrap();
        let encoded = serde_json::to_vec(document.project()).unwrap();
        let reopened =
            FigureDocument::from_project(serde_json::from_slice(&encoded).unwrap()).unwrap();
        let ArtistProperties::Scatter { marker, .. } =
            reopened.artist_record("node-17").unwrap().properties
        else {
            panic!("showcase marker missing")
        };
        assert_eq!(marker.shape, shape);
        assert!(!marker.filled);
        let layout = reopened.layout_figure().unwrap();
        assert!(layout.result.display_list.items.iter().any(|item| {
            matches!(
                item,
                instplot_render::DisplayItem::Path {
                    source,
                    fill: None,
                    stroke: Some(_),
                    ..
                } if layout.project_ids.get(source).map(String::as_str) == Some("node-17")
            )
        }));
    }
}
