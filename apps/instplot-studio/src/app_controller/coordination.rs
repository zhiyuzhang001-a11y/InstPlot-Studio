use super::*;

impl StudioApp {
    pub(crate) fn apply_ranges(&mut self, ranges: AxisRanges, group: EditGroup) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        match ApplicationController::execute(
            state,
            AppAction::EditDocument {
                command: Box::new(EditCommand::SetAxisRanges(ranges)),
                group: Some(group),
            },
        ) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::DocumentEdited { changed } = effect else {
                    unreachable!("axis edit must produce an edit effect");
                };
                if changed {
                    self.set_success(self.language.text(Text::AxesOperation).to_owned());
                    self.clear_message("invalid-axes");
                }
            }
            Err(error) => self.push_error(
                "invalid-axes",
                self.language.operation_failed(
                    self.language.text(Text::AxesOperation),
                    &error.diagnostics.join("；"),
                ),
            ),
        }
    }

    pub(crate) fn undo(&mut self) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        if let Ok(outcome) = ApplicationController::execute(state, AppAction::Undo) {
            let effect = self.commit_app_outcome(outcome);
            let AppEffect::HistoryMoved {
                description,
                redo: false,
            } = effect
            else {
                unreachable!("undo action must produce an undo effect");
            };
            self.clear_all_axis_numeric_drafts();
            self.sync_axis_editors();
            self.set_success(self.language.undo(&description));
        }
    }

    pub(crate) fn redo(&mut self) {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        if let Ok(outcome) = ApplicationController::execute(state, AppAction::Redo) {
            let effect = self.commit_app_outcome(outcome);
            let AppEffect::HistoryMoved {
                description,
                redo: true,
            } = effect
            else {
                unreachable!("redo action must produce a redo effect");
            };
            self.clear_all_axis_numeric_drafts();
            self.sync_axis_editors();
            self.set_success(self.language.redo(&description));
        }
    }

    pub(crate) fn request_replacement(&mut self, action: PendingAction) {
        if self.edit_history.is_dirty(&self.document) {
            self.pending_action = Some(action);
        } else {
            self.perform_action(action);
        }
    }

    pub(crate) fn perform_action(&mut self, action: PendingAction) {
        self.pending_action = None;
        match action {
            PendingAction::NewProject => self.new_project(),
            PendingAction::OpenLiteHandoff => self.open_lite_handoff(),
            PendingAction::OpenProject => self.open_project(),
            PendingAction::OpenProjectPath(path) => self.open_project_path(path),
            PendingAction::Exit => {
                // The close command is issued by the confirmation dialog, which has the context.
            }
            PendingAction::RestartForUpdate => {
                let drafts = self.pending_update_drafts();
                if drafts.is_empty() {
                    self.update
                        .launch_helper(self.workspace.project_path().map(Path::to_path_buf));
                } else {
                    self.update.explain_blocked(drafts.join("\n"));
                }
            }
        }
    }

    pub(crate) fn unsaved_dialog(&mut self, context: &egui::Context) {
        let Some(action) = self.pending_action.clone() else {
            return;
        };
        egui::Window::new(self.language.text(Text::UnsavedChanges))
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .min_width(340.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(studio_card_frame(context.theme() == egui::Theme::Dark))
            .show(context, |ui| {
                studio_dialog_heading(ui, self.language.text(Text::UnsavedChanges));
                ui.label(self.language.text(Text::UnsavedExplanation));
                ui.horizontal(|ui| {
                    if ui.button(self.language.text(Text::Save)).clicked()
                        && self.save_project(false)
                    {
                        if action == PendingAction::Exit {
                            self.allow_close = true;
                            context.send_viewport_cmd(egui::ViewportCommand::Close);
                            self.pending_action = None;
                        } else {
                            self.perform_action(action.clone());
                        }
                    }
                    if ui.button(self.language.text(Text::Discard)).clicked() {
                        if action == PendingAction::Exit {
                            self.allow_close = true;
                            context.send_viewport_cmd(egui::ViewportCommand::Close);
                            self.pending_action = None;
                        } else {
                            self.perform_action(action);
                        }
                    }
                    if ui.button(self.language.text(Text::Cancel)).clicked() {
                        self.pending_action = None;
                    }
                });
            });
    }

    pub(crate) fn set_success(&mut self, text: String) {
        self.status = Some((text, Instant::now()));
    }

    pub(crate) fn push_warning(&mut self, code: impl Into<String>, text: String) {
        self.push_message(code, MessageLevel::Warning, text);
    }

    pub(crate) fn push_error(&mut self, code: impl Into<String>, text: String) {
        self.push_message(code, MessageLevel::Error, text);
    }

    pub(crate) fn push_message(
        &mut self,
        code: impl Into<String>,
        level: MessageLevel,
        text: String,
    ) {
        let code = code.into();
        if let Some(existing) = self
            .messages
            .iter_mut()
            .find(|message| message.code == code)
        {
            existing.level = level;
            existing.text = text.clone();
        } else {
            self.messages.push(AppMessage {
                code,
                level,
                text: text.clone(),
            });
        }
        self.status = Some((text, Instant::now()));
    }

    pub(crate) fn clear_message(&mut self, code: &str) {
        self.messages.retain(|message| message.code != code);
    }

    pub(crate) fn commit_app_outcome(&mut self, outcome: AppOutcome) -> AppEffect {
        self.document = outcome.document;
        self.session = outcome.session;
        self.workspace = outcome.workspace;
        self.edit_history = outcome.edit_history;
        self.resolved = outcome.resolved;
        self.publication_report = outcome.publication_report;
        outcome.effect
    }

    pub(crate) fn execute_document_edit(&mut self, command: EditCommand, success: &str) -> bool {
        self.execute_document_edit_with_group(command, success, None)
    }

    pub(crate) fn execute_document_edit_with_group(
        &mut self,
        command: EditCommand,
        success: &str,
        group: Option<EditGroup>,
    ) -> bool {
        let state = AppTransactionState {
            document: &self.document,
            session: &self.session,
            workspace: &self.workspace,
            edit_history: &self.edit_history,
        };
        match ApplicationController::execute(
            state,
            AppAction::EditDocument {
                command: Box::new(command),
                group,
            },
        ) {
            Ok(outcome) => {
                let effect = self.commit_app_outcome(outcome);
                let AppEffect::DocumentEdited { changed } = effect else {
                    unreachable!("edit action must produce an edit effect");
                };
                if changed {
                    self.set_success(success.to_owned());
                    self.clear_message("edit");
                    true
                } else {
                    false
                }
            }
            Err(error) => {
                self.push_error(error.code, error.diagnostics.join("；"));
                false
            }
        }
    }
}
