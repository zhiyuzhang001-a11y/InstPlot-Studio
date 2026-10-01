use super::*;

#[derive(Clone)]
pub(super) enum DataRemovalRequest {
    File { label: String, ids: Vec<String> },
    All(Vec<String>),
}

impl DataRemovalRequest {
    pub(super) fn ids(&self) -> Vec<String> {
        match self {
            Self::File { ids, .. } | Self::All(ids) => ids.clone(),
        }
    }
}

pub(super) struct StudioApp {
    pub(super) branding: instplot_ui::Branding,
    pub(super) features: instplot_ui::FeatureSet,
    pub(super) session: StudioSession,
    pub(super) document: FigureDocument,
    pub(super) resolved: ResolvedFigure,
    pub(super) publication_report: PublicationReport,
    pub(super) preview: EguiPreviewAdapter,
    pub(super) selected_series: Option<String>,
    pub(super) selected_canvas_node: Option<String>,
    pub(super) selected_canvas_role: Option<SelectableRole>,
    pub(super) selection_candidates: Vec<CanvasHit>,
    pub(super) selected_dataset: Option<String>,
    pub(super) binding_x: String,
    pub(super) binding_y: String,
    pub(super) binding_error: String,
    pub(super) pending_data_removal: Option<DataRemovalRequest>,
    pub(super) pending_managed_save_conflict: Option<ManagedSaveConflict>,
    pub(super) label_inputs: BTreeMap<String, LabelInputState>,
    pub(super) numeric_inputs: BTreeMap<String, DeferredNumericInput>,
    pub(super) axis_numeric_scale_sessions: Vec<AxisNumericScaleSession>,
    pub(super) axis_scale_transitions: Vec<AxisScaleTransitionDraft>,
    pub(super) x_fixed_ticks: String,
    pub(super) y_fixed_ticks: String,
    pub(super) workspace: WorkspaceState,
    pub(super) edit_history: EditHistory,
    pub(super) pending_action: Option<PendingAction>,
    pub(super) pending_export: Option<ExportKind>,
    pub(super) allow_close: bool,
    pub(super) canvas_zoom: f32,
    pub(super) canvas_scroll: egui::Vec2,
    pub(super) last_canvas_figure_center: Option<egui::Vec2>,
    pub(super) hover_data_coordinates: Option<HoverDataCoordinates>,
    pub(super) trackpad_scroll_active: bool,
    pub(super) show_layers: bool,
    pub(super) show_inspector: bool,
    pub(super) show_palette: bool,
    pub(super) show_messages: bool,
    pub(super) focus_inspector: bool,
    pub(super) focus_palette: bool,
    pub(super) focus_manual_data: bool,
    pub(super) show_axis_visibility: bool,
    pub(super) focus_axis_visibility: bool,
    pub(super) axis_visibility_identity: AxisIdentity,
    pub(super) context_editor_focus_target: Option<CanvasHit>,
    pub(super) context_editor_targets: Vec<CanvasHit>,
    pub(super) active_artist_drag: Option<ArtistDrag>,
    pub(super) drawing_tool: DrawingTool,
    pub(super) tool_draft: Option<ToolDraft>,
    pub(super) reference_draft: Option<ReferenceDraft>,
    pub(super) focus_reference_draft: bool,
    pub(super) messages: Vec<AppMessage>,
    pub(super) status: Option<(String, Instant)>,
    pub(super) manual_data: ManualDataState,
    pub(super) marker_size_for_all: bool,
    pub(super) marker_interval_for_all: bool,
    pub(super) marker_fill_for_all: bool,
    pub(super) language: UiLanguage,
    pub(super) update: AppUpdateState,
    #[cfg(target_os = "macos")]
    pub(super) update_health: Option<crate::update_macos::HealthStartup>,
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    pub(super) update_health: Option<instplot_studio::update_windows::WindowsHealthStartup>,
    #[cfg(target_os = "macos")]
    pub(super) macos_open_files: Option<crate::macos_open_files::MacOpenFiles>,
    pub(super) first_frame: bool,
    pub(super) started: Instant,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum DrawingTool {
    #[default]
    Select,
    Reference {
        orientation: ReferenceOrientation,
        axes: AxisBinding,
    },
    Measurement {
        axes: AxisBinding,
        constraint: MeasurementConstraint,
        start_arrow: bool,
        end_arrow: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ToolDraft {
    pub(super) start: (f64, f64),
    pub(super) end: (f64, f64),
    pub(super) axes: AxisBinding,
    pub(super) constraint: MeasurementConstraint,
    pub(super) start_arrow: bool,
    pub(super) end_arrow: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ReferenceDraft {
    pub(super) orientation: ReferenceOrientation,
    pub(super) value: f64,
    pub(super) axes: AxisBinding,
    pub(super) stroke: StrokeStyle,
    pub(super) include_in_autoscale: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CanvasToolEvent {
    pub(super) clicked: bool,
    pub(super) started: bool,
    pub(super) stopped: bool,
    pub(super) press: Option<HoverDataCoordinates>,
    pub(super) current: Option<HoverDataCoordinates>,
    pub(super) shift: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PendingAction {
    NewProject,
    OpenLiteHandoff,
    OpenProject,
    OpenProjectPath(PathBuf),
    Exit,
    RestartForUpdate,
}

#[derive(Clone, Debug)]
pub(super) struct ManagedSaveConflict {
    pub(super) project_path: PathBuf,
    pub(super) explanation: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExportKind {
    Pdf,
    Png,
    Svg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MessageLevel {
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub(super) struct AppMessage {
    pub(super) code: String,
    pub(super) level: MessageLevel,
    pub(super) text: String,
}

#[derive(Clone, Debug)]
pub(super) struct ArtistDrag {
    pub(super) id: String,
    pub(super) start_x: f64,
    pub(super) start_y: f64,
    pub(super) bounds: (f64, f64, f64, f64),
    pub(super) total_delta: egui::Vec2,
    pub(super) press_pointer: Option<(f64, f64)>,
    pub(super) legend: Option<LegendDragContext>,
    pub(super) mode: ArtistDragMode,
    pub(super) preview_bounds: (f64, f64, f64, f64),
    pub(super) candidate_grid: Option<LegendGrid>,
    pub(super) candidate_placement: Option<LegendPlacement>,
    pub(super) connector_index: Option<usize>,
    pub(super) connector_text_bounds: Option<(f64, f64, f64, f64)>,
    pub(super) role: SelectableRole,
    pub(super) measurement: Option<MeasurementDragContext>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MeasurementDragContext {
    pub(super) start: (f64, f64),
    pub(super) end: (f64, f64),
    pub(super) press: (f64, f64),
    pub(super) label_offset: (f64, f64),
    pub(super) axes: AxisBinding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArtistDragMode {
    Move,
    ResizeColumns,
    ResizeRows,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LegendDragContext {
    pub(super) axes_left: f64,
    pub(super) axes_right: f64,
    pub(super) axes_top: f64,
    pub(super) axes_bottom: f64,
    pub(super) canvas_top: f64,
    pub(super) entries: usize,
    pub(super) cell_width: f64,
}

impl LegendDragContext {
    pub(super) fn placement_at(&self, pointer_x: f64, pointer_y: f64) -> LegendPlacement {
        if pointer_y < self.axes_top {
            LegendPlacement::Above
        } else if pointer_x > self.axes_right {
            LegendPlacement::Right
        } else {
            LegendPlacement::Inside
        }
    }

    pub(super) fn preview_bounds(
        &self,
        bounds: (f64, f64, f64, f64),
        placement: LegendPlacement,
    ) -> (f64, f64, f64, f64) {
        let width = bounds.2 - bounds.0;
        let height = bounds.3 - bounds.1;
        match placement {
            LegendPlacement::Right => {
                let x = self.axes_right + 6.0;
                let y = bounds.1.max(0.0);
                (x, y, x + width, y + height)
            }
            LegendPlacement::Above => {
                let x = bounds.0.max(0.0);
                let y = bounds
                    .1
                    .max(0.0)
                    .min((self.axes_top - height - 4.0).max(0.0));
                (x, y, x + width, y + height)
            }
            LegendPlacement::Inside | LegendPlacement::Auto => {
                let x = bounds
                    .0
                    .max(self.axes_left)
                    .min((self.axes_right - width).max(self.axes_left));
                let y = bounds
                    .1
                    .max(self.axes_top)
                    .min((self.axes_bottom - height).max(self.axes_top));
                (x, y, x + width, y + height)
            }
        }
    }

    pub(super) fn stored_position(
        &self,
        bounds: (f64, f64, f64, f64),
        placement: LegendPlacement,
    ) -> (f64, f64) {
        let bounds = self.preview_bounds(bounds, placement);
        let y = if placement == LegendPlacement::Above {
            bounds.1
        } else {
            bounds.1 - self.canvas_top
        };
        (bounds.0.max(0.0), y.max(0.0))
    }
}

impl ArtistDrag {
    pub(super) fn frame_delta(
        &self,
        pointer: Option<(f64, f64)>,
        fallback: egui::Vec2,
        zoom: f32,
    ) -> egui::Vec2 {
        if let (Some(press), Some(pointer)) = (self.press_pointer, pointer) {
            egui::vec2(
                (pointer.0 - press.0) as f32 * zoom,
                (pointer.1 - press.1) as f32 * zoom,
            ) - self.total_delta
        } else {
            fallback
        }
    }

    pub(super) fn position_after(
        &mut self,
        frame_delta: egui::Vec2,
        zoom: f32,
        width: f64,
        height: f64,
    ) -> (f64, f64) {
        self.total_delta += frame_delta;
        let dx = f64::from(self.total_delta.x / zoom)
            .clamp(-self.bounds.0, (width - self.bounds.2).max(-self.bounds.0));
        let dy = f64::from(self.total_delta.y / zoom)
            .clamp(-self.bounds.1, (height - self.bounds.3).max(-self.bounds.1));
        (self.start_x + dx, self.start_y + dy)
    }
}

#[derive(Clone, Debug)]
pub(super) struct CanvasDragEvent {
    pub(super) id: String,
    pub(super) role: SelectableRole,
    pub(super) bounds: (f64, f64, f64, f64),
    pub(super) delta: egui::Vec2,
    pub(super) pointer: Option<(f64, f64)>,
    pub(super) press_pointer: Option<(f64, f64)>,
    pub(super) mode: ArtistDragMode,
    pub(super) started: bool,
    pub(super) stopped: bool,
    pub(super) data_index: Option<usize>,
    pub(super) shift: bool,
}

#[derive(Clone, Debug)]
pub(super) struct LabelInputState {
    pub(super) source_nodes: Vec<LabelNode>,
    pub(super) text: String,
}

#[derive(Clone, Debug)]
pub(super) struct DeferredNumericInput {
    pub(super) source_value: f64,
    pub(super) text: String,
    pub(super) error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AxisNumericScaleSession {
    pub(super) identity: AxisIdentity,
    pub(super) exponent: i32,
}

#[derive(Clone, Debug)]
pub(super) struct AxisScaleTransitionDraft {
    pub(super) identity: AxisIdentity,
    pub(super) exponent: i32,
    pub(super) label_text: String,
    pub(super) error: Option<String>,
}

#[derive(Default)]
pub(super) struct ManualDataState {
    pub(super) open: bool,
    pub(super) input: ManualDataInput,
    pub(super) editing_group_id: Option<String>,
    pub(super) error: Option<String>,
}

#[derive(Clone, Copy)]
pub(super) enum LabelInputTarget<'a> {
    Axis(AxisIdentity),
    Semantic(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HoverDataCoordinates {
    pub(super) x1: f64,
    pub(super) y1: f64,
    pub(super) x2: Option<f64>,
    pub(super) y2: Option<f64>,
}
