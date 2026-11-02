use std::collections::HashMap;

use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::tr;
use planner_app::{Part, ProcessReference};
use tracing::trace;

use crate::forms::NamedResponse;
use crate::project::data::{ProjectData, Synced};
use crate::project::tables::parts::{PartTableUi, PartTableUiAction, PartTableUiCommand, PartTableUiContext};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PartsTabUi {
    #[derivative(Debug = "ignore")]
    part_table_ui: PartTableUi,

    selection: Option<Vec<Part>>,

    selected_process: Option<ProcessReference>,

    /// the revisions of the parts and overview views used to build the table
    synced: Synced<(u64, u64)>,

    pub component: ComponentState<PartsTabUiCommand>,
}

impl PartsTabUi {
    pub fn new() -> Self {
        let component: ComponentState<PartsTabUiCommand> = Default::default();

        let mut part_table_ui = PartTableUi::new(Vec::default());
        part_table_ui
            .component
            .configure_mapper(component.sender.clone(), |part_table_command| {
                trace!("part table mapper. command: {:?}", part_table_command);
                PartsTabUiCommand::PartTableUiCommand(part_table_command)
            });

        Self {
            part_table_ui,

            selection: None,
            selected_process: None,
            synced: Synced::default(),

            component,
        }
    }

    /// Rebuilds the table's rows if the views they are built from have changed.
    pub fn sync(&mut self, data: &ProjectData) {
        let Some(part_states) = data.parts.get() else {
            return;
        };

        if self
            .synced
            .changed((data.parts.revision(), data.overview.revision()))
        {
            self.part_table_ui
                .update_processes(data.processes().to_vec());
            self.part_table_ui
                .update_parts(part_states.clone());
        }
    }
}

#[derive(Debug, Clone)]
pub enum PartsTabUiCommand {
    None,

    // internal
    PartTableUiCommand(PartTableUiCommand),
    PartsActionClicked(PartsAction),
    ProcessChanged(ProcessReference),
}

#[derive(Debug, Clone)]
pub enum PartsAction {
    AddProcess,
    RemoveProcess,
}

#[derive(Debug, Clone)]
pub enum PartsTabUiAction {
    None,
    UpdateProcessesForPart {
        part: Part,
        processes: HashMap<ProcessReference, bool>,
    },
    RequestRepaint,
    ApplyPartsAction(Vec<Part>, PartsTabUiApplyAction),
}

#[derive(Debug, Clone)]
pub enum PartsTabUiApplyAction {
    AddProcess(ProcessReference),
    RemoveProcess(ProcessReference),
}

#[derive(Debug, Clone)]
pub struct PartsTabUiContext<'context> {
    pub data: &'context ProjectData,
}

impl UiComponent for PartsTabUi {
    type UiContext<'context> = PartsTabUiContext<'context>;
    type UiCommand = PartsTabUiCommand;
    type UiAction = PartsTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        ui.horizontal(|ui| {
            self.part_table_ui.filter_ui(ui);

            ui.separator();

            egui::ComboBox::from_id_salt(ui.id().with("process_selection"))
                .selected_text(match &self.selected_process {
                    Some(process) => format!("{}", process),
                    None => tr!("form-common-choice-process"),
                })
                .show_ui(ui, |ui| {
                    for process in context.data.processes() {
                        if ui
                            .add(egui::Button::selectable(
                                matches!(&self.selected_process, Some(selected_process) if selected_process == process),
                                process.to_string(),
                            ))
                            .clicked()
                        {
                            self.component
                                .sender
                                .send(PartsTabUiCommand::ProcessChanged(process.clone()))
                                .expect("sent");
                        }
                    }
                })
                .response
                .named(tr!("form-common-choice-process"));

            let have_selection = self.selection.is_some();
            let have_process = self.selected_process.is_some();

            ui.add_enabled_ui(have_selection && have_process, |ui| {
                egui::ComboBox::from_id_salt(ui.id().with("process_action"))
                    .selected_text(tr!("common-actions"))
                    .show_ui(ui, |ui| {
                        if ui
                            .add(egui::Button::selectable(false, tr!("form-common-button-add")))
                            .clicked()
                        {
                            self.component
                                .sender
                                .send(PartsTabUiCommand::PartsActionClicked(PartsAction::AddProcess))
                                .expect("sent");
                        }
                        if ui
                            .add(egui::Button::selectable(false, tr!("form-common-button-remove")))
                            .clicked()
                        {
                            self.component
                                .sender
                                .send(PartsTabUiCommand::PartsActionClicked(PartsAction::RemoveProcess))
                                .expect("sent");
                        }
                    })
                    .response
                    .named(tr!("common-actions"));
            });
        });

        ui.separator();

        self.part_table_ui
            .ui(ui, &mut PartTableUiContext::default());
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PartsTabUiCommand::None => Some(PartsTabUiAction::None),
            PartsTabUiCommand::ProcessChanged(process) => {
                self.selected_process = Some(process);
                None
            }
            PartsTabUiCommand::PartTableUiCommand(command) => self
                .part_table_ui
                .update(command, &mut PartTableUiContext::default())
                .map(|action| match action {
                    PartTableUiAction::None => PartsTabUiAction::None,
                    PartTableUiAction::RequestRepaint => PartsTabUiAction::RequestRepaint,
                    PartTableUiAction::ItemUpdated {
                        index,
                        item,
                        original_item,
                    } => {
                        let _ = index;

                        // the table has changed its own rows, the core may reject the change.
                        self.synced.invalidate();

                        // the only thing that can change in the item, is the processes, otherwise we would need to look
                        // at the item and original item to determine what changed and handle accordingly.

                        let iter = context
                            .data
                            .processes()
                            .iter()
                            .map(|process| (process.clone(), item.processes.contains(process)));

                        let processes: HashMap<ProcessReference, bool> = HashMap::from_iter(iter);

                        PartsTabUiAction::UpdateProcessesForPart {
                            part: original_item.part,
                            processes,
                        }
                    }
                    PartTableUiAction::ApplySelection(selection) => {
                        self.selection = Some(selection);

                        PartsTabUiAction::None
                    }
                }),
            PartsTabUiCommand::PartsActionClicked(action) => {
                if let (Some(selection), Some(process)) = (&self.selection, &self.selected_process) {
                    let apply_action = match action {
                        PartsAction::AddProcess => PartsTabUiApplyAction::AddProcess(process.clone()),
                        PartsAction::RemoveProcess => PartsTabUiApplyAction::RemoveProcess(process.clone()),
                    };
                    Some(PartsTabUiAction::ApplyPartsAction(selection.clone(), apply_action))
                } else {
                    None
                }
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct PartsTab {}

impl Tab for PartsTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-parts-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.parts_tab_ui, ui, &mut PartsTabUiContext {
            data: &state.data,
        });
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
