use super::*;

impl ProjectDocument {
    pub fn fixed_fixture() -> Self {
        let artist_ids = [10_u64, 11, 12, 13, 14, 15].map(|id| format!("node-{id}"));
        let embedded_columns = vec![
            EmbeddedColumn {
                name: "line_x".to_owned(),
                values: vec![-3.2, 0.0, 3.2],
                valid: Vec::new(),
            },
            EmbeddedColumn {
                name: "line_y".to_owned(),
                values: vec![-2.34, 0.0, 2.34],
                valid: Vec::new(),
            },
            EmbeddedColumn {
                name: "scatter_x".to_owned(),
                values: vec![-3.2, 0.0, 3.2],
                valid: Vec::new(),
            },
            EmbeddedColumn {
                name: "scatter_y".to_owned(),
                values: vec![-2.38, 0.01, 2.34],
                valid: Vec::new(),
            },
            EmbeddedColumn {
                name: "error".to_owned(),
                values: vec![0.0, 0.08, 0.0],
                valid: Vec::new(),
            },
        ];
        let embedded_alive = vec![true; 3];
        let embedded_sha256 = embedded_digest(&embedded_columns, 3, &embedded_alive)
            .expect("the fixed project fixture has serializable embedded data");
        let blue_stroke = || StrokeStyle {
            color_id: "blue".to_owned(),
            width_pt: 0.9,
            dash_pt: Vec::new(),
        };
        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            producer_version: env!("CARGO_PKG_VERSION").to_owned(),
            figure: FigureRecord {
                id: "node-1".to_owned(),
                width_mm: 85.0,
                height_mm: 65.0,
                axes: vec![AxesRecord {
                    id: "node-2".to_owned(),
                    x: AxisRecord {
                        id: "node-3".to_owned(),
                        label_id: "label-x".to_owned(),
                        minimum: -3.0,
                        maximum: 3.0,
                        scale: AxisScale::Linear,
                        locator: LocatorSpec::Auto { target_count: 6 },
                        minor_interval: None,
                        formatter: FormatterSpec::Auto,
                        autoscale: false,
                        appearance: AxisAppearanceRecord::default(),
                    },
                    y: AxisRecord {
                        id: "node-4".to_owned(),
                        label_id: "label-y".to_owned(),
                        minimum: -2.5,
                        maximum: 2.5,
                        scale: AxisScale::Linear,
                        locator: LocatorSpec::Auto { target_count: 6 },
                        minor_interval: None,
                        formatter: FormatterSpec::Auto,
                        autoscale: false,
                        appearance: AxisAppearanceRecord::default(),
                    },
                    artist_ids: artist_ids.to_vec(),
                }],
                artists: vec![
                    ArtistRecord {
                        id: artist_ids[0].clone(),
                        kind: ArtistKind::ReferenceLine,
                        role: ArtistRole::Baseline,
                        visible: true,
                        properties: ArtistProperties::ReferenceLine {
                            orientation: ReferenceOrientation::Horizontal,
                            value: 0.0,
                            stroke: StrokeStyle {
                                color_id: "gray".to_owned(),
                                width_pt: 0.7,
                                dash_pt: vec![1.4, 1.4],
                            },
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[1].clone(),
                        kind: ArtistKind::Line,
                        role: ArtistRole::Fit,
                        visible: true,
                        properties: ArtistProperties::Line {
                            binding: binding("fixture-data", "line_x", "line_y"),
                            stroke: blue_stroke(),
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[2].clone(),
                        kind: ArtistKind::ErrorBar,
                        role: ArtistRole::Data,
                        visible: true,
                        properties: ArtistProperties::ErrorBar {
                            binding: binding("fixture-data", "scatter_x", "scatter_y"),
                            x_error_column: None,
                            y_error_column: "error".to_owned(),
                            cap_width_pt: 4.0,
                            stroke: StrokeStyle {
                                width_pt: 0.7,
                                ..blue_stroke()
                            },
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[3].clone(),
                        kind: ArtistKind::Scatter,
                        role: ArtistRole::Data,
                        visible: true,
                        properties: ArtistProperties::Scatter {
                            binding: binding("fixture-data", "scatter_x", "scatter_y"),
                            marker: MarkerStyle {
                                color_id: "blue".to_owned(),
                                shape: MarkerShape::Circle,
                                size_pt: 2.0,
                                filled: true,
                                interval: 1,
                            },
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[4].clone(),
                        kind: ArtistKind::Annotation,
                        role: ArtistRole::Annotation,
                        visible: true,
                        properties: ArtistProperties::Annotation {
                            label_id: "label-temperature".to_owned(),
                            x_pt: 48.0,
                            y_pt: 28.0,
                            connectors: Vec::new(),
                        },
                    },
                    ArtistRecord {
                        id: artist_ids[5].clone(),
                        kind: ArtistKind::Legend,
                        role: ArtistRole::Legend,
                        visible: true,
                        properties: ArtistProperties::Legend {
                            entries: vec![
                                LegendEntry {
                                    artist_id: artist_ids[3].clone(),
                                    label_id: "label-experiment".to_owned(),
                                    visible: true,
                                },
                                LegendEntry {
                                    artist_id: artist_ids[1].clone(),
                                    label_id: "label-fit".to_owned(),
                                    visible: true,
                                },
                            ],
                            x_pt: 110.0,
                            y_pt: 12.0,
                            placement: LegendPlacement::Inside,
                            position_custom: false,
                            grid: LegendGrid::Auto,
                        },
                    },
                ],
            },
            data_sources: vec![DataSourceRecord {
                id: "fixture-data".to_owned(),
                label: "B2 fixed publication fixture".to_owned(),
                kind: DataSourceKind::Source,
                payload: DataSourcePayload::Embedded {
                    columns: embedded_columns,
                    row_count: 3,
                    alive: embedded_alive,
                    sha256: embedded_sha256,
                },
                fit: None,
                origin_path: None,
            }],
            semantic_registry: vec![
                SemanticLabel {
                    id: "label-x".to_owned(),
                    nodes: vec![
                        LabelNode::GreekVariable('μ'),
                        LabelNode::VariableSubscript(vec![LabelNode::Number("0".to_owned())]),
                        LabelNode::Variable("H".to_owned()),
                        LabelNode::DescriptiveSubscript(vec![LabelNode::Text("DL".to_owned())]),
                        LabelNode::Text(" (".to_owned()),
                        LabelNode::Unit("mT".to_owned()),
                        LabelNode::Text(")".to_owned()),
                    ],
                },
                SemanticLabel {
                    id: "label-temperature".to_owned(),
                    nodes: vec![
                        LabelNode::Variable("T".to_owned()),
                        LabelNode::Text(" ".to_owned()),
                        LabelNode::Operator("≤".to_owned()),
                        LabelNode::Text(" ".to_owned()),
                        LabelNode::Number("300".to_owned()),
                        LabelNode::Text(" ".to_owned()),
                        LabelNode::Unit("K".to_owned()),
                    ],
                },
                SemanticLabel {
                    id: "label-experiment".to_owned(),
                    nodes: vec![LabelNode::Text("Experiment".to_owned())],
                },
                SemanticLabel {
                    id: "label-fit".to_owned(),
                    nodes: vec![LabelNode::Text("Fit".to_owned())],
                },
                SemanticLabel {
                    id: "label-y".to_owned(),
                    nodes: vec![
                        LabelNode::Text("Current density ".to_owned()),
                        LabelNode::Variable("J".to_owned()),
                        LabelNode::VariableSubscript(vec![LabelNode::Variable("e".to_owned())]),
                        LabelNode::Text(" (".to_owned()),
                        LabelNode::Unit("A".to_owned()),
                        LabelNode::UnitSeparator,
                        LabelNode::Unit("m".to_owned()),
                        LabelNode::Superscript(vec![LabelNode::Number("−2".to_owned())]),
                        LabelNode::Text(")".to_owned()),
                    ],
                },
            ],
            palette: PaletteRegistry {
                id: "publication-default-v1".to_owned(),
                colors: vec![
                    PaletteColor {
                        id: "blue".to_owned(),
                        rgba: [68, 119, 170, 255],
                    },
                    PaletteColor {
                        id: "gray".to_owned(),
                        rgba: [102, 102, 102, 255],
                    },
                    PaletteColor {
                        id: "object-black".to_owned(),
                        rgba: [0, 0, 0, 255],
                    },
                ],
            },
            typography: default_typography(),
            overrides: Vec::new(),
            export_preferences: ExportPreferences {
                vector_format: "pdf".to_owned(),
                raster_dpi: vec![300, 600, 1200],
                selected_raster_dpi: 300,
                transparent_background: false,
            },
            provenance: vec![ProvenanceRecord {
                id: "provenance-create".to_owned(),
                operation: "create_fixed_fixture".to_owned(),
                input_ids: Vec::new(),
                parameters: BTreeMap::new(),
            }],
        }
    }

    /// Product-facing sample. The fixed Part A fixture remains unchanged for
    /// deterministic render/regression tests.
    pub fn showcase_fixture() -> Self {
        let mut project = Self::fixed_fixture();
        project.figure.axes[0].artist_ids.clear();
        project
            .figure
            .artists
            .retain(|artist| matches!(artist.id.as_str(), "node-14" | "node-15"));
        project.data_sources.clear();
        project.semantic_registry.retain(|label| {
            matches!(
                label.id.as_str(),
                "label-x" | "label-y" | "label-temperature"
            )
        });
        if let ArtistProperties::Annotation { x_pt, y_pt, .. } = &mut project
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.id == "node-14")
            .expect("showcase annotation")
            .properties
        {
            *x_pt = 150.0;
            *y_pt = 25.0;
        }
        // Keep these editable objects in the document without covering the
        // seven-series opening example on the publication-sized canvas.
        for artist in &mut project.figure.artists {
            if matches!(artist.id.as_str(), "node-14" | "node-15") {
                artist.visible = false;
            }
        }
        project.palette.id = "studio-showcase-v1".to_owned();
        for (id, rgba) in [
            ("object-red", [238, 102, 119, 255]),
            ("object-green", [34, 136, 51, 255]),
            ("object-yellow", [204, 187, 68, 255]),
            ("object-cyan", [102, 204, 238, 255]),
            ("object-purple", [170, 51, 119, 255]),
            ("object-light-gray", [187, 187, 187, 255]),
        ] {
            project.palette.colors.push(PaletteColor {
                id: id.to_owned(),
                rgba,
            });
        }
        let curves: [(&str, MarkerShape, &[f64]); 7] = [
            ("blue", MarkerShape::Circle, &[]),
            ("object-red", MarkerShape::Triangle, &[4.0, 2.4]),
            ("object-green", MarkerShape::Square, &[0.8, 1.8]),
            ("object-yellow", MarkerShape::Diamond, &[4.0, 2.0, 0.8, 2.0]),
            ("object-cyan", MarkerShape::TriangleDown, &[8.0, 3.0]),
            (
                "object-purple",
                MarkerShape::Pentagon,
                &[8.0, 2.0, 2.0, 2.0],
            ),
            ("gray", MarkerShape::Star, &[6.0, 2.0, 0.8, 2.0, 0.8, 2.0]),
        ];
        let mut legend_entries = Vec::new();
        for (index, (color_id, shape, dash_pt)) in curves.into_iter().enumerate() {
            let letter = (b'A' + index as u8) as char;
            let label = format!("Series {letter}");
            let label_id = format!("label-series-{letter}");
            let line_source_id = format!("showcase-line-{letter}");
            let marker_source_id = format!("showcase-markers-{letter}");
            let line_id = format!("node-{}", 16 + index * 2);
            let marker_id = format!("node-{}", 17 + index * 2);
            let line_x = (0..=56)
                .map(|step| (f64::from(step) - 28.0) / 10.0)
                .collect::<Vec<_>>();
            let line_y = line_x
                .iter()
                .map(|&x| showcase_curve_value(index, x))
                .collect::<Vec<_>>();
            let marker_x = (0..=7).map(|step| line_x[step * 8]).collect::<Vec<_>>();
            let marker_y = (0..=7).map(|step| line_y[step * 8]).collect::<Vec<_>>();
            for (source_id, x_values, y_values) in [
                (&line_source_id, line_x, line_y),
                (&marker_source_id, marker_x, marker_y),
            ] {
                let row_count = x_values.len();
                let columns = vec![
                    EmbeddedColumn {
                        name: "x".to_owned(),
                        values: x_values,
                        valid: Vec::new(),
                    },
                    EmbeddedColumn {
                        name: "y".to_owned(),
                        values: y_values,
                        valid: Vec::new(),
                    },
                ];
                let alive = vec![true; row_count];
                let sha256 = embedded_digest(&columns, row_count, &alive)
                    .expect("the showcase has serializable embedded data");
                project.data_sources.push(DataSourceRecord {
                    id: source_id.clone(),
                    label: label.clone(),
                    kind: if source_id == &line_source_id {
                        DataSourceKind::Fit
                    } else {
                        DataSourceKind::Source
                    },
                    payload: DataSourcePayload::Embedded {
                        columns,
                        row_count,
                        alive,
                        sha256,
                    },
                    fit: (source_id == &line_source_id).then(|| FitIdentity {
                        parent_data_source_id: marker_source_id.clone(),
                        source_x_column: "x".to_owned(),
                        source_y_column: "y".to_owned(),
                        equation: Some("showcase curve".to_owned()),
                        display_equation: None,
                    }),
                    origin_path: None,
                });
            }
            project.figure.artists.push(ArtistRecord {
                id: line_id.clone(),
                kind: ArtistKind::Line,
                role: ArtistRole::Fit,
                visible: true,
                properties: ArtistProperties::Line {
                    binding: binding(&line_source_id, "x", "y"),
                    stroke: StrokeStyle {
                        color_id: color_id.to_owned(),
                        width_pt: 1.05,
                        dash_pt: dash_pt.to_vec(),
                    },
                },
            });
            project.figure.artists.push(ArtistRecord {
                id: marker_id.clone(),
                kind: ArtistKind::Scatter,
                role: ArtistRole::Data,
                visible: true,
                properties: ArtistProperties::Scatter {
                    binding: binding(&marker_source_id, "x", "y"),
                    marker: MarkerStyle {
                        color_id: color_id.to_owned(),
                        shape,
                        size_pt: if matches!(shape, MarkerShape::Pentagon | MarkerShape::Star) {
                            4.4
                        } else {
                            3.3
                        },
                        filled: true,
                        interval: 1,
                    },
                },
            });
            project.figure.axes[0].artist_ids.push(line_id);
            project.figure.axes[0].artist_ids.push(marker_id.clone());
            legend_entries.push(LegendEntry {
                artist_id: marker_id,
                label_id: label_id.clone(),
                visible: true,
            });
            project.semantic_registry.push(SemanticLabel {
                id: label_id,
                nodes: vec![LabelNode::Text(label)],
            });
        }
        project.figure.axes[0]
            .artist_ids
            .extend(["node-14".to_owned(), "node-15".to_owned()]);
        if let ArtistProperties::Legend {
            entries,
            x_pt,
            y_pt,
            placement,
            ..
        } = &mut project
            .figure
            .artists
            .iter_mut()
            .find(|artist| artist.id == "node-15")
            .expect("showcase legend")
            .properties
        {
            *entries = legend_entries;
            *x_pt = 177.0;
            *y_pt = 15.0;
            *placement = LegendPlacement::Auto;
        }
        project.provenance[0].operation = "create_showcase_fixture".to_owned();
        project.validate().expect("showcase fixture must validate");
        project
    }
}
