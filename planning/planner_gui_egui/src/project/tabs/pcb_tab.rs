use std::path::PathBuf;

use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_extras::Column;
use egui_i18n::tr;
use tracing::debug;

use crate::project::data::ProjectData;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PcbTabUi {
    project_path: PathBuf,
    pcb_index: u16,

    pub component: ComponentState<PcbTabUiCommand>,
}

impl PcbTabUi {
    pub fn new(path: PathBuf, pcb_index: u16) -> Self {
        Self {
            project_path: path,
            pcb_index,
            component: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PcbTabUiCommand {
    None,
    CreateUnitAssignmentClicked,
    ShowPcbClicked,
}

#[derive(Debug, Clone)]
pub enum PcbTabUiAction {
    None,
    ShowUnitAssignments(u16),
    ShowPcb(PathBuf),
}

#[derive(Debug, Clone)]
pub struct PcbTabUiContext<'context> {
    pub data: &'context ProjectData,
}

impl UiComponent for PcbTabUi {
    type UiContext<'context> = PcbTabUiContext<'context>;
    type UiCommand = PcbTabUiCommand;
    type UiAction = PcbTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let (Some(project_pcb_overview), Some(pcb_overview)) = (
            context.data.project_pcb(self.pcb_index),
            context
                .data
                .pcb_overview(self.pcb_index),
        ) else {
            ui.spinner();
            return;
        };

        //
        // toolbar
        //

        ui.horizontal(|ui| {
            if ui
                .button(tr!("project-pcb-toolbar-button-show-pcb"))
                .clicked()
            {
                self.component
                    .send(PcbTabUiCommand::ShowPcbClicked)
            }

            if ui
                .button(tr!("project-pcb-toolbar-button-create-unit-assignment"))
                .clicked()
            {
                self.component
                    .send(PcbTabUiCommand::CreateUnitAssignmentClicked)
            }
        });

        ui.separator();

        //
        // overview
        //
        ui.label(
            &project_pcb_overview
                .pcb_file
                .to_string(),
        );
        ui.label(&pcb_overview.name.to_string());

        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        ui.separator();

        //
        // designs table
        //
        ui.heading(tr!("project-pcb-designs-header"));

        // TODO put this in a resizable container, minimum height, full width.
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::remainder())
            .header(text_height, |mut header| {
                header.col(|ui| {
                    ui.strong(tr!("table-designs-column-index"));
                });
                header.col(|ui| {
                    ui.strong(tr!("table-designs-column-name"));
                });
            })
            .body(|mut body| {
                for (index, design) in pcb_overview.designs.iter().enumerate() {
                    body.row(text_height, |mut row| {
                        row.col(|ui| {
                            ui.label(index.to_string());
                        });

                        row.col(|ui| {
                            ui.label(design.to_string());
                        });
                    })
                }
            });

        ui.separator();
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("PcbTabUi command. command: {:?}", command);
        match command {
            PcbTabUiCommand::None => Some(PcbTabUiAction::None),
            PcbTabUiCommand::CreateUnitAssignmentClicked => Some(PcbTabUiAction::ShowUnitAssignments(self.pcb_index)),
            PcbTabUiCommand::ShowPcbClicked => context
                .data
                .project_pcb(self.pcb_index)
                .map(|project_pcb_overview| PcbTabUiAction::ShowPcb(project_pcb_overview.pcb_path.clone())),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct PcbTab {
    pub pcb_index: u16,
}

impl PcbTab {
    pub fn new(pcb_index: u16) -> Self {
        Self {
            pcb_index,
        }
    }
}

impl Tab for PcbTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let pcb = format!("{}", self.pcb_index).to_string();
        egui::widget_text::WidgetText::from(tr!("project-pcb-tab-label", {pcb: pcb}))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(pcb_ui) = state
            .pcb_tab_uis
            .get(&(self.pcb_index as usize))
        else {
            ui.spinner();
            return;
        };

        UiComponent::ui(pcb_ui, ui, &mut PcbTabUiContext {
            data: &state.data,
        });
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> OnCloseResponse {
        let mut state = context.state.lock().unwrap();
        if let Some(_pcb_ui) = state
            .pcb_tab_uis
            .remove(&(self.pcb_index as usize))
        {
            debug!("removed orphaned pcb ui. pcb_index: {}", self.pcb_index);
        }
        OnCloseResponse::Close
    }
}
