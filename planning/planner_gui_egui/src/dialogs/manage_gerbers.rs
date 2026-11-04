use std::path::PathBuf;

use derivative::Derivative;
use egui::Modal;
use egui_extras::Column;
use egui_i18n::tr;
use planner_app::{GerberFileFunction, GerberFileFunctionDiscriminants, PcbGerberItem, PcbSide, PcbSideRequirement};
use strum::VariantArray;
use tracing::debug;

use crate::file_picker::Picker;
use crate::i18n::conversions::{gerber_file_function_discriminant_to_i18n_key, pcb_side_to_i18n_key};
use crate::sync::Value;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ManageGerbersModal {
    title: String,
    gerbers: Vec<PcbGerberItem>,
    gerber_file_functions: Vec<(Option<GerberFileFunctionDiscriminants>, Option<PcbSide>)>,

    file_picker: Value<Picker>,

    pub component: ComponentState<ManagerGerbersModalUiCommand>,
}

impl ManageGerbersModal {
    pub fn new(title: String, gerbers: Vec<PcbGerberItem>) -> Self {
        let gerber_file_functions = gerbers
            .iter()
            .map(|meh| {
                let discriminant = meh
                    .function
                    .map(|function| function.into());
                let pcb_side = meh
                    .function
                    .map(|function| function.pcb_side())
                    .flatten();

                (discriminant, pcb_side)
            })
            .collect::<Vec<_>>();
        Self {
            title,
            gerbers,
            gerber_file_functions,
            component: Default::default(),
            file_picker: Default::default(),
        }
    }

    pub fn update_gerbers(&mut self, gerbers: Vec<PcbGerberItem>) {
        self.gerbers = gerbers.clone();
    }
}

#[derive(Debug, Clone)]
pub enum ManagerGerbersModalUiCommand {
    Close,
    Remove {
        index: usize,
    },
    Add,
    GerberFilesPicked {
        picked_files: Vec<PathBuf>,
    },
    Refresh,
    FunctionChanged {
        index: usize,
        function_discriminant: Option<GerberFileFunctionDiscriminants>,
    },
    PcbSideChanged {
        index: usize,
        pcb_side: Option<PcbSide>,
    },
    Apply,
}

#[derive(Debug, Clone)]
pub enum ManagerGerberModalAction {
    CloseDialog,
    RemoveGerberFiles {
        files: Vec<PathBuf>,
    },
    AddGerberFiles {
        files: Vec<PathBuf>,
    },
    RefreshGerberFiles,
    ApplyGerberFileFunctions {
        file_functions: Vec<(PathBuf, Option<GerberFileFunction>)>,
    },
}

impl UiComponent for ManageGerbersModal {
    type UiContext<'context> = ();
    type UiCommand = ManagerGerbersModalUiCommand;
    type UiAction = ManagerGerberModalAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        if let Ok(picked_files) = self
            .file_picker
            .lock()
            .unwrap()
            .picked_multi()
        {
            self.component
                .send(ManagerGerbersModalUiCommand::GerberFilesPicked {
                    picked_files,
                });
        }

        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps when using taffy
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let modal_id = ui.id().with("manage_gerbers_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            ui.heading(tr!("modal-manager-gerbers-title", { design: self.title.to_string() }));

            let text_height = egui::TextStyle::Body
                .resolve(ui.style())
                .size
                .max(ui.spacing().interact_size.y);

            // FUTURE perhaps a data-table would be better here?
            egui_extras::TableBuilder::new(ui)
                .striped(true)
                .column(Column::auto())
                .column(Column::remainder())
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::auto())
                .header(text_height, |mut header| {
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-index"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-file"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-gerber-file-function"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-pcb-side"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-actions"));
                    });
                })
                .body(|mut body| {
                    for (
                        (
                            index,
                            PcbGerberItem {
                                path,
                                function: _function,
                            },
                        ),
                        choice,
                    ) in self
                        .gerbers
                        .iter()
                        .enumerate()
                        .zip(self.gerber_file_functions.iter())
                    {
                        body.row(text_height, |mut row| {
                            row.col(|ui| {
                                ui.label(index.to_string());
                            });
                            row.col(|ui| {
                                ui.label(
                                    path.file_stem()
                                        .unwrap()
                                        .to_string_lossy()
                                        .to_string(),
                                );
                            });
                            row.col(|ui| {
                                egui::ComboBox::from_id_salt(ui.id().with("file_function"))
                                    .selected_text(match choice.0 {
                                        Some(function_choice) => {
                                            tr!(gerber_file_function_discriminant_to_i18n_key(&function_choice))
                                        }
                                        None => tr!("form-common-combo-select"),
                                    })
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .add(egui::Button::selectable(
                                                choice.0.is_none(),
                                                tr!("form-common-combo-none"),
                                            ))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ManagerGerbersModalUiCommand::FunctionChanged {
                                                    index,
                                                    function_discriminant: None,
                                                });
                                        }
                                        for discriminant in GerberFileFunctionDiscriminants::VARIANTS {
                                            let selected = choice
                                                .0
                                                .map(|function_choice| discriminant.eq(&function_choice))
                                                .unwrap_or(false);

                                            if ui
                                                .add(egui::Button::selectable(
                                                    selected,
                                                    tr!(gerber_file_function_discriminant_to_i18n_key(discriminant)),
                                                ))
                                                .clicked()
                                            {
                                                self.component
                                                    .send(ManagerGerbersModalUiCommand::FunctionChanged {
                                                        index,
                                                        function_discriminant: Some(*discriminant),
                                                    });
                                            }
                                        }
                                    });
                            });
                            row.col(|ui| {
                                let pcb_side_requirement = choice
                                    .0
                                    .map(|choice| choice.pcb_side_requirement());

                                match pcb_side_requirement {
                                    Some(requirement)
                                        if requirement == PcbSideRequirement::Required
                                            || requirement == PcbSideRequirement::Optional =>
                                    {
                                        let pcb_side = choice.1;

                                        egui::ComboBox::from_id_salt(ui.id().with("pcb_side"))
                                            .selected_text({
                                                let label_key = pcb_side
                                                    .map(|pcb_side| pcb_side_to_i18n_key(&pcb_side))
                                                    .unwrap_or("common-value-not-available");

                                                tr!(label_key)
                                            })
                                            .show_ui(ui, |ui| {
                                                let is_top = pcb_side
                                                    .map(|pcb_side| pcb_side == PcbSide::Top)
                                                    .unwrap_or(false);

                                                let is_bottom = pcb_side
                                                    .map(|pcb_side| pcb_side == PcbSide::Bottom)
                                                    .unwrap_or(false);

                                                if ui
                                                    .add(egui::Button::selectable(
                                                        is_top,
                                                        tr!("form-common-choice-pcb-side-top"),
                                                    ))
                                                    .clicked()
                                                {
                                                    self.component
                                                        .send(ManagerGerbersModalUiCommand::PcbSideChanged {
                                                            index,
                                                            pcb_side: Some(PcbSide::Top),
                                                        });
                                                }
                                                if ui
                                                    .add(egui::Button::selectable(
                                                        is_bottom,
                                                        tr!("form-common-choice-pcb-side-bottom"),
                                                    ))
                                                    .clicked()
                                                {
                                                    self.component
                                                        .send(ManagerGerbersModalUiCommand::PcbSideChanged {
                                                            index,
                                                            pcb_side: Some(PcbSide::Bottom),
                                                        });
                                                }
                                            });
                                    }
                                    Some(_requirement) => {
                                        ui.label(tr!("form-common-value-not-available"));
                                    }
                                    _ => {
                                        ui.label(tr!("form-common-value-not-available"));
                                    }
                                }
                            });
                            row.col(|ui| {
                                if ui
                                    .button(tr!("form-common-button-remove"))
                                    .clicked()
                                {
                                    self.component
                                        .send(ManagerGerbersModalUiCommand::Remove {
                                            index,
                                        });
                                }
                            });
                        })
                    }
                });

            egui::Sides::new().show(
                ui,
                |ui| {
                    if ui
                        .button(tr!("form-common-button-add"))
                        .clicked()
                    {
                        self.component
                            .send(ManagerGerbersModalUiCommand::Add);
                    }
                    if ui
                        .button(tr!("form-common-button-refresh"))
                        .clicked()
                    {
                        self.component
                            .send(ManagerGerbersModalUiCommand::Refresh);
                    }
                    if ui
                        .button(tr!("form-common-button-apply"))
                        .clicked()
                    {
                        self.component
                            .send(ManagerGerbersModalUiCommand::Apply);
                    }
                },
                |ui| {
                    if ui
                        .button(tr!("form-common-button-close"))
                        .clicked()
                    {
                        self.component
                            .send(ManagerGerbersModalUiCommand::Close);
                    }
                },
            );
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("ManageGerbersModal command. command: {:?}", command);
        match command {
            ManagerGerbersModalUiCommand::Close => Some(ManagerGerberModalAction::CloseDialog),
            ManagerGerbersModalUiCommand::Remove {
                index,
            } => {
                let files = vec![self.gerbers[index].path.clone()];
                Some(ManagerGerberModalAction::RemoveGerberFiles {
                    files,
                })
            }
            ManagerGerbersModalUiCommand::Add => {
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_files();
                None
            }
            ManagerGerbersModalUiCommand::Refresh => Some(ManagerGerberModalAction::RefreshGerberFiles),
            ManagerGerbersModalUiCommand::GerberFilesPicked {
                picked_files,
            } => Some(ManagerGerberModalAction::AddGerberFiles {
                files: picked_files,
            }),
            ManagerGerbersModalUiCommand::FunctionChanged {
                index,
                function_discriminant,
            } => {
                let choice = self
                    .gerber_file_functions
                    .get_mut(index)
                    .unwrap();
                choice.0 = function_discriminant;
                None
            }
            ManagerGerbersModalUiCommand::PcbSideChanged {
                index,
                pcb_side,
            } => {
                let choice = self
                    .gerber_file_functions
                    .get_mut(index)
                    .unwrap();
                choice.1 = pcb_side;
                None
            }
            ManagerGerbersModalUiCommand::Apply => {
                debug!("gerber_file_functions: {:?}", self.gerber_file_functions);
                let file_functions: Vec<(PathBuf, Option<GerberFileFunction>)> = self
                    .gerber_file_functions
                    .iter()
                    .map(|choice| {
                        let pcb_side = choice.1;
                        choice
                            .0
                            .map(|discriminant| match (discriminant, pcb_side) {
                                //
                                // Required pcb side
                                //
                                (GerberFileFunctionDiscriminants::Assembly, Some(pcb_side)) => {
                                    Some(GerberFileFunction::Assembly(pcb_side))
                                }
                                (GerberFileFunctionDiscriminants::Component, Some(pcb_side)) => {
                                    Some(GerberFileFunction::Component(pcb_side))
                                }
                                (GerberFileFunctionDiscriminants::Copper, Some(pcb_side)) => {
                                    Some(GerberFileFunction::Copper(pcb_side))
                                }
                                (GerberFileFunctionDiscriminants::Legend, Some(pcb_side)) => {
                                    Some(GerberFileFunction::Legend(pcb_side))
                                }
                                (GerberFileFunctionDiscriminants::Paste, Some(pcb_side)) => {
                                    Some(GerberFileFunction::Paste(pcb_side))
                                }
                                (GerberFileFunctionDiscriminants::Solder, Some(pcb_side)) => {
                                    Some(GerberFileFunction::Solder(pcb_side))
                                }
                                //
                                // No pcb side
                                //
                                (GerberFileFunctionDiscriminants::Profile, None) => Some(GerberFileFunction::Profile),
                                //
                                // Option pcb side
                                //
                                (GerberFileFunctionDiscriminants::Other, pcb_side) => {
                                    Some(GerberFileFunction::Other(pcb_side))
                                }
                                // Invalid/unfinished selections
                                _ => None,
                            })
                    })
                    .zip(self.gerbers.iter())
                    .filter_map(|(choice, item)| choice.map(|choice| (item.path.clone(), choice)))
                    .collect::<Vec<_>>();

                Some(ManagerGerberModalAction::ApplyGerberFileFunctions {
                    file_functions,
                })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum GerberPcbSideChoice {
    Both,
    Top,
    Bottom,
}

impl From<GerberPcbSideChoice> for Option<PcbSide> {
    fn from(value: GerberPcbSideChoice) -> Self {
        match value {
            GerberPcbSideChoice::Top => Some(PcbSide::Top),
            GerberPcbSideChoice::Bottom => Some(PcbSide::Bottom),
            GerberPcbSideChoice::Both => None,
        }
    }
}

impl From<&Option<PcbSide>> for GerberPcbSideChoice {
    fn from(value: &Option<PcbSide>) -> Self {
        match value {
            None => GerberPcbSideChoice::Both,
            Some(PcbSide::Top) => GerberPcbSideChoice::Top,
            Some(PcbSide::Bottom) => GerberPcbSideChoice::Bottom,
        }
    }
}
