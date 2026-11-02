use egui::scroll_area::ScrollBarVisibility;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use planner_app::{IssueKind, ProjectReport};

use crate::project::data::ProjectData;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct IssuesTabUi {
    pub component: ComponentState<IssuesTabUiCommand>,
}

impl IssuesTabUi {
    const TABLE_SCROLL_HEIGHT_MIN: f32 = 100.0;

    pub fn new() -> Self {
        Self {
            component: Default::default(),
        }
    }

    fn show_issues(&self, ui: &mut Ui, text_height: f32, report: &ProjectReport) {
        TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::remainder())
            .column(Column::remainder())
            .column(Column::auto().resizable(false))
            .striped(true)
            .resizable(true)
            .auto_shrink([false, false])
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong(tr!("table-issues-column-index"));
                });
                header.col(|ui| {
                    ui.strong(tr!("table-issues-column-severity"));
                });
                header.col(|ui| {
                    ui.strong(tr!("table-issues-column-message"));
                });
                header.col(|ui| {
                    ui.strong(tr!("table-issues-column-details"));
                });
                header.col(|ui| {
                    ui.strong(tr!("table-issues-column-actions"));
                });
            })
            .body(|body| {
                body.rows(text_height, report.issues.len(), |mut row| {
                    let index = row.index();
                    if let Some(issue) = report.issues.get(index) {
                        row.col(|ui| {
                            ui.label(index.to_string());
                        });
                        row.col(|ui| {
                            // TODO translate
                            ui.label(format!("{:?}", issue.severity));
                        });
                        row.col(|ui| {
                            // TODO translate - this highlights an issue with the message generation, it
                            //      needs an i18n key and args, not an actual message
                            ui.label(&issue.message);
                        });
                        row.col(|ui| match &issue.kind {
                            IssueKind::NoPcbsAssigned => {}
                            IssueKind::NoPhasesCreated => {}
                            IssueKind::UnassignedPlacement {
                                object_path,
                            } => {
                                ui.label(object_path.to_string());
                            }
                            IssueKind::UnassignedPartFeeder {
                                phase,
                                part,
                            } => {
                                ui.label(format!("{} - {} {}", phase, part.mpn, part.manufacturer));
                            }
                            IssueKind::PcbWithNoUnitAssignments {
                                file,
                            } => {
                                ui.label(file.to_string());
                            }
                            IssueKind::NoPlacements => {}
                            IssueKind::PhaseWithNoPlacements {
                                phase,
                            } => {
                                ui.label(phase.to_string());
                            }
                        });
                        row.col(|ui| {
                            let _ = ui;
                            // TODO translate
                            match &issue.kind {
                                IssueKind::NoPcbsAssigned => {
                                    // TODO add button to show add pcbs form or similar
                                }
                                IssueKind::NoPhasesCreated => {
                                    // TODO add button to create a phase
                                }
                                IssueKind::UnassignedPlacement {
                                    object_path,
                                } => {
                                    // TODO add button to show the placement in the list of placements
                                    let _ = object_path;
                                }
                                IssueKind::UnassignedPartFeeder {
                                    phase,
                                    part,
                                } => {
                                    // TODO add button to show the part in the phase placements
                                    let (_, _) = (phase, part);
                                }
                                IssueKind::PcbWithNoUnitAssignments {
                                    file,
                                } => {
                                    // TODO add button to show the PCB's unit assignment
                                    let _ = file;
                                }
                                IssueKind::NoPlacements => {}
                                IssueKind::PhaseWithNoPlacements {
                                    phase,
                                } => {
                                    // TODO add button to show all placements so that assignments can be made
                                    let _ = phase;
                                }
                            }
                        });
                    }
                });
            });
    }
}

#[derive(Debug, Clone)]
pub enum IssuesTabUiCommand {
    None,
    RefreshClicked,
}

#[derive(Debug, Clone)]
pub enum IssuesTabUiAction {
    None,
    RefreshIssues,
}

#[derive(Debug, Clone)]
pub struct IssuesTabUiContext<'context> {
    pub data: &'context ProjectData,
}

impl UiComponent for IssuesTabUi {
    type UiContext<'context> = IssuesTabUiContext<'context>;
    type UiCommand = IssuesTabUiCommand;
    type UiAction = IssuesTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        ui.horizontal(|ui| {
            if ui
                .button(tr!("form-common-button-refresh"))
                .clicked()
            {
                self.component
                    .send(IssuesTabUiCommand::RefreshClicked);
            }
        });

        ui.separator();

        if let Some(report) = context.data.report.get() {
            self.show_issues(ui, text_height, report);
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            IssuesTabUiCommand::None => Some(IssuesTabUiAction::None),
            IssuesTabUiCommand::RefreshClicked => Some(IssuesTabUiAction::RefreshIssues),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct IssuesTab {}

impl Tab for IssuesTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-issues-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.issues_ui, ui, &mut IssuesTabUiContext {
            data: &state.data,
        });
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
