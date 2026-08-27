use std::collections::HashSet;

use derivative::Derivative;
use eda_units::eda_units::dimension_unit::{DimensionUnitPoint2, DimensionUnitPoint2Ext};
use eda_units::eda_units::unit_system::UnitSystem;
use eframe::epaint::Color32;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use planner_app::{ObjectPath, PanelSizing, PcbOverview, PcbSide, PlacementPositionUnit};
use tracing::trace;

use crate::i18n::conversions::{gerber_file_function_to_i18n_key, pcb_side_to_i18n_key};
use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_components::gerber_viewer_ui::{
    GerberViewerMode, GerberViewerUi, GerberViewerUiAction, GerberViewerUiCommand, GerberViewerUiContext,
    GerberViewerUiInstanceArgs, LayersMap,
};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct GerberViewerTabUi {
    #[derivative(Debug = "ignore")]
    gerber_viewer_ui: GerberViewerUi,

    coord_input: (String, String),
    unit_system: UnitSystem,

    pub component: ComponentState<GerberViewerTabUiCommand>,
}

impl GerberViewerTabUi {
    const TABLE_SCROLL_HEIGHT_MIN: f32 = 40.0;

    pub fn new(args: GerberViewerUiInstanceArgs) -> Self {
        let component: ComponentState<GerberViewerTabUiCommand> = Default::default();

        let mut gerber_viewer_ui = GerberViewerUi::new(args);
        gerber_viewer_ui
            .component
            .configure_mapper(component.sender.clone(), |gerber_viewer_command| {
                trace!("gerber_viewer mapper. command: {:?}", gerber_viewer_command);
                GerberViewerTabUiCommand::GerberViewerUiCommand(gerber_viewer_command)
            });

        Self {
            gerber_viewer_ui,
            component,
            // TODO allow the user to change this
            unit_system: UnitSystem::Millimeters,
            coord_input: Default::default(),
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        self.gerber_viewer_ui
            .update_layers_from_pcb_overview(pcb_overview);
    }

    pub fn update_panel_sizing(&mut self, panel_sizing: PanelSizing) {
        self.gerber_viewer_ui
            .set_panel_sizing(panel_sizing);
    }

    fn show_layers_table(ui: &mut Ui, layers: &LayersMap) {
        ui.style_mut()
            .interaction
            .selectable_labels = false;

        let show_function_column = layers
            .iter()
            .all(|((_, function), _)| function.is_some());

        let show_file_column = layers
            .iter()
            .all(|((path, _), _)| path.is_some());

        let pcb_sides = layers
            .iter()
            .filter_map(|((_, function), _)| {
                function
                    .map(|function| function.pcb_side())
                    .flatten()
            })
            .collect::<HashSet<_>>();

        let show_pcb_side_column = pcb_sides.len() > 1;

        let column_count = [true, show_function_column, show_pcb_side_column, show_file_column]
            .iter()
            .filter(|&&show| show)
            .count();

        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        let mut table_builder = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            //.auto_shrink([false, true])
            .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
            //.scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
            .sense(egui::Sense::click());

        for index in 0..column_count {
            let is_last = index == column_count - 1;

            let column = if is_last { Column::remainder() } else { Column::auto() };
            table_builder = table_builder.column(column);
        }

        table_builder
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong(tr!("table-gerber-viewer-layers-column-index"));
                });
                if show_function_column {
                    header.col(|ui| {
                        ui.strong(tr!("table-gerber-viewer-layers-column-gerber-file-function"));
                    });
                }
                if show_pcb_side_column {
                    header.col(|ui| {
                        ui.strong(tr!("table-gerber-viewer-layers-column-pcb-side"));
                    });
                }
                if show_file_column {
                    header.col(|ui| {
                        ui.strong(tr!("table-gerber-viewer-layers-column-file"));
                    });
                }
            })
            .body(|mut body| {
                for (row_index, ((path, function), _)) in layers.iter().enumerate() {
                    body.row(text_height, |mut row| {
                        row.col(|ui| {
                            ui.label(row_index.to_string());
                        });

                        if show_function_column {
                            row.col(|ui| {
                                if let Some(function) = function {
                                    ui.label(tr!(gerber_file_function_to_i18n_key(function)));
                                }
                            });
                        }

                        if show_pcb_side_column {
                            row.col(|ui| {
                                if let Some(pcb_side) = function
                                    .map(|function| function.pcb_side())
                                    .flatten()
                                {
                                    ui.label(tr!(pcb_side_to_i18n_key(&pcb_side)));
                                }
                            });
                        }

                        if show_file_column {
                            row.col(|ui| {
                                if let Some(path) = path {
                                    ui.label(format!(
                                        "{}",
                                        path.file_name()
                                            .unwrap()
                                            .to_string_lossy()
                                    ));
                                }
                            });
                        }
                    });
                }
            });
    }
}

#[derive(Debug, Clone)]
pub enum GerberViewerTabUiCommand {
    None,
    GerberViewerUiCommand(GerberViewerUiCommand),
    GoToClicked(f64, f64),
    CoordinatesChanged(String, String),
    LocateComponent {
        object_path: ObjectPath,
        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    },
}

#[derive(Debug, Clone)]
pub enum GerberViewerTabUiAction {
    None,
}

#[derive(Debug, Clone)]
pub struct GerberViewerTabUiContext {
    pub args: GerberViewerUiInstanceArgs,
}

impl UiComponent for GerberViewerTabUi {
    type UiContext<'context> = GerberViewerTabUiContext;
    type UiCommand = GerberViewerTabUiCommand;
    type UiAction = GerberViewerTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.gerber_viewer_ui
                .ui(ui, &mut GerberViewerUiContext {});

            //
            // tool windows
            //
            // these must be rendered AFTER the gerber viewer ui otherwise the windows appear behind the gerber layers

            let tool_windows_id = ui.id();
            egui_tool_windows::ToolWindows::new().windows(ui, {
                move |builder| {
                    builder
                        .add_window(tool_windows_id.with("actions"))
                        .default_pos([20.0, 20.0])
                        .default_size([200.0, 60.0])
                        .show(tr!("common-actions"), {
                            let mut x_coord = self.coord_input.0.clone();
                            let mut y_coord = self.coord_input.1.clone();
                            let sender = self.component.sender.clone();

                            move |ui| {
                                ui.horizontal(|ui| {
                                    let mut coordinates_changed = false;

                                    let x_is_valid = x_coord.parse::<f64>().is_ok();

                                    let mut x_editor = egui::TextEdit::singleline(&mut x_coord)
                                        .desired_width(50.0)
                                        .hint_text(tr!("form-common-input-x"));

                                    if !x_is_valid {
                                        x_editor = x_editor.text_color(Color32::RED);
                                    }
                                    coordinates_changed |= ui.add(x_editor).changed();

                                    let y_is_valid = y_coord.parse::<f64>().is_ok();

                                    let mut y_editor = egui::TextEdit::singleline(&mut y_coord)
                                        .desired_width(50.0)
                                        .hint_text(tr!("form-common-input-y"));
                                    if !y_is_valid {
                                        y_editor = y_editor.text_color(Color32::RED);
                                    }
                                    coordinates_changed |= ui.add(y_editor).changed();

                                    let x = x_coord.parse::<f64>();
                                    let y = y_coord.parse::<f64>();

                                    let enabled = x.is_ok() && y.is_ok();

                                    if coordinates_changed {
                                        sender
                                            .send(GerberViewerTabUiCommand::CoordinatesChanged(x_coord, y_coord))
                                            .expect("sent");
                                    }

                                    ui.add_enabled_ui(enabled, |ui| {
                                        if ui
                                            .button(format!("⛶ {}", tr!("pcb-gerber-viewer-input-go-to")))
                                            .clicked()
                                        {
                                            // Safety: ui is disabled unless x and y are `Result::ok`
                                            sender
                                                .send(GerberViewerTabUiCommand::GoToClicked(x.unwrap(), y.unwrap()))
                                                .expect("sent");
                                        }
                                    });
                                });
                            }
                        });

                    builder
                        .add_window(tool_windows_id.with("layers"))
                        .default_pos([20.0, 80.0])
                        .default_size([200.0, 200.0])
                        .show(tr!("pcb-gerber-viewer-layers-window-title"), {
                            let layers_binding = self.gerber_viewer_ui.layers();
                            move |ui| {
                                let layers_map = layers_binding.lock().unwrap();

                                Self::show_layers_table(ui, &layers_map);
                            }
                        });
                }
            });
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            GerberViewerTabUiCommand::None => Some(GerberViewerTabUiAction::None),
            GerberViewerTabUiCommand::GerberViewerUiCommand(command) => {
                let action = self
                    .gerber_viewer_ui
                    .update(command, &mut GerberViewerUiContext {});
                match action {
                    None => None,
                    Some(GerberViewerUiAction::None) => Some(GerberViewerTabUiAction::None),
                }
            }
            GerberViewerTabUiCommand::GoToClicked(x, y) => {
                let point = DimensionUnitPoint2::new_dim_f64(x, y, self.unit_system);
                self.gerber_viewer_ui
                    .component
                    .send(GerberViewerUiCommand::LocateView(point));
                None
            }
            GerberViewerTabUiCommand::CoordinatesChanged(x, y) => {
                self.coord_input = (x.to_string(), y.to_string());
                None
            }
            GerberViewerTabUiCommand::LocateComponent {
                object_path,
                pcb_side,
                design_position,
                unit_position,
            } => {
                if matches!(context.args.pcb_side, Some(candidate_pcb_side) if candidate_pcb_side.ne(&pcb_side)) {
                    return None;
                }

                trace!(
                    "Locate Component. object_path: {}, pcb_side: {:?}, design_position: {:?}, unit_position: {:?}",
                    object_path, pcb_side, design_position, unit_position
                );

                let position = match context.args.mode {
                    GerberViewerMode::Panel => unit_position,
                    GerberViewerMode::Design(_) => design_position,
                };

                let coords = position.coords;

                self.gerber_viewer_ui
                    .component
                    .send(GerberViewerUiCommand::ShowPlacementMarker(position));
                self.gerber_viewer_ui
                    .component
                    .send(GerberViewerUiCommand::LocateView(coords));
                None
            }
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct GerberViewerTab {
    pub(crate) args: GerberViewerUiInstanceArgs,
}

impl GerberViewerTab {
    pub fn new(args: GerberViewerUiInstanceArgs) -> Self {
        Self {
            args,
        }
    }
}

impl Tab for GerberViewerTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let title = match self.args.mode {
            GerberViewerMode::Panel => tr!("pcb-gerber-viewer-tab-label-panel"),
            // TODO improve the tab title by using the design name, not the index
            GerberViewerMode::Design(design_index) => {
                tr!("pcb-gerber-viewer-tab-label-design", { index:  design_index })
            }
        };

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(instance) = state
            .gerber_viewer_tab_uis
            .get(&self.args)
        else {
            ui.spinner();
            return;
        };

        UiComponent::ui(instance, ui, &mut GerberViewerTabUiContext {
            args: self.args.clone(),
        });
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
