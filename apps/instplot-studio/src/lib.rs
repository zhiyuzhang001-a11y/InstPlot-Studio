//! Product boundary and non-UI application services for InstPlot Studio.

mod data;
mod document;
mod editing;
mod export;
mod handoff;
pub mod label_input;
mod palette;
mod preview;
mod project;
mod publication;
mod render;
mod semantic;
mod session;
mod text_edit;

pub use data::{
    DATA_FORMAT_CAPABILITIES, DataDiagnostic, DataFormatCapability, DataImporter, ErrorStatistic,
    ImportOutcome, ManualAxisInput, ManualDataGroupInput, ManualDataInput, ParsedManualData,
    ParsedManualGroup, ParsedManualSeries, parse_manual_data, parse_numeric_column,
};
pub use document::{
    AutoscalePolicy, AxisDimension, AxisRanges, DataBounds, DocumentLayout, DocumentLayoutError,
    FigureDocument, MoveDirection, SeriesCreationStyle, SeriesDescriptor, SeriesKind, VisualBounds,
    apply_visual_padding, compute_data_bounds, layout_label_from_nodes,
};
pub use editing::{EditCommand, EditGroup, EditHistory, EditOutcome};
pub use export::{
    FixedPdfExportError, figure_pdf, figure_png, figure_png_with_background, figure_svg,
    fixed_figure_pdf, fixed_figure_png, resolved_figure_pdf, resolved_figure_png_with_background,
    resolved_figure_svg, save_figure_pdf, save_figure_png, save_figure_png_with_background,
    save_figure_svg, save_fixed_figure_pdf, save_fixed_figure_png, save_resolved_figure_pdf,
    save_resolved_figure_png_with_background, save_resolved_figure_svg,
};
pub use handoff::{
    HANDOFF_EXTENSION, HANDOFF_SCHEMA_VERSION, HandoffCleanup, HandoffError, HandoffImport,
    encode_handoff, import_handoff, write_handoff,
};
pub use palette::{
    PALETTE_DATA_SCHEMA_VERSION, PaletteKind, PaletteMetadata, PaletteOrdering, PaletteReview,
    ReviewStatus, USER_PALETTE_IDS, builtin_palette, builtin_palette_registry, builtin_palettes,
    palette_series_color_ids, registry_matches_metadata,
};
pub use preview::{EguiPreviewAdapter, PreviewAdapter, PreviewMetrics};
pub use project::{
    AnnotationConnectorRecord, ArtistKind, ArtistProperties, ArtistRecord, ArtistRole, AxesRecord,
    AxisAppearanceRecord, AxisRecord, AxisScale, DEFAULT_CURVE_WIDTH_PT,
    DEFAULT_ERROR_BAR_WIDTH_PT, DataBinding, DataSourceKind, DataSourceOrigin, DataSourcePayload,
    DataSourceRecord, EmbeddedColumn, ExportPreferences, FigureRecord, FitIdentity, FontFaceRecord,
    FontStyle, FormatterSpec, LabelNode, LegendEntry, LegendGrid, LegendPlacement, LocatorSpec,
    ManagedDataFile, ManagedDataFormat, ManualDataRecipe, ManualErrorStatistic,
    ManualMeasurementRecord, ManualPlotStyle, MarkerShape, MarkerStyle, OpenProjectReport,
    OpenProjectSource, OverrideRecord, PROJECT_SCHEMA_VERSION, PaletteColor, PaletteRegistry,
    ProjectDocument, ProjectError, ProvenanceRecord, ReferenceOrientation, SemanticLabel,
    SourceFingerprint, SourceState, StrokeStyle, TickDirection, TypographyProfile, decode_project,
    open_project, save_project,
};
pub use publication::{
    CVD_SIMULATION_VERSION, CheckSeverity, PUBLICATION_RULES_VERSION, PublicationFinding,
    PublicationReport, check_publication,
};
pub use render::{ResolvedFigure, resolve_document};
pub use semantic::{
    ColorPolicy, RequiredNonColorChannel, SEMANTIC_REGISTRY_VERSION, SemanticPolicy,
    non_color_signature, policy_for,
};
pub use session::StudioSession;
pub use text_edit::{BracketEdit, BracketMode, pair_bracket_edit};

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
