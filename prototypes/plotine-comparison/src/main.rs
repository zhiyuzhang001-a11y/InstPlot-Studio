use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts"));
    fs::create_dir_all(&output)?;
    let vector = plotine_comparison::fixture(72.0);
    fs::write(output.join("plotine-a1.pdf"), vector.render_pdf()?)?;
    fs::write(output.join("plotine-a1.svg"), vector.render_svg()?)?;
    fs::write(
        output.join("plotine-a1-300dpi.png"),
        plotine_comparison::fixture(300.0).render_png()?,
    )?;
    Ok(())
}
