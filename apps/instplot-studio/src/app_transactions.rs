use super::*;
use instplot_studio::{
    HandoffCleanup, import_handoff, save_resolved_figure_pdf,
    save_resolved_figure_png_with_background, save_resolved_figure_svg,
};

pub(super) enum AppAction {
    NewProject {
        untitled: String,
    },
    OpenHandoff {
        path: PathBuf,
        untitled: String,
    },
    ImportFiles {
        paths: Vec<PathBuf>,
        preferred_columns: Option<(String, String)>,
    },
    CommitPreparedData {
        document: Box<FigureDocument>,
        datasets: Vec<DataSet>,
        source_path: PathBuf,
        selected_dataset: String,
    },
    RemoveDataSources(Vec<String>),
    OpenProject(PathBuf),
    SaveProject(PathBuf),
    Undo,
    Redo,
    EditDocument {
        command: Box<EditCommand>,
        group: Option<EditGroup>,
    },
    ExportFigure {
        path: PathBuf,
        format: FigureExport,
    },
}

pub(super) enum FigureExport {
    Pdf,
    Svg,
    Png {
        dpi: u32,
        transparent_background: bool,
    },
}

#[derive(Clone, Debug)]
pub(super) enum AppEffect {
    NewProject,
    OpenedHandoff {
        producer_name: String,
        producer_version: String,
    },
    Imported {
        paths: Vec<PathBuf>,
        imported_ids: BTreeSet<String>,
        read: usize,
        added: usize,
        replaced: usize,
        diagnostics: Vec<String>,
        replaced_showcase: bool,
    },
    PreparedDataCommitted {
        selected_dataset: String,
    },
    RemovedData,
    OpenedProject {
        path: PathBuf,
        source: OpenProjectSource,
        warnings: Vec<String>,
    },
    SavedProject {
        path: PathBuf,
    },
    HistoryMoved {
        description: String,
        redo: bool,
    },
    DocumentEdited {
        changed: bool,
    },
    Exported {
        path: PathBuf,
        bytes: usize,
    },
}

pub(super) struct AppOutcome {
    pub(super) document: FigureDocument,
    pub(super) session: StudioSession,
    pub(super) workspace: WorkspaceState,
    pub(super) edit_history: EditHistory,
    pub(super) resolved: ResolvedFigure,
    pub(super) publication_report: PublicationReport,
    pub(super) effect: AppEffect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AppError {
    pub(super) code: &'static str,
    pub(super) diagnostics: Vec<String>,
}

pub(super) struct AppTransactionState<'a> {
    pub(super) document: &'a FigureDocument,
    pub(super) session: &'a StudioSession,
    pub(super) workspace: &'a WorkspaceState,
    pub(super) edit_history: &'a EditHistory,
}

pub(super) struct ApplicationController;

impl ApplicationController {
    pub(super) fn execute(
        state: AppTransactionState<'_>,
        action: AppAction,
    ) -> Result<AppOutcome, AppError> {
        match action {
            AppAction::NewProject { untitled } => Self::new_project(state, untitled),
            AppAction::OpenHandoff { path, untitled } => Self::open_handoff(state, path, untitled),
            AppAction::ImportFiles {
                paths,
                preferred_columns,
            } => Self::import_files(state, paths, preferred_columns),
            AppAction::CommitPreparedData {
                document,
                datasets,
                source_path,
                selected_dataset,
            } => Self::commit_prepared_data(
                state,
                *document,
                datasets,
                source_path,
                selected_dataset,
            ),
            AppAction::RemoveDataSources(ids) => Self::remove_data_sources(state, ids),
            AppAction::OpenProject(path) => Self::open_project(state, path),
            AppAction::SaveProject(path) => Self::save_project(state, path),
            AppAction::Undo => Self::move_history(state, false),
            AppAction::Redo => Self::move_history(state, true),
            AppAction::EditDocument { command, group } => {
                Self::edit_document(state, *command, group)
            }
            AppAction::ExportFigure { path, format } => Self::export_figure(state, path, format),
        }
    }

    fn new_project(
        state: AppTransactionState<'_>,
        untitled: String,
    ) -> Result<AppOutcome, AppError> {
        let document = FigureDocument::showcase();
        let resolved = resolved_preview(&document).map_err(|error| AppError {
            code: "new-project-layout",
            diagnostics: vec![error.to_string()],
        })?;
        let publication_report = check_publication(
            &document,
            &resolved,
            document.export_preferences().selected_raster_dpi,
        );
        let (session, warnings) = StudioSession::from_project(document.project());
        if !warnings.is_empty() {
            return Err(AppError {
                code: "new-project-session",
                diagnostics: warnings,
            });
        }
        let mut edit_history = state.edit_history.clone();
        edit_history.reset(&document, true);
        Ok(AppOutcome {
            document,
            session,
            workspace: WorkspaceState::new(&untitled),
            edit_history,
            resolved,
            publication_report,
            effect: AppEffect::NewProject,
        })
    }

    fn open_handoff(
        state: AppTransactionState<'_>,
        path: PathBuf,
        untitled: String,
    ) -> Result<AppOutcome, AppError> {
        let imported =
            import_handoff(&path, HandoffCleanup::DeleteAfterImport).map_err(|error| AppError {
                code: "handoff",
                diagnostics: vec![error.to_string()],
            })?;
        let resolved = resolved_preview(&imported.document).map_err(|error| AppError {
            code: "handoff-layout",
            diagnostics: vec![error.to_string()],
        })?;
        let publication_report = check_publication(
            &imported.document,
            &resolved,
            imported.document.export_preferences().selected_raster_dpi,
        );
        let mut session = StudioSession::default();
        session.replace_datasets(imported.datasets);
        let mut edit_history = state.edit_history.clone();
        edit_history.reset(&imported.document, false);
        Ok(AppOutcome {
            document: imported.document,
            session,
            workspace: WorkspaceState::from_lite(&untitled),
            edit_history,
            resolved,
            publication_report,
            effect: AppEffect::OpenedHandoff {
                producer_name: imported.producer_name,
                producer_version: imported.producer_version,
            },
        })
    }

    fn import_files(
        state: AppTransactionState<'_>,
        paths: Vec<PathBuf>,
        preferred_columns: Option<(String, String)>,
    ) -> Result<AppOutcome, AppError> {
        let replace_showcase = state.workspace.should_replace_showcase_on_import();
        let mut candidate_session = if replace_showcase {
            StudioSession::default()
        } else {
            state.session.clone()
        };
        let mut candidate_document = state.document.clone();
        let mut imported_ids = BTreeSet::new();
        let mut imported_paths = Vec::new();
        let mut diagnostics = Vec::new();
        let (mut read, mut added, mut replaced) = (0, 0, 0);
        let mut pending = paths;

        // A fit-only file may precede its source in the picker. Retry rejected files after
        // another file succeeds, while each individual file remains an all-or-nothing change.
        while !pending.is_empty() {
            let accepted_before = imported_paths.len();
            let mut rejected = Vec::new();
            for path in pending {
                let (trial_datasets, outcome) =
                    match DataImporter::import_file(candidate_session.datasets(), &path) {
                        Ok(imported) => imported,
                        Err(error) => {
                            // Keep stable diagnostic IDs in the data layer for tests and
                            // deduplication, but never expose them as user-facing copy.
                            rejected.push((path, error.reason));
                            continue;
                        }
                    };
                let mut trial_session = StudioSession::default();
                trial_session.replace_datasets(trial_datasets);
                let file_ids = trial_session
                    .datasets()
                    .iter()
                    .filter(|dataset| dataset.source == path)
                    .map(|dataset| dataset.plot_id.clone())
                    .collect::<BTreeSet<_>>();
                let trial_document = match document_with_imported_datasets(
                    &candidate_document,
                    trial_session.datasets(),
                    &file_ids,
                    replace_showcase && imported_paths.is_empty(),
                    preferred_columns
                        .as_ref()
                        .map(|(x, y)| (x.as_str(), y.as_str())),
                ) {
                    Ok(document) => document,
                    Err(error) => {
                        rejected.push((path, error));
                        continue;
                    }
                };
                if let Err(error) = resolved_preview(&trial_document) {
                    rejected.push((path, error.to_string()));
                    continue;
                }
                read += outcome.read;
                added += outcome.added;
                replaced += outcome.replaced;
                imported_ids.extend(file_ids);
                if outcome.skipped_unlinked_fits > 0 {
                    diagnostics.push(format!(
                        "{}：已导入原始数据；跳过 {} 条多来源拟合，因为文件未记录每条拟合对应的来源，无法安全恢复关联",
                        path.display(),
                        outcome.skipped_unlinked_fits
                    ));
                }
                imported_paths.push(path);
                candidate_session = trial_session;
                candidate_document = trial_document;
            }
            if accepted_before == imported_paths.len() {
                diagnostics.extend(
                    rejected
                        .into_iter()
                        .map(|(path, error)| format!("{}：{error}", path.display())),
                );
                break;
            }
            pending = rejected.into_iter().map(|(path, _)| path).collect();
        }

        if imported_paths.is_empty() {
            return Err(AppError {
                code: "import",
                diagnostics,
            });
        }

        let resolved = resolved_preview(&candidate_document).map_err(|error| AppError {
            code: "import-layout",
            diagnostics: vec![error.to_string()],
        })?;
        let publication_report = check_publication(
            &candidate_document,
            &resolved,
            candidate_document.export_preferences().selected_raster_dpi,
        );
        let mut workspace = state.workspace.clone();
        workspace.note_data_import(&imported_paths[0]);
        let mut edit_history = state.edit_history.clone();
        edit_history.rebase_after_external_change();

        Ok(AppOutcome {
            document: candidate_document,
            session: candidate_session,
            workspace,
            edit_history,
            resolved,
            publication_report,
            effect: AppEffect::Imported {
                paths: imported_paths,
                imported_ids,
                read,
                added,
                replaced,
                diagnostics,
                replaced_showcase: replace_showcase,
            },
        })
    }

    fn commit_prepared_data(
        state: AppTransactionState<'_>,
        document: FigureDocument,
        datasets: Vec<DataSet>,
        source_path: PathBuf,
        selected_dataset: String,
    ) -> Result<AppOutcome, AppError> {
        document.project().validate().map_err(|error| AppError {
            code: "prepared-data",
            diagnostics: vec![error.to_string()],
        })?;
        let resolved = resolved_preview(&document).map_err(|error| AppError {
            code: "prepared-data-layout",
            diagnostics: vec![error.to_string()],
        })?;
        let publication_report = check_publication(
            &document,
            &resolved,
            document.export_preferences().selected_raster_dpi,
        );
        let mut session = StudioSession::default();
        session.replace_datasets(datasets);
        let mut workspace = state.workspace.clone();
        workspace.note_data_import(&source_path);
        let mut edit_history = state.edit_history.clone();
        edit_history.rebase_after_external_change();
        Ok(AppOutcome {
            document,
            session,
            workspace,
            edit_history,
            resolved,
            publication_report,
            effect: AppEffect::PreparedDataCommitted { selected_dataset },
        })
    }

    fn remove_data_sources(
        state: AppTransactionState<'_>,
        ids: Vec<String>,
    ) -> Result<AppOutcome, AppError> {
        let mut document = state.document.clone();
        let mut edit_history = state.edit_history.clone();
        let edit = edit_history
            .execute(
                &mut document,
                EditCommand::DeleteDataSources {
                    data_source_ids: ids,
                },
                None,
            )
            .map_err(|error| AppError {
                code: "remove-data",
                diagnostics: vec![error],
            })?;
        if !edit.changed {
            return Err(AppError {
                code: "remove-data-unchanged",
                diagnostics: vec![
                    "the requested data sources did not change the document".to_owned(),
                ],
            });
        }
        let resolved = resolved_preview(&document).map_err(|error| AppError {
            code: "remove-data-layout",
            diagnostics: vec![error.to_string()],
        })?;
        let publication_report = check_publication(
            &document,
            &resolved,
            document.export_preferences().selected_raster_dpi,
        );
        let (session, warnings) = StudioSession::from_project(document.project());
        if !warnings.is_empty() {
            return Err(AppError {
                code: "remove-data-session",
                diagnostics: warnings,
            });
        }
        Ok(AppOutcome {
            document,
            session,
            workspace: state.workspace.clone(),
            edit_history,
            resolved,
            publication_report,
            effect: AppEffect::RemovedData,
        })
    }

    fn open_project(state: AppTransactionState<'_>, path: PathBuf) -> Result<AppOutcome, AppError> {
        let (document, report) = FigureDocument::open(&path).map_err(|error| AppError {
            code: "open-project",
            diagnostics: vec![error.to_string()],
        })?;
        let resolved = resolved_preview(&document).map_err(|error| AppError {
            code: "project-layout",
            diagnostics: vec![error.to_string()],
        })?;
        let publication_report = check_publication(
            &document,
            &resolved,
            document.export_preferences().selected_raster_dpi,
        );
        let (session, session_warnings) = StudioSession::from_project(document.project());
        let opened_primary = report.source == OpenProjectSource::Primary;
        let mut edit_history = state.edit_history.clone();
        edit_history.reset(&document, opened_primary);
        let workspace = WorkspaceState::from_project(&path, !opened_primary);
        let warnings = report
            .warnings
            .into_iter()
            .filter(|warning| !warning.starts_with("External data source"))
            .chain(session_warnings)
            .collect();
        Ok(AppOutcome {
            document,
            session,
            workspace,
            edit_history,
            resolved,
            publication_report,
            effect: AppEffect::OpenedProject {
                path,
                source: report.source,
                warnings,
            },
        })
    }

    fn save_project(state: AppTransactionState<'_>, path: PathBuf) -> Result<AppOutcome, AppError> {
        state.document.save(&path).map_err(|error| AppError {
            code: "save-project",
            diagnostics: vec![error.to_string()],
        })?;
        let mut workspace = state.workspace.clone();
        workspace.note_saved(path.clone());
        let mut edit_history = state.edit_history.clone();
        edit_history.mark_saved(state.document);
        let resolved = resolved_preview(state.document).map_err(|error| AppError {
            code: "save-project-layout",
            diagnostics: vec![error.to_string()],
        })?;
        let publication_report = check_publication(
            state.document,
            &resolved,
            state.document.export_preferences().selected_raster_dpi,
        );
        Ok(AppOutcome {
            document: state.document.clone(),
            session: state.session.clone(),
            workspace,
            edit_history,
            resolved,
            publication_report,
            effect: AppEffect::SavedProject { path },
        })
    }

    fn move_history(state: AppTransactionState<'_>, redo: bool) -> Result<AppOutcome, AppError> {
        let mut document = state.document.clone();
        let mut edit_history = state.edit_history.clone();
        let description = if redo {
            edit_history.redo(&mut document)
        } else {
            edit_history.undo(&mut document)
        }
        .ok_or_else(|| AppError {
            code: if redo { "redo-empty" } else { "undo-empty" },
            diagnostics: Vec::new(),
        })?;
        let resolved = resolved_preview(&document).map_err(|error| AppError {
            code: if redo { "redo-layout" } else { "undo-layout" },
            diagnostics: vec![error.to_string()],
        })?;
        let publication_report = check_publication(
            &document,
            &resolved,
            document.export_preferences().selected_raster_dpi,
        );
        let (session, warnings) = StudioSession::from_project(document.project());
        if !warnings.is_empty() {
            return Err(AppError {
                code: if redo { "redo-session" } else { "undo-session" },
                diagnostics: warnings,
            });
        }
        Ok(AppOutcome {
            document,
            session,
            workspace: state.workspace.clone(),
            edit_history,
            resolved,
            publication_report,
            effect: AppEffect::HistoryMoved { description, redo },
        })
    }

    fn edit_document(
        state: AppTransactionState<'_>,
        command: EditCommand,
        group: Option<EditGroup>,
    ) -> Result<AppOutcome, AppError> {
        let mut document = state.document.clone();
        let mut edit_history = state.edit_history.clone();
        let edit = edit_history
            .execute(&mut document, command, group)
            .map_err(|error| AppError {
                code: "edit",
                diagnostics: vec![error],
            })?;
        let display = instplot_export::resolve(&edit.layout.result.display_list);
        let resolved = ResolvedFigure {
            layout: edit.layout,
            display,
        };
        let publication_report = check_publication(
            &document,
            &resolved,
            document.export_preferences().selected_raster_dpi,
        );
        Ok(AppOutcome {
            document,
            session: state.session.clone(),
            workspace: state.workspace.clone(),
            edit_history,
            resolved,
            publication_report,
            effect: AppEffect::DocumentEdited {
                changed: edit.changed,
            },
        })
    }

    fn export_figure(
        state: AppTransactionState<'_>,
        path: PathBuf,
        format: FigureExport,
    ) -> Result<AppOutcome, AppError> {
        let resolved = resolved_preview(state.document).map_err(|error| AppError {
            code: "export-layout",
            diagnostics: vec![error.to_string()],
        })?;
        let bytes = match format {
            FigureExport::Pdf => save_resolved_figure_pdf(&resolved, &path),
            FigureExport::Svg => save_resolved_figure_svg(&resolved, &path),
            FigureExport::Png {
                dpi,
                transparent_background,
            } => save_resolved_figure_png_with_background(
                &resolved,
                &path,
                dpi,
                transparent_background,
            ),
        }
        .map_err(|error| AppError {
            code: "export",
            diagnostics: vec![error.to_string()],
        })?;
        let publication_report = check_publication(
            state.document,
            &resolved,
            state.document.export_preferences().selected_raster_dpi,
        );
        Ok(AppOutcome {
            document: state.document.clone(),
            session: state.session.clone(),
            workspace: state.workspace.clone(),
            edit_history: state.edit_history.clone(),
            resolved,
            publication_report,
            effect: AppEffect::Exported { path, bytes },
        })
    }
}
