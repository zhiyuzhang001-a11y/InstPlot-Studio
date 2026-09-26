use std::path::PathBuf;

use instplot_export::{Background, encode_png, rasterize_direct, resolve, to_pdf, to_svg};
use layout_engine_spike::{layout, marker_gallery_fixture, publication_fixture};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts"));
    std::fs::create_dir_all(&output)?;
    let result = layout(&publication_fixture())?;
    let resolved = resolve(&result.display_list);
    std::fs::write(output.join("a7-single-axes.pdf"), to_pdf(&resolved)?)?;
    std::fs::write(output.join("a7-single-axes.svg"), to_svg(&resolved))?;
    let image = rasterize_direct(&resolved, 300, Background::White)?;
    std::fs::write(
        output.join("a7-single-axes-300dpi.png"),
        encode_png(&image)?,
    )?;
    std::fs::write(
        output.join("a7-display-list.txt"),
        result.display_list.debug_snapshot(),
    )?;
    std::fs::write(output.join("a7-layout.txt"), result.snapshot())?;

    let gallery = layout(&marker_gallery_fixture())?;
    let gallery = resolve(&gallery.display_list);
    std::fs::write(output.join("a7-marker-gallery.svg"), to_svg(&gallery))?;
    let image = rasterize_direct(&gallery, 300, Background::White)?;
    std::fs::write(
        output.join("a7-marker-gallery-300dpi.png"),
        encode_png(&image)?,
    )?;
    Ok(())
}
