//! PDF, SVG, and raster backends consuming one resolved InstPlot display list.

mod pdf;
mod raster;
mod resolve;
mod svg;

pub use pdf::{ExportError, to_pdf};
#[cfg(feature = "comparison-raster")]
pub use raster::rasterize_via_svg;
pub use raster::{
    Background, MAX_RASTER_PIXELS, RasterError, RasterImage, checked_raster_dimensions, encode_png,
    rasterize_direct,
};
pub use resolve::{
    BundledFontFace, FontDiagnostic, FontOrigin, RasterAsset, ResolvedDisplayList, ResolvedGlyph,
    ResolvedItem, ResolvedRun, ResolvedText, bundled_font_faces, bundled_relation_face, resolve,
    resolve_with_resources,
};
pub use svg::to_svg;
