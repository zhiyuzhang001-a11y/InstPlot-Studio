use core::fmt;
use std::io::Write;
use std::path::Path;

use atomicwrites::{AllowOverwrite, AtomicFile};

use crate::{DocumentLayoutError, FigureDocument, resolve_document};

#[derive(Debug)]
pub enum FixedPdfExportError {
    Layout(DocumentLayoutError),
    Render(export_backend_spike::ExportError),
    EncodePng(String),
    AtomicWrite(String),
}

impl fmt::Display for FixedPdfExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(error) => write!(formatter, "layout figure: {error}"),
            Self::Render(error) => write!(formatter, "render fixed figure PDF: {error}"),
            Self::EncodePng(error) => write!(formatter, "encode figure PNG: {error}"),
            Self::AtomicWrite(error) => write!(formatter, "atomically write figure: {error}"),
        }
    }
}

impl std::error::Error for FixedPdfExportError {}

pub fn fixed_figure_pdf() -> Result<Vec<u8>, FixedPdfExportError> {
    figure_pdf(&FigureDocument::fixed())
}

pub fn figure_pdf(document: &FigureDocument) -> Result<Vec<u8>, FixedPdfExportError> {
    let resolved = resolve_document(document).map_err(FixedPdfExportError::Layout)?;
    export_backend_spike::to_pdf(&resolved.display).map_err(FixedPdfExportError::Render)
}

pub fn fixed_figure_png(dpi: u32) -> Result<Vec<u8>, FixedPdfExportError> {
    figure_png(&FigureDocument::fixed(), dpi)
}

pub fn figure_png(document: &FigureDocument, dpi: u32) -> Result<Vec<u8>, FixedPdfExportError> {
    figure_png_with_background(document, dpi, false)
}

pub fn figure_png_with_background(
    document: &FigureDocument,
    dpi: u32,
    transparent_background: bool,
) -> Result<Vec<u8>, FixedPdfExportError> {
    let resolved = resolve_document(document).map_err(FixedPdfExportError::Layout)?;
    let background = if transparent_background {
        export_backend_spike::Background::Transparent
    } else {
        export_backend_spike::Background::White
    };
    let image = export_backend_spike::rasterize_direct(&resolved.display, dpi, background);
    export_backend_spike::encode_png(&image)
        .map_err(|error| FixedPdfExportError::EncodePng(error.to_string()))
}

pub fn save_fixed_figure_pdf(path: &Path) -> Result<usize, FixedPdfExportError> {
    save_figure_pdf(&FigureDocument::fixed(), path)
}

pub fn save_figure_pdf(
    document: &FigureDocument,
    path: &Path,
) -> Result<usize, FixedPdfExportError> {
    let bytes = figure_pdf(document)?;
    atomic_write(path, &bytes)?;
    Ok(bytes.len())
}

pub fn save_fixed_figure_png(path: &Path, dpi: u32) -> Result<usize, FixedPdfExportError> {
    save_figure_png(&FigureDocument::fixed(), path, dpi)
}

pub fn save_figure_png(
    document: &FigureDocument,
    path: &Path,
    dpi: u32,
) -> Result<usize, FixedPdfExportError> {
    let bytes = figure_png(document, dpi)?;
    atomic_write(path, &bytes)?;
    Ok(bytes.len())
}

pub fn save_figure_png_with_background(
    document: &FigureDocument,
    path: &Path,
    dpi: u32,
    transparent_background: bool,
) -> Result<usize, FixedPdfExportError> {
    let bytes = figure_png_with_background(document, dpi, transparent_background)?;
    atomic_write(path, &bytes)?;
    Ok(bytes.len())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), FixedPdfExportError> {
    AtomicFile::new(path, AllowOverwrite)
        .write(|file| file.write_all(bytes))
        .map_err(|error| FixedPdfExportError::AtomicWrite(error.to_string()))
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

    #[test]
    fn fixed_png_uses_the_formal_resolved_figure() {
        let png = fixed_figure_png(300).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(png.len() > 1_000);
        assert!(u32::from_be_bytes(png[16..20].try_into().unwrap()) > 1_000);
        assert!(u32::from_be_bytes(png[20..24].try_into().unwrap()) > 700);
    }

    #[test]
    fn transparent_and_white_png_exports_are_distinct_valid_outputs() {
        let document = FigureDocument::fixed();
        let white = figure_png_with_background(&document, 300, false).unwrap();
        let transparent = figure_png_with_background(&document, 300, true).unwrap();
        assert_eq!(&white[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&transparent[..8], b"\x89PNG\r\n\x1a\n");
        assert_ne!(white, transparent);
    }
}
