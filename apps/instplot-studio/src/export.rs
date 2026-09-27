use core::fmt;
use std::io::Write;
use std::path::Path;

use atomicwrites::{AllowOverwrite, AtomicFile};

use crate::{DocumentLayoutError, FigureDocument, ResolvedFigure, resolve_document};

#[derive(Debug)]
pub enum FixedPdfExportError {
    Layout(DocumentLayoutError),
    Render(instplot_export::ExportError),
    Raster(instplot_export::RasterError),
    EncodePng(String),
    AtomicWrite(String),
}

impl fmt::Display for FixedPdfExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(error) => write!(formatter, "layout figure: {error}"),
            Self::Render(error) => write!(formatter, "render fixed figure PDF: {error}"),
            Self::Raster(error) => write!(formatter, "render figure PNG: {error}"),
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
    resolved_figure_pdf(&resolved)
}

pub fn resolved_figure_pdf(resolved: &ResolvedFigure) -> Result<Vec<u8>, FixedPdfExportError> {
    instplot_export::to_pdf(&resolved.display).map_err(FixedPdfExportError::Render)
}

pub fn figure_svg(document: &FigureDocument) -> Result<Vec<u8>, FixedPdfExportError> {
    let resolved = resolve_document(document).map_err(FixedPdfExportError::Layout)?;
    Ok(resolved_figure_svg(&resolved))
}

pub fn resolved_figure_svg(resolved: &ResolvedFigure) -> Vec<u8> {
    instplot_export::to_svg(&resolved.display).into_bytes()
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
    resolved_figure_png_with_background(&resolved, dpi, transparent_background)
}

pub fn resolved_figure_png_with_background(
    resolved: &ResolvedFigure,
    dpi: u32,
    transparent_background: bool,
) -> Result<Vec<u8>, FixedPdfExportError> {
    let background = if transparent_background {
        instplot_export::Background::Transparent
    } else {
        instplot_export::Background::White
    };
    let image = instplot_export::rasterize_direct(&resolved.display, dpi, background)
        .map_err(FixedPdfExportError::Raster)?;
    instplot_export::encode_png(&image)
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

pub fn save_resolved_figure_pdf(
    resolved: &ResolvedFigure,
    path: &Path,
) -> Result<usize, FixedPdfExportError> {
    let bytes = resolved_figure_pdf(resolved)?;
    atomic_write(path, &bytes)?;
    Ok(bytes.len())
}

pub fn save_figure_svg(
    document: &FigureDocument,
    path: &Path,
) -> Result<usize, FixedPdfExportError> {
    let bytes = figure_svg(document)?;
    atomic_write(path, &bytes)?;
    Ok(bytes.len())
}

pub fn save_resolved_figure_svg(
    resolved: &ResolvedFigure,
    path: &Path,
) -> Result<usize, FixedPdfExportError> {
    let bytes = resolved_figure_svg(resolved);
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

pub fn save_resolved_figure_png_with_background(
    resolved: &ResolvedFigure,
    path: &Path,
    dpi: u32,
    transparent_background: bool,
) -> Result<usize, FixedPdfExportError> {
    let bytes = resolved_figure_png_with_background(resolved, dpi, transparent_background)?;
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
    use crate::{AxisBinding, AxisMode, SeriesKind, XAxisSlot, YAxisSlot};

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
    fn oversized_png_returns_a_clear_error_without_allocating() {
        let mut document = FigureDocument::fixed();
        document.set_figure_size_mm(500.0, 500.0).unwrap();
        assert!(matches!(
            figure_png(&document, 1200),
            Err(FixedPdfExportError::Raster(
                instplot_export::RasterError::TooLarge { .. }
            ))
        ));
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

    #[test]
    fn svg_uses_the_same_resolved_canvas() {
        let resolved = resolve_document(&FigureDocument::fixed()).unwrap();
        let svg = String::from_utf8(resolved_figure_svg(&resolved)).unwrap();
        assert!(svg.contains(&format!(
            "viewBox=\"0 0 {:.5} {:.5}\"",
            resolved.display.width, resolved.display.height
        )));
    }

    #[test]
    fn dual_axis_pdf_svg_and_png_share_the_formal_resolved_canvas() {
        let mut document = FigureDocument::fixed();
        document.set_axis_mode(AxisMode::DualY).unwrap();
        let series_id = document
            .series()
            .into_iter()
            .find(|series| matches!(series.kind, SeriesKind::Line | SeriesKind::Scatter))
            .unwrap()
            .id;
        document
            .set_series_axis_binding(
                &series_id,
                AxisBinding {
                    x: XAxisSlot::X1,
                    y: YAxisSlot::Y2,
                },
            )
            .unwrap();
        let resolved = resolve_document(&document).unwrap();
        assert!(resolved.layout.result.y2_axis.is_some());

        let pdf = resolved_figure_pdf(&resolved).unwrap();
        let svg = String::from_utf8(resolved_figure_svg(&resolved)).unwrap();
        let png = resolved_figure_png_with_background(&resolved, 300, false).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(svg.contains(&format!(
            "viewBox=\"0 0 {:.5} {:.5}\"",
            resolved.display.width, resolved.display.height
        )));
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let expected_width = (f64::from(resolved.display.width) / 72.0 * 300.0).round() as u32;
        let expected_height = (f64::from(resolved.display.height) / 72.0 * 300.0).round() as u32;
        assert_eq!(
            u32::from_be_bytes(png[16..20].try_into().unwrap()),
            expected_width
        );
        assert_eq!(
            u32::from_be_bytes(png[20..24].try_into().unwrap()),
            expected_height
        );
    }

    #[test]
    fn failed_export_does_not_replace_an_existing_file() {
        // Rust test names contain `::`, which Windows rejects in filenames.
        // This test writes only once per test process, so the process id is unique enough.
        let path =
            std::env::temp_dir().join(format!("instplot-export-atomic-{}.png", std::process::id()));
        std::fs::write(&path, b"existing-good-output").unwrap();
        let mut document = FigureDocument::fixed();
        document.set_figure_size_mm(500.0, 500.0).unwrap();
        assert!(save_figure_png(&document, &path, 1200).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"existing-good-output");
        std::fs::remove_file(path).unwrap();
    }
}
