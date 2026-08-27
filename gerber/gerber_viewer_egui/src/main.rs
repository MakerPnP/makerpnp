use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eda_units::eda_units::dimension_unit::{
    DimensionUnit, DimensionUnitPoint2, DimensionUnitPoint2Ext, DimensionUnitVector2, DimensionUnitVector2Ext,
    Vector2DimensionUnitExt,
};
use eda_units::eda_units::unit_system::UnitSystem;
use eframe::emath::Vec2;
use eframe::{CreationContext, NativeOptions, egui, run_native};
use egui::style::ScrollStyle;
use egui::{Color32, Context, Frame, Id, Modal, Response, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{Size, Style};
use egui_taffy::{TuiBuilderLogic, taffy, tui};
use epaint::FontFamily;
use gerber::GerberViewState;
use gerber_viewer::gerber_parser::parse;
use gerber_viewer::gerber_parser::{GerberDoc, ParseError};
use gerber_viewer::gerber_types::Unit;
use gerber_viewer::{
    DisplayInfo, GerberLayer, GerberRenderer, Mirroring, RenderConfiguration, draw_crosshair, draw_outline,
    generate_pastel_color,
};
use log::{debug, error, info, trace};
use logging::AppLogItem;
use nalgebra::{Point2, Vector2};
use rfd::FileDialog;
use thiserror::Error;

use self::gerber::LayerViewState;

mod gerber;
mod logging;

type Vector = Vector2<f64>;
type Position = Point2<f64>;

const VECTOR_ZERO: Vector = Vector::new(0.0, 0.0);

const INITIAL_GERBER_AREA_PERCENT: f32 = 0.95;
const DEFAULT_STEP: f64 = 0.05;
const STEP_SPEED: f64 = 0.05;
const STEP_SCALE: f64 = 0.5;

fn main() -> eframe::Result<()> {
    env_logger::init(); // Log to stderr (optional).
    let native_options = NativeOptions::default();
    run_native(
        "Gerber Viewer",
        native_options,
        Box::new(|cc| Ok(Box::new(GerberViewer::new(cc)))),
    )
}
struct GerberViewer {
    state: Arc<Mutex<Option<GerberViewState>>>,
    log: Vec<AppLogItem>,
    coord_input: (String, String),
    unit_system: UnitSystem,

    show_bounding_box_outline: bool,
    show_origin_crosshair: bool,
    show_center_crosshair: bool,

    is_about_modal_open: bool,
    step: f64,
    config: RenderConfiguration,
    display_info: DisplayInfo,
}

impl eframe::App for GerberViewer {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Disable text wrapping
        //
        // egui text layouting tries to utilize minimal width possible
        ctx.global_style_mut(|style| {
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        egui::Panel::top("top_panel").show(ui, |ui| {
            self.render_menu_bar(ui);

            self.render_toolbar(&ctx, ui);
        });

        let panel_fill_color = ctx.global_style().visuals.panel_fill;
        // We just want to get rid of the margin in the panel, but we have to find the right color too...
        let panel_frame = Frame::default()
            .inner_margin(0.0)
            .fill(panel_fill_color);

        egui::Panel::bottom("bottom_panel")
            .resizable(true)
            .default_size(150.0)
            .min_size(80.0)
            .frame(panel_frame)
            .show(ui, |ui| {
                self.bottom_panel_content(ui);
            });

        egui::CentralPanel::default().show(ui, |ui| {
            self.central_panel_content(ui);
        });

        //
        // modals
        //

        if self.is_about_modal_open {
            self.render_about_modal(ui);
        }
    }
}

impl GerberViewer {
    pub fn new(_cc: &CreationContext) -> Self {
        _cc.egui_ctx
            .global_style_mut(|style| style.spacing.scroll = ScrollStyle::solid());
        Self {
            state: Arc::new(Mutex::new(None)),
            log: Vec::new(),
            coord_input: ("0.0".to_string(), "0.0".to_string()),
            config: RenderConfiguration::default(),
            show_bounding_box_outline: true,
            show_origin_crosshair: true,
            show_center_crosshair: true,
            unit_system: UnitSystem::Millimeters,

            is_about_modal_open: false,
            step: DEFAULT_STEP,

            // TODO update the display information based on the current monitor
            display_info: DisplayInfo::new()
                // Example based on an ACER Predator 37" monitor
                .with_dpi(3840.0 / 37.0, 2160.0 / 20.875),
        }
    }

    //
    // gerber handling
    //

    /// FIXME: Blocks main thread when file selector is open
    fn add_layer_files(&mut self) {
        self.open_gerber_file_inner()
            .inspect_err(|e| {
                let message = format!("Error opening file: {:?}", e);
                error!("{}", message);
                self.log
                    .push(AppLogItem::Error(message.to_string()));
            })
            .ok();
    }

    fn open_gerber_file_inner(&mut self) -> Result<(), AppError> {
        let paths = FileDialog::new()
            .add_filter("Gerber Files", &[
                "gbr", "gbl", "gbo", "gbs", "gko", "gto", "gdl", "gtl", "gtp", "gts",
            ])
            .add_filter("All Files", &["*"])
            .pick_files()
            .ok_or(AppError::NoFileSelected)?;

        for path in paths {
            self.add_gerber_layer_from_file(path)?;
        }

        Ok(())
    }

    pub fn add_gerber_layer_from_file(&mut self, path: PathBuf) -> Result<(), AppError> {
        let (gerber_doc, commands) = Self::parse_gerber(&mut self.log, &path)?;

        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.get_or_insert_default();

        let layer_count = state.layers.len();

        let scale = state
            .layers
            .first()
            .map(|(_, _, _, first_layer_doc)| {
                let target_unit_system = UnitSystem::from_gerber_unit(&first_layer_doc.units);
                // scale this layer to match the unit system used by the first layer
                let layer_unit_system = UnitSystem::from_gerber_unit(&gerber_doc.units);
                layer_unit_system.scale_f64_for(target_unit_system)
            })
            .unwrap_or(1.0);

        let color = generate_pastel_color(layer_count as u64);

        let layer = GerberLayer::new(commands);
        let layer_view_state = LayerViewState::new(color, scale);

        state.add_layer(path, layer_view_state, layer, gerber_doc);

        Ok(())
    }

    fn parse_gerber(
        log: &mut Vec<AppLogItem>,
        path: &PathBuf,
    ) -> Result<(GerberDoc, Vec<gerber_viewer::gerber_types::Command>), AppError> {
        let file = File::open(path.clone())
            .inspect_err(|error| {
                let message = format!(
                    "Error parsing gerber file: {}, cause: {}",
                    path.to_str().unwrap(),
                    error
                );
                error!("{}", message);
                log.push(AppLogItem::Error(message.to_string()));
            })
            .map_err(AppError::IoError)?;

        let reader = BufReader::new(file);

        let gerber_doc: GerberDoc = parse(reader).map_err(|(_partial_doc, error)| AppError::ParserError(error))?;

        let log_entries = gerber_doc
            .commands
            .iter()
            .map(|c| match c {
                Ok(command) => AppLogItem::Info(format!("{:?}", command)),
                Err(error) => AppLogItem::Error(format!("{:?}", error)),
            })
            .collect::<Vec<_>>();
        log.extend(log_entries);

        let message = format!("Gerber file parsed successfully. path: {}", path.to_str().unwrap());
        info!("{}", message);
        log.push(AppLogItem::Info(message.to_string()));

        let commands = gerber_doc
            .commands
            .iter()
            .filter_map(|c| match c {
                Ok(command) => Some(command.clone()),
                Err(_) => None,
            })
            .collect::<Vec<gerber_viewer::gerber_parser::gerber_types::Command>>();

        Ok((gerber_doc, commands))
    }

    pub fn reload_all_layer_files(&mut self) {
        let mut state_guard = self.state.lock().unwrap();

        let Some(state) = &mut *state_guard else { return };

        for (path, _layer_state, layer, doc) in state.layers.iter_mut() {
            if let Ok((gerber_doc, commands)) = Self::parse_gerber(&mut self.log, &path) {
                *layer = GerberLayer::new(commands);
                *doc = gerber_doc;
            }
        }
        state.request_bbox_reset();
    }

    pub fn close_all(&mut self) {
        let mut state_guard = self.state.lock().unwrap();

        *state_guard = None;
    }

    //
    // logging
    //

    pub fn clear_log(&mut self) {
        self.log.clear();
    }

    //
    // ui
    //

    fn handle_quit(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn show_about_modal(&mut self) {
        if self.is_about_modal_open {
            return;
        }

        self.is_about_modal_open = true;
    }

    //
    // ui content
    //

    fn central_panel_content(&mut self, ui: &mut Ui) {
        if let Some(state) = &mut *self.state.lock().unwrap() {
            let response = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::drag());
            let viewport = response.rect;

            if state.needs_bbox_update {
                state.update_bbox_from_layers();
            }

            if state.needs_view_fitting {
                state.fit_view(viewport);
            }

            if state.needs_view_centering {
                state.center_view(viewport);
            }

            state
                .ui_state
                .update(ui, &viewport, &response, &mut state.view);

            let bbox_screen_vertices = state
                .bounding_box_vertices
                .iter()
                .map(|position| state.gerber_to_screen_coords(*position))
                .collect::<Vec<_>>();

            trace!(
                "view bbox screen vertices: {:?}, offset: {:?}, scale: {:?}, translation: {:?}",
                bbox_screen_vertices, state.transform.origin, state.view.scale, state.view.translation
            );

            trace!(
                "view: {:?}, view bbox scale: {}, viewport_center: {}, origin_screen_pos: {}",
                state.view,
                INITIAL_GERBER_AREA_PERCENT,
                state.ui_state.center_screen_pos,
                state.ui_state.origin_screen_pos
            );

            let painter = ui.painter().with_clip_rect(viewport);
            for (_, layer_view_state, layer, _doc) in state.layers.iter() {
                if layer_view_state.enabled {
                    let layer_transform = layer_view_state.transform;

                    let mut unit_aligned_layer_transform = layer_transform;
                    unit_aligned_layer_transform.scale *= layer_view_state.unit_system_scale_factor;

                    let transform = unit_aligned_layer_transform.combine(&state.transform);

                    GerberRenderer::new(&self.config, state.view, &transform, layer)
                        .paint_layer(&painter, layer_view_state.color);
                }
            }

            if self.show_origin_crosshair {
                draw_crosshair(&painter, state.ui_state.origin_screen_pos, Color32::BLUE);
            }
            if self.show_center_crosshair {
                draw_crosshair(&painter, state.ui_state.center_screen_pos, Color32::LIGHT_GRAY);
            }

            if self.show_bounding_box_outline && !bbox_screen_vertices.is_empty() {
                draw_outline(&painter, bbox_screen_vertices, Color32::RED);
            }
        } else {
            let default_style = || Style {
                padding: length(8.),
                gap: length(8.),
                ..Default::default()
            };

            tui(ui, ui.id().with("no-file-loaded-panel"))
                .reserve_available_space()
                .style(Style {
                    justify_content: Some(taffy::JustifyContent::Center),
                    align_items: Some(taffy::AlignItems::Center),
                    flex_direction: taffy::FlexDirection::Column,
                    size: Size {
                        width: percent(1.),
                        height: percent(1.),
                    },
                    ..default_style()
                })
                .show(|tui| {
                    tui.style(Style {
                        flex_direction: taffy::FlexDirection::Column,
                        ..default_style()
                    })
                    .add(|tui| {
                        tui.egui_style_mut()
                            .interaction
                            .selectable_labels = false;
                        tui.label(
                            RichText::new("MakerPnP")
                                .size(48.0)
                                .family(FontFamily::Proportional),
                        );
                        tui.label(
                            RichText::new("Gerber Viewer")
                                .size(32.0)
                                .family(FontFamily::Proportional),
                        );
                    });
                });
        }

        let tool_windows_id = ui.id();
        egui_tool_windows::ToolWindows::new().windows(ui, {
            move |builder| {
                builder
                    .add_window(tool_windows_id.with("actions"))
                    .default_pos([20.0, 20.0])
                    .default_size([400.0, 200.0])
                    .show("Layers".to_string(), {
                        let state = self.state.clone();
                        let step = self.step;
                        let unit_system = self.unit_system;

                        move |ui| {
                            Self::layer_view_content(state, ui, step, unit_system);
                        }
                    });
            }
        });
    }

    fn layer_view_content(state: Arc<Mutex<Option<GerberViewState>>>, ui: &mut Ui, step: f64, unit_system: UnitSystem) {
        if let Some(state) = &mut *state.lock().unwrap() {
            let mut request_bbox_reset = false;
            for (path, layer_view_state, _layer, doc) in state.layers.iter_mut() {
                ui.horizontal(|ui| {
                    ui.color_edit_button_srgba(&mut layer_view_state.color);
                    let height = ui.min_size().y;

                    let mut changed = false;
                    let layer_unit_system = UnitSystem::from_gerber_unit(&doc.units);

                    let origin: Vector2<f64> = layer_view_state.transform.origin + layer_view_state.transform.offset;
                    let layer_origin: DimensionUnitVector2 = origin.to_dimension_unit(layer_unit_system);
                    let mut origin = layer_origin.in_unit_system(unit_system);

                    let layer_offset = layer_view_state
                        .transform
                        .offset
                        .to_dimension_unit(layer_unit_system);
                    let mut offset = layer_offset.in_unit_system(unit_system);

                    changed |= ui
                        .add_sized([50.0, height], |ui: &mut Ui| {
                            ui.drag_angle(&mut layer_view_state.transform.rotation)
                        })
                        .changed();

                    changed |= ui
                        .toggle_value(&mut layer_view_state.transform.mirroring.x, "X")
                        .changed();

                    changed |= ui
                        .add_sized([50.0, height], |ui: &mut Ui| {
                            unit_system_drag_value(ui, &mut origin.x, step)
                        })
                        .changed();

                    changed |= ui
                        .add_sized([50.0, height], |ui: &mut Ui| {
                            unit_system_drag_value(ui, &mut offset.x, step)
                        })
                        .changed();

                    changed |= ui
                        .toggle_value(&mut layer_view_state.transform.mirroring.y, "Y")
                        .changed();

                    changed |= ui
                        .add_sized([50.0, height], |ui: &mut Ui| {
                            unit_system_drag_value(ui, &mut origin.y, step)
                        })
                        .changed();

                    changed |= ui
                        .add_sized([50.0, height], |ui: &mut Ui| {
                            unit_system_drag_value(ui, &mut offset.y, step)
                        })
                        .changed();

                    changed |= ui
                        .add_sized([50.0, height], |ui: &mut Ui| {
                            ui.add(
                                egui::DragValue::new(&mut layer_view_state.transform.scale)
                                    .fixed_decimals(4)
                                    .range(0.0..=100.0)
                                    .speed(STEP_SPEED * STEP_SCALE),
                            )
                        })
                        .changed();

                    changed |= ui
                        .checkbox(
                            &mut layer_view_state.enabled,
                            path.file_stem()
                                .unwrap()
                                .to_string_lossy()
                                .to_string(),
                        )
                        .clicked();

                    ui.label(layer_unit_system.display_name());

                    if changed {
                        let layer_origin = origin.to_vector2(layer_unit_system);
                        let layer_offset = offset.to_vector2(layer_unit_system);

                        layer_view_state.transform.offset = layer_offset;
                        layer_view_state.transform.origin = layer_origin - layer_view_state.transform.offset;

                        request_bbox_reset = true;
                    }
                });
            }

            if request_bbox_reset {
                state.request_bbox_reset();
                ui.ctx().request_repaint();
            }
        } else {
            ui.centered_and_justified(|ui| {
                Frame::default().show(ui, |ui| {
                    ui.label("Add layer files...");
                });
            });
        }
    }

    fn bottom_panel_content(&mut self, ui: &mut Ui) {
        let cell_style = Style {
            ..Style::default()
        };

        let light_panel_fill_color = ui
            .ctx()
            .global_style()
            .visuals
            .widgets
            .inactive
            .bg_fill;

        egui_taffy::tui(ui, ui.id().with("bottom_panel_content"))
            .reserve_available_space()
            .style(Style {
                flex_direction: taffy::FlexDirection::Column,
                size: percent(1.),
                justify_items: Some(taffy::AlignItems::FlexStart),
                align_items: Some(taffy::AlignItems::Stretch),
                ..Style::default()
            })
            .show(|tui| {
                tui.style(Style {
                    flex_grow: 0.0,
                    ..cell_style.clone()
                })
                .add(|tui| {
                    tui.ui(|ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Clear").clicked() {
                                self.clear_log();
                            }
                        });
                    });
                });

                tui.separator();

                tui.style(Style {
                    flex_grow: 1.0,
                    min_size: Size {
                        width: auto(),
                        height: length(100.0),
                    },
                    ..cell_style.clone()
                })
                .add(|tui| {
                    tui.ui(|ui| {
                        let text_height = egui::TextStyle::Body
                            .resolve(ui.style())
                            .size
                            .max(ui.spacing().interact_size.y);

                        let text_color = ui.style().visuals.text_color();

                        TableBuilder::new(ui)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .min_scrolled_height(80.0)
                            .column(Column::auto())
                            .column(Column::remainder())
                            .striped(true)
                            .resizable(true)
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.strong("Log Level");
                                });
                                header.col(|ui| {
                                    ui.strong("Message");
                                });
                            })
                            .body(|body| {
                                body.rows(text_height, self.log.len(), |mut row| {
                                    if let Some(log_item) = self.log.get(row.index()) {
                                        let color = match log_item {
                                            AppLogItem::Error(_) => Color32::LIGHT_RED,
                                            AppLogItem::Info(_) => text_color,
                                            AppLogItem::Warning(_) => Color32::LIGHT_YELLOW,
                                        };

                                        row.col(|ui| {
                                            ui.colored_label(color, log_item.level());
                                        });
                                        row.col(|ui| {
                                            // FIXME the width of this column expands when rows with longer messages are scrolled-to.
                                            //       the issue is apparent after loading a gerber file, and then expanding the window horizontally
                                            //       you'll see that table's scrollbar is not on the right of the panel, but somewhere in the middle.
                                            //       if you then scroll the table, the scrollbar will move to the right.
                                            ui.colored_label(color, log_item.message());
                                        });
                                    }
                                });
                            });
                    })
                });

                let style = tui.egui_style_mut();
                style.visuals.panel_fill = light_panel_fill_color;

                // Status bar
                tui.style(Style {
                    flex_grow: 0.0,
                    ..cell_style.clone()
                })
                .add_with_background_color(|tui| {
                    tui.ui(|ui| {
                        ui.horizontal(|ui| {
                            let state = self.state.lock().unwrap();

                            if let Some(state) = &*state {
                                ui.separator();
                                let gerber_units = UnitSystem::from_gerber_unit(&state.layers.first().unwrap().3.units);

                                let (x, y) = state
                                    .ui_state
                                    .cursor_gerber_coords
                                    .map(|position| {
                                        fn format_coord(coord: DimensionUnit) -> String {
                                            format!("{}", coord)
                                        }

                                        let source_point =
                                            DimensionUnitPoint2::new_dim_f64(position.x, position.y, gerber_units);
                                        let target_point = source_point.in_unit_system(self.unit_system);
                                        (format_coord(target_point.x), format_coord(target_point.y))
                                    })
                                    .unwrap_or(("N/A".to_string(), "N/A".to_string()));

                                ui.label(format!("Cursor: X={} Y={}", x, y));
                            } else {
                                ui.label("No file loaded");
                            }
                        });
                    });
                });
            });
    }

    fn render_menu_bar(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("🗁 Add layers...").clicked() {
                    self.add_layer_files();
                }

                let have_state = self.state.lock().unwrap().is_some();

                ui.add_enabled_ui(have_state, |ui| {
                    if ui
                        .button("🔃 Reload all layers")
                        .clicked()
                    {
                        self.reload_all_layer_files();
                    }
                    if ui.button("Close all").clicked() {
                        self.close_all();
                    }
                });
                if ui.button("Quit").clicked() {
                    self.handle_quit(ui.ctx());
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.config.use_unique_shape_colors, "🎉 Unique shape colors");
                ui.checkbox(&mut self.config.use_vertex_numbering, "# Vertex numbering (polygons)");
                ui.checkbox(&mut self.config.use_shape_numbering, "＃ Shape numbering");
                ui.checkbox(&mut self.config.use_shape_bboxes, "◽ Shape bounding boxes");
                ui.checkbox(&mut self.show_bounding_box_outline, "◻ Layer bounding box");
                ui.checkbox(&mut self.show_origin_crosshair, "⌖ Origin Crosshair");
                ui.checkbox(&mut self.show_center_crosshair, "⊞ Center Crosshair");

                ui.menu_button("Units...", |ui| {
                    ui.radio_value(&mut self.unit_system, UnitSystem::Millimeters, "Millimeters");
                    ui.radio_value(&mut self.unit_system, UnitSystem::Inches, "Inches");
                    ui.radio_value(&mut self.unit_system, UnitSystem::Mils, "Mils");
                    ui.radio_value(&mut self.unit_system, UnitSystem::Si, "Si (丝)");
                })
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    self.show_about_modal();
                }
            })
        });
    }

    fn render_toolbar(&mut self, ctx: &Context, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("🗁").clicked() {
                self.add_layer_files();
            }
            let have_state = self.state.lock().unwrap().is_some();
            ui.add_enabled_ui(have_state, |ui| {
                if ui.button("🔃").clicked() {
                    self.reload_all_layer_files();
                }
            });

            ui.separator();

            ui.toggle_value(&mut self.config.use_unique_shape_colors, "🎉")
                .on_hover_text("Unique shape colors");
            ui.toggle_value(&mut self.config.use_vertex_numbering, "#")
                .on_hover_text("Vertex numbering");
            ui.toggle_value(&mut self.config.use_shape_numbering, "＃")
                .on_hover_text("Shape numbering");
            ui.toggle_value(&mut self.config.use_shape_bboxes, "◽")
                .on_hover_text("Shape bounding boxes");
            ui.toggle_value(&mut self.show_bounding_box_outline, "◻")
                .on_hover_text("Layer bounding box");
            ui.toggle_value(&mut self.show_origin_crosshair, "⌖")
                .on_hover_text("Origin crosshair");
            ui.toggle_value(&mut self.show_center_crosshair, "⊞")
                .on_hover_text("Center crosshair");

            ui.separator();

            ui.add_enabled_ui(have_state, |ui| {
                let x_is_valid = self
                    .coord_input
                    .0
                    .parse::<f64>()
                    .is_ok();
                let mut x_editor = egui::TextEdit::singleline(&mut self.coord_input.0)
                    .desired_width(50.0)
                    .hint_text("X");
                if !x_is_valid {
                    x_editor = x_editor.background_color(Color32::DARK_RED);
                }
                ui.add(x_editor);

                let y_is_valid = self
                    .coord_input
                    .1
                    .parse::<f64>()
                    .is_ok();
                let mut y_editor = egui::TextEdit::singleline(&mut self.coord_input.1)
                    .desired_width(50.0)
                    .hint_text("Y");
                if !y_is_valid {
                    y_editor = y_editor.background_color(Color32::DARK_RED);
                }
                ui.add(y_editor);

                let x = self.coord_input.0.parse::<f64>();
                let y = self.coord_input.1.parse::<f64>();

                let enabled = x.is_ok() && y.is_ok();

                let source_unit_system = self.unit_system;

                let mut state = self.state.lock().unwrap();

                ui.add_enabled_ui(enabled, |ui| {
                    if ui.button("⛶ Go To").clicked() {
                        // Safety: ui is disabled unless x and y are `Result::ok`
                        let (x, y) = (x.as_ref().unwrap(), y.as_ref().unwrap());
                        let point = Point2::<DimensionUnit>::new_dim_f64(*x, *y, source_unit_system);

                        state
                            .as_mut()
                            .unwrap()
                            .locate_view(point);
                    }
                    if ui.button("➡ Move by").clicked() {
                        // Safety: ui is disabled unless x and y are `Result::ok`
                        let (x, y) = (x.as_ref().unwrap(), y.as_ref().unwrap());
                        let vector = Vector2::<DimensionUnit>::new_dim_f64(*x, *y, source_unit_system);

                        state
                            .as_mut()
                            .unwrap()
                            .move_view(vector);
                    }
                });

                ui.separator();

                let mut changed = false;

                ui.label("Zoom:");
                let zoom: Option<(f32, Unit)> = state
                    .as_mut()
                    .map(|state| {
                        state
                            .layers
                            .first()
                            .unwrap()
                            .3
                            .units
                            .map(|units| (state, units))
                    })
                    .flatten()
                    .map(|(state, units)| {
                        (
                            state
                                .view
                                .zoom_level_percent(units, &self.display_info),
                            units,
                        )
                    });

                let mut zoom_level = zoom.map_or(100.0, |(zoom, _)| zoom);

                changed |= ui
                    .add(egui::DragValue::new(&mut zoom_level))
                    .changed();

                let mut translation = state
                    .as_ref()
                    .map_or(Vec2::ZERO, |state| state.view.translation);
                ui.label("X:");
                changed |= ui
                    .add(egui::DragValue::new(&mut translation.x))
                    .changed();

                ui.label("Y:");
                changed |= ui
                    .add(egui::DragValue::new(&mut translation.y))
                    .changed();

                ui.separator();

                ui.label("Step:");
                ui.add(
                    egui::DragValue::new(&mut self.step)
                        .fixed_decimals(2)
                        .range(0.0..=100.0)
                        .speed(STEP_SPEED * STEP_SCALE),
                );

                let target_design_origin = state.as_ref().map_or(
                    Vector2::<DimensionUnit>::new_dim_f64(0.0, 0.0, self.unit_system),
                    |state| {
                        let vector = state.transform.origin + state.transform.offset;
                        Vector2::<DimensionUnit>::new_dim_f64(vector.x, vector.y, state.target_unit_system)
                    },
                );
                let mut design_origin = target_design_origin.in_unit_system(self.unit_system);

                ui.label("Rotation/Mirror Origin X:");
                changed |= unit_system_drag_value(ui, &mut design_origin.x, self.step).changed();

                ui.label("Y:");
                changed |= unit_system_drag_value(ui, &mut design_origin.y, self.step).changed();

                let mut rotation = state
                    .as_ref()
                    .map_or(0.0, |state| state.transform.rotation);
                changed |= ui.drag_angle(&mut rotation).changed();

                ui.separator();

                let target_design_offset = state.as_ref().map_or(
                    Vector2::<DimensionUnit>::new_dim_f64(0.0, 0.0, self.unit_system),
                    |state| {
                        let vector = state.transform.offset;
                        Vector2::<DimensionUnit>::new_dim_f64(vector.x, vector.y, state.target_unit_system)
                    },
                );
                let mut design_offset = target_design_offset.in_unit_system(self.unit_system);

                ui.label("Design Offset X:");
                changed |= unit_system_drag_value(ui, &mut design_offset.x, self.step).changed();

                ui.label("Y:");
                changed |= unit_system_drag_value(ui, &mut design_offset.y, self.step).changed();

                ui.separator();

                ui.label("Mirror");
                let mut mirroring = state
                    .as_ref()
                    .map_or(Mirroring::default(), |state| state.transform.mirroring);
                changed |= ui
                    .toggle_value(&mut mirroring.x, "X")
                    .changed();
                changed |= ui
                    .toggle_value(&mut mirroring.y, "Y")
                    .changed();

                if changed {
                    if let Some(state) = &mut *state {
                        match zoom {
                            Some((initial_zoom_level, units)) if zoom_level != initial_zoom_level => {
                                state
                                    .view
                                    .set_zoom_level_percent(zoom_level, units, &self.display_info);
                                state.needs_view_centering = true;
                            }
                            _ => {}
                        }

                        let target_design_origin = design_origin.to_vector2(state.target_unit_system);
                        let target_design_offset = design_offset.to_vector2(state.target_unit_system);

                        let target_design_origin = target_design_origin - target_design_offset;

                        state.view.translation = translation;
                        state.transform.offset = target_design_offset;
                        state.transform.origin = target_design_origin;
                        state.transform.rotation = rotation;
                        state.transform.mirroring = mirroring;

                        debug!("transform: {:?}", state.transform);

                        state.request_bbox_reset();
                    }

                    ctx.request_repaint();
                }

                if ui.button("Reset").clicked() {
                    state.as_mut().unwrap().reset();
                    self.step = DEFAULT_STEP;
                    ctx.request_repaint();
                }

                ui.separator();

                if ui.button("Fit").clicked() {
                    state
                        .as_mut()
                        .unwrap()
                        .request_fit_view();
                    ctx.request_repaint();
                }

                if ui.button("Center").clicked() {
                    state
                        .as_mut()
                        .unwrap()
                        .request_center_view();
                    ctx.request_repaint();
                }
            });
            ui.separator();
        });
    }

    fn render_about_modal(&mut self, ui: &Ui) {
        let modal = Modal::new(Id::new("About")).show(ui.ctx(), |ui| {
            use egui::special_emojis::GITHUB;

            ui.set_width(250.0);

            ui.vertical_centered(|ui| {
                ui.heading("About");
                ui.separator();
                ui.label("MakerPnP - Gerber Viewer");
                ui.label("Written by Dominic Clifton");
                ui.hyperlink_to(format!("💰 ko-fi"), "https://ko-fi.com/dominicclifton");
                ui.separator();
                ui.separator();
                ui.label("A pure-rust Gerber Viewer.");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Source:");
                    ui.hyperlink_to(format!("{GITHUB} MakerPnP"), "https://github.com/MakerPnP/makerpnp");
                });
                ui.separator();
                ui.label("Acknowledgements:");
                ui.horizontal(|ui| {
                    ui.label("UI framework: ");
                    ui.hyperlink_to(format!("{GITHUB} egui"), "https://github.com/emilk/egui");
                });
                ui.horizontal(|ui| {
                    ui.label("Gerber types: ");
                    ui.hyperlink_to(
                        format!("{GITHUB} gerber-types-rs"),
                        "https://github.com/dbrgn/gerber-types-rs",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Gerber parser: ");
                    ui.hyperlink_to(
                        format!("{GITHUB} gerber_parser"),
                        "https://github.com/NemoAndrea/gerber-parser",
                    );
                });

                ui.separator();

                if ui.button("Ok").clicked() {
                    self.is_about_modal_open = false;
                }
            });
        });

        if modal.should_close() {
            self.is_about_modal_open = false;
        }
    }
}

#[derive(Error, Debug)]
enum AppError {
    #[error("No file selected")]
    NoFileSelected,
    #[error("IO Error. cause: {0:?}")]
    IoError(io::Error),

    #[error("Parser error. cause: {0:?}")]
    ParserError(ParseError),
}

fn unit_system_drag_value(ui: &mut egui::Ui, dimension_unit: &mut DimensionUnit, step: f64) -> Response {
    let mut value = dimension_unit.value_f64();

    let response = ui.add(
        egui::DragValue::new(&mut value)
            .fixed_decimals(dimension_unit.precision())
            .speed(step * STEP_SCALE),
    );

    if response.changed() {
        *dimension_unit = DimensionUnit::from_f64(value, dimension_unit.unit_system());
    }

    response
}
