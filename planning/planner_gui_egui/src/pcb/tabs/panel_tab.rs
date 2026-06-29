use std::collections::HashMap;
use std::fmt::Debug;
use std::io::BufWriter;
use std::sync::mpsc::Sender;

use derivative::Derivative;
use eframe::emath::Vec2;
use egui::scroll_area::ScrollBarVisibility;
use egui::{Resize, Style, Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::Value;
use gerber_viewer::gerber_types::{
    Aperture, Circle, Command, CoordinateFormat, CoordinateMode, ExtendedCode, GerberCode, GerberError, ImageMirroring,
    ImageOffset, ImageRotation, InterpolationMode, ZeroOmission,
};
use gerber_viewer::{ToPosition, ToVector};
use math::ops::Ops2D;
use nalgebra::{Point, Point2, Vector2};
use num_traits::{FromPrimitive, ToPrimitive};
use planner_app::{
    DesignIndex, DesignSizing, Dimensions, FiducialParameters, GerberFileFunction, PanelSizing, PcbAssemblyFlip,
    PcbAssemblyOrientation, PcbOverview, PcbSide, PcbUnitIndex, PcbUnitPositioning, Unit,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tracing::{debug, trace};

use crate::i18n::conversions::pcb_orientation_pitch_flip_to_i18n_key;
use crate::pcb::tabs::PcbTabContext;
use crate::pcb::tabs::panel_tab::gerber_builder::GerberBuilder;
use crate::pcb::tabs::panel_tab::gerber_util::{
    gerber_line_commands, gerber_path_commands, gerber_point_commands, gerber_rectangle_commands,
};
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_components::gerber_viewer_ui::{
    GerberViewerMode, GerberViewerUi, GerberViewerUiAction, GerberViewerUiCommand, GerberViewerUiContext,
    GerberViewerUiInstanceArgs,
};

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DragSliderParameters {
    pub speed: f64,
    pub fixed_decimals: usize,
}

pub mod defaults {
    use std::collections::HashMap;
    use std::sync::LazyLock;

    use planner_app::Unit;

    use super::DragSliderParameters;

    pub static DRAG_SLIDER: LazyLock<HashMap<Unit, DragSliderParameters>> = LazyLock::new(|| {
        HashMap::from_iter([
            (Unit::Millimeters, DragSliderParameters {
                speed: 0.1,
                fixed_decimals: 3,
            }),
            (Unit::Inches, DragSliderParameters {
                speed: 0.1,
                fixed_decimals: 4,
            }),
        ])
    });

    pub static DRAG_ANGLE_SPEED: f64 = 0.1;
    pub static DRAG_ANGLE_FIXED_DECIMALS: usize = 3;
}

#[derive(Derivative, Debug)]
#[derivative(Default)]
struct PanelTabUiState {
    #[derivative(Default(value = "PcbSide::Top"))]
    pcb_side: PcbSide,
    new_fiducial: FiducialParameters,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PanelTabUi {
    pcb_overview: Option<PcbOverview>,

    // (current, initial)
    panel_sizing: Option<(PanelSizing, PanelSizing)>,
    assembly_orientation: Option<(PcbAssemblyOrientation, PcbAssemblyOrientation)>,

    panel_tab_ui_state: PanelTabUiState,

    #[derivative(Debug = "ignore")]
    gerber_viewer_ui: Value<GerberViewerUi>,

    pub component: ComponentState<PanelTabUiCommand>,
}

impl PanelTabUi {
    const TABLE_HEIGHT_MAX: f32 = 400.0;
    const TABLE_SCROLL_HEIGHT_MIN: f32 = 30.0;

    pub fn new() -> Self {
        let component: ComponentState<PanelTabUiCommand> = Default::default();

        // FIXME probably the gerber viewer UI needs different args now
        let args = GerberViewerUiInstanceArgs {
            mode: GerberViewerMode::Panel,
            pcb_side: Some(PcbSide::Top),
        };

        let mut gerber_viewer_ui = GerberViewerUi::new(args);
        gerber_viewer_ui
            .component
            .configure_mapper(component.sender.clone(), |gerber_viewer_command| {
                trace!("gerber_viewer mapper. command: {:?}", gerber_viewer_command);
                PanelTabUiCommand::GerberViewerUiCommand(gerber_viewer_command)
            });

        Self {
            panel_sizing: None,
            assembly_orientation: None,
            pcb_overview: None,
            panel_tab_ui_state: Default::default(),
            gerber_viewer_ui: Value::new(gerber_viewer_ui),
            component,
        }
    }

    pub fn reset(&mut self) {
        self.pcb_overview = None;
        self.panel_sizing = None;
        self.assembly_orientation = None;
        self.panel_tab_ui_state = Default::default();
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        let assembly_orientation = pcb_overview.orientation.clone();
        self.assembly_orientation
            .replace((assembly_orientation.clone(), assembly_orientation.clone()));

        {
            let mut gerber_viewer_ui = self.gerber_viewer_ui.lock().unwrap();
            gerber_viewer_ui.set_assembly_orientation(assembly_orientation);
            gerber_viewer_ui.set_unit_map(pcb_overview.unit_map.clone());
        }

        self.pcb_overview.replace(pcb_overview);
        self.update_panel_preview();
    }

    pub fn update_panel_sizing(&mut self, panel_sizing: PanelSizing) {
        self.gerber_viewer_ui
            .lock()
            .unwrap()
            .set_panel_sizing(panel_sizing.clone());
        self.panel_sizing
            .replace((panel_sizing.clone(), panel_sizing));
        self.update_panel_preview();
    }

    fn left_panel_content(
        ui: &mut Ui,
        panel_sizing: &PanelSizing,
        initial_panel_sizing: &PanelSizing,
        state: &PanelTabUiState,
        sender: Sender<PanelTabUiCommand>,
        pcb_overview: &PcbOverview,
        assembly_orientation: &PcbAssemblyOrientation,
        initial_assembly_orientation: &PcbAssemblyOrientation,
    ) {
        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        egui::ScrollArea::both().show(ui, |ui| {
            let is_changed =
                panel_sizing.ne(initial_panel_sizing) || assembly_orientation.ne(initial_assembly_orientation);

            egui::Sides::new().show(
                ui,
                |ui| {
                    if ui
                        .add_enabled(is_changed, egui::Button::new(tr!("form-common-button-reset")))
                        .clicked()
                    {
                        sender
                            .send(PanelTabUiCommand::Reset)
                            .expect("sent");
                    }

                    if ui
                        .add_enabled(is_changed, egui::Button::new(tr!("form-common-button-apply")))
                        .clicked()
                    {
                        sender
                            .send(PanelTabUiCommand::Apply)
                            .expect("sent");
                    }
                },
                |_ui| {},
            );
            ui.separator();

            // TODO let the user choose units (MM, Inches, etc)

            Self::top_bottom_controls(state, &sender, ui);
            ui.separator();
            Self::orientation_controls(&assembly_orientation, state, &sender, ui);
            ui.separator();
            Self::panel_size_controls(panel_sizing, &sender, ui);
            ui.separator();
            Self::edge_rails_controls(panel_sizing, &sender, ui);
            ui.separator();
            Self::fiducials_controls(panel_sizing, state, &sender, text_height, ui);
            ui.separator();
            Self::design_configuration_controls(panel_sizing, pcb_overview, &sender, text_height, ui);
            ui.separator();
            Self::unit_positions_controls(panel_sizing, pcb_overview, &sender, text_height, ui);
        });
    }

    /// show top/bottom selector
    fn top_bottom_controls(state: &PanelTabUiState, sender: &Sender<PanelTabUiCommand>, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(tr!("pcb-panel-tab-input-show-top-bottom"));

            egui::ComboBox::from_id_salt(ui.id().with("pcb_side"))
                .width(ui.available_width())
                .selected_text(match state.pcb_side {
                    PcbSide::Top => tr!("form-common-choice-pcb-side-top"),
                    PcbSide::Bottom => tr!("form-common-choice-pcb-side-bottom"),
                })
                .show_ui(ui, |ui| {
                    if ui
                        .add(egui::Button::selectable(
                            state.pcb_side == PcbSide::Top,
                            tr!("form-common-choice-pcb-side-top"),
                        ))
                        .clicked()
                    {
                        sender
                            .send(PanelTabUiCommand::PcbSideChanged(PcbSide::Top))
                            .expect("sent");
                    }
                    if ui
                        .add(egui::Button::selectable(
                            state.pcb_side == PcbSide::Bottom,
                            tr!("form-common-choice-pcb-side-bottom"),
                        ))
                        .clicked()
                    {
                        sender
                            .send(PanelTabUiCommand::PcbSideChanged(PcbSide::Bottom))
                            .expect("sent");
                    }
                });
        });
    }

    /// show assembly orientation controls
    fn orientation_controls(
        assembly_orientation: &PcbAssemblyOrientation,
        _state: &PanelTabUiState,
        sender: &Sender<PanelTabUiCommand>,
        ui: &mut Ui,
    ) {
        fn flip_ui(ui: &mut Ui, flip: &mut PcbAssemblyFlip, salt: &str) -> egui::Response {
            let response = egui::ComboBox::from_id_salt(salt)
                .selected_text(tr!(pcb_orientation_pitch_flip_to_i18n_key(*flip)))
                .show_ui(ui, |ui| {
                    if ui
                        .add(egui::Button::selectable(
                            *flip == PcbAssemblyFlip::None,
                            tr!(pcb_orientation_pitch_flip_to_i18n_key(PcbAssemblyFlip::None)),
                        ))
                        .clicked()
                    {
                        *flip = PcbAssemblyFlip::None;
                    }
                    if ui
                        .add(egui::Button::selectable(
                            *flip == PcbAssemblyFlip::Pitch,
                            tr!(pcb_orientation_pitch_flip_to_i18n_key(PcbAssemblyFlip::Pitch)),
                        ))
                        .clicked()
                    {
                        *flip = PcbAssemblyFlip::Pitch;
                    }
                    if ui
                        .add(egui::Button::selectable(
                            *flip == PcbAssemblyFlip::Roll,
                            tr!(pcb_orientation_pitch_flip_to_i18n_key(PcbAssemblyFlip::Roll)),
                        ))
                        .clicked()
                    {
                        *flip = PcbAssemblyFlip::Roll;
                    }
                });

            let response = response.response;

            let response = response.on_hover_text(tr!("pcb-assembly-orientation-flip-tooltip"));

            response
        }

        //
        // orientation
        //

        let mut new_assembly_orientation = assembly_orientation.clone();

        ui.label(tr!("pcb-panel-tab-panel-orientation-header"));
        ui.horizontal(|ui| {
            // TODO allow orientation/flipping to be changed from the UI.
            egui::Frame::group(&Style {
                ..Style::default()
            })
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(tr!("pcb-side-top"));

                    flip_ui(ui, &mut new_assembly_orientation.top.flip, "top_flip");

                    ui.label(tr!("pcb-assembly-orientation-rotation"));

                    let mut rotation = new_assembly_orientation
                        .top
                        .rotation
                        .to_f32()
                        .unwrap()
                        .to_radians();
                    if ui.drag_angle(&mut rotation).changed() {
                        new_assembly_orientation.top.rotation =
                            Decimal::from_f32(rotation.to_degrees()).unwrap_or_default();
                    }
                });
            });
            egui::Frame::group(&Style::default()).show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(tr!("pcb-side-bottom"));

                    flip_ui(ui, &mut new_assembly_orientation.bottom.flip, "bottom_flip");

                    ui.label(tr!("pcb-assembly-orientation-rotation"));

                    let mut rotation = new_assembly_orientation
                        .bottom
                        .rotation
                        .to_f32()
                        .unwrap()
                        .to_radians();
                    if ui.drag_angle(&mut rotation).changed() {
                        new_assembly_orientation.bottom.rotation =
                            Decimal::from_f32(rotation.to_degrees()).unwrap_or_default();
                    }
                });
            });
        });
        ui.label(tr!("pcb-assembly-orientation-flip-help"));

        if new_assembly_orientation.ne(&assembly_orientation) {
            sender
                .send(PanelTabUiCommand::AssemblyOrientationChanged(new_assembly_orientation))
                .expect("sent");
        }
    }

    fn panel_size_controls(panel_sizing: &PanelSizing, sender: &Sender<PanelTabUiCommand>, ui: &mut Ui) {
        ui.label(tr!("pcb-panel-tab-panel-size-header"));

        let mut size = panel_sizing.size;

        ui.horizontal(|ui| {
            ui.label(tr!("form-common-input-x"));
            ui.add(
                egui::DragValue::new(&mut size.x)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );
        });
        ui.horizontal(|ui| {
            ui.label(tr!("form-common-input-y"));
            ui.add(
                egui::DragValue::new(&mut size.y)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );
        });

        if !size.eq(&panel_sizing.size) {
            sender
                .send(PanelTabUiCommand::SizeChanged(size))
                .expect("sent");
        }
    }

    fn edge_rails_controls(panel_sizing: &PanelSizing, sender: &Sender<PanelTabUiCommand>, ui: &mut Ui) {
        let mut edge_rails = panel_sizing.edge_rails.clone();

        ui.label(tr!("pcb-panel-tab-panel-edge-rails-header"));

        egui::Grid::new("edge_rails_grid")
            .num_columns(3)
            .show(ui, |ui| {
                ui.label(""); // placeholder
                ui.horizontal(|ui| {
                    ui.label(tr!("form-common-input-top"));
                    ui.add(
                        egui::DragValue::new(&mut edge_rails.top)
                            .range(0.0..=f64::MAX)
                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                            .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
                    );
                });
                ui.label(""); // placeholder
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(tr!("form-common-input-left"));
                    ui.add(
                        egui::DragValue::new(&mut edge_rails.left)
                            .range(0.0..=f64::MAX)
                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                            .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
                    );
                });
                ui.label(""); // placeholder
                ui.horizontal(|ui| {
                    ui.label(tr!("form-common-input-right"));
                    ui.add(
                        egui::DragValue::new(&mut edge_rails.right)
                            .range(0.0..=f64::MAX)
                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                            .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
                    );
                });
                ui.end_row();

                ui.label(""); // placeholder
                ui.horizontal(|ui| {
                    ui.label(tr!("form-common-input-bottom"));
                    ui.add(
                        egui::DragValue::new(&mut edge_rails.bottom)
                            .range(0.0..=f64::MAX)
                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                            .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
                    );
                });
                ui.label(""); // placeholder
                ui.end_row();
            });

        if !edge_rails.eq(&panel_sizing.edge_rails) {
            sender
                .send(PanelTabUiCommand::EdgeRailsChanged(edge_rails))
                .expect("sent");
        }
    }

    fn fiducials_controls(
        panel_sizing: &PanelSizing,
        state: &PanelTabUiState,
        sender: &Sender<PanelTabUiCommand>,
        text_height: f32,
        ui: &mut Ui,
    ) {
        ui.label(tr!("pcb-panel-tab-panel-fiducials-header"));

        ui.horizontal(|ui| {
            let mut new_fiducial = state.new_fiducial;

            ui.label(tr!("form-common-input-x"));
            ui.add(
                egui::DragValue::new(&mut new_fiducial.position.x)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );
            ui.label(tr!("form-common-input-y"));
            ui.add(
                egui::DragValue::new(&mut new_fiducial.position.y)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );

            ui.label(tr!("form-common-input-copper-diameter"));
            ui.add(
                egui::DragValue::new(&mut new_fiducial.copper_diameter)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );

            ui.label(tr!("form-common-input-mask-diameter"));
            ui.add(
                egui::DragValue::new(&mut new_fiducial.mask_diameter)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );

            if let Some(ratio) = new_fiducial.copper_to_mask_ratio() {
                ui.label(tr!("ratio", {numerator: ratio.numer(), denominator: ratio.denom() }));
            } else {
                ui.label(tr!("ratio-error"));
            }

            if !new_fiducial.eq(&state.new_fiducial) {
                sender
                    .send(PanelTabUiCommand::NewFiducialChanged(new_fiducial))
                    .expect("sent");
            }

            if ui
                .button(tr!("form-common-button-add"))
                .clicked()
            {
                sender
                    .send(PanelTabUiCommand::AddFiducial(new_fiducial))
                    .expect("sent");
            }
        });

        ui.push_id("fiducials", |ui| {
            let initial_size = calculate_initial_table_height(panel_sizing.fiducials.len(), text_height, ui);

            Resize::default()
                .resizable([false, true])
                .default_size(initial_size)
                .min_width(ui.available_width())
                .max_width(ui.available_width())
                .max_height(Self::TABLE_HEIGHT_MAX)
                .show(ui, |ui| {
                    // HACK: search codebase for 'HACK: table-resize-hack' for details
                    egui::Frame::new()
                        .outer_margin(4.0)
                        .show(ui, |ui| {
                            ui.style_mut()
                                .interaction
                                .selectable_labels = false;

                            TableBuilder::new(ui)
                                .striped(true)
                                .resizable(true)
                                .auto_shrink([false, false])
                                .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
                                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                .sense(egui::Sense::click())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::remainder())
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-index"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-x"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-y"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-mask-diameter"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-copper-diameter"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-actions"));
                                    });
                                })
                                .body(|mut body| {
                                    for (fiducial_index, parameters) in panel_sizing
                                        .fiducials
                                        .iter()
                                        .enumerate()
                                    {
                                        let mut new_parameters = parameters.clone();

                                        body.row(text_height, |mut row| {
                                            row.col(|ui| {
                                                ui.label((fiducial_index + 1).to_string());
                                            });

                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut new_parameters.position.x)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut new_parameters.position.y)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut new_parameters.mask_diameter)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut new_parameters.copper_diameter)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });

                                            row.col(|ui| {
                                                if ui
                                                    .button(tr!("form-common-button-delete"))
                                                    .clicked()
                                                {
                                                    sender
                                                        .send(PanelTabUiCommand::DeleteFiducial(fiducial_index))
                                                        .expect("sent");
                                                }
                                            });
                                        });

                                        if !new_parameters.eq(parameters) {
                                            sender
                                                .send(PanelTabUiCommand::UpdateFiducial {
                                                    index: fiducial_index,
                                                    parameters: new_parameters,
                                                })
                                                .expect("sent");
                                        }
                                    }
                                });
                        });
                });
        });
    }

    fn design_configuration_controls(
        panel_sizing: &PanelSizing,
        pcb_overview: &PcbOverview,
        sender: &Sender<PanelTabUiCommand>,
        text_height: f32,
        ui: &mut Ui,
    ) {
        ui.label(tr!("pcb-panel-tab-panel-design-configuration-header"));

        ui.push_id("design_configuration", |ui| {
            let initial_size = calculate_initial_table_height(pcb_overview.designs.len(), text_height, ui);

            Resize::default()
                .resizable([false, true])
                .default_size(initial_size)
                .min_width(ui.available_width())
                .max_width(ui.available_width())
                .max_height(Self::TABLE_HEIGHT_MAX)
                .show(ui, |ui| {
                    // HACK: search codebase for 'HACK: table-resize-hack' for details
                    egui::Frame::new()
                        .outer_margin(4.0)
                        .show(ui, |ui| {
                            ui.style_mut()
                                .interaction
                                .selectable_labels = false;

                            TableBuilder::new(ui)
                                .striped(true)
                                .resizable(true)
                                .auto_shrink([false, false])
                                .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
                                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                .sense(egui::Sense::click())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::remainder())
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-index"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-x-placement-offset"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-y-placement-offset"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-x-gerber-offset"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-y-gerber-offset"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-x-origin"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-y-origin"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-x-size"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-y-size"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-design-name"));
                                    });

                                    // TODO maybe add per-design mirroring?
                                })
                                .body(|mut body| {
                                    for (design_index, design_name) in pcb_overview.designs.iter().enumerate() {
                                        let design_number = design_index + 1;

                                        let Some(mut design_sizing) = panel_sizing
                                            .design_sizings
                                            .get(design_index)
                                            .cloned()
                                        else {
                                            continue;
                                        };

                                        body.row(text_height, |mut row| {
                                            row.col(|ui| {
                                                ui.label(design_number.to_string());
                                            });

                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.gerber_offset.x)
                                                        .range(f64::MIN..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.gerber_offset.y)
                                                        .range(f64::MIN..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.placement_offset.x)
                                                        .range(f64::MIN..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.placement_offset.y)
                                                        .range(f64::MIN..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.origin.x)
                                                        .range(f64::MIN..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.origin.y)
                                                        .range(f64::MIN..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.size.x)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.size.y)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });

                                            row.col(|ui| {
                                                ui.label(design_name.to_string());
                                            });
                                        });

                                        if design_sizing != panel_sizing.design_sizings[design_index] {
                                            sender
                                                .send(PanelTabUiCommand::DesignSizingChanged {
                                                    design_index,
                                                    design_sizing,
                                                })
                                                .expect("sent");
                                        }
                                    }
                                });
                        });
                });
        });
    }

    fn unit_positions_controls(
        panel_sizing: &PanelSizing,
        pcb_overview: &PcbOverview,
        sender: &Sender<PanelTabUiCommand>,
        text_height: f32,
        ui: &mut Ui,
    ) {
        ui.label(tr!("pcb-panel-tab-panel-unit-positions-header"));

        ui.push_id("unit_positions", |ui| {
            let initial_size = calculate_initial_table_height(pcb_overview.units as usize, text_height, ui);

            Resize::default()
                .resizable([false, true])
                .default_size(initial_size)
                .min_width(ui.available_width())
                .max_width(ui.available_width())
                .max_height(Self::TABLE_HEIGHT_MAX)
                .show(ui, |ui| {
                    // HACK: search codebase for 'HACK: table-resize-hack' for details
                    egui::Frame::new()
                        .outer_margin(4.0)
                        .show(ui, |ui| {
                            ui.style_mut()
                                .interaction
                                .selectable_labels = false;

                            TableBuilder::new(ui)
                                .striped(true)
                                .resizable(true)
                                .auto_shrink([false, false])
                                .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
                                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                .sense(egui::Sense::click())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::remainder())
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong(tr!("table-unit-positions-column-index"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-unit-positions-column-x"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-unit-positions-column-y"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-unit-positions-column-rotation"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-unit-positions-column-design-name"));
                                    });
                                })
                                .body(|mut body| {
                                    for pcb_unit_index in 0..pcb_overview.units {
                                        if let Some(assigned_design_index) = pcb_overview
                                            .unit_map
                                            .get(&pcb_unit_index)
                                        {
                                            let Some(mut pcb_unit_positioning) = panel_sizing
                                                .pcb_unit_positionings
                                                .get(pcb_unit_index as usize)
                                                .cloned()
                                            else {
                                                continue;
                                            };

                                            let design_name = &pcb_overview.designs[*assigned_design_index];

                                            body.row(text_height, |mut row| {
                                                row.col(|ui| {
                                                    ui.label((pcb_unit_index + 1).to_string());
                                                });

                                                row.col(|ui| {
                                                    ui.add(
                                                        egui::DragValue::new(&mut pcb_unit_positioning.offset.x)
                                                            .range(0.0..=f64::MAX)
                                                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                            .fixed_decimals(
                                                                defaults::DRAG_SLIDER[&panel_sizing.units]
                                                                    .fixed_decimals,
                                                            ),
                                                    );
                                                });
                                                row.col(|ui| {
                                                    ui.add(
                                                        egui::DragValue::new(&mut pcb_unit_positioning.offset.y)
                                                            .range(0.0..=f64::MAX)
                                                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                            .fixed_decimals(
                                                                defaults::DRAG_SLIDER[&panel_sizing.units]
                                                                    .fixed_decimals,
                                                            ),
                                                    );
                                                });
                                                row.col(|ui| {
                                                    let mut degrees = pcb_unit_positioning
                                                        .rotation
                                                        .to_f64()
                                                        .unwrap_or(0.0);

                                                    ui.add(
                                                        egui::DragValue::new(&mut degrees)
                                                            .range(0.0..=360.0)
                                                            .speed(defaults::DRAG_ANGLE_SPEED)
                                                            .suffix("°")
                                                            .fixed_decimals(defaults::DRAG_ANGLE_FIXED_DECIMALS),
                                                    );

                                                    // wrap round to 0 again.
                                                    let degrees = Decimal::from_f64(degrees).unwrap() % dec!(360);

                                                    pcb_unit_positioning.rotation = degrees;
                                                });

                                                row.col(|ui| {
                                                    ui.label(design_name.to_string());
                                                });
                                            });

                                            if !pcb_unit_positioning
                                                .eq(&panel_sizing.pcb_unit_positionings[pcb_unit_index as usize])
                                            {
                                                sender
                                                    .send(PanelTabUiCommand::UnitPositioningChanged {
                                                        pcb_unit_index,
                                                        pcb_unit_positioning,
                                                    })
                                                    .expect("sent");
                                            }
                                        } else {
                                            body.row(text_height, |mut row| {
                                                row.col(|ui| {
                                                    ui.label((pcb_unit_index + 1).to_string());
                                                });
                                                row.col(|ui| {
                                                    ui.label(tr!("common-value-not-available"));
                                                });
                                                row.col(|ui| {
                                                    ui.label(tr!("common-value-not-available"));
                                                });
                                                row.col(|ui| {
                                                    ui.label(tr!("common-value-not-available"));
                                                });
                                            })
                                        }
                                    }
                                });
                        });
                });
        });
    }

    fn central_panel_content(ui: &mut Ui, gerber_viewer_ui: &mut GerberViewerUi) {
        gerber_viewer_ui.ui(ui, &mut GerberViewerUiContext::default());
    }

    fn update_panel_preview(&mut self) {
        let (Some((panel_sizing, _)), Some((assembly_orientation, _)), Some(pcb_overview)) =
            (&self.panel_sizing, &self.assembly_orientation, &self.pcb_overview)
        else {
            return;
        };

        let mut gerber_viewer_ui = self.gerber_viewer_ui.lock().unwrap();

        if let Ok(commands) = build_panel_preview_commands(
            panel_sizing,
            assembly_orientation,
            &pcb_overview.gerber_offset,
            self.panel_tab_ui_state.pcb_side,
            &pcb_overview.unit_map,
        ) {
            dump_gerber_source(&commands);
            gerber_viewer_ui.clear_layers();
            gerber_viewer_ui.set_panel_sizing(panel_sizing.clone());
            gerber_viewer_ui.set_assembly_orientation(assembly_orientation.clone());
            gerber_viewer_ui.add_layer(
                Some(GerberFileFunction::Other(Some(self.panel_tab_ui_state.pcb_side))),
                commands,
            );
            gerber_viewer_ui.request_center_view();
        } else {
            // TODO show an error message if the gerber preview could not be generated
        }
    }
}

#[derive(Debug, Clone)]
pub enum PanelTabUiCommand {
    None,

    PcbSideChanged(PcbSide),
    AssemblyOrientationChanged(PcbAssemblyOrientation),
    NewFiducialChanged(FiducialParameters),
    AddFiducial(FiducialParameters),
    DeleteFiducial(usize),
    UpdateFiducial {
        index: usize,
        parameters: FiducialParameters,
    },
    SizeChanged(Vector2<f64>),
    EdgeRailsChanged(Dimensions<f64>),
    GerberViewerUiCommand(GerberViewerUiCommand),
    DesignSizingChanged {
        design_index: usize,
        design_sizing: DesignSizing,
    },
    UnitPositioningChanged {
        pcb_unit_index: PcbUnitIndex,
        pcb_unit_positioning: PcbUnitPositioning,
    },

    Apply,
    Reset,

    PanelSizingSaved,
    AssemblyOrientationSaved,
    ApplyPanelSizing(PanelSizing),
    ApplyAssemblyOrientation(PcbAssemblyOrientation),
    RefreshPcbRequested,
}

#[derive(Debug)]
pub enum PanelTabUiAction {
    None,
    ApplyPanelSizing(PanelSizing),
    ApplyAssemblyOrientation(PcbAssemblyOrientation),
    Task(Task<PanelTabUiCommand>),
    UiCommand(PanelTabUiCommand),
    RefreshPcb,
}

#[derive(Debug, Clone, Default)]
pub struct PanelTabUiContext {}

impl UiComponent for PanelTabUi {
    type UiContext<'context> = PanelTabUiContext;
    type UiCommand = PanelTabUiCommand;
    type UiAction = PanelTabUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let (
            Some(pcb_overview),
            Some((panel_sizing, initial_panel_sizing)),
            Some((assembly_orientation, initial_assembly_orientation)),
        ) = (&self.pcb_overview, &self.panel_sizing, &self.assembly_orientation)
        else {
            ui.spinner();
            return;
        };

        egui::Panel::left(ui.id().with("left_panel"))
            .resizable(true)
            .default_size(300.0)
            .show_inside(ui, |ui| {
                // specifically NON-mutable state here
                let state = &self.panel_tab_ui_state;
                let sender = self.component.sender.clone();

                Self::left_panel_content(
                    ui,
                    &panel_sizing,
                    &initial_panel_sizing,
                    state,
                    sender,
                    &pcb_overview,
                    &assembly_orientation,
                    &initial_assembly_orientation,
                );
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            // specifically NON-mutable state here
            let mut gerber_viewer_ui = self.gerber_viewer_ui.lock().unwrap();

            Self::central_panel_content(ui, &mut gerber_viewer_ui);
        });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        debug!("PanelTabUi::update, command: {:?}", command);
        let mut update_panel_preview = false;
        let action = match command {
            PanelTabUiCommand::None => Some(PanelTabUiAction::None),
            PanelTabUiCommand::PcbSideChanged(pcb_side) => {
                let state = &mut self.panel_tab_ui_state;
                state.pcb_side = pcb_side;
                update_panel_preview = true;
                None
            }
            PanelTabUiCommand::SizeChanged(size) => {
                if let Some((panel_sizing, _)) = &mut self.panel_sizing {
                    panel_sizing.size = size;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::EdgeRailsChanged(edge_rails) => {
                if let Some((panel_sizing, _)) = &mut self.panel_sizing {
                    panel_sizing.edge_rails = edge_rails;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::NewFiducialChanged(parameters) => {
                let state = &mut self.panel_tab_ui_state;
                state.new_fiducial = parameters;
                update_panel_preview = true;
                None
            }
            PanelTabUiCommand::AddFiducial(parameters) => {
                if let Some((panel_sizing, _)) = &mut self.panel_sizing {
                    panel_sizing.fiducials.push(parameters);
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::DeleteFiducial(index) => {
                if let Some((panel_sizing, _)) = &mut self.panel_sizing {
                    panel_sizing.fiducials.remove(index);
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::UpdateFiducial {
                index,
                parameters,
            } => {
                if let Some((panel_sizing, _)) = &mut self.panel_sizing {
                    panel_sizing.fiducials[index] = parameters;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::DesignSizingChanged {
                design_index,
                design_sizing,
            } => {
                if let Some((panel_sizing, _)) = &mut self.panel_sizing {
                    panel_sizing.design_sizings[design_index] = design_sizing;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::UnitPositioningChanged {
                pcb_unit_index,
                pcb_unit_positioning,
            } => {
                if let Some((panel_sizing, _)) = &mut self.panel_sizing {
                    panel_sizing.pcb_unit_positionings[pcb_unit_index as usize] = pcb_unit_positioning;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::GerberViewerUiCommand(command) => {
                let mut gerber_viewer_ui = self.gerber_viewer_ui.lock().unwrap();
                let action = gerber_viewer_ui.update(command, &mut GerberViewerUiContext::default());
                match action {
                    None => None,
                    Some(GerberViewerUiAction::None) => None,
                }
            }
            PanelTabUiCommand::Reset => {
                if let (
                    Some((panel_sizing, initial_panel_sizing)),
                    Some((assembly_orientation, initial_assembly_orientation)),
                ) = (&mut self.panel_sizing, &mut self.assembly_orientation)
                {
                    *panel_sizing = initial_panel_sizing.clone();
                    *assembly_orientation = initial_assembly_orientation.clone();
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::Apply => {
                if let (Some((panel_sizing, _)), Some((assembly_orientation, _))) =
                    (&mut self.panel_sizing, &mut self.assembly_orientation)
                {
                    Some(PanelTabUiAction::Task(Task::batch(vec![
                        Task::done(PanelTabUiCommand::ApplyPanelSizing(panel_sizing.clone())),
                        Task::done(PanelTabUiCommand::ApplyAssemblyOrientation(
                            assembly_orientation.clone(),
                        )),
                        Task::done(PanelTabUiCommand::RefreshPcbRequested),
                    ])))
                } else {
                    None
                }
            }
            PanelTabUiCommand::PanelSizingSaved => {
                if let Some((panel_sizing, initial_panel_sizing)) = &mut self.panel_sizing {
                    *initial_panel_sizing = panel_sizing.clone();
                }
                None
            }
            PanelTabUiCommand::AssemblyOrientationChanged(new_assembly_orientation) => {
                if let Some((assembly_orientation, _)) = &mut self.assembly_orientation {
                    *assembly_orientation = new_assembly_orientation;
                }
                update_panel_preview = true;
                None
            }
            PanelTabUiCommand::AssemblyOrientationSaved => {
                if let Some((assembly_orientation, initial_assembly_orientation)) = &mut self.assembly_orientation {
                    *initial_assembly_orientation = assembly_orientation.clone();
                }
                None
            }
            PanelTabUiCommand::ApplyPanelSizing(panel_ui_sizing) => {
                Some(PanelTabUiAction::ApplyPanelSizing(panel_ui_sizing))
            }
            PanelTabUiCommand::ApplyAssemblyOrientation(assembly_orientation) => {
                Some(PanelTabUiAction::ApplyAssemblyOrientation(assembly_orientation))
            }
            PanelTabUiCommand::RefreshPcbRequested => Some(PanelTabUiAction::RefreshPcb),
        };

        if update_panel_preview {
            self.update_panel_preview();
        }

        action
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct PanelTab {}

impl Tab for PanelTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("pcb-panel-tab-label");

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.panel_tab_ui, ui, &mut PanelTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}

fn calculate_initial_table_height(item_count: usize, text_height: f32, ui: &mut Ui) -> Vec2 {
    // FIXME this calculation isn't exact, but it's close enough for now
    //       it should be the size of the resize widget so it contains the table precisely.
    let initial_height = (text_height + 11.0) + ((text_height + 3.0) * item_count as f32);
    let initial_width = ui.available_width();
    let initial_size = egui::Vec2::new(initial_width, initial_height);
    initial_size
}

fn dump_gerber_source(commands: &Vec<Command>) {
    let gerber_source = gerber_commands_to_source(commands);

    debug!("Gerber source:\n{}", gerber_source);
}

fn gerber_commands_to_source(commands: &Vec<Command>) -> String {
    let mut buf = BufWriter::new(Vec::new());
    commands
        .serialize(&mut buf)
        .expect("Could not generate Gerber code");
    let bytes = buf.into_inner().unwrap();
    let gerber_source = String::from_utf8(bytes).unwrap();
    gerber_source
}

trait IntoGerberUnit {
    fn into_gerber_unit(&self) -> gerber_viewer::gerber_types::Unit;
}

impl IntoGerberUnit for Unit {
    fn into_gerber_unit(&self) -> gerber_viewer::gerber_types::Unit {
        match self {
            Unit::Inches => gerber_viewer::gerber_types::Unit::Inches,
            Unit::Millimeters => gerber_viewer::gerber_types::Unit::Millimeters,
        }
    }
}

fn build_panel_preview_commands(
    panel_sizing: &PanelSizing,
    assembly_orientation: &PcbAssemblyOrientation,
    gerber_offset: &Vector2<f64>,
    side: PcbSide,
    unit_map: &HashMap<PcbUnitIndex, DesignIndex>,
) -> Result<Vec<Command>, GerberError> {
    // FUTURE generate multiple, real, gerber layers instead of a 'preview' layer
    //        i.e. 'board outline, top mask, top copper layers, bottom mask, bottom copper layer, v-score/cut (rails)'

    let coordinate_format = CoordinateFormat::new(ZeroOmission::Leading, CoordinateMode::Absolute, 4, 6);

    let mut gerber_builder = GerberBuilder::new()
        .with_units(panel_sizing.units.into_gerber_unit())
        .with_coordinate_format(coordinate_format);

    let orientation = match side {
        PcbSide::Top => &assembly_orientation.top,
        PcbSide::Bottom => &assembly_orientation.bottom,
    };

    match orientation.flip {
        PcbAssemblyFlip::None => {}
        PcbAssemblyFlip::Pitch => {
            gerber_builder.push_command(Command::ExtendedCode(ExtendedCode::MirrorImage(ImageMirroring::B)));
            gerber_builder.push_command(Command::ExtendedCode(ExtendedCode::OffsetImage(ImageOffset {
                a: 0.0,
                b: -panel_sizing.size.y,
            })));
        }
        PcbAssemblyFlip::Roll => {
            gerber_builder.push_command(Command::ExtendedCode(ExtendedCode::MirrorImage(ImageMirroring::A)));
            gerber_builder.push_command(Command::ExtendedCode(ExtendedCode::OffsetImage(ImageOffset {
                a: -panel_sizing.size.x,
                b: 0.0,
            })));
        }
    }

    let orientation_map = HashMap::from([
        (dec!(0.0), ImageRotation::None),
        (dec!(90.0), ImageRotation::CCW_90),
        (dec!(180.0), ImageRotation::CCW_180),
        (dec!(270.0), ImageRotation::CCW_270),
    ]);

    if let Some(rotation) = orientation_map.get(&orientation.rotation) {
        gerber_builder.push_command(Command::ExtendedCode(ExtendedCode::RotateImage(*rotation)));
    }

    gerber_builder.set_interpolation_mode(InterpolationMode::Linear);
    let drawing_aperture_code = gerber_builder.define_aperture(Aperture::Circle(Circle {
        diameter: 0.1,
        hole_diameter: None,
    }));

    gerber_builder.select_aperture(drawing_aperture_code);
    // NOTE the '-' sign here, since the gerber offset is usually negative, we need to apply the same offset which
    //      was used when exporting the gerber files.
    let origin = -gerber_offset.to_position();

    gerber_builder.push_commands(gerber_rectangle_commands(coordinate_format, origin, panel_sizing.size)?);

    //
    // rails
    //
    if panel_sizing.edge_rails.left > 0.0 {
        let start = origin.add_x(panel_sizing.edge_rails.left);
        let end = start.add_y(panel_sizing.size.y);
        let rail_commands = gerber_line_commands(coordinate_format, start, end)?;
        gerber_builder.push_commands(rail_commands);
    }
    if panel_sizing.edge_rails.right > 0.0 {
        let start = origin.add_x(panel_sizing.size.x - panel_sizing.edge_rails.right);
        let end = start.add_y(panel_sizing.size.y);
        let rail_commands = gerber_line_commands(coordinate_format, start, end)?;
        gerber_builder.push_commands(rail_commands);
    }

    if panel_sizing.edge_rails.bottom > 0.0 {
        let start = origin.add_y(panel_sizing.edge_rails.bottom);
        let end = start.add_x(panel_sizing.size.x);
        let rail_commands = gerber_line_commands(coordinate_format, start, end)?;
        gerber_builder.push_commands(rail_commands);
    }

    if panel_sizing.edge_rails.top > 0.0 {
        let start = origin.add_y(panel_sizing.size.y - panel_sizing.edge_rails.top);
        let end = start.add_x(panel_sizing.size.x);
        let rail_commands = gerber_line_commands(coordinate_format, start, end)?;
        gerber_builder.push_commands(rail_commands);
    }

    //
    // units
    //
    for (pcb_unit_index, pcb_unit_positioning) in panel_sizing
        .pcb_unit_positionings
        .iter()
        .enumerate()
    {
        let Some(design_index) = unit_map.get(&(pcb_unit_index as PcbUnitIndex)) else {
            continue;
        };

        let design_sizing = &panel_sizing.design_sizings[*design_index];

        let unit_position = origin
            .add_x(pcb_unit_positioning.offset.x)
            .add_y(pcb_unit_positioning.offset.y);
        let unit_size = design_sizing.size;
        let unit_rotation = pcb_unit_positioning
            .rotation
            .to_f64()
            .unwrap_or(0.0)
            .to_radians();

        let (rotated_origin, rotated_vectors) = make_rotated_box_path(
            &unit_position,
            &unit_size,
            unit_rotation,
            &design_sizing.origin.to_position(),
        );

        // Now call gerber_path_commands with the rotated data
        gerber_builder.push_commands(gerber_path_commands(
            coordinate_format,
            rotated_origin,
            &rotated_vectors,
        )?);
    }

    //
    // fiducials
    //

    for fiducial in &panel_sizing.fiducials {
        let aperture_code = gerber_builder.define_circle_with_hole(fiducial.mask_diameter, fiducial.copper_diameter);
        gerber_builder.select_aperture(aperture_code);
        gerber_builder.push_commands(gerber_point_commands(
            coordinate_format,
            origin + fiducial.position.to_vector(),
        )?);
    }

    Ok(gerber_builder.as_commands())
}

/// rotation is in radians, positive anti-clockwise
fn make_rotated_box_path(
    unit_position: &Point2<f64>,
    unit_size: &Vector2<f64>,
    rotation: f64,
    design_origin: &Point2<f64>,
) -> (Point<f64, 2>, Vec<Vector2<f64>>) {
    // Create path vectors based on unit_size
    // For example, to create a path that forms a rectangle:
    let path_vectors = [
        Vector2::new(unit_size.x, 0.0),  // Move right
        Vector2::new(0.0, unit_size.y),  // Move up
        Vector2::new(-unit_size.x, 0.0), // Move left
        Vector2::new(0.0, -unit_size.y), // Move down, closing the rectangle
    ];

    // Calculate the center of rotation
    // This is unit_position + design_origin
    let rotation_center_x = unit_position.x + design_origin.x;
    let rotation_center_y = unit_position.y + design_origin.y;

    // Create the isometry for rotation around the calculated center:
    // 1. Create a translation from rotation center to origin (0,0)
    // 2. Create a rotation
    // 3. Create a translation back from origin to rotation center
    let translation_to_origin = nalgebra::Translation2::new(-rotation_center_x, -rotation_center_y);
    let rotation = nalgebra::Rotation2::new(rotation);
    let translation_from_origin = nalgebra::Translation2::new(rotation_center_x, rotation_center_y);

    // Combine these transformations into a single isometry
    // The order is important: first translate to origin, then rotate, then translate back
    let isometry = translation_from_origin * rotation * translation_to_origin;

    // Apply the isometry to the origin point to get the rotated origin
    let rotated_origin = isometry.transform_point(unit_position);

    // Apply rotation to the path vectors (direction only, not position)
    // For vectors, we only apply the rotation part, not the translation
    let rotated_vectors: Vec<Vector2<f64>> = path_vectors
        .iter()
        .map(|vector| rotation * vector)
        .collect();
    (rotated_origin, rotated_vectors)
}

mod gerber_util {
    use gerber_viewer::gerber_types::{
        Command, CoordinateFormat, CoordinateNumber, Coordinates, DCode, FunctionCode, GerberError, Operation,
    };
    use nalgebra::{Point2, Vector2};

    #[allow(dead_code)]
    pub fn x_y_to_gerber(x: f64, y: f64, format: CoordinateFormat) -> Result<Option<Coordinates>, GerberError> {
        let x = CoordinateNumber::try_from(x)?;
        let y = CoordinateNumber::try_from(y)?;

        x.gerber(&format)?;
        y.gerber(&format)?;

        Ok(Some(Coordinates {
            x: Some(x),
            y: Some(y),
            format,
        }))
    }

    #[allow(dead_code)]
    pub fn is_valid(value: f64, format: &CoordinateFormat) -> bool {
        let Ok(value) = CoordinateNumber::try_from(value) else {
            return false;
        };

        value.gerber(format).is_ok()
    }

    pub fn gerber_rectangle_commands(
        coordinate_format: CoordinateFormat,
        origin: Point2<f64>,
        size: Vector2<f64>,
    ) -> Result<Vec<Command>, GerberError> {
        Ok(vec![
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Move(x_y_to_gerber(
                origin.x,
                origin.y,
                coordinate_format,
            )?)))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(origin.x + size.x, origin.y, coordinate_format)?,
                None,
            )))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(origin.x + size.x, origin.y + size.y, coordinate_format)?,
                None,
            )))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(origin.x, origin.y + size.y, coordinate_format)?,
                None,
            )))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(origin.x, origin.y, coordinate_format)?,
                None,
            )))),
        ])
    }

    pub fn gerber_path_commands(
        coordinate_format: CoordinateFormat,
        origin: Point2<f64>,
        vectors: &[Vector2<f64>],
    ) -> Result<Vec<Command>, GerberError> {
        if vectors.is_empty() {
            return Ok(vec![]);
        }

        let mut commands = Vec::with_capacity(vectors.len() + 1);

        // Move to the starting point (origin)
        commands.push(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(
            Operation::Move(x_y_to_gerber(origin.x, origin.y, coordinate_format)?),
        ))));

        // Connect all points with line segments
        let mut current_point = origin;
        for vector in vectors {
            let next_point = Point2::new(current_point.x + vector.x, current_point.y + vector.y);
            commands.push(Command::FunctionCode(FunctionCode::DCode(DCode::Operation(
                Operation::Interpolate(x_y_to_gerber(next_point.x, next_point.y, coordinate_format)?, None),
            ))));
            current_point = next_point;
        }

        Ok(commands)
    }

    pub fn gerber_line_commands(
        coordinate_format: CoordinateFormat,
        origin: Point2<f64>,
        end: Point2<f64>,
    ) -> Result<Vec<Command>, GerberError> {
        Ok(vec![
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Move(x_y_to_gerber(
                origin.x,
                origin.y,
                coordinate_format,
            )?)))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(end.x, end.y, coordinate_format)?,
                None,
            )))),
        ])
    }

    pub fn gerber_point_commands(
        coordinate_format: CoordinateFormat,
        origin: Point2<f64>,
    ) -> Result<Vec<Command>, GerberError> {
        Ok(vec![Command::FunctionCode(FunctionCode::DCode(DCode::Operation(
            Operation::Flash(x_y_to_gerber(origin.x, origin.y, coordinate_format)?),
        )))])
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use indoc::indoc;
    use nalgebra::{Point2, Vector2};
    use planner_app::{PcbAssemblyOrientation, PcbSide, Unit};
    use rust_decimal_macros::dec;

    use crate::pcb::tabs::panel_tab::{
        DesignSizing, Dimensions, FiducialParameters, PanelSizing, PcbUnitPositioning, build_panel_preview_commands,
        gerber_commands_to_source,
    };

    // TODO expand this test to cover the Top and Bottom, currently it only checks the output for `PcbSide::Top`
    #[test]
    pub fn test_build_panel_preview_layer() {
        // given
        let panel_sizing = PanelSizing {
            size: Vector2::new(100.0, 80.0),
            edge_rails: Dimensions {
                left: 5.0,
                right: 10.0,
                top: 6.0,
                bottom: 12.0,
            },
            pcb_unit_positionings: vec![
                PcbUnitPositioning {
                    offset: Vector2::new(5.0, 12.0),
                    rotation: dec!(0.0),
                },
                PcbUnitPositioning {
                    offset: Vector2::new(55.0, 12.0),
                    rotation: dec!(0.0),
                },
                PcbUnitPositioning {
                    offset: Vector2::new(5.0, 40.0),
                    rotation: dec!(0.0),
                },
                PcbUnitPositioning {
                    offset: Vector2::new(55.0, 40.0),
                    rotation: dec!(0.0),
                },
            ],
            design_sizings: vec![
                DesignSizing {
                    origin: Default::default(),
                    gerber_offset: Default::default(),
                    placement_offset: Default::default(),
                    size: Vector2::new(30.0, 25.0),
                },
                DesignSizing {
                    origin: Default::default(),
                    gerber_offset: Default::default(),
                    placement_offset: Default::default(),
                    size: Vector2::new(40.0, 20.0),
                },
            ],
            fiducials: vec![
                // bottom row
                FiducialParameters {
                    position: Point2::new(10.0, 12.0 / 2.0),
                    mask_diameter: 2.0,
                    copper_diameter: 1.0,
                },
                FiducialParameters {
                    position: Point2::new(90.0, 12.0 / 2.0),
                    mask_diameter: 2.0,
                    copper_diameter: 1.0,
                },
                // top row
                FiducialParameters {
                    position: Point2::new(20.0, 80.0 - 3.0),
                    mask_diameter: 2.0,
                    copper_diameter: 1.0,
                },
                FiducialParameters {
                    position: Point2::new(90.0, 80.0 - 3.0),
                    mask_diameter: 2.0,
                    copper_diameter: 1.0,
                },
            ],
            units: Unit::Millimeters,
        };

        let unit_map = HashMap::from_iter([(0, 0), (1, 1), (2, 0), (2, 1)]);
        let gerber_offset = Vector2::new(-10.0, -5.0);
        let assembly_orientation = PcbAssemblyOrientation::default();

        // and
        let expected_source = indoc!(
            r#"
            %MOMM*%
            %FSLAX46Y46*%
            %IR0*%
            G01*
            %ADD10C,0.1*%
            D10*
            X10000000Y5000000D02*
            X110000000Y5000000D01*
            X110000000Y85000000D01*
            X10000000Y85000000D01*
            X10000000Y5000000D01*
            X15000000Y5000000D02*
            X15000000Y85000000D01*
            X100000000Y5000000D02*
            X100000000Y85000000D01*
            X10000000Y17000000D02*
            X110000000Y17000000D01*
            X10000000Y79000000D02*
            X110000000Y79000000D01*
            X15000000Y17000000D02*
            X45000000Y17000000D01*
            X45000000Y42000000D01*
            X15000000Y42000000D01*
            X15000000Y17000000D01*
            X65000000Y17000000D02*
            X105000000Y17000000D01*
            X105000000Y37000000D01*
            X65000000Y37000000D01*
            X65000000Y17000000D01*
            X15000000Y45000000D02*
            X55000000Y45000000D01*
            X55000000Y65000000D01*
            X15000000Y65000000D01*
            X15000000Y45000000D01*
            %ADD11C,2X1*%
            D11*
            X20000000Y11000000D03*
            X100000000Y11000000D03*
            X30000000Y82000000D03*
            X100000000Y82000000D03*
        "#
        );

        // when
        let commands = build_panel_preview_commands(
            &panel_sizing,
            &assembly_orientation,
            &gerber_offset,
            PcbSide::Top,
            &unit_map,
        )
        .unwrap();

        // then
        let source = gerber_commands_to_source(&commands);
        println!("{}", source);

        assert_eq!(source, expected_source);
    }
}

#[allow(dead_code)]
mod gerber_builder {
    use std::collections::HashMap;

    use gerber_viewer::gerber_types::{
        Aperture, ApertureDefinition, Circle, Command, CoordinateFormat, DCode, ExtendedCode, FunctionCode, GCode,
        InterpolationMode, Unit,
    };
    use num_traits::FromPrimitive;
    use rust_decimal::Decimal;

    pub struct GerberBuilder {
        commands: Vec<Command>,

        next_aperture_code: i32,
        current_aperture_code: Option<i32>,

        circle_apertures: HashMap<(Decimal, Decimal), i32>,
    }

    impl GerberBuilder {
        pub fn new() -> Self {
            Self {
                commands: Vec::new(),
                current_aperture_code: None,
                next_aperture_code: 10,
                circle_apertures: HashMap::new(),
            }
        }

        pub fn next_aperture_code(&mut self) -> i32 {
            let result = self.next_aperture_code;
            self.next_aperture_code += 1;
            result
        }

        pub fn push_commands(&mut self, commands: Vec<Command>) {
            self.commands.extend(commands);
        }

        pub fn push_command(&mut self, command: Command) {
            self.commands.push(command);
        }

        pub fn with_units(mut self, units: Unit) -> Self {
            self.commands
                .push(Command::ExtendedCode(ExtendedCode::Unit(units)));
            self
        }

        pub fn with_coordinate_format(mut self, coordinate_format: CoordinateFormat) -> Self {
            self.commands
                .push(Command::ExtendedCode(ExtendedCode::CoordinateFormat(coordinate_format)));
            self
        }

        pub fn set_interpolation_mode(&mut self, interpolation_mode: InterpolationMode) {
            self.commands
                .push(Command::FunctionCode(FunctionCode::GCode(GCode::InterpolationMode(
                    interpolation_mode,
                ))))
        }

        pub fn define_aperture(&mut self, aperture: Aperture) -> i32 {
            let code = self.next_aperture_code();
            let definition = ApertureDefinition {
                code,
                aperture,
            };
            self.commands
                .push(Command::ExtendedCode(ExtendedCode::ApertureDefinition(definition)));
            code
        }

        pub fn select_aperture(&mut self, code: i32) {
            match self.current_aperture_code {
                Some(current_code) if code == current_code => {}
                _ => {
                    self.current_aperture_code.replace(code);
                    self.commands
                        .push(Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(code))))
                }
            }
        }

        /// defines a circle with a hole
        /// if the circle is already defined, the previously allocated aperture code is returned
        pub fn define_circle_with_hole(&mut self, outer_diameter: f64, inner_diameter: f64) -> i32 {
            let key = (
                Decimal::from_f64(outer_diameter).unwrap(),
                Decimal::from_f64(inner_diameter).unwrap(),
            );

            if let Some(&code) = self.circle_apertures.get(&key) {
                return code;
            }

            let code = self.define_aperture(Aperture::Circle(Circle {
                diameter: outer_diameter,
                hole_diameter: Some(inner_diameter),
            }));

            self.circle_apertures.insert(key, code);
            code
        }

        pub fn as_commands(self) -> Vec<Command> {
            self.commands
        }
    }
}
