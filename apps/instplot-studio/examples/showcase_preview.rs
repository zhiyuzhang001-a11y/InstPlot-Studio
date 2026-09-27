use std::error::Error;
use std::fs;
use std::path::PathBuf;

use instplot_studio::{
    ArtistKind, ArtistProperties, FigureDocument, LegendGrid, LegendPlacement, figure_pdf,
    figure_png,
};

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: showcase_preview OUTPUT_DIRECTORY")?;
    fs::create_dir_all(&output)?;
    let document = FigureDocument::showcase();
    fs::write(output.join("showcase.png"), figure_png(&document, 300)?)?;
    fs::write(output.join("showcase.pdf"), figure_pdf(&document)?)?;
    let mut hollow = document.clone();
    let scatter_ids = hollow
        .project()
        .figure
        .artists
        .iter()
        .filter(|artist| artist.kind == ArtistKind::Scatter)
        .map(|artist| artist.id.clone())
        .collect::<Vec<_>>();
    for id in scatter_ids {
        let mut artist = hollow
            .artist_record(&id)
            .ok_or("showcase marker is missing")?;
        if let ArtistProperties::Scatter { marker, .. } = &mut artist.properties {
            marker.filled = false;
        }
        hollow.set_artist_record(artist)?;
    }
    fs::write(
        output.join("showcase-hollow-markers.png"),
        figure_png(&hollow, 300)?,
    )?;
    fs::write(
        output.join("showcase-hollow-markers.pdf"),
        figure_pdf(&hollow)?,
    )?;
    for (name, placement) in [
        ("auto", LegendPlacement::Auto),
        ("above", LegendPlacement::Above),
        ("right", LegendPlacement::Right),
    ] {
        let mut with_legend = document.clone();
        let mut record = with_legend
            .project()
            .figure
            .artists
            .iter()
            .find(|artist| artist.kind == ArtistKind::Legend)
            .cloned()
            .ok_or("showcase legend is missing")?;
        record.visible = true;
        if let ArtistProperties::Legend {
            placement: position,
            ..
        } = &mut record.properties
        {
            *position = placement;
        }
        with_legend.set_artist_record(record)?;
        fs::write(
            output.join(format!("showcase-legend-{name}.png")),
            figure_png(&with_legend, 300)?,
        )?;
    }
    for (name, placement, grid) in [
        (
            "above-two-rows",
            LegendPlacement::Above,
            LegendGrid::Rows(2),
        ),
        (
            "right-two-columns",
            LegendPlacement::Right,
            LegendGrid::Columns(2),
        ),
    ] {
        let mut with_legend = document.clone();
        let mut record = with_legend
            .project()
            .figure
            .artists
            .iter()
            .find(|artist| artist.kind == ArtistKind::Legend)
            .cloned()
            .ok_or("showcase legend is missing")?;
        record.visible = true;
        if let ArtistProperties::Legend {
            placement: position,
            grid: arrangement,
            ..
        } = &mut record.properties
        {
            *position = placement;
            *arrangement = grid;
        }
        with_legend.set_artist_record(record)?;
        fs::write(
            output.join(format!("showcase-legend-{name}.png")),
            figure_png(&with_legend, 300)?,
        )?;
    }
    Ok(())
}
