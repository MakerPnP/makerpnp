use std::path::PathBuf;

use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use tracing::debug;

use crate::pcb::tabs::PcbTabs;
use crate::pcb::{Pcb, PcbAction, PcbContext, PcbKey, PcbUiCommand};
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};

/// This is persisted between application restarts
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct PcbTab {
    pub pcb_key: PcbKey,

    // path is required here so the pcb can be loaded when the application restarts
    pub path: PathBuf,
    pub label: String,

    #[serde(skip)]
    pub modified: bool,

    #[serde(skip)]
    pub component: ComponentState<PcbTabUiCommand>,

    pub pcb_tabs: Value<PcbTabs>,
}

#[derive(Debug, Clone)]
pub enum PcbTabUiCommand {
    PcbCommand { key: PcbKey, command: PcbUiCommand },
}

#[derive(Debug)]
pub enum PcbTabAction {
    PcbTask(PcbKey, Task<PcbAction>),
    SetModifiedState(bool),
    RequestRepaint,
}

pub struct PcbTabContext {
    pub tab_key: TabKey,
    pub pcbs: Value<SlotMap<PcbKey, Pcb>>,
}

impl PcbTab {
    pub fn new(label: String, path: PathBuf, pcb_key: PcbKey, pcb_tabs: Value<PcbTabs>) -> Self {
        debug!("Creating pcb tab. key: {:?}, path: {:?}", &pcb_key, &path);
        Self {
            pcb_key,
            pcb_tabs,
            path,
            label,
            modified: false,
            component: ComponentState::default(),
        }
    }
}

impl Tab for PcbTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let mut label = egui::RichText::new(self.label.clone());

        if self.modified {
            label = label.italics();
        }

        egui::widget_text::WidgetText::from(label)
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, tab_context: &mut Self::Context) {
        let mut pcb_tab_context = PcbTabContext {
            tab_key: tab_key.clone(),
            pcbs: tab_context.pcbs.clone(),
        };

        UiComponent::ui(self, ui, &mut pcb_tab_context);
    }

    fn on_close(&mut self, _tab_key: &TabKey, _tab_context: &mut Self::Context) -> OnCloseResponse {
        debug!("closing pcb. key: {:?}", self.pcb_key);
        let mut pcbs = _tab_context.pcbs.lock().unwrap();
        pcbs.remove(self.pcb_key);

        OnCloseResponse::Close
    }
}

impl UiComponent for PcbTab {
    type UiContext<'context> = PcbTabContext;
    type UiCommand = PcbTabUiCommand;
    type UiAction = PcbTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let pcbs = context.pcbs.lock().unwrap();
        let pcb = pcbs.get(self.pcb_key).unwrap();

        let mut pcb_context = PcbContext {
            key: self.pcb_key,
        };

        pcb.ui(ui, &mut pcb_context);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PcbTabUiCommand::PcbCommand {
                key,
                command,
            } => {
                let mut pcbs = context.pcbs.lock().unwrap();
                let pcb = pcbs.get_mut(self.pcb_key).unwrap();

                let mut pcb_context = PcbContext {
                    key: self.pcb_key,
                };

                let action: Option<PcbAction> = pcb.update((key, command), &mut pcb_context);
                match action {
                    Some(PcbAction::Task(key, task)) => Some(PcbTabAction::PcbTask(key, task)),
                    Some(PcbAction::SetModifiedState(modified_state)) => {
                        Some(PcbTabAction::SetModifiedState(modified_state))
                    }
                    None => None,
                    Some(PcbAction::UiCommand(command)) => pcb
                        .update((key, command), &mut pcb_context)
                        .map(|action| PcbTabAction::PcbTask(key, Task::done(action))),
                    Some(PcbAction::RequestRepaint) => Some(PcbTabAction::RequestRepaint),
                }
            }
        }
    }
}
