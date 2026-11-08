//! A process document, a process's definition in its own file.
//!
//! A project doesn't use the document, it keeps its own copy of each process it uses, made when the process is added to
//! the project and updated when the project's processes are refreshed.

use std::path::PathBuf;

use derivative::Derivative;
use egui::Ui;
use egui_i18n::tr;
use planner_app::{Event, ProcessDefinition, ProcessReference, ProcessView};
use slotmap::new_key_type;
use tracing::debug;

use crate::planner_app_core::{PlannerAction, PlannerCoreService, PlannerError};
use crate::process::editor::{ProcessEditorUi, ProcessEditorUiAction, ProcessEditorUiCommand, ProcessEditorUiContext};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_components::errors::{Errors, ErrorsUiCommand};

pub mod editor;

new_key_type! {
    /// A key for a process document
    pub struct ProcessKey;
}

#[derive(Debug)]
pub enum ProcessAction {
    Task(ProcessKey, Task<ProcessAction>),
    SetModifiedState(bool),
    UiCommand(ProcessUiCommand),
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Process {
    #[derivative(Debug = "ignore")]
    planner_core_service: PlannerCoreService,

    path: PathBuf,
    modified: bool,
    /// the core's generation of the modified state last applied, see [`PlannerAction::SetModifiedState`].
    modified_generation: u64,

    /// the definition last received from the core.
    definition: Option<ProcessDefinition>,
    /// the last error, shown until the user's next change is applied.
    error: Option<String>,
    /// The errors the tab has dismissed, its own and its editor's, until the user next does something in it
    /// (`FORM-13`).
    errors: Errors,

    editor: ProcessEditorUi,

    pub component: ComponentState<(ProcessKey, ProcessUiCommand)>,
}

#[derive(Debug, Clone)]
pub enum ProcessUiCommand {
    None,

    SetModifiedState {
        processes_modified: bool,
        generation: u64,
    },

    Create {
        reference: ProcessReference,
        preset: Option<String>,
    },
    Load,
    Save,
    Saved,

    Error(PlannerError),

    ProcessView(ProcessView),
    RequestView,

    EditorCommand(ProcessEditorUiCommand),
    /// An error's command, such as "Dismiss" clicked (`FORM-13`).
    Errors(ErrorsUiCommand),
}

impl ProcessUiCommand {
    /// Whether the user did it, a change to the process in its editor, or saving it, so the tab shows its dismissed
    /// errors again, as it may have caused them again, rather than the core, or the tab itself (`FORM-13`).
    fn is_users(&self) -> bool {
        matches!(self, ProcessUiCommand::EditorCommand(_) | ProcessUiCommand::Save)
    }
}

impl Process {
    pub fn from_path(path: PathBuf, key: ProcessKey) -> (Self, Vec<ProcessUiCommand>) {
        Self::new_inner(path, key, ProcessUiCommand::Load)
    }

    pub fn new(
        path: PathBuf,
        key: ProcessKey,
        reference: ProcessReference,
        preset: Option<String>,
    ) -> (Self, Vec<ProcessUiCommand>) {
        Self::new_inner(path, key, ProcessUiCommand::Create {
            reference,
            preset,
        })
    }

    fn new_inner(path: PathBuf, key: ProcessKey, initial_command: ProcessUiCommand) -> (Self, Vec<ProcessUiCommand>) {
        debug!("Creating process instance. path: {:?}", path);

        let component: ComponentState<(ProcessKey, ProcessUiCommand)> = ComponentState::default();

        let mut editor = ProcessEditorUi::new();
        editor
            .component
            .configure_mapper(component.sender.clone(), move |command| {
                (key, ProcessUiCommand::EditorCommand(command))
            });

        let instance = Self {
            planner_core_service: PlannerCoreService::new(),
            path,
            modified: false,
            modified_generation: 0,
            definition: None,
            error: None,
            errors: Errors::default(),
            editor,
            component,
        };

        (instance, vec![initial_command])
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    fn request_view(&mut self, key: ProcessKey) -> Option<ProcessAction> {
        let event = Event::RequestProcessView {
            path: self.path.clone(),
        };
        when_ok(self.planner_core_service.query(event), key, None)
    }
}

pub struct ProcessContext {
    pub key: ProcessKey,
}

impl UiComponent for Process {
    type UiContext<'context> = ProcessContext;
    type UiCommand = (ProcessKey, ProcessUiCommand);
    type UiAction = ProcessAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let key = context.key;
        let send_errors = |command: ErrorsUiCommand| {
            self.component
                .send((key, ProcessUiCommand::Errors(command)))
        };
        egui::Frame::new()
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.label(tr!("process-detail-path", { path: self.path.display().to_string() }));

                // in the tab's error frame, which the user can dismiss (`FORM-12`, `FORM-13`).
                if let Some(error) = &self.error {
                    self.errors
                        .show(ui, "process", error, &send_errors);
                }

                ui.separator();

                self.editor
                    .ui(ui, &mut ProcessEditorUiContext {
                        errors: Some((&self.errors, &send_errors)),
                    });
            });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("Process command. command: {:?}", command);
        let (key, command) = command;
        if command.is_users() {
            self.errors.clear();
        }

        match command {
            ProcessUiCommand::None => None,
            ProcessUiCommand::Create {
                reference,
                preset,
            } => {
                let event = Event::CreateProcess {
                    path: self.path.clone(),
                    reference,
                    preset,
                };
                when_ok(
                    self.planner_core_service.update(event),
                    key,
                    Some(ProcessUiCommand::RequestView),
                )
            }
            ProcessUiCommand::Load => {
                let event = Event::LoadProcess {
                    path: self.path.clone(),
                };
                when_ok(
                    self.planner_core_service.update(event),
                    key,
                    Some(ProcessUiCommand::RequestView),
                )
            }
            ProcessUiCommand::Save => {
                let event = Event::SaveProcess {
                    path: self.path.clone(),
                };
                when_ok(
                    self.planner_core_service.update(event),
                    key,
                    Some(ProcessUiCommand::Saved),
                )
            }
            ProcessUiCommand::Saved => None,
            ProcessUiCommand::Errors(command) => {
                self.errors.update(command);
                None
            }
            ProcessUiCommand::Error(error) => {
                let (PlannerError::CoreError((_, message)) | PlannerError::Other((_, message))) = error;
                self.error = Some(message);
                None
            }
            ProcessUiCommand::RequestView => self.request_view(key),
            ProcessUiCommand::ProcessView(view) => {
                match view {
                    ProcessView::Definition {
                        definition, ..
                    } => {
                        self.editor
                            .update_definition(definition.clone());
                        self.definition = Some(definition);
                    }
                }
                None
            }
            ProcessUiCommand::SetModifiedState {
                processes_modified,
                generation,
            } => {
                if generation < self.modified_generation {
                    debug!(
                        "ignoring an older modified state. generation: {}, applied: {}",
                        generation, self.modified_generation
                    );
                    return None;
                }
                self.modified_generation = generation;
                // each process document has a core of its own, so this is whether *this* process is modified.
                self.modified = processes_modified;
                Some(ProcessAction::SetModifiedState(processes_modified))
            }
            ProcessUiCommand::EditorCommand(command) => {
                match self
                    .editor
                    .update(command, &mut ProcessEditorUiContext::default())
                {
                    None | Some(ProcessEditorUiAction::None) => None,
                    Some(ProcessEditorUiAction::Reset) => {
                        if let Some(definition) = &self.definition {
                            self.editor
                                .reset_definition(definition.clone());
                        }
                        None
                    }
                    Some(ProcessEditorUiAction::Apply(process_definition)) => {
                        self.error = None;
                        let event = Event::ApplyProcessDefinition {
                            path: self.path.clone(),
                            process_definition,
                        };
                        when_ok(
                            self.planner_core_service.update(event),
                            key,
                            Some(ProcessUiCommand::RequestView),
                        )
                    }
                }
            }
        }
    }
}

/// The actions for the core's effects, followed by the command, if any, or the action for the core's error.
#[must_use]
fn when_ok(
    result: Result<Vec<PlannerAction>, PlannerError>,
    key: ProcessKey,
    command: Option<ProcessUiCommand>,
) -> Option<ProcessAction> {
    match result {
        Ok(actions) => {
            let mut tasks: Vec<Task<ProcessAction>> = actions
                .into_iter()
                .map(|action| Task::done(ProcessAction::UiCommand(into_command(action))))
                .collect();

            if let Some(command) = command {
                tasks.push(Task::done(ProcessAction::UiCommand(command)));
            }

            Some(ProcessAction::Task(key, Task::batch(tasks)))
        }
        Err(error) => Some(ProcessAction::UiCommand(ProcessUiCommand::Error(error))),
    }
}

fn into_command(action: PlannerAction) -> ProcessUiCommand {
    match action {
        PlannerAction::SetModifiedState {
            processes_modified,
            generation,
            ..
        } => ProcessUiCommand::SetModifiedState {
            processes_modified,
            generation,
        },
        PlannerAction::ProcessView(view) => ProcessUiCommand::ProcessView(view),
        PlannerAction::ProjectView(_) | PlannerAction::PcbView(_) => {
            // a process document's core has no project or PCBs.
            ProcessUiCommand::None
        }
    }
}
