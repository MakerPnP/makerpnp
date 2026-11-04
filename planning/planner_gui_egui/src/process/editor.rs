//! The editor of a process document's definition: its reference, operations, their tasks, and its rules.

use std::str::FromStr;

use derivative::Derivative;
use egui::{Frame, TextEdit, Ui, Widget};
use egui_i18n::tr;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Size, Style};
use egui_taffy::tui;
use planner_app::{
    OperationDefinition, OperationReference, PROCESS_RULES, ProcessDefinition, ProcessReference, ProcessRuleReference,
    TaskDefinition, TaskKind, TaskReference,
};
use tracing::debug;
use validator::Validate;

use crate::forms::{Form, NamedResponse};
use crate::sync::Value;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ProcessEditorUi {
    state: Option<ProcessEditorUiState>,

    pub component: ComponentState<ProcessEditorUiCommand>,
}

#[derive(Debug)]
struct ProcessEditorUiState {
    fields: Value<ProcessFields>,
    new_operation_reference: String,
    /// the definition the form was built from, the form has unsaved edits when it differs from this.
    initial: ProcessDefinition,
}

impl ProcessEditorUi {
    pub fn new() -> Self {
        Self {
            state: None,
            component: ComponentState::default(),
        }
    }

    /// Rebuilds the form from the definition, unless that would discard the user's unsaved edits.
    pub fn update_definition(&mut self, definition: ProcessDefinition) {
        if let Some(state) = &self.state {
            let fields = state.fields.lock().unwrap();
            if !fields.can_be_replaced_by(&state.initial, &definition) {
                debug!("keeping unsaved edits. process: {:?}", state.initial.reference);
                return;
            }
        }

        self.reset_definition(definition);
    }

    /// Discards any unsaved edits.
    pub fn reset_definition(&mut self, definition: ProcessDefinition) {
        self.state = Some(ProcessEditorUiState {
            fields: Value::new(ProcessFields::from_process_definition(&definition)),
            new_operation_reference: tr!("common-value-operation-reference-default"),
            initial: definition,
        });
    }

    fn show_reference(&self, ui: &mut Ui, form: &Form<ProcessFields, ProcessEditorUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("process_form"))
            .reserve_available_width()
            .style(Style {
                align_items: Some(AlignItems::Center),
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: percent(1.),
                    height: auto(),
                },
                padding: length(8.),
                gap: length(8.),
                ..default_style()
            })
            .show(|tui| {
                form.show_fields_vertical(tui, |form, tui| {
                    form.add_field_ui("process_reference", tr!("form-common-input-process-reference"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let mut reference_clone = fields.process_reference.clone();
                            let output = TextEdit::singleline(&mut reference_clone)
                                .desired_width(ui.available_width().min(200.0))
                                .show(ui);

                            if !fields
                                .process_reference
                                .eq(&reference_clone)
                            {
                                sender
                                    .send(ProcessEditorUiCommand::ReferenceChanged(reference_clone))
                                    .expect("sent")
                            }

                            output.response.response
                        }
                    });
                });
            });
    }

    fn show_operations(&self, ui: &mut Ui, state: &ProcessEditorUiState) {
        let fields = state.fields.lock().unwrap();
        let operation_count = fields.operations.len();

        ui.strong(tr!("form-process-operations"));

        for (operation_index, operation) in fields.operations.iter().enumerate() {
            // built with `format!`, as Fluent would isolate the number with marks that assistive technology would read.
            let operation_name = format!("{} {}", tr!("process-editor-operation"), operation_index + 1);

            Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut reference = operation.reference.clone();
                    if TextEdit::singleline(&mut reference)
                        .desired_width(200.0)
                        .ui(ui)
                        .named(operation_name.clone())
                        .changed()
                    {
                        self.component
                            .send(ProcessEditorUiCommand::OperationReferenceChanged {
                                operation: operation_index,
                                reference,
                            });
                    }

                    let button = |ui: &mut Ui, enabled: bool, text: String, action: String| {
                        ui.add_enabled(enabled, egui::Button::new(text))
                            .named(format!("{operation_name}: {action}"))
                            .clicked()
                    };

                    if button(
                        ui,
                        operation_index > 0,
                        "⏶".to_string(),
                        tr!("process-editor-button-move-up"),
                    ) {
                        self.component
                            .send(ProcessEditorUiCommand::MoveOperation {
                                operation: operation_index,
                                up: true,
                            });
                    }
                    if button(
                        ui,
                        operation_index + 1 < operation_count,
                        "⏷".to_string(),
                        tr!("process-editor-button-move-down"),
                    ) {
                        self.component
                            .send(ProcessEditorUiCommand::MoveOperation {
                                operation: operation_index,
                                up: false,
                            });
                    }
                    if button(
                        ui,
                        true,
                        format!("🗑 {}", tr!("form-common-button-delete")),
                        tr!("form-common-button-delete"),
                    ) {
                        self.component
                            .send(ProcessEditorUiCommand::RemoveOperation {
                                operation: operation_index,
                            });
                    }
                });

                ui.indent(ui.id().with(("tasks", operation_index)), |ui| {
                    for (task_index, task) in operation.tasks.iter().enumerate() {
                        let task_name = format!("{operation_name}, {} {}", tr!("process-editor-task"), task_index + 1);

                        ui.horizontal(|ui| {
                            let mut reference = task.reference.clone();
                            if TextEdit::singleline(&mut reference)
                                .desired_width(200.0)
                                .ui(ui)
                                .named(task_name.clone())
                                .changed()
                            {
                                self.component
                                    .send(ProcessEditorUiCommand::TaskReferenceChanged {
                                        operation: operation_index,
                                        task: task_index,
                                        reference,
                                    });
                            }

                            if let Some(kind) = task_kind_combo_box(
                                ui,
                                ui.id()
                                    .with(("task_kind", operation_index, task_index)),
                                task.kind,
                                format!("{task_name}: {}", tr!("process-editor-input-task-kind")),
                            ) {
                                self.component
                                    .send(ProcessEditorUiCommand::TaskKindChanged {
                                        operation: operation_index,
                                        task: task_index,
                                        kind,
                                    });
                            }

                            if ui
                                .button(format!("🗑 {}", tr!("form-common-button-delete")))
                                .named(format!("{task_name}: {}", tr!("form-common-button-delete")))
                                .clicked()
                            {
                                self.component
                                    .send(ProcessEditorUiCommand::RemoveTask {
                                        operation: operation_index,
                                        task: task_index,
                                    });
                            }
                        });
                    }

                    // a new task
                    ui.horizontal(|ui| {
                        let new_task_name = format!("{operation_name}: {}", tr!("process-editor-input-new-task"));

                        let mut reference = operation.new_task.reference.clone();
                        if TextEdit::singleline(&mut reference)
                            .desired_width(200.0)
                            .hint_text(tr!("process-editor-input-new-task-placeholder"))
                            .ui(ui)
                            .named(new_task_name.clone())
                            .changed()
                        {
                            self.component
                                .send(ProcessEditorUiCommand::NewTaskReferenceChanged {
                                    operation: operation_index,
                                    reference,
                                });
                        }

                        if let Some(kind) = task_kind_combo_box(
                            ui,
                            ui.id()
                                .with(("new_task_kind", operation_index)),
                            operation.new_task.kind,
                            format!("{new_task_name}: {}", tr!("process-editor-input-task-kind")),
                        ) {
                            self.component
                                .send(ProcessEditorUiCommand::NewTaskKindChanged {
                                    operation: operation_index,
                                    kind,
                                });
                        }

                        let is_valid = TaskReference::from_str(&operation.new_task.reference).is_ok();
                        if ui
                            .add_enabled(is_valid, egui::Button::new(tr!("process-editor-button-add-task")))
                            .named(format!("{operation_name}: {}", tr!("process-editor-button-add-task")))
                            .clicked()
                        {
                            self.component
                                .send(ProcessEditorUiCommand::AddTaskClicked {
                                    operation: operation_index,
                                });
                        }
                    });
                });
            });
        }

        // a new operation
        ui.horizontal(|ui| {
            let mut reference = state.new_operation_reference.clone();
            if TextEdit::singleline(&mut reference)
                .desired_width(200.0)
                .ui(ui)
                .named(tr!("process-editor-input-new-operation"))
                .changed()
            {
                self.component
                    .send(ProcessEditorUiCommand::NewOperationReferenceChanged(reference));
            }

            let is_valid = OperationReference::from_str(&state.new_operation_reference).is_ok();
            if ui
                .add_enabled(is_valid, egui::Button::new(tr!("process-editor-button-add-operation")))
                .clicked()
            {
                self.component
                    .send(ProcessEditorUiCommand::AddOperationClicked);
            }
        });
    }

    fn show_rules(&self, ui: &mut Ui, state: &ProcessEditorUiState) {
        let fields = state.fields.lock().unwrap();

        ui.strong(tr!("process-editor-rules"));
        for rule in PROCESS_RULES {
            let rule = ProcessRuleReference::from_raw_str(rule);
            let mut enabled = fields.rules.contains(&rule);
            if ui
                .checkbox(&mut enabled, rule.to_string())
                .changed()
            {
                self.component
                    .send(ProcessEditorUiCommand::RuleChanged {
                        rule,
                        enabled,
                    });
            }
        }
    }
}

/// A combo box for choosing a task's kind, named for assistive technology and tests.  Returns the kind chosen, if any.
fn task_kind_combo_box(ui: &mut Ui, id: egui::Id, kind: TaskKind, name: String) -> Option<TaskKind> {
    let mut chosen = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(task_kind_name(kind))
        .show_ui(ui, |ui| {
            for candidate in TaskKind::ALL {
                if ui
                    .add(egui::Button::selectable(candidate == kind, task_kind_name(candidate)))
                    .clicked()
                {
                    chosen = Some(candidate);
                }
            }
        })
        .response
        .named(name);

    chosen.filter(|chosen| *chosen != kind)
}

pub fn task_kind_name(kind: TaskKind) -> String {
    match kind {
        TaskKind::Generic => tr!("process-task-kind-generic"),
        TaskKind::LoadPcbs => tr!("process-task-kind-load-pcbs"),
        TaskKind::PlaceComponents => tr!("process-task-kind-place-components"),
    }
}

#[derive(Clone, Debug, Default, Validate, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ProcessFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    #[validate(custom(function = "crate::forms::validation::CommonValidation::validate_reference"))]
    process_reference: String,

    operations: Vec<OperationFields>,

    rules: Vec<ProcessRuleReference>,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct OperationFields {
    reference: String,
    tasks: Vec<TaskFields>,
    /// the task to add to the operation, not part of the definition until it is added.
    new_task: TaskFields,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct TaskFields {
    reference: String,
    kind: TaskKind,
}

impl Default for TaskFields {
    fn default() -> Self {
        Self {
            reference: String::new(),
            kind: TaskKind::Generic,
        }
    }
}

impl ProcessFields {
    pub fn from_process_definition(process: &ProcessDefinition) -> Self {
        Self {
            process_reference: process.reference.to_string(),
            operations: process
                .operations
                .iter()
                .map(|operation| OperationFields {
                    reference: operation.reference.to_string(),
                    tasks: operation
                        .tasks
                        .iter()
                        .map(|task| TaskFields {
                            reference: task.reference.to_string(),
                            kind: task.kind,
                        })
                        .collect(),
                    new_task: TaskFields::default(),
                })
                .collect(),
            rules: process.rules.clone(),
        }
    }

    /// The definition the fields describe, or why they don't describe a valid one.
    pub fn build_definition(&self) -> Result<ProcessDefinition, String> {
        let reference = ProcessReference::from_str(&self.process_reference)
            .map_err(|_| tr!("form-input-error-process-reference-invalid"))?;

        let operations = self
            .operations
            .iter()
            .enumerate()
            .map(|(operation_index, operation)| {
                let reference = OperationReference::from_str(&operation.reference).map_err(|_| {
                    tr!("process-editor-error-operation-reference-invalid", { operation: operation_index + 1 })
                })?;

                let tasks = operation
                    .tasks
                    .iter()
                    .enumerate()
                    .map(|(task_index, task)| {
                        TaskReference::from_str(&task.reference)
                            .map(|reference| TaskDefinition::new(reference, task.kind))
                            .map_err(|_| {
                                tr!("process-editor-error-task-reference-invalid", { operation: operation_index + 1, number: task_index + 1 })
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(OperationDefinition {
                    reference,
                    tasks,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let definition = ProcessDefinition {
            reference,
            operations,
            rules: self.rules.clone(),
        };

        definition
            .validate()
            .map_err(|error| error.to_string())?;

        Ok(definition)
    }

    fn matches(&self, process: &ProcessDefinition) -> bool {
        self.build_definition()
            .is_ok_and(|definition| definition.eq(process))
    }

    /// The form can be replaced if there are no unsaved edits, or if they are what the new definition now contains,
    /// which is the case after the user applies their edits.
    fn can_be_replaced_by(&self, initial: &ProcessDefinition, new: &ProcessDefinition) -> bool {
        self.matches(initial) || self.matches(new)
    }
}

#[derive(Debug, Clone)]
pub enum ProcessEditorUiCommand {
    None,
    ReferenceChanged(String),
    OperationReferenceChanged {
        operation: usize,
        reference: String,
    },
    MoveOperation {
        operation: usize,
        up: bool,
    },
    RemoveOperation {
        operation: usize,
    },
    NewOperationReferenceChanged(String),
    AddOperationClicked,
    TaskReferenceChanged {
        operation: usize,
        task: usize,
        reference: String,
    },
    TaskKindChanged {
        operation: usize,
        task: usize,
        kind: TaskKind,
    },
    RemoveTask {
        operation: usize,
        task: usize,
    },
    NewTaskReferenceChanged {
        operation: usize,
        reference: String,
    },
    NewTaskKindChanged {
        operation: usize,
        kind: TaskKind,
    },
    AddTaskClicked {
        operation: usize,
    },
    RuleChanged {
        rule: ProcessRuleReference,
        enabled: bool,
    },
    ApplyClicked,
    ResetClicked,
}

#[derive(Debug, Clone)]
pub enum ProcessEditorUiAction {
    None,
    Reset,
    Apply(ProcessDefinition),
}

#[derive(Debug, Clone, Default)]
pub struct ProcessEditorUiContext {}

impl UiComponent for ProcessEditorUi {
    type UiContext<'context> = ProcessEditorUiContext;
    type UiCommand = ProcessEditorUiCommand;
    type UiAction = ProcessEditorUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let Some(state) = &self.state else {
            ui.spinner();
            return;
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            let form = Form::new(&state.fields, &self.component.sender, ());
            self.show_reference(ui, &form);

            self.show_operations(ui, state);

            ui.separator();

            self.show_rules(ui, state);

            ui.separator();

            let definition = state
                .fields
                .lock()
                .unwrap()
                .build_definition();

            if let Err(error) = &definition {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }

            let is_changed = definition
                .as_ref()
                .map_or(true, |definition| !definition.eq(&state.initial));

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(is_changed, egui::Button::new(tr!("form-common-button-reset")))
                    .clicked()
                {
                    self.component
                        .send(ProcessEditorUiCommand::ResetClicked);
                }

                if ui
                    .add_enabled(
                        is_changed && definition.is_ok(),
                        egui::Button::new(tr!("form-common-button-apply")),
                    )
                    .clicked()
                {
                    self.component
                        .send(ProcessEditorUiCommand::ApplyClicked);
                }
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
        debug!("ProcessEditorUi command. command: {:?}", command);
        let Some(state) = &mut self.state else {
            return match command {
                ProcessEditorUiCommand::None => Some(ProcessEditorUiAction::None),
                // there are no other commands that can be processed without the state
                _ => None,
            };
        };

        let mut fields = state.fields.lock().unwrap();
        match command {
            ProcessEditorUiCommand::None => Some(ProcessEditorUiAction::None),
            ProcessEditorUiCommand::ReferenceChanged(reference) => {
                fields.process_reference = reference;
                None
            }
            ProcessEditorUiCommand::OperationReferenceChanged {
                operation,
                reference,
            } => {
                if let Some(operation) = fields.operations.get_mut(operation) {
                    operation.reference = reference;
                }
                None
            }
            ProcessEditorUiCommand::MoveOperation {
                operation,
                up,
            } => {
                let other = match up {
                    true => operation.checked_sub(1),
                    false => Some(operation + 1),
                };
                if let Some(other) = other.filter(|other| *other < fields.operations.len()) {
                    fields.operations.swap(operation, other);
                }
                None
            }
            ProcessEditorUiCommand::RemoveOperation {
                operation,
            } => {
                if operation < fields.operations.len() {
                    fields.operations.remove(operation);
                }
                None
            }
            ProcessEditorUiCommand::NewOperationReferenceChanged(reference) => {
                state.new_operation_reference = reference;
                None
            }
            ProcessEditorUiCommand::AddOperationClicked => {
                fields.operations.push(OperationFields {
                    reference: state.new_operation_reference.clone(),
                    tasks: vec![],
                    new_task: TaskFields::default(),
                });
                None
            }
            ProcessEditorUiCommand::TaskReferenceChanged {
                operation,
                task,
                reference,
            } => {
                if let Some(task) = fields
                    .operations
                    .get_mut(operation)
                    .and_then(|operation| operation.tasks.get_mut(task))
                {
                    task.reference = reference;
                }
                None
            }
            ProcessEditorUiCommand::TaskKindChanged {
                operation,
                task,
                kind,
            } => {
                if let Some(task) = fields
                    .operations
                    .get_mut(operation)
                    .and_then(|operation| operation.tasks.get_mut(task))
                {
                    task.kind = kind;
                }
                None
            }
            ProcessEditorUiCommand::RemoveTask {
                operation,
                task,
            } => {
                if let Some(operation) = fields.operations.get_mut(operation) {
                    if task < operation.tasks.len() {
                        operation.tasks.remove(task);
                    }
                }
                None
            }
            ProcessEditorUiCommand::NewTaskReferenceChanged {
                operation,
                reference,
            } => {
                if let Some(operation) = fields.operations.get_mut(operation) {
                    operation.new_task.reference = reference;
                }
                None
            }
            ProcessEditorUiCommand::NewTaskKindChanged {
                operation,
                kind,
            } => {
                if let Some(operation) = fields.operations.get_mut(operation) {
                    operation.new_task.kind = kind;
                }
                None
            }
            ProcessEditorUiCommand::AddTaskClicked {
                operation,
            } => {
                if let Some(operation) = fields.operations.get_mut(operation) {
                    let new_task = std::mem::take(&mut operation.new_task);
                    operation.tasks.push(new_task);
                }
                None
            }
            ProcessEditorUiCommand::RuleChanged {
                rule,
                enabled,
            } => {
                fields
                    .rules
                    .retain(|candidate| !candidate.eq(&rule));
                if enabled {
                    fields.rules.push(rule);
                }
                None
            }
            ProcessEditorUiCommand::ApplyClicked => fields
                .build_definition()
                .ok()
                .map(ProcessEditorUiAction::Apply),
            ProcessEditorUiCommand::ResetClicked => Some(ProcessEditorUiAction::Reset),
        }
    }
}

#[cfg(test)]
mod tests {
    use planner_app::{
        OperationDefinition, OperationReference, ProcessDefinition, ProcessReference, TaskDefinition, TaskKind,
        TaskReference,
    };

    use super::ProcessFields;

    fn definition(tasks: &[(&str, TaskKind)]) -> ProcessDefinition {
        ProcessDefinition {
            reference: ProcessReference::from_raw_str("pnp"),
            operations: vec![OperationDefinition {
                reference: OperationReference::from_raw_str("place"),
                tasks: tasks
                    .iter()
                    .map(|(task, kind)| TaskDefinition::new(TaskReference::from_raw_str(task), *kind))
                    .collect(),
            }],
            rules: vec![],
        }
    }

    #[test]
    fn an_unedited_form_can_be_replaced() {
        let initial = definition(&[("load_pcbs", TaskKind::LoadPcbs)]);
        let fields = ProcessFields::from_process_definition(&initial);

        assert!(fields.can_be_replaced_by(
            &initial,
            &definition(&[("place_components", TaskKind::PlaceComponents)])
        ));
    }

    #[test]
    fn a_form_with_unsaved_edits_is_not_replaced() {
        let initial = definition(&[("load_pcbs", TaskKind::LoadPcbs)]);
        let fields = ProcessFields::from_process_definition(&definition(&[("solder", TaskKind::Generic)]));

        assert!(!fields.can_be_replaced_by(
            &initial,
            &definition(&[("place_components", TaskKind::PlaceComponents)])
        ));
    }

    #[test]
    fn a_form_whose_edits_were_applied_can_be_replaced() {
        let initial = definition(&[("load_pcbs", TaskKind::LoadPcbs)]);
        let applied = definition(&[("solder", TaskKind::Generic)]);
        let fields = ProcessFields::from_process_definition(&applied);

        assert!(fields.can_be_replaced_by(&initial, &applied));
    }

    #[test]
    fn a_form_with_two_place_components_tasks_is_invalid() {
        let fields = ProcessFields::from_process_definition(&definition(&[
            ("place_top", TaskKind::PlaceComponents),
            ("place_bottom", TaskKind::PlaceComponents),
        ]));

        assert!(fields.build_definition().is_err());
    }
}
