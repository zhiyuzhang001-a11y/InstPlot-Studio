use std::fs;
use std::path::PathBuf;

use instplot_export::{Background, encode_png, rasterize_direct, resolve, to_pdf, to_svg};
use instplot_render::{compile, fixed_figure};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts"));
    fs::create_dir_all(&output)?;
    let resolved = resolve(&compile(&fixed_figure())?);
    fs::write(output.join("a4-fixture.pdf"), to_pdf(&resolved)?)?;
    fs::write(output.join("a4-fixture.svg"), to_svg(&resolved))?;
    for dpi in [300, 600, 1200] {
        let image = rasterize_direct(&resolved, dpi, Background::White)?;
        fs::write(
            output.join(format!("a4-fixture-{dpi}dpi.png")),
            encode_png(&image)?,
        )?;
    }
    let transparent = rasterize_direct(&resolved, 300, Background::Transparent)?;
    fs::write(
        output.join("a4-fixture-300dpi-transparent.png"),
        encode_png(&transparent)?,
    )?;
    Ok(())
}
