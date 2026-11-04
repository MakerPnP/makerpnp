//! The project's copy of a process.
//!
//! The copy can't be edited here, it's edited in its process document, see [`crate::process`].  The project's copy is
//! updated from the document's file when the project's processes are refreshed.

use derivative::Derivative;
use egui::{Frame, Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::tr;
use planner_app::{FileReference, ProcessReference, ProjectProcess, Reference};
use tracing::debug;

use crate::forms::NamedResponse;
use crate::process::editor::task_kind_name;
use crate::project::data::ProjectData;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ProcessTabUi {
    process: ProcessReference,

    pub component: ComponentState<ProcessTabUiCommand>,
}

impl ProcessTabUi {
    pub fn new(process: ProcessReference) -> Self {
        Self {
            process,
            component: ComponentState::default(),
        }
    }

    fn show_process(&self, ui: &mut Ui, project_process: &ProjectProcess) {
        let definition = &project_process.definition;

        //
        // toolbar
        //
        ui.horizontal(|ui| {
            if let Some(process_file) = &project_process.process_file {
                if ui
                    .button(tr!("project-process-tab-button-open"))
                    .clicked()
                {
                    self.component
                        .send(ProcessTabUiCommand::OpenClicked(process_file.clone()));
                }
            }

            if ui
                .button(tr!("project-process-tab-button-refresh"))
                .clicked()
            {
                self.component
                    .send(ProcessTabUiCommand::RefreshClicked);
            }

            if ui
                .button(format!("🗑 {}", tr!("project-process-tab-button-remove")))
                .named(tr!("project-process-tab-button-remove"))
                .clicked()
            {
                self.component
                    .send(ProcessTabUiCommand::RemoveClicked);
            }
        });

        ui.separator();

        let file = match &project_process.process_file {
            Some(FileReference::Relative(path)) | Some(FileReference::Absolute(path)) => path.display().to_string(),
            None => tr!("common-value-not-available"),
        };
        ui.label(tr!("project-process-tab-detail-file", { file: file }));
        ui.label(tr!("project-process-tab-detail-reference", { reference: definition.reference.to_string() }));

        ui.add_space(8.0);
        ui.strong(tr!("form-process-operations"));
        for operation in definition.operations.iter() {
            Frame::group(ui.style()).show(ui, |ui| {
                ui.label(operation.reference.to_string());
                ui.indent(
                    ui.id()
                        .with(("tasks", operation.reference.to_string())),
                    |ui| {
                        for task in operation.tasks.iter() {
                            ui.label(tr!("project-process-tab-task", {
                                task: task.reference.to_string(),
                                kind: task_kind_name(task.kind)
                            }));
                        }
                    },
                );
            });
        }

        ui.add_space(8.0);
        ui.strong(tr!("process-editor-rules"));
        if definition.rules.is_empty() {
            ui.label(tr!("form-common-combo-none"));
        }
        for rule in definition.rules.iter() {
            ui.label(rule.to_string());
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProcessTabUiCommand {
    None,
    OpenClicked(FileReference),
    RefreshClicked,
    RemoveClicked,
}

#[derive(Debug, Clone)]
pub enum ProcessTabUiAction {
    None,
    Open(FileReference),
    Refresh,
    Remove(ProcessReference),
}

pub struct ProcessTabUiContext<'a> {
    pub data: &'a ProjectData,
}

impl UiComponent for ProcessTabUi {
    type UiContext<'context> = ProcessTabUiContext<'context>;
    type UiCommand = ProcessTabUiCommand;
    type UiAction = ProcessTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let Some(project_process) = context
            .data
            .process_definition(&self.process)
        else {
            ui.spinner();
            return;
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            Frame::new()
                .inner_margin(8.0)
                .show(ui, |ui| self.show_process(ui, project_process));
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ProcessTabUiCommand::None => Some(ProcessTabUiAction::None),
            ProcessTabUiCommand::OpenClicked(process_file) => Some(ProcessTabUiAction::Open(process_file)),
            ProcessTabUiCommand::RefreshClicked => Some(ProcessTabUiAction::Refresh),
            ProcessTabUiCommand::RemoveClicked => Some(ProcessTabUiAction::Remove(self.process.clone())),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct ProcessTab {
    pub process: ProcessReference,
}

impl ProcessTab {
    pub fn new(process: Reference) -> Self {
        Self {
            process,
        }
    }
}

impl Tab for ProcessTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("project-process-tab-label", {process: self.process.to_string()});
        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(process_ui) = state.process_tab_uis.get(&self.process) else {
            ui.spinner();
            return;
        };
        UiComponent::ui(process_ui, ui, &mut ProcessTabUiContext {
            data: &state.data,
        });
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> OnCloseResponse {
        let mut state = context.state.lock().unwrap();
        if let Some(_process_ui) = state
            .process_tab_uis
            .remove(&self.process)
        {
            debug!("removed orphaned process ui. process: {:?}", &self.process);
        }
        OnCloseResponse::Close
    }
}
