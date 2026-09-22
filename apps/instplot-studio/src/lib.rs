//! Product boundary and non-UI application services for InstPlot Studio.

mod document;
mod export;
mod preview;
mod session;

pub use document::{AxisRanges, FigureDocument, SeriesDescriptor, SeriesKind};
pub use export::{FixedPdfExportError, fixed_figure_pdf, save_fixed_figure_pdf};
pub use preview::{EguiPreviewAdapter, PreviewAdapter, PreviewMetrics};
pub use session::{ImportOutcome, StudioSession};

pub const PRODUCT_NAME: &str = "InstPlot Studio";
pub const BINARY_NAME: &str = "instplot-studio";

pub fn product_info() -> String {
    format!(
        "{PRODUCT_NAME}\t{BINARY_NAME}\t{}",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_identity_is_stable_and_independent() {
        assert_eq!(PRODUCT_NAME, "InstPlot Studio");
        assert_eq!(BINARY_NAME, "instplot-studio");
        assert_eq!(product_info(), "InstPlot Studio\tinstplot-studio\t0.1.0");
    }

    #[test]
    fn a_new_session_uses_shared_core_without_implicit_data() {
        let session = StudioSession::default();
        assert_eq!(session.dataset_count(), 0);
        assert!(session.datasets().is_empty());
    }
}
