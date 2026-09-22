use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use instplot_studio::{
    FigureDocument, FormatterSpec, LabelNode, ProjectDocument, SemanticLabel, save_figure_pdf,
};

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: axes_spacing_v2 OUTPUT_DIRECTORY")?;
    fs::create_dir_all(&output)?;

    write_case(
        &output,
        "normal-labels",
        vec![
            LabelNode::Variable("x".into()),
            LabelNode::Text(" (".into()),
            LabelNode::Unit("s".into()),
            LabelNode::Text(")".into()),
        ],
        vec![
            LabelNode::Text("Signal ".into()),
            LabelNode::Variable("V".into()),
            LabelNode::Text(" (".into()),
            LabelNode::Unit("mV".into()),
            LabelNode::Text(")".into()),
        ],
        false,
    )?;
    write_case(
        &output,
        "uppercase-greek",
        vec![
            LabelNode::GreekVariable('Δ'),
            LabelNode::Text(" (".into()),
            LabelNode::Unit("rad".into()),
            LabelNode::Text(")".into()),
        ],
        vec![
            LabelNode::GreekVariable('Ω'),
            LabelNode::Text(" (".into()),
            LabelNode::Unit("Ω".into()),
            LabelNode::Text(")".into()),
        ],
        false,
    )?;
    write_case(
        &output,
        "subscript-superscript",
        vec![
            LabelNode::GreekVariable('μ'),
            LabelNode::VariableSubscript(vec![LabelNode::Text("maximum".into())]),
            LabelNode::Superscript(vec![LabelNode::Number("2".into())]),
        ],
        vec![
            LabelNode::Variable("J".into()),
            LabelNode::VariableSubscript(vec![LabelNode::GreekVariable('μ')]),
            LabelNode::Text(" (".into()),
            LabelNode::Unit("A".into()),
            LabelNode::UnitSeparator,
            LabelNode::Unit("m".into()),
            LabelNode::Superscript(vec![LabelNode::Number("−2".into())]),
            LabelNode::Text(")".into()),
        ],
        false,
    )?;
    write_case(
        &output,
        "scientific-ticks",
        vec![
            LabelNode::Text("Frequency (".into()),
            LabelNode::Unit("Hz".into()),
            LabelNode::Text(")".into()),
        ],
        vec![
            LabelNode::Text("Amplitude (".into()),
            LabelNode::Unit("V".into()),
            LabelNode::Text(")".into()),
        ],
        true,
    )?;
    write_case(
        &output,
        "long-units",
        vec![
            LabelNode::Text("Magnetic field ".into()),
            LabelNode::Variable("H".into()),
            LabelNode::Text(" (".into()),
            LabelNode::Unit("kA".into()),
            LabelNode::UnitSeparator,
            LabelNode::Unit("m".into()),
            LabelNode::Superscript(vec![LabelNode::Number("−1".into())]),
            LabelNode::Text(")".into()),
        ],
        vec![
            LabelNode::Text("Current density ".into()),
            LabelNode::Variable("J".into()),
            LabelNode::Text(" (".into()),
            LabelNode::Unit("MA".into()),
            LabelNode::UnitSeparator,
            LabelNode::Unit("cm".into()),
            LabelNode::Superscript(vec![LabelNode::Number("−2".into())]),
            LabelNode::Text(")".into()),
        ],
        false,
    )?;

    println!("axes spacing PDFs: {}", output.display());
    Ok(())
}

fn write_case(
    output: &Path,
    name: &str,
    x_nodes: Vec<LabelNode>,
    y_nodes: Vec<LabelNode>,
    scientific_ticks: bool,
) -> Result<(), Box<dyn Error>> {
    let mut project = ProjectDocument::fixed_fixture();
    project.figure.width_mm = 85.0;
    project.figure.height_mm = 65.0;
    replace_label(&mut project.semantic_registry, "label-x", x_nodes);
    replace_label(&mut project.semantic_registry, "label-y", y_nodes);
    if scientific_ticks {
        project.figure.axes[0].x.minimum = -3.0e6;
        project.figure.axes[0].x.maximum = 3.0e6;
        project.figure.axes[0].x.formatter = FormatterSpec::Scientific { precision: 1 };
        project.figure.axes[0].y.minimum = -2.5e-6;
        project.figure.axes[0].y.maximum = 2.5e-6;
        project.figure.axes[0].y.formatter = FormatterSpec::Scientific { precision: 1 };
    }
    let document = FigureDocument::from_project(project)?;
    let layout = document.layout_axes()?;
    println!(
        "{name}: axes={:?} x_label={:?} y_label={:?}",
        layout.result.axes, layout.result.x_label_bounds, layout.result.y_label_bounds
    );
    save_figure_pdf(&document, &output.join(format!("{name}.pdf")))?;
    Ok(())
}

fn replace_label(labels: &mut [SemanticLabel], id: &str, nodes: Vec<LabelNode>) {
    labels
        .iter_mut()
        .find(|label| label.id == id)
        .expect("fixed fixture label")
        .nodes = nodes;
}
