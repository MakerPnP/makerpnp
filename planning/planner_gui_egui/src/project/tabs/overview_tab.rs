use egui::{Resize, Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use planner_app::{PcbInstanceIndex, PhaseOverview, PhaseReference, ProjectOverview};

use crate::forms::NamedResize;
use crate::i18n::conversions::phase_status_to_i18n_key;
use crate::project::data::ProjectData;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct OverviewTabUi {
    pub component: ComponentState<OverviewTabUiCommand>,
}

impl OverviewTabUi {
    const TABLE_HEIGHT_MAX: f32 = 200.0;

    pub fn new() -> Self {
        Self {
            component: Default::default(),
        }
    }

    fn show_phases(&self, ui: &mut Ui, text_height: f32, phases: Option<&Vec<PhaseOverview>>) {
        egui::Sides::new().show(
            ui,
            |ui| {
                ui.heading(tr!("project-overview-phases-header"));
            },
            |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button(tr!("form-common-button-add"))
                        .clicked()
                    {
                        self.component
                            .send(OverviewTabUiCommand::AddPhaseClicked);
                    }
                });
            },
        );

        let available_size = ui.available_size();

        Resize::default()
            .resizable([false, true])
            .default_height(available_size.y / 2.0)
            .default_width(available_size.x)
            .min_width(available_size.x)
            .max_width(available_size.x)
            .max_height(Self::TABLE_HEIGHT_MAX)
            .show_named(ui, "project-overview-phases-header", |ui| {
                // HACK: search codebase for 'HACK: table-resize-hack' for details
                egui::Frame::new()
                    .outer_margin(4.0)
                    .show(ui, |ui| {
                        if let Some(phases) = phases {
                            TableBuilder::new(ui)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                .min_scrolled_height(80.0)
                                .column(Column::auto())
                                .column(Column::remainder())
                                .column(Column::auto())
                                .column(Column::auto().resizable(false))
                                .striped(true)
                                .resizable(true)
                                .auto_shrink([false, false])
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong(tr!("table-phases-column-index"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-phases-column-name"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-phases-column-status"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-phases-column-actions"));
                                    });
                                })
                                .body(|body| {
                                    body.rows(text_height, phases.len(), |mut row| {
                                        let index = row.index();
                                        if let Some(phase_overview) = phases.get(index) {
                                            row.col(|ui| {
                                                ui.label(index.to_string());
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    phase_overview
                                                        .phase_reference
                                                        .to_string(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(tr!(phase_status_to_i18n_key(&phase_overview.state.status())));
                                            });
                                            row.col(|ui| {
                                                let can_delete = phase_overview.state.is_pending();

                                                ui.add_enabled_ui(can_delete, |ui| {
                                                    if ui
                                                        .button(tr!("form-common-button-delete"))
                                                        .clicked()
                                                    {
                                                        self.component
                                                            .send(OverviewTabUiCommand::PhaseDeleteClicked(
                                                                phase_overview.phase_reference.clone(),
                                                            ));
                                                    }
                                                });
                                            });
                                        }
                                    });
                                });
                        }
                    });
            });
    }

    fn show_pcbs(&self, ui: &mut Ui, text_height: f32, overview: Option<&ProjectOverview>) {
        egui::Sides::new().show(
            ui,
            |ui| {
                ui.heading(tr!("project-overview-pcbs-header"));
            },
            |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button(tr!("form-common-button-add"))
                        .clicked()
                    {
                        self.component
                            .send(OverviewTabUiCommand::AddPcbClicked);
                    }
                });
            },
        );

        let available_size = ui.available_size();

        Resize::default()
            .resizable([false, true])
            .default_height(available_size.y / 2.0)
            .default_width(available_size.x)
            .min_width(available_size.x)
            .max_width(available_size.x)
            .max_height(Self::TABLE_HEIGHT_MAX)
            .show_named(ui, "project-overview-pcbs-header", |ui| {
                // HACK: search codebase for 'HACK: table-resize-hack' for details
                egui::Frame::new()
                    .outer_margin(4.0)
                    .show(ui, |ui| {
                        if let Some(overview) = overview {
                            TableBuilder::new(ui)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                .min_scrolled_height(80.0)
                                .column(Column::auto())
                                .column(Column::remainder())
                                .column(Column::auto().resizable(false))
                                .striped(true)
                                .resizable(true)
                                .auto_shrink([false, false])
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong(tr!("table-pcbs-column-index"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-pcbs-column-file"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-pcbs-column-actions"));
                                    });
                                })
                                .body(|body| {
                                    body.rows(text_height, overview.pcbs.len(), |mut row| {
                                        let index = row.index();

                                        if let Some(project_pcb) = overview.pcbs.get(index) {
                                            row.col(|ui| {
                                                ui.label(index.to_string());
                                            });
                                            row.col(|ui| {
                                                ui.label(project_pcb.pcb_file.to_string());
                                            });
                                            row.col(|ui| {
                                                let can_delete = project_pcb.unit_assignments.is_empty();

                                                ui.add_enabled_ui(can_delete, |ui| {
                                                    if ui
                                                        .button(tr!("form-common-button-remove"))
                                                        .clicked()
                                                    {
                                                        self.component
                                                            .send(OverviewTabUiCommand::PcbRemoveClicked(
                                                                index as PcbInstanceIndex,
                                                            ));
                                                    }
                                                })
                                                .response
                                                .on_disabled_hover_text(tr!(
                                                    "project-overview-pcbs-input-remove-disabled-hover-text-in-use-1"
                                                ));
                                            });
                                        }
                                    });
                                });
                        }
                    });
            });
    }
}

#[derive(Debug, Clone)]
pub enum OverviewTabUiCommand {
    None,
    AddPhaseClicked,
    PhaseDeleteClicked(PhaseReference),
    AddPcbClicked,
    PcbRemoveClicked(PcbInstanceIndex),
}

#[derive(Debug, Clone)]
pub enum OverviewTabUiAction {
    None,
    AddPhase,
    DeletePhase(PhaseReference),
    AddPcb,
    RemovePcb(PcbInstanceIndex),
}

#[derive(Debug, Clone)]
pub struct OverviewTabUiContext<'context> {
    pub data: &'context ProjectData,
}

impl UiComponent for OverviewTabUi {
    type UiContext<'context> = OverviewTabUiContext<'context>;
    type UiCommand = OverviewTabUiCommand;
    type UiAction = OverviewTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        let overview = context.data.overview.get();
        let phases = context
            .data
            .phases
            .get()
            .map(|phases| &phases.phases);

        if let Some(overview) = overview {
            ui.label(tr!("project-overview-detail-name", { name: &overview.name }));
        }

        ui.separator();

        ui.push_id("phases", |ui| {
            self.show_phases(ui, text_height, phases);
        });

        ui.push_id("pcbs", |ui| {
            self.show_pcbs(ui, text_height, overview);
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            OverviewTabUiCommand::None => Some(OverviewTabUiAction::None),
            OverviewTabUiCommand::AddPhaseClicked => Some(OverviewTabUiAction::AddPhase),
            OverviewTabUiCommand::PhaseDeleteClicked(reference) => Some(OverviewTabUiAction::DeletePhase(reference)),
            OverviewTabUiCommand::AddPcbClicked => Some(OverviewTabUiAction::AddPcb),
            OverviewTabUiCommand::PcbRemoveClicked(pcb_instance_index) => {
                Some(OverviewTabUiAction::RemovePcb(pcb_instance_index))
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct OverviewTab {}

impl Tab for OverviewTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-overview-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.overview_ui, ui, &mut OverviewTabUiContext {
            data: &state.data,
        });
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
