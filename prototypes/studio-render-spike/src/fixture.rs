use crate::{
    Artist, Axes, Axis, Color, ErrorBar, Figure, Legend, Line, LineCap, LineJoin, Mm, NodeId, Pt,
    ReferenceLine, Scatter, Stroke, Text,
};

fn pt(value: f64) -> Pt {
    Pt::new(value).unwrap()
}

fn stroke(color: Color, width: f64, dash: &[f64]) -> Stroke {
    Stroke {
        color,
        width: pt(width),
        cap: LineCap::Butt,
        join: LineJoin::Miter,
        dash: dash.iter().copied().map(pt).collect(),
    }
}

/// The shared A1/A2 publication fixture used by later backend spikes.
pub fn fixed_figure() -> Figure {
    let blue = Color(68, 119, 170, 255);
    let gray = Color(102, 102, 102, 255);
    Figure {
        id: NodeId(1),
        width: Mm::new(89.0).unwrap(),
        height: Mm::new(65.0).unwrap(),
        axes: vec![Axes {
            id: NodeId(2),
            left: pt(38.0),
            top: pt(16.0),
            width: pt(190.0),
            height: pt(132.0),
            x: Axis {
                id: NodeId(3),
                label: "μ₀H_DL (mT)".into(),
                minimum: -3.0,
                maximum: 3.0,
            },
            y: Axis {
                id: NodeId(4),
                label: "Current density J_e (A m⁻²)".into(),
                minimum: -2.5,
                maximum: 2.5,
            },
            artists: vec![
                Artist::ReferenceLine(ReferenceLine {
                    id: NodeId(10),
                    y: 0.0,
                    stroke: stroke(Color(160, 160, 160, 255), 0.7, &[1.4, 1.4]),
                }),
                Artist::Line(Line {
                    id: NodeId(11),
                    points: vec![(-3.2, -2.34), (0.0, 0.0), (3.2, 2.34)],
                    stroke: stroke(blue, 0.9, &[]),
                }),
                Artist::ErrorBar(ErrorBar {
                    id: NodeId(12),
                    points: vec![(0.0, 0.01, 0.08)],
                    cap_width: pt(4.0),
                    stroke: stroke(blue, 0.7, &[]),
                }),
                Artist::Scatter(Scatter {
                    id: NodeId(13),
                    points: vec![(-3.2, -2.38), (0.0, 0.01)],
                    radius: pt(2.0),
                    color: blue,
                }),
                Artist::Text(Text {
                    id: NodeId(14),
                    x: pt(48.0),
                    y: pt(28.0),
                    value: "温度 T (K)".into(),
                    size: pt(8.0),
                    color: gray,
                }),
                Artist::Legend(Legend {
                    id: NodeId(15),
                    x: pt(164.0),
                    y: pt(30.0),
                    entries: vec!["Experiment".into(), "Fit".into()],
                    size: pt(8.0),
                    color: gray,
                }),
            ],
        }],
    }
}
