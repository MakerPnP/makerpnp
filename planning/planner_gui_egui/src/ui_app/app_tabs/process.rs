use std::path::PathBuf;

use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use tracing::debug;

use crate::process::{Process, ProcessAction, ProcessContext, ProcessKey, ProcessUiCommand};
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};

/// This is persisted between application restarts
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct ProcessTab {
    pub process_key: ProcessKey,

    // path is required here so the process can be loaded when the application restarts
    pub path: PathBuf,
    pub label: String,

    #[serde(skip)]
    pub modified: bool,

    #[serde(skip)]
    pub component: ComponentState<ProcessTabUiCommand>,
}

#[derive(Debug, Clone)]
pub enum ProcessTabUiCommand {
    ProcessCommand { key: ProcessKey, command: ProcessUiCommand },
}

#[derive(Debug)]
pub enum ProcessTabAction {
    ProcessTask(ProcessKey, Task<ProcessAction>),
    SetModifiedState(bool),
}

pub struct ProcessTabContext {
    pub tab_key: TabKey,
    pub processes: Value<SlotMap<ProcessKey, Process>>,
}

impl ProcessTab {
    pub fn new(label: String, path: PathBuf, process_key: ProcessKey) -> Self {
        debug!("Creating process tab. key: {:?}, path: {:?}", &process_key, &path);
        Self {
            process_key,
            path,
            label,
            modified: false,
            component: ComponentState::default(),
        }
    }
}

impl Tab for ProcessTab {
    type Context = ProcessTabContext;

    fn label(&self) -> WidgetText {
        let mut label = egui::RichText::new(self.label.clone());

        if self.modified {
            label = label.italics();
        }

        egui::widget_text::WidgetText::from(label)
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, tab_context: &mut Self::Context) {
        let mut process_tab_context = ProcessTabContext {
            tab_key: tab_key.clone(),
            processes: tab_context.processes.clone(),
        };

        UiComponent::ui(self, ui, &mut process_tab_context);
    }

    fn on_close(&mut self, _tab_key: &TabKey, tab_context: &mut Self::Context) -> OnCloseResponse {
        debug!("closing process. key: {:?}", self.process_key);
        let mut processes = tab_context.processes.lock().unwrap();
        processes.remove(self.process_key);

        OnCloseResponse::Close
    }
}

impl UiComponent for ProcessTab {
    type UiContext<'context> = ProcessTabContext;
    type UiCommand = ProcessTabUiCommand;
    type UiAction = ProcessTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let processes = context.processes.lock().unwrap();
        let Some(process) = processes.get(self.process_key) else {
            ui.spinner();
            return;
        };

        let mut process_context = ProcessContext {
            key: self.process_key,
        };

        process.ui(ui, &mut process_context);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("ProcessTab command. command: {:?}", command);
        match command {
            ProcessTabUiCommand::ProcessCommand {
                key,
                command,
            } => {
                let mut processes = context.processes.lock().unwrap();
                let process = processes.get_mut(self.process_key)?;

                let mut process_context = ProcessContext {
                    key: self.process_key,
                };

                match process.update((key, command), &mut process_context) {
                    Some(ProcessAction::Task(key, task)) => Some(ProcessTabAction::ProcessTask(key, task)),
                    Some(ProcessAction::SetModifiedState(modified_state)) => {
                        Some(ProcessTabAction::SetModifiedState(modified_state))
                    }
                    None => None,
                    Some(ProcessAction::UiCommand(command)) => process
                        .update((key, command), &mut process_context)
                        .map(|action| ProcessTabAction::ProcessTask(key, Task::done(action))),
                }
            }
        }
    }
}
