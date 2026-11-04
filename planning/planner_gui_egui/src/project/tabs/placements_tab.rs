use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::tr;
use planner_app::{ObjectPath, PcbSide, PhaseReference, PlacementPositionUnit, PlacementState, PlacementsItem};
use tracing::debug;

use crate::forms::NamedResponse;
use crate::project::data::{ProjectData, Synced};
use crate::project::tables::placements::{
    PlacementsTableUi, PlacementsTableUiAction, PlacementsTableUiCommand, PlacementsTableUiContext,
};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PlacementsTabUi {
    #[derivative(Debug = "ignore")]
    placements_table_ui: PlacementsTableUi,

    selection: Option<Vec<PlacementsItem>>,
    selected_phase: Option<PhaseReference>,

    /// how many placements the last "Apply" left unchanged as they're on the other side of the PCB to the phase.
    skipped: Option<usize>,

    /// the revisions of the placements and phases views used to build the table
    synced: Synced<(u64, u64)>,

    pub component: ComponentState<PlacementsTabUiCommand>,
}

impl PlacementsTabUi {
    pub fn new() -> Self {
        let component: ComponentState<PlacementsTabUiCommand> = Default::default();

        let mut placements_table_ui = PlacementsTableUi::new();
        placements_table_ui
            .component
            .configure_mapper(component.sender.clone(), |placements_table_command| {
                PlacementsTabUiCommand::PlacementsTableUiCommand(placements_table_command)
            });

        Self {
            placements_table_ui,
            selection: None,
            selected_phase: None,
            skipped: None,
            synced: Synced::default(),
            component,
        }
    }

    /// Rebuilds the table's rows if the views they are built from have changed.
    pub fn sync(&mut self, data: &ProjectData) {
        let Some(placements) = data.placements.get() else {
            return;
        };

        if self
            .synced
            .changed((data.placements.revision(), data.phases.revision()))
        {
            self.placements_table_ui
                .update_placements(placements.placements.clone(), data.phase_list().to_vec());

            // the rows stay selected, so what's known about them has to stay up to date, otherwise an action compares
            // the new phase with the phase a placement had when it was selected.
            if let Some(selection) = &mut self.selection {
                *selection = selection
                    .iter()
                    .filter_map(|selected| {
                        placements
                            .placements
                            .iter()
                            .find(|placement| placement.path.eq(&selected.path))
                            .cloned()
                    })
                    .collect();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlacementsTabUiCommand {
    None,
    PlacementsTableUiCommand(PlacementsTableUiCommand),

    PlacementActionClicked(PlacementAction),
    PhaseChanged(PhaseReference),
}

#[derive(Debug, Clone)]
pub enum PlacementAction {
    ApplyPhase,
    RemovePhase,
}

#[derive(Debug, Clone)]
pub enum PlacementsTabUiAction {
    None,
    RequestRepaint,
    UpdatePlacement {
        object_path: ObjectPath,
        new_placement: PlacementState,
        old_placement: PlacementState,
    },
    LocatePlacement {
        /// Full object path of the component
        object_path: ObjectPath,
        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    },
    ApplyPlacementsAction(Vec<PlacementsItem>, PlacementsTabUiApplyAction),
}

#[derive(Debug, Clone)]
pub enum PlacementsTabUiApplyAction {
    ApplyPhase(PhaseReference),
    RemovePhase(PhaseReference),
}

#[derive(Debug, Clone)]
pub struct PlacementsTabUiContext<'context> {
    pub data: &'context ProjectData,
}

impl UiComponent for PlacementsTabUi {
    type UiContext<'context> = PlacementsTabUiContext<'context>;
    type UiCommand = PlacementsTabUiCommand;
    type UiAction = PlacementsTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let phases = context.data.phase_list();
        let can_change_phase = phases
            .iter()
            .all(|it| it.state.is_pending());

        ui.horizontal(|ui| {
            // FIXME layout-pain - due to some vertical padding and sizing issues we render the filter first because it is tallest.
            //       attempts were made to fix this, but ultimately all resulted in failure, so we gave up and went
            //       with the simplest code, even though we WANT the buttons BEFORE the filter.
            //       see the commit history/git-blame for details
            //       the same issue exists on three tabs: placements tab, phase tab, parts tab.

            self.placements_table_ui.filter_ui(ui);

            ui.separator();

            let have_selection = self
                .selection
                .as_ref()
                .is_some_and(|selection| !selection.is_empty());
            let have_phases = !phases.is_empty();

            ui.add_enabled_ui(have_phases, |ui|{
                egui::ComboBox::from_id_salt(ui.id().with("phase_selection"))
                    .selected_text(match &self.selected_phase {
                        Some(phase) => format!("{}", phase),
                        None => tr!("form-common-choice-phase"),
                    })
                    .show_ui(ui, |ui| {
                        for phase in phases {
                            if ui
                                .add(egui::Button::selectable(
                                    matches!(&self.selected_phase, Some(selection) if selection.eq(&phase.phase_reference)),
                                    format!("{}", phase.phase_reference),
                                ))
                                .clicked()
                            {
                                self.component
                                    .sender
                                    .send(PlacementsTabUiCommand::PhaseChanged(phase.phase_reference.clone()))
                                    .expect("sent");
                            }
                        }
                    })
                    .response
                    .named(tr!("form-common-choice-phase"));
            })
                .response
                    // FIXME the `on_hover_text` does not work, no tooltip appears when hovering over an ENABLED combobox.
                    //       the `on_disabled_hover_text` DOES work though.
                    .on_hover_text(if have_selection {
                        tr!("project-placements-tab-phase-hover-text-with-selection")
                    } else {
                        tr!("project-placements-tab-phase-hover-text-no-selection")
                    })
                    .on_disabled_hover_text(tr!("project-placements-tab-phase-hover-text-no-phases"));

            let have_phase = self.selected_phase.is_some();

            ui.add_enabled_ui(have_selection && have_phase, |ui| {
                egui::ComboBox::from_id_salt(ui.id().with("phase_action"))
                    .selected_text(tr!("common-actions"))
                    .show_ui(ui, |ui| {
                        ui.add_enabled_ui(can_change_phase, |ui| {
                            if ui
                                .add(egui::Button::selectable(false, tr!("form-common-button-apply")))
                                .clicked()
                            {
                                self.component
                                    .sender
                                    .send(PlacementsTabUiCommand::PlacementActionClicked(
                                        PlacementAction::ApplyPhase,
                                    ))
                                    .expect("sent");
                            }
                            if ui
                                .add(egui::Button::selectable(false, tr!("form-common-button-remove")))
                                .clicked()
                            {
                                self.component
                                    .sender
                                    .send(PlacementsTabUiCommand::PlacementActionClicked(
                                        PlacementAction::RemovePhase,
                                    ))
                                    .expect("sent");
                            }
                        });
                    })
                    .response
                    .named(tr!("common-actions"));
            });

            if let Some(skipped) = self.skipped {
                ui.label(tr!("project-placements-tab-skipped-other-side", { count: skipped }));
            }
        });

        ui.separator();

        self.placements_table_ui
            .ui(ui, &mut PlacementsTableUiContext::default())
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("PlacementsTabUi command. command: {:?}", command);
        match command {
            PlacementsTabUiCommand::None => Some(PlacementsTabUiAction::None),
            PlacementsTabUiCommand::PhaseChanged(phase) => {
                self.selected_phase = Some(phase);
                self.skipped = None;
                None
            }
            PlacementsTabUiCommand::PlacementsTableUiCommand(command) => {
                let action = self
                    .placements_table_ui
                    .update(command, &mut PlacementsTableUiContext::default());
                match action {
                    None => None,
                    Some(PlacementsTableUiAction::None) => None,
                    Some(PlacementsTableUiAction::RequestRepaint) => Some(PlacementsTabUiAction::RequestRepaint),
                    Some(PlacementsTableUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => {
                        // the table has changed its own rows, the core may reject the change, e.g. when pasting a
                        // 'Top' phase onto a placement with a side of 'Bottom'.
                        self.synced.invalidate();

                        Some(PlacementsTabUiAction::UpdatePlacement {
                            object_path,
                            new_placement,
                            old_placement,
                        })
                    }
                    Some(PlacementsTableUiAction::LocatePlacement {
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }) => Some(PlacementsTabUiAction::LocatePlacement {
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }),
                    Some(PlacementsTableUiAction::ApplySelection(selection)) => {
                        self.selection = Some(selection);
                        self.skipped = None;
                        None
                    }
                }
            }
            PlacementsTabUiCommand::PlacementActionClicked(action) => {
                self.skipped = None;

                let (Some(selection), Some(phase)) = (&self.selection, &self.selected_phase) else {
                    return None;
                };

                let (placements, apply_action) = match action {
                    PlacementAction::ApplyPhase => {
                        // the core only assigns placements on the phase's side of the PCB (`PLACEMENTS-2`)
                        let pcb_side = context
                            .data
                            .phase_list()
                            .iter()
                            .find(|overview| overview.phase_reference.eq(phase))?
                            .pcb_side;

                        let (placements, skipped): (Vec<_>, Vec<_>) = selection
                            .iter()
                            .cloned()
                            .partition(|item| {
                                item.state
                                    .placement
                                    .pcb_side
                                    .eq(&pcb_side)
                            });

                        if !skipped.is_empty() {
                            self.skipped = Some(skipped.len());
                        }

                        (placements, PlacementsTabUiApplyAction::ApplyPhase(phase.clone()))
                    }
                    PlacementAction::RemovePhase => {
                        // only from the chosen phase (`PLACEMENTS-3`)
                        let placements = selection
                            .iter()
                            .filter(|item| item.state.phase.as_ref() == Some(phase))
                            .cloned()
                            .collect::<Vec<_>>();

                        (placements, PlacementsTabUiApplyAction::RemovePhase(phase.clone()))
                    }
                };

                if placements.is_empty() {
                    return None;
                }

                Some(PlacementsTabUiAction::ApplyPlacementsAction(placements, apply_action))
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct PlacementsTab {}

impl Tab for PlacementsTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-placements-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.placements_ui, ui, &mut PlacementsTabUiContext {
            data: &state.data,
        });
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
