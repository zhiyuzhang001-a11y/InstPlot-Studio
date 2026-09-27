use instplot_render::{Color, NodeId};
use instplot_text::Label;
use std::collections::BTreeMap;

use crate::{
    Annotation, AnnotationPosition, AxisSpec, Chart, DashStyle, DataPoint, ErrorBar, ErrorStyle,
    Formatter, GridSpec, LegendErrorStyle, LegendPosition, LegendSpec, LineStyle, Locator,
    MarkerShape, MarkerStyle, Scale, Series,
};

const CSV: &str = include_str!("../../../fixtures/publication-v1/data.csv");

fn x_axis_label() -> Label {
    Label::Group(vec![
        Label::GreekVariable('μ'),
        Label::VariableSubscript(Box::new(Label::Number("0".into()))),
        Label::Variable("H".into()),
        Label::DescriptiveSubscript(Box::new(Label::Text("DL".into()))),
        Label::Text(" (".into()),
        Label::Unit("mT".into()),
        Label::Text(")".into()),
    ])
}

fn y_axis_label() -> Label {
    Label::Group(vec![
        Label::Text("Current density ".into()),
        Label::Variable("J".into()),
        Label::VariableSubscript(Box::new(Label::Variable("e".into()))),
        Label::Text(" (".into()),
        Label::Unit("A".into()),
        Label::UnitSeparator,
        Label::Unit("m".into()),
        Label::Superscript(Box::new(Label::Number("−2".into()))),
        Label::Text(")".into()),
    ])
}

pub fn publication_fixture() -> Chart {
    let rows: Vec<Vec<f64>> = CSV
        .lines()
        .skip(1)
        .map(|line| {
            line.split(',')
                .map(|value| value.parse::<f64>().unwrap())
                .collect()
        })
        .collect();
    let points = |column: usize| {
        rows.iter()
            .map(|row| DataPoint {
                x: row[0],
                y: row[column],
            })
            .collect()
    };
    let y_errors = |column: usize| {
        rows.iter()
            .map(|row| ErrorBar {
                x_minus: 0.0,
                x_plus: 0.0,
                y_minus: row[column],
                y_plus: row[column],
            })
            .collect()
    };
    let blue = Color(68, 119, 170, 255);
    let orange = Color(221, 132, 82, 255);
    Chart {
        id: NodeId(1),
        width_pt: 89.0 / 25.4 * 72.0,
        height_pt: 65.0 / 25.4 * 72.0,
        x: AxisSpec {
            id: NodeId(2),
            label: x_axis_label(),
            minimum: -3.0,
            maximum: 3.0,
            scale: Scale::Linear,
            locator: Locator::Auto {
                target_spacing_pt: 34.0,
            },
            minor_interval: None,
            formatter: Formatter::Auto,
            grid: GridSpec::default(),
            appearance: crate::AxisAppearance::default(),
            has_data: true,
        },
        y: AxisSpec {
            id: NodeId(3),
            label: y_axis_label(),
            minimum: -2.5,
            maximum: 2.5,
            scale: Scale::Linear,
            locator: Locator::Auto {
                target_spacing_pt: 28.0,
            },
            minor_interval: None,
            formatter: Formatter::Auto,
            grid: GridSpec::default(),
            appearance: crate::AxisAppearance::default(),
            has_data: true,
        },
        x2: None,
        y2: None,
        series_axes: BTreeMap::new(),
        series: vec![
            Series {
                id: NodeId(10),
                label: "Reference".into(),
                legend_label: None,
                points: points(7),
                line: Some(LineStyle {
                    width: 0.7,
                    dash: DashStyle::Dotted,
                }),
                marker: None,
                legend_marker: None,
                legend_error: None,
                errors: Vec::new(),
                error_style: None,
                color: Color(150, 150, 150, 255),
            },
            Series {
                id: NodeId(11),
                label: "Theory".into(),
                legend_label: None,
                points: points(6),
                line: Some(LineStyle {
                    width: 0.9,
                    dash: DashStyle::Dashed,
                }),
                marker: None,
                legend_marker: None,
                legend_error: None,
                errors: Vec::new(),
                error_style: None,
                color: Color(70, 70, 70, 255),
            },
            Series {
                id: NodeId(12),
                label: "Fit A".into(),
                legend_label: None,
                points: points(5),
                line: Some(LineStyle {
                    width: 1.0,
                    dash: DashStyle::Solid,
                }),
                marker: None,
                legend_marker: None,
                legend_error: None,
                errors: Vec::new(),
                error_style: None,
                color: blue,
            },
            Series {
                id: NodeId(13),
                label: "Experiment A".into(),
                legend_label: None,
                points: points(1),
                line: None,
                marker: Some(MarkerStyle {
                    shape: MarkerShape::Circle,
                    size: 4.0,
                    filled: false,
                    interval: 1,
                }),
                legend_marker: None,
                legend_error: Some(LegendErrorStyle {
                    style: ErrorStyle {
                        width: 0.7,
                        cap_width: 4.0,
                        dash: DashStyle::Solid,
                    },
                    color: blue,
                }),
                errors: y_errors(2),
                error_style: Some(ErrorStyle {
                    width: 0.7,
                    cap_width: 4.0,
                    dash: DashStyle::Solid,
                }),
                color: blue,
            },
            Series {
                id: NodeId(14),
                label: "Experiment B".into(),
                legend_label: None,
                points: points(3),
                line: None,
                marker: Some(MarkerStyle {
                    shape: MarkerShape::Square,
                    size: 4.0,
                    filled: true,
                    interval: 1,
                }),
                legend_marker: None,
                legend_error: Some(LegendErrorStyle {
                    style: ErrorStyle {
                        width: 0.7,
                        cap_width: 4.0,
                        dash: DashStyle::Solid,
                    },
                    color: orange,
                }),
                errors: y_errors(4),
                error_style: Some(ErrorStyle {
                    width: 0.7,
                    cap_width: 4.0,
                    dash: DashStyle::Solid,
                }),
                color: orange,
            },
        ],
        annotations: vec![Annotation {
            id: NodeId(20),
            labels: vec![Label::Text("T ≤ 300 K".into())],
            position: AnnotationPosition::Data(DataPoint { x: -2.8, y: 2.1 }),
            offset_pt: (2.0, 0.0),
            connectors: Vec::new(),
        }],
        legend: Some(LegendSpec {
            id: NodeId(1),
            position: LegendPosition::Right,
            manual_position: None,
            grid: crate::LegendGrid::Auto,
            entry_order: Vec::new(),
        }),
    }
}

pub fn marker_gallery_fixture() -> Chart {
    let mut chart = publication_fixture();
    chart.annotations.clear();
    let shapes = [
        MarkerShape::Circle,
        MarkerShape::Square,
        MarkerShape::TriangleUp,
        MarkerShape::TriangleDown,
        MarkerShape::Diamond,
        MarkerShape::Plus,
        MarkerShape::Cross,
    ];
    chart.series = shapes
        .iter()
        .enumerate()
        .map(|(index, shape)| Series {
            id: NodeId(100 + index as u64),
            label: format!("marker-{index}"),
            legend_label: None,
            points: vec![
                DataPoint {
                    x: -2.5 + index as f64 * 0.75,
                    y: -1.7 + index as f64 * 0.5,
                },
                DataPoint {
                    x: -2.2 + index as f64 * 0.75,
                    y: -1.5 + index as f64 * 0.5,
                },
            ],
            line: Some(LineStyle {
                width: 0.8,
                dash: [
                    DashStyle::Solid,
                    DashStyle::Dashed,
                    DashStyle::Dotted,
                    DashStyle::DashDot,
                    DashStyle::LongDash,
                    DashStyle::LongShortDash,
                    DashStyle::DashDotDot,
                ][index % 7],
            }),
            marker: Some(MarkerStyle {
                shape: *shape,
                size: 5.0,
                filled: index % 2 == 0,
                interval: 1,
            }),
            legend_marker: None,
            legend_error: (index == 0).then_some(LegendErrorStyle {
                style: ErrorStyle {
                    width: 0.7,
                    cap_width: 4.0,
                    dash: DashStyle::Solid,
                },
                color: Color(40, 80, 160, 255),
            }),
            errors: if index == 0 {
                vec![
                    ErrorBar {
                        x_minus: 0.1,
                        x_plus: 0.15,
                        y_minus: 0.2,
                        y_plus: 0.25,
                    };
                    2
                ]
            } else {
                Vec::new()
            },
            error_style: (index == 0).then_some(ErrorStyle {
                width: 0.7,
                cap_width: 4.0,
                dash: DashStyle::Solid,
            }),
            color: Color(40, 80, 160, 255),
        })
        .collect();
    chart
}
