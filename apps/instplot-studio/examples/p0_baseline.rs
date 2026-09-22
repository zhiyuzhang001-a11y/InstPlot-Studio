use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use instplot_core::DataSet;
use instplot_studio::{FigureDocument, save_figure_pdf, save_figure_png, write_handoff};
use serde::Serialize;

#[derive(Serialize)]
struct BaselineManifest {
    schema_version: u32,
    phase: &'static str,
    valid_cases: Vec<CaseManifest>,
    expected_rejections: Vec<RejectionManifest>,
}

#[derive(Serialize)]
struct CaseManifest {
    id: String,
    fixture: String,
    dataset_count: usize,
    data_source_count: usize,
    artist_count: usize,
    row_count: usize,
    alive_count: usize,
    project: String,
    handoff: String,
    pdf: String,
    png: String,
    project_bytes: u64,
    handoff_bytes: u64,
    pdf_bytes: u64,
    png_bytes: u64,
}

#[derive(Serialize)]
struct RejectionManifest {
    id: String,
    fixture: String,
    stage: &'static str,
    error: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: p0_baseline OUTPUT_DIRECTORY")?;
    fs::create_dir_all(&output)?;
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

    let mut valid_cases = Vec::new();
    valid_cases.push(write_case(
        "single-source",
        &fixtures.join("smoke.csv"),
        instplot_io::read_data_file(&fixtures.join("smoke.csv"))?,
        &output,
    )?);
    valid_cases.push(write_case(
        "source-fit",
        &fixtures.join("lite-source-fit.txt"),
        instplot_io::read_data_file(&fixtures.join("lite-source-fit.txt"))?,
        &output,
    )?);
    valid_cases.push(write_case(
        "multi-source",
        &fixtures.join("p0-multi-source.txt"),
        instplot_io::read_data_file(&fixtures.join("p0-multi-source.txt"))?,
        &output,
    )?);

    let disabled_fixture = fixtures.join("p0-disabled-source.csv");
    let mut disabled = instplot_io::read_data_file(&disabled_fixture)?;
    disabled[0].alive[2] = false;
    valid_cases.push(write_case(
        "disabled-row",
        &disabled_fixture,
        disabled,
        &output,
    )?);

    let missing_fixture = fixtures.join("p0-missing-values.csv");
    let missing = instplot_io::read_data_file(&missing_fixture)?;
    let error = FigureDocument::from_datasets(&missing)
        .expect_err("non-finite missing value must be rejected explicitly")
        .to_string();
    let expected_rejections = vec![RejectionManifest {
        id: "missing-value".to_owned(),
        fixture: relative_fixture(&missing_fixture),
        stage: "figure_document_creation",
        error,
    }];

    let manifest = BaselineManifest {
        schema_version: 1,
        phase: "B5P-P0",
        valid_cases,
        expected_rejections,
    };
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!("P0 baseline artifacts: {}", output.display());
    Ok(())
}

fn write_case(
    id: &str,
    fixture: &Path,
    datasets: Vec<DataSet>,
    output: &Path,
) -> Result<CaseManifest, Box<dyn Error>> {
    let document = FigureDocument::from_datasets(&datasets)?;
    let project = output.join(format!("{id}.instplot"));
    let handoff = output.join(format!("{id}.instplot-handoff"));
    let pdf = output.join(format!("{id}.pdf"));
    let png = output.join(format!("{id}.png"));
    document.save(&project)?;
    write_handoff(
        &handoff,
        &datasets,
        "InstPlot Studio P0 baseline",
        env!("CARGO_PKG_VERSION"),
    )?;
    save_figure_pdf(&document, &pdf)?;
    save_figure_png(&document, &png, 300)?;

    Ok(CaseManifest {
        id: id.to_owned(),
        fixture: relative_fixture(fixture),
        dataset_count: datasets.len(),
        data_source_count: document.project().data_sources.len(),
        artist_count: document.project().figure.artists.len(),
        row_count: datasets.iter().map(|dataset| dataset.row_count).sum(),
        alive_count: datasets
            .iter()
            .flat_map(|dataset| &dataset.alive)
            .filter(|alive| **alive)
            .count(),
        project: file_name(&project),
        handoff: file_name(&handoff),
        pdf: file_name(&pdf),
        png: file_name(&png),
        project_bytes: fs::metadata(project)?.len(),
        handoff_bytes: fs::metadata(handoff)?.len(),
        pdf_bytes: fs::metadata(pdf)?.len(),
        png_bytes: fs::metadata(png)?.len(),
    })
}

fn relative_fixture(path: &Path) -> String {
    format!("apps/instplot-studio/tests/fixtures/{}", file_name(path))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .expect("artifact path has a file name")
        .to_string_lossy()
        .into_owned()
}
