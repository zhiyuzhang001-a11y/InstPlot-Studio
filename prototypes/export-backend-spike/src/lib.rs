//! A4 validation spike: one resolved Display List consumed by PDF, SVG, and raster backends.

mod pdf;
mod raster;
mod resolve;
mod svg;

pub use pdf::{ExportError, to_pdf};
#[cfg(feature = "comparison-raster")]
pub use raster::rasterize_via_svg;
pub use raster::{Background, RasterImage, encode_png, rasterize_direct};
pub use resolve::{
    RasterAsset, ResolvedDisplayList, ResolvedGlyph, ResolvedItem, ResolvedRun, ResolvedText,
    resolve, resolve_with_resources,
};
pub use svg::to_svg;
