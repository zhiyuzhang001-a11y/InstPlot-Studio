use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

mod app_controller;
mod app_state;
mod app_transactions;
mod app_ui;
mod canvas_support;
mod editor_support;
mod import_flow;
mod startup;
mod text_input;
mod ui_chrome;
mod ui_text;
mod workspace;

use app_state::*;
use app_transactions::*;
use canvas_support::*;
use editor_support::*;
use eframe::egui;
use import_flow::*;
use instplot_core::{DataSet, DataSetKind};
use instplot_export::{ResolvedItem, ResolvedText};
use instplot_layout::SelectableRole;
use instplot_render::{DisplayItem, PathVerb};
use instplot_studio::label_input;
use instplot_studio::{
    AnnotationConnectorRecord, ArtistProperties, ArtistRole, AxisDimension, AxisRanges, AxisRecord,
    AxisScale, CheckSeverity, DATA_FORMAT_CAPABILITIES, DataImporter, EditCommand, EditGroup,
    EditHistory, EguiPreviewAdapter, ErrorStatistic, FigureDocument, FormatterSpec, HandoffImport,
    LabelNode, LegendGrid, LegendPlacement, LocatorSpec, ManualAxisInput, ManualDataInput,
    ManualYInput, MarkerShape, OpenProjectSource, PRODUCT_NAME, PaletteKind, PreviewAdapter,
    PublicationReport, ReferenceOrientation, ResolvedFigure, SeriesCreationStyle, SeriesDescriptor,
    SeriesKind, StrokeStyle, StudioSession, USER_PALETTE_IDS, builtin_palette,
    builtin_palette_registry, check_publication, palette_series_color_ids, resolve_document,
};
use ui_chrome::*;
use ui_text::{Text, UiLanguage};
use workspace::WorkspaceState;

const BUILD_ID: &str = match option_env!("INSTPLOT_BUILD_ID") {
    Some(value) => value,
    None => "dev",
};

const DATA_SIDEBAR_DEFAULT_WIDTH: f32 = 260.0;
const DATA_SIDEBAR_MIN_WIDTH: f32 = 170.0;
const DATA_SIDEBAR_MAX_WIDTH: f32 = 520.0;
const DATA_SIDEBAR_CONTENT_MIN_WIDTH: f32 = 210.0;
#[cfg(test)]
const SIDEBAR_CLOSE_BUTTON_SIZE: f32 = 34.0;
const SIDEBAR_FILE_CLOSE_BUTTON_SIZE: f32 = 28.0;
const SIDEBAR_FILE_CARD_INNER_WIDTH: f32 = 190.0;
const _: () = {
    assert!(DATA_SIDEBAR_MIN_WIDTH < DATA_SIDEBAR_DEFAULT_WIDTH);
    assert!(DATA_SIDEBAR_DEFAULT_WIDTH < DATA_SIDEBAR_MAX_WIDTH);
};

fn main() {
    if let Err(error) = startup::run(std::env::args_os().skip(1)) {
        eprintln!("InstPlot Studio: {error}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod app_tests;
