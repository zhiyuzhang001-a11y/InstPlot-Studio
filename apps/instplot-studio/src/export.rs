use core::fmt;
use std::path::Path;

use crate::FigureDocument;

#[derive(Debug)]
pub enum FixedPdfExportError {
    Compile(studio_render_spike::CompileError),
    Render(export_backend_spike::ExportError),
    Write(std::io::Error),
}

impl fmt::Display for FixedPdfExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => write!(formatter, "compile fixed figure: {error}"),
            Self::Render(error) => write!(formatter, "render fixed figure PDF: {error}"),
            Self::Write(error) => write!(formatter, "write fixed figure PDF: {error}"),
        }
    }
}

impl std::error::Error for FixedPdfExportError {}

pub fn fixed_figure_pdf() -> Result<Vec<u8>, FixedPdfExportError> {
    let display = FigureDocument::fixed()
        .compile()
        .map_err(FixedPdfExportError::Compile)?;
    let resolved = export_backend_spike::resolve(&display);
    export_backend_spike::to_pdf(&resolved).map_err(FixedPdfExportError::Render)
}

pub fn save_fixed_figure_pdf(path: &Path) -> Result<usize, FixedPdfExportError> {
    let bytes = fixed_figure_pdf()?;
    std::fs::write(path, &bytes).map_err(FixedPdfExportError::Write)?;
    Ok(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_export_is_a_nonempty_pdf() {
        let pdf = fixed_figure_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1_000);
    }
}
