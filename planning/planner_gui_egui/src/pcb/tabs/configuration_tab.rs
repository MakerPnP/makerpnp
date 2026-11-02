use std::borrow::Cow;
use std::cmp::max;
use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::PathBuf;

use eframe::epaint::{Color32, StrokeKind};
use egui::scroll_area::ScrollBarVisibility;
use egui::{Resize, TextEdit, Ui, Widget, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_double_slider::DoubleSlider;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_taffy::taffy::prelude::{auto, fit_content, fr, length, percent, span};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use nalgebra::Vector2;
use planner_app::{DesignIndex, DesignName, GerberFileFunction, PcbOverview, PcbUnitIndex};
use tracing::{debug, trace};
use util::range_utils::{RangeIntoUsize, clamp_inclusive_range};
use validator::{Validate, ValidationError};

use crate::dialogs::manage_gerbers::{ManageGerbersModal, ManagerGerberModalAction, ManagerGerbersModalUiCommand};
use crate::forms::transforms::resize_x_transform;
use crate::forms::{Form, NamedResize, TABLE_TWO_ROWS_HEIGHT, TuiTextButton};
use crate::pcb::tabs::PcbTabContext;
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::tui_container_size;

#[derive(Debug)]
enum ManagerGerbersModalMode {
    Panel,
    Design(DesignIndex),
}

#[derive(Debug)]
pub struct ConfigurationUi {
    pcb_overview: Option<PcbOverview>,

    fields: Value<DesignAssignmentsFields>,
    initial_args: PcbUnitConfigurationArgs,

    manage_gerbers_modal: Option<(ManagerGerbersModalMode, ManageGerbersModal)>,

    pub component: ComponentState<ConfigurationTabUiCommand>,
}

impl ConfigurationUi {
    const TABLE_DEBUG_MODE: bool = false;

    const TABLE_HEIGHT_MAX: f32 = 200.0;

    // IMPORTANT SYNC LAYOUT CHANGES WITH [`unit_assignments_tab.rs`]

    fn show_form(
        &self,
        ui: &mut Ui,
        form: &Form<DesignAssignmentsFields, ConfigurationTabUiCommand>,
        pcb_overview: &PcbOverview,
    ) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        let container_style = || Style {
            padding: length(0.),
            margin: length(0.),
            gap: length(5.),
            ..Default::default()
        };

        tui(
            ui,
            ui.id()
                .with("configure_pcb_form"),
        )
            .reserve_available_width()
            .style(Style {
                align_items: Some(AlignItems::Center),
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: percent(1.),
                    height: auto(),
                },
                padding: length(8.),
                gap: length(8.),
                ..default_style()
            })
            .show(|tui| {
                form.show_fields_vertical(tui, |form, tui| {
                    form.add_field_ui("units", tr!("form-configure-pcb-input-units"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let mut units = fields.units;
                            let response = ui.add(egui::DragValue::new(&mut units).range(1..=u16::MAX));

                            if units != fields.units {
                                sender
                                    .send(ConfigurationTabUiCommand::UnitsChanged(units))
                                    .expect("sent");
                            }

                            response
                        }
                    });


                    form.add_field_tui(
                        "gerber_offset",
                        tr!("form-configure-pcb-input-gerber-offset"),
                        tui,
                        {
                            move |tui: &mut Tui, fields, sender| {
                                let mut gerber_offset_x = fields.gerber_offset.x;
                                let mut gerber_offset_y = fields.gerber_offset.y;

                                //
                                // grid container
                                //
                                tui.style(Style {
                                    flex_grow: 1.0,
                                    display: Display::Grid,
                                    grid_template_columns: vec![fit_content(percent(1.))],
                                    grid_template_rows: vec![fr(1.)],

                                    // ensure items are centered vertically on rows
                                    align_items: Some(AlignItems::Center),
                                    ..default_style()
                                })
                                    .add(|tui| {
                                        //
                                        // row 1
                                        //
                                        tui.style(Style {
                                            display: Display::Flex,
                                            align_content: Some(AlignContent::Stretch),
                                            align_items: Some(AlignItems::Center),
                                            flex_grow: 1.0,
                                            ..container_style()
                                        })
                                            .add(|tui| {
                                                tui.label(tr!("form-common-input-x"));

                                                tui.style(Style {
                                                    flex_grow: 0.0,
                                                    min_size: Size {
                                                        width: length(50.0),
                                                        height: auto(),
                                                    },
                                                    ..default_style()
                                                })
                                                    .ui_add(
                                                        egui::DragValue::new(&mut gerber_offset_x)
                                                            .speed(0.05)
                                                    );

                                                tui.label(tr!("form-common-input-y"));

                                                tui.style(Style {
                                                    flex_grow: 0.0,
                                                    min_size: Size {
                                                        width: length(50.0),
                                                        height: auto(),
                                                    },
                                                    ..default_style()
                                                })
                                                    .ui_add(
                                                        egui::DragValue::new(&mut gerber_offset_y)
                                                            .speed(0.05)
                                                    );
                                            });

                                        //
                                        // row 2
                                        //
                                        tui.style(Style {
                                            display: Display::Flex,
                                            align_content: Some(AlignContent::Stretch),
                                            align_items: Some(AlignItems::Center),
                                            flex_grow: 1.0,
                                            ..container_style()
                                        })
                                            .add(|tui| {
                                                tui.label(tr!("form-configure-pcb-gerber-offset-help"));
                                            });


                                        // end of grid container content
                                    });

                                let gerber_offset = Vector2::new(gerber_offset_x, gerber_offset_y);
                                if fields.gerber_offset.ne(&gerber_offset) {
                                    sender
                                        .send(ConfigurationTabUiCommand::GerberOffsetChanged(gerber_offset))
                                        .expect("sent");
                                }
                            }
                        });

                    form.add_section_tui(
                        "unit_map",
                        tr!("form-configure-pcb-group-unit-map"),
                        tui,
                        move |tui: &mut Tui| {
                            //
                            // design controls row
                            //
                                form.show_fields_vertical(tui, |form, tui| {
                                    tui.style(Style {
                                        flex_grow: 1.0,
                                        display: Display::Flex,
                                        align_content: Some(AlignContent::Stretch),
                                        // FIXME This `span` is only required because the `field_error` call also uses `grid_column: span(2)`, without it the width is ~50% of the horizontal space.
                                        grid_column: span(2),
                                        ..container_style()
                                    })
                                        .add(|tui| {
                                            let design_name_label = tui.style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                                .label(tr!("form-configure-pcb-input-design-name"));

                                            tui.style(Style {
                                                flex_grow: 0.6,
                                                min_size: Size {
                                                    width: length(100.0),
                                                    height: auto(),
                                                },
                                                ..default_style()
                                            })
                                                .ui_add_manual(|ui| {
                                                    let fields = self.fields.lock().unwrap();
                                                    let sender = self.component.sender.clone();

                                                    let mut design_name_clone = fields.design_name.clone();
                                                    let response = TextEdit::singleline(&mut design_name_clone)
                                                        .hint_text(tr!("form-configure-pcb-input-design-name-placeholder"))
                                                        .desired_width(ui.available_width())
                                                        .ui(ui)
                                                        .labelled_by(design_name_label.id);

                                                    if !fields
                                                        .design_name
                                                        .eq(&design_name_clone)
                                                    {
                                                        sender
                                                            .send(ConfigurationTabUiCommand::DesignNameChanged(design_name_clone))
                                                            .expect("sent")
                                                    }
                                                    response
                                                }, resize_x_transform);

                                            let is_design_name_ok = !self.fields.lock().unwrap().design_name.is_empty();

                                            if tui
                                                .style(Style {
                                                    flex_grow: 0.0,
                                                    ..default_style()
                                                })
                                                .enabled_ui(is_design_name_ok)
                                                .text_button(tr!("form-common-button-add"))
                                                .clicked()
                                            {
                                                self.component
                                                    .send(ConfigurationTabUiCommand::AddDesignClicked);
                                            }
                                        });

                                    form.field_error(tui, "design_name");
                                });

                            //
                            // available design variants
                            //
                            tui.style(Style {
                                flex_grow: 1.0,
                                size: Size {
                                    width: percent(1.0),
                                    height: auto(),
                                },
                                ..default_style()
                            })
                                .add(|tui: &mut Tui| {
                                    let available_size = tui_container_size(tui);

                                    tui.ui_finite(|ui: &mut Ui| {
                                        Resize::default()
                                            .resizable([false, true])
                                            .default_size(available_size)
                                            .min_width(available_size.x)
                                            .max_width(available_size.x)
                                            .max_height(Self::TABLE_HEIGHT_MAX)
                                            .show_named(ui, "project-pcb-designs-header", |ui| {
                                                // HACK: search codebase for 'HACK: table-resize-hack' for details
                                                egui::Frame::new()
                                                    .outer_margin(4.0)
                                                    .show(ui, |ui| {
                                                        ui.style_mut().interaction.selectable_labels = false;

                                                        let fields = self.fields.lock().unwrap();

                                                        let text_height = egui::TextStyle::Body
                                                            .resolve(ui.style())
                                                            .size
                                                            .max(ui.spacing().interact_size.y);

                                                        let table_response = TableBuilder::new(ui)
                                                            .striped(true)
                                                            .resizable(true)
                                                            .auto_shrink([false, false])
                                                            .min_scrolled_height(TABLE_TWO_ROWS_HEIGHT)
                                                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                                            .sense(egui::Sense::click())
                                                            .column(Column::auto())
                                                            .column(Column::auto())
                                                            .column(Column::remainder())
                                                            .header(20.0, |mut header| {
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-designs-column-index"));
                                                                });
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-designs-column-actions"));
                                                                });
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-designs-column-name"));
                                                                });
                                                            })
                                                            .body(|mut body| {
                                                                let mut designs_selected_index = fields.designs_selected_index;
                                                                for (
                                                                    row_index,
                                                                    design_name,
                                                                ) in fields
                                                                    .designs
                                                                    .iter()
                                                                    .enumerate()
                                                                {
                                                                    body.row(text_height, |mut row| {
                                                                        let is_selected = matches!(designs_selected_index, Some(selected_index) if selected_index == row_index);
                                                                        let design_exists = matches!(pcb_overview.designs.get(row_index), Some(existing_design_name) if existing_design_name == design_name);

                                                                        row.set_selected(is_selected);

                                                                        row.col(|ui| {
                                                                            ui.label((row_index + 1).to_string());
                                                                        });

                                                                        row.col(|ui| {
                                                                            ui.horizontal(|ui| {
                                                                                ui.add_enabled_ui(design_exists, |ui|{
                                                                                    if ui
                                                                                        .button(tr!("form-configure-pcb-button-design-gerbers"))
                                                                                        .clicked()
                                                                                    {
                                                                                        self.component
                                                                                            .send(ConfigurationTabUiCommand::ManageDesignGerbersClicked {
                                                                                                design_index: row_index,
                                                                                            });
                                                                                    }
                                                                                });
                                                                                if ui
                                                                                    .button(tr!("form-common-button-delete"))
                                                                                    .clicked()
                                                                                {
                                                                                    self.component
                                                                                        .send(ConfigurationTabUiCommand::DeleteDesign {
                                                                                            design_index: row_index,
                                                                                        })
                                                                                }
                                                                            });

                                                                        });

                                                                        row.col(|ui| {
                                                                            ui.label(design_name.to_string());
                                                                        });

                                                                        if row.response().clicked() {
                                                                            match is_selected {
                                                                                true => designs_selected_index = None,
                                                                                false => designs_selected_index = Some(row_index),
                                                                            }
                                                                        }
                                                                    });
                                                                }
                                                                if fields.designs_selected_index != designs_selected_index {
                                                                    self.component.send(ConfigurationTabUiCommand::DesignVariantSelectionChanged(designs_selected_index));
                                                                }
                                                            });
                                                        if Self::TABLE_DEBUG_MODE {
                                                            ui.painter().rect_stroke(table_response.inner_rect, 0.0, (1.0, Color32::CYAN), StrokeKind::Inside);
                                                            ui.painter().rect_stroke(ui.response().rect, 0.0, (1.0, Color32::ORANGE), StrokeKind::Inside);
                                                        }
                                                    });
                                            });
                                    });
                                });

                            //
                            // unit range
                            //

                            form.add_field_tui(
                                "pcb_unit_range",
                                tr!("form-configure-pcb-input-pcb-unit-range"),
                                tui,
                                {
                                    move |tui: &mut Tui, fields, sender| {
                                        let mut pcb_unit_start = fields.pcb_unit_range.start().clone();
                                        let mut pcb_unit_end = fields.pcb_unit_range.end().clone();
                                        let range = 1..=fields.units;
                                        // FIXME due to egui/double slider bugs, a range of x..=x causes a panic, so disable the range controls in this case
                                        let is_range_valid = range.end() > range.start();
                                        // both are number fields with no label of their own.
                                        let name_range_field = |response: &egui::Response, part: String| {
                                            let name = format!("{}: {}", tr!("form-configure-pcb-input-pcb-unit-range"), part);
                                            response
                                                .ctx
                                                .accesskit_node_builder(response.id, |node| node.set_label(name));
                                        };
                                        //trace!("pcb_unit_start: {}, pcb_unit_end: {}, range: {:?}", pcb_unit_start, pcb_unit_start, range);

                                        tui.style(Style {
                                            display: Display::Flex,
                                            align_content: Some(AlignContent::Stretch),
                                            flex_grow: 1.0,
                                            ..container_style()
                                        })
                                            .enabled_ui(is_range_valid)
                                            .add(|tui| {
                                                tui.style(Style {
                                                    flex_grow: 1.0,
                                                    ..default_style()
                                                })
                                                    .ui_add_manual(|ui|{
                                                        // always 0 the first sizing pass
                                                        let available_width = ui.available_width();
                                                        let width = if ui.is_sizing_pass() {
                                                            200.0
                                                        } else {
                                                            available_width
                                                        };
                                                        let response = DoubleSlider::new(
                                                            &mut pcb_unit_start,
                                                            &mut pcb_unit_end,
                                                            range.clone(),
                                                        )
                                                            .separation_distance(0)
                                                            .width(width)
                                                            .ui(ui);

                                                        response
                                                    }, resize_x_transform
                                                    );

                                                let start_response = tui.style(Style {
                                                    flex_grow: 0.0,
                                                    min_size: Size {
                                                        width: length(50.0),
                                                        height: auto(),
                                                    },
                                                    ..default_style()
                                                })
                                                    .ui_add(
                                                        egui::DragValue::new(&mut pcb_unit_start).range(1..=pcb_unit_end)
                                                    );
                                                name_range_field(&start_response, tr!("form-common-input-start"));

                                                let end_response = tui.style(Style {
                                                    flex_grow: 0.0,
                                                    min_size: Size {
                                                        width: length(50.0),
                                                        height: auto(),
                                                    },
                                                    ..default_style()
                                                })
                                                    .ui_add(
                                                        egui::DragValue::new(&mut pcb_unit_end)
                                                            .range(pcb_unit_start..=*(range.end()))
                                                    );
                                                name_range_field(&end_response, tr!("form-common-input-end"));
                                            });

                                        let pcb_unit_range = RangeInclusive::new(pcb_unit_start, pcb_unit_end);

                                        if fields.pcb_unit_range != pcb_unit_range {
                                            sender
                                                .send(ConfigurationTabUiCommand::PcbUnitRangeChanged(pcb_unit_range.clone()))
                                                .expect("sent")
                                        }

                                        let assignment_range_1_based = pcb_unit_range.to_usize_range();
                                        let assignment_range = (assignment_range_1_based.start() - 1)..=(assignment_range_1_based.end() - 1);

                                        let is_design_selected = fields.designs_selected_index.is_some();

                                        if tui
                                            .style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                            .enabled_ui(is_design_selected)
                                            .text_button(tr!("form-common-button-apply-range"))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ConfigurationTabUiCommand::ApplyRangeClicked(
                                                    fields.designs_selected_index.unwrap(),
                                                ));
                                        }

                                        if tui
                                            .style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                            .enabled_ui(is_design_selected)
                                            .text_button(tr!("form-common-button-apply-all"))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ConfigurationTabUiCommand::ApplyAllClicked(
                                                    fields.designs_selected_index.unwrap(),
                                                ));
                                        }

                                        if tui
                                            .style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                            .enabled_ui(is_design_selected)
                                            .text_button(tr!("form-common-button-unassign-from-range"))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ConfigurationTabUiCommand::UnassignFromRange(
                                                    fields.designs_selected_index.unwrap(),
                                                ));
                                        }

                                        let have_assigned_items_in_range = fields.unit_map[assignment_range].iter().any(|assignment| assignment.is_some());

                                        if tui
                                            .style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                            .enabled_ui(have_assigned_items_in_range)
                                            .text_button(tr!("form-common-button-unassign-range"))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ConfigurationTabUiCommand::UnassignRange);
                                        }
                                    }
                                },
                            );

                            //
                            // design assignments
                            //

                            tui.style(Style {
                                flex_grow: 1.0,
                                size: Size {
                                    width: percent(1.0),
                                    height: auto(),
                                },
                                ..container_style()
                            })
                                .add(|tui| {
                                    let available_size = tui_container_size(tui);

                                    tui.ui_finite(|ui: &mut Ui| {
                                        Resize::default()
                                            .resizable([false, true])
                                            .default_size(available_size)
                                            .min_width(available_size.x)
                                            .max_width(available_size.x)
                                            .max_height(Self::TABLE_HEIGHT_MAX)
                                            .show_named(ui, "form-configure-pcb-group-unit-map", |ui| {
                                                // HACK: search codebase for 'HACK: table-resize-hack' for details
                                                egui::Frame::new()
                                                    .outer_margin(4.0)
                                                    .show(ui, |ui| {
                                                        ui.style_mut().interaction.selectable_labels = false;

                                                        let mut fields = self.fields.lock().unwrap();

                                                        let text_height = egui::TextStyle::Body
                                                            .resolve(ui.style())
                                                            .size
                                                            .max(ui.spacing().interact_size.y);

                                                        let table_response = TableBuilder::new(ui)
                                                            .auto_shrink([false, false])
                                                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                                            .striped(true)
                                                            .resizable(true)
                                                            .min_scrolled_height(TABLE_TWO_ROWS_HEIGHT)
                                                            .sense(egui::Sense::click())
                                                            .column(Column::auto())
                                                            .column(Column::remainder())
                                                            .header(20.0, |mut header| {
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-design-assignments-column-pcb-unit"));
                                                                });
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-design-assignments-column-design"));
                                                                });
                                                            })
                                                            .body(|mut body| {
                                                                let mut unit_map_selected_indexes = fields.unit_map_selected_indexes.clone();

                                                                // use 'take' to ensure we only display the ones that should be visible
                                                                for (pcb_unit_index, design_index) in
                                                                    fields.unit_map.iter().take(fields.units as usize).enumerate()
                                                                {
                                                                    body.row(text_height, |mut row| {
                                                                        let is_selected = unit_map_selected_indexes.contains(&pcb_unit_index);
                                                                        row.set_selected(is_selected);

                                                                        row.col(|ui| {
                                                                            ui.label((pcb_unit_index + 1).to_string());
                                                                        });

                                                                        row.col(|ui| {
                                                                            let label = design_index
                                                                                .map(|design_index| fields.designs[design_index].to_string())
                                                                                .unwrap_or(tr!("assignment-unassigned"));
                                                                            ui.label(label);
                                                                        });

                                                                        if row.response().clicked() {
                                                                            match is_selected {
                                                                                true => {
                                                                                    unit_map_selected_indexes.retain(|&x| x != pcb_unit_index)
                                                                                }
                                                                                false => unit_map_selected_indexes.push(pcb_unit_index),
                                                                            }
                                                                        }
                                                                    });
                                                                }
                                                                fields.unit_map_selected_indexes = unit_map_selected_indexes;
                                                            });

                                                        if Self::TABLE_DEBUG_MODE {
                                                            ui.painter().rect_stroke(table_response.inner_rect, 0.0, (1.0, Color32::CYAN), StrokeKind::Inside);
                                                            ui.painter().rect_stroke(ui.response().rect, 0.0, (1.0, Color32::ORANGE), StrokeKind::Inside);
                                                        }
                                                    });
                                            });

                                        ui.response()
                                    });
                                });

                            // If the unit_map was grown, then the last item was selected; then the unit count was
                            // decreased, the map will still be the size after it was grown and the selection
                            // will still contain an entry that is not visible or usable for selection operations.
                            let unit_map_visible_selected_indexes = {
                                let fields = self.fields.lock().unwrap();

                                fields.unit_map_selected_indexes
                                    .iter()
                                    .filter(|index| **index < fields.units as usize)
                                    .cloned()
                                    .collect::<Vec<_>>()
                            };

                            //
                            // button row
                            //

                            form.show_fields_vertical(tui, |_form, tui| {
                                tui.style(Style {
                                    flex_grow: 1.0,
                                    display: Display::Flex,
                                    align_content: Some(AlignContent::Stretch),
                                    flex_direction: FlexDirection::Row,
                                    ..container_style()
                                })
                                    .add(|tui| {
                                        tui.style(Style {
                                            flex_grow: 1.0,
                                            ..container_style()
                                        })
                                            .add(|tui| {
                                                let fields = self.fields.lock().unwrap();
                                                let is_selection_ok = !unit_map_visible_selected_indexes.is_empty();
                                                let is_design_name_ok = fields.designs_selected_index.is_some();
                                                if tui
                                                    .style(Style {
                                                        flex_grow: 1.0,
                                                        ..default_style()
                                                    })
                                                    .enabled_ui(is_selection_ok && is_design_name_ok)
                                                    .text_button(tr!("form-common-button-assign-selected"))
                                                    .clicked()
                                                {
                                                    self.component
                                                        .send(ConfigurationTabUiCommand::AssignSelection(
                                                            fields.designs_selected_index.unwrap(),
                                                            unit_map_visible_selected_indexes.clone(),
                                                        ));
                                                }

                                                let have_assigned_selection = unit_map_visible_selected_indexes.iter().any(|index|{
                                                    let assignment = &fields.unit_map[*index];
                                                    assignment.is_some()
                                                });
                                                if tui
                                                    .style(Style {
                                                        flex_grow: 1.0,
                                                        ..default_style()
                                                    })
                                                    .enabled_ui(have_assigned_selection)
                                                    .text_button(tr!("form-common-button-unassign-selected"))
                                                    .clicked()
                                                {
                                                    self.component
                                                        .send(ConfigurationTabUiCommand::UnassignSelection(
                                                            unit_map_visible_selected_indexes.clone(),
                                                        ));
                                                }

                                                let have_assigned_items = fields.unit_map.iter().any(|assignment| assignment.is_some());
                                                if tui
                                                    .style(Style {
                                                        flex_grow: 1.0,
                                                        ..default_style()
                                                    })
                                                    .enabled_ui(have_assigned_items)
                                                    .text_button(tr!("form-common-button-unassign-all"))
                                                    .clicked()
                                                {
                                                    self.component
                                                        .send(ConfigurationTabUiCommand::UnassignAllClicked);
                                                }

                                            });
                                    });
                            });
                        },
                    );
                });
            });
    }

    fn show_manage_gerbers_modal(&mut self, mode: ManagerGerbersModalMode) {
        let Some(pcb_overview) = &self.pcb_overview else { return };

        let (title, gerbers) = match mode {
            ManagerGerbersModalMode::Panel => ("Panel".to_string(), pcb_overview.pcb_gerbers.clone()),
            ManagerGerbersModalMode::Design(design_index) => {
                let design_name = pcb_overview.designs[design_index].clone();
                let gerbers = pcb_overview.design_gerbers[design_index].clone();
                (design_name.to_string(), gerbers)
            }
        };

        let mut modal = ManageGerbersModal::new(title, gerbers);
        modal
            .component
            .configure_mapper(self.component.sender.clone(), move |command| {
                trace!("manage gerbers modal mapper. command: {:?}", command);
                ConfigurationTabUiCommand::ManageGerbersModalUiCommand(command)
            });

        self.manage_gerbers_modal = Some((mode, modal));
    }

    pub fn new() -> Self {
        Self {
            pcb_overview: None,
            manage_gerbers_modal: None,
            component: Default::default(),
            fields: Value::default(),
            initial_args: Default::default(),
        }
    }

    pub fn reset(&mut self) {
        self.fields = Value::default();
        self.pcb_overview = None;
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        match &mut self.manage_gerbers_modal {
            None => {}
            Some((ManagerGerbersModalMode::Panel, modal)) => {
                modal.update_gerbers(pcb_overview.pcb_gerbers.clone());
            }
            Some((ManagerGerbersModalMode::Design(design_index), modal)) => {
                modal.update_gerbers(pcb_overview.design_gerbers[*design_index].clone());
            }
        }

        let mut fields = self.fields.lock().unwrap();

        fields.update_units(pcb_overview.units);
        fields.designs = pcb_overview.designs.clone();
        fields.gerber_offset = pcb_overview.gerber_offset;

        // preserve the existing elements of the unit map if the new map has a smaller unit range (e.g. existing vec = 0..=10, new map = 0..=5)
        fields.unit_map = (0..max(fields.unit_map.len() as u16, pcb_overview.units))
            .map(|pcb_unit_index| {
                pcb_overview
                    .unit_map
                    .get(&pcb_unit_index)
                    .cloned()
            })
            .collect::<Vec<_>>();

        self.initial_args = fields.as_args();

        self.pcb_overview.replace(pcb_overview);
    }
}

type UnitMap = Vec<Option<DesignIndex>>;

/// The run-time values will be updated from the [`PcbOverview`] view.
#[derive(Clone, Debug, Validate, serde::Deserialize, serde::Serialize)]
#[validate(context = DesignAssignmentsValidationContext)]
pub struct DesignAssignmentsFields {
    /// the number of individual units the PCB has
    #[validate(range(min = 1, max = u16::MAX, code = "form-input-error-range"))]
    units: u16,

    gerber_offset: Vector2<f64>,

    /// allows the user to type in a design name, the current value is used when making assignments or
    /// sizing the unit_map
    design_name: String,

    pcb_unit_range: RangeInclusive<u16>,

    designs: Vec<DesignName>,

    /// index of the vec is the pcb unit index (0-based)
    #[validate(custom(function = "DesignAssignmentsFields::validate_unit_map", use_context))]
    unit_map: UnitMap,

    designs_selected_index: Option<usize>,
    unit_map_selected_indexes: Vec<usize>,
}

impl Default for DesignAssignmentsFields {
    fn default() -> Self {
        const MINIMUM_UNITS: u16 = 1;

        Self {
            units: MINIMUM_UNITS,
            gerber_offset: Vector2::new(0.0, 0.0),
            designs: vec![],
            // See [`Self::grow_unit_map`]
            unit_map: vec![],
            design_name: "".to_string(),

            pcb_unit_range: 1..=1,
            designs_selected_index: None,
            unit_map_selected_indexes: vec![],
        }
    }
}

pub struct DesignAssignmentsValidationContext {
    units: u16,
}

impl DesignAssignmentsFields {
    fn as_args(&self) -> PcbUnitConfigurationArgs {
        // convert from the ui-friendly vector to a space-friendly map
        let unit_map = self
            .unit_map
            .iter()
            .take(self.units as usize)
            .enumerate()
            .filter_map(|(pcb_unit_index, assignment)| {
                assignment.map(|design_index| (pcb_unit_index as PcbUnitIndex, design_index))
            })
            .collect::<BTreeMap<PcbUnitIndex, DesignIndex>>();

        PcbUnitConfigurationArgs {
            units: self.units,
            gerber_offset: self.gerber_offset,
            designs: self.designs.clone(),
            unit_map,
        }
    }

    fn validate_unit_map(
        unit_map: &UnitMap,
        context: &DesignAssignmentsValidationContext,
    ) -> Result<(), ValidationError> {
        // code elsewhere should grow the vector to the amount of units, but never shorten it
        // we take the elements we need when it is submitted
        let len = unit_map.len();
        if len < context.units as usize {
            let mut error = ValidationError::new("form-input-error-map-incorrect-entry-count");
            error.add_param(Cow::from("required"), &context.units);
            error.add_param(Cow::from("actual"), &len);

            return Err(error);
        }

        if Self::is_unit_map_fully_populated_inner(unit_map, context.units as usize) {
            return Err(ValidationError::new("form-input-error-map-unassigned-entries"));
        }

        Ok(())
    }

    fn is_unit_map_fully_populated_inner(unit_map: &UnitMap, units: usize) -> bool {
        unit_map
            .iter()
            .take(units)
            .any(|design| design.is_none())
    }

    /// Grow the unit map but never shrink it.  If we shrink it, we will lose the mappings already
    /// created if the user changes the number of units.
    fn grow_unit_map(&mut self, new_size: usize) {
        if self.unit_map.len() < new_size {
            self.unit_map.resize(new_size, None);
        }
    }

    fn update_units(&mut self, units: u16) {
        let old_limits = 1..=self.units;
        let new_limits = 1..=units;

        let old_range = self.pcb_unit_range.clone();

        self.pcb_unit_range = clamp_inclusive_range(&old_limits, &new_limits, &self.pcb_unit_range);
        debug!(
            "clamped pcb unit range. old_limits: {:?}, new_limits: {:?}, old_range: {:?}, result: {:?}",
            old_limits, new_limits, old_range, self.pcb_unit_range
        );

        self.units = units;

        self.grow_unit_map(units as usize);
    }
}

#[derive(Debug, Clone)]
pub enum ConfigurationTabUiCommand {
    None,
    ManageDesignGerbersClicked {
        design_index: usize,
    },
    ManageGerbersModalUiCommand(ManagerGerbersModalUiCommand),

    UnitsChanged(u16),
    GerberOffsetChanged(Vector2<f64>),
    DesignNameChanged(String),

    PcbUnitRangeChanged(RangeInclusive<u16>),

    /// apply the selected design, specified by the index, and the pcb unit range to the unit map
    ApplyRangeClicked(usize),
    UnassignFromRange(usize),
    ApplyAllClicked(usize),

    UnassignAllClicked,

    UnassignSelection(Vec<usize>),
    AssignSelection(usize, Vec<usize>),
    DesignVariantSelectionChanged(Option<usize>),

    AddDesignClicked,
    UnassignRange,
    Reset,
    Apply,
    ManagePcbGerbersClicked,
    DeleteDesign {
        design_index: usize,
    },
}

#[derive(Debug, Clone)]
pub enum ConfigurationTabUiAction {
    None,
    UpdateGerberFiles {
        path: PathBuf,
        design: Option<DesignName>,
        files: Vec<(PathBuf, Option<GerberFileFunction>)>,
    },
    RemoveGerberFiles {
        path: PathBuf,
        design: Option<DesignName>,
        files: Vec<PathBuf>,
    },
    RefreshGerberFiles {
        path: PathBuf,
        design: Option<DesignName>,
    },
    Reset,
    Apply(PcbUnitConfigurationArgs),
    ApplyGerberFileFunctions {
        path: PathBuf,
        file_functions: Vec<(PathBuf, Option<GerberFileFunction>)>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ConfigurationTabUiContext {}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PcbUnitConfigurationArgs {
    pub units: u16,
    pub gerber_offset: Vector2<f64>,
    pub designs: Vec<DesignName>,
    pub unit_map: BTreeMap<PcbUnitIndex, DesignIndex>,
}

impl UiComponent for ConfigurationUi {
    type UiContext<'context> = ConfigurationTabUiContext;
    type UiCommand = ConfigurationTabUiCommand;
    type UiAction = ConfigurationTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let Some(pcb_overview) = &self.pcb_overview else {
            ui.spinner();
            return;
        };

        ui.label(tr!("pcb-configuration-detail-name", { name: &pcb_overview.name }));

        ui.separator();

        //
        // toolbar
        //

        ui.horizontal(|ui| {
            if ui
                .button(tr!("form-configure-pcb-button-panel-gerbers"))
                .clicked()
            {
                self.component
                    .send(ConfigurationTabUiCommand::ManagePcbGerbersClicked);
            }
        });

        ui.separator();

        //
        // form
        //

        let validation_context = DesignAssignmentsValidationContext {
            units: self.fields.lock().unwrap().units,
        };

        let form = Form::new(&self.fields, &self.component.sender, &validation_context);

        self.show_form(ui, &form, pcb_overview);

        let is_changed = self.fields.lock().unwrap().as_args() != self.initial_args;

        egui::Sides::new().show(
            ui,
            |ui| {
                if ui
                    .add_enabled(is_changed, egui::Button::new(tr!("form-common-button-reset")))
                    .clicked()
                {
                    self.component
                        .send(ConfigurationTabUiCommand::Reset);
                }

                if ui
                    .add_enabled(
                        is_changed && form.is_valid(),
                        egui::Button::new(tr!("form-common-button-apply")),
                    )
                    .clicked()
                {
                    self.component
                        .send(ConfigurationTabUiCommand::Apply);
                }
            },
            |_ui| {},
        );

        //
        // Modals
        //
        if let Some((_mode, modal)) = &self.manage_gerbers_modal {
            modal.ui(ui, &mut ());
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ConfigurationTabUiCommand::None => Some(ConfigurationTabUiAction::None),

            //
            // fields
            //
            ConfigurationTabUiCommand::UnitsChanged(units) => {
                let mut fields = self.fields.lock().unwrap();
                fields.update_units(units);
                None
            }
            ConfigurationTabUiCommand::GerberOffsetChanged(gerber_offset) => {
                let mut fields = self.fields.lock().unwrap();
                fields.gerber_offset = gerber_offset;
                None
            }
            ConfigurationTabUiCommand::DesignNameChanged(name) => {
                self.fields.lock().unwrap().design_name = name;
                None
            }
            ConfigurationTabUiCommand::PcbUnitRangeChanged(value) => {
                self.fields
                    .lock()
                    .unwrap()
                    .pcb_unit_range = value;
                None
            }
            ConfigurationTabUiCommand::DeleteDesign {
                design_index,
            } => {
                let mut fields = self.fields.lock().unwrap();

                debug!("fields (before): {:?}", *fields);
                // unassign design if assigned to units, adjust other indexes in the map if necessary
                for assigned_design_index in fields.unit_map.iter_mut() {
                    if let Some(index) = assigned_design_index {
                        if *index == design_index {
                            *assigned_design_index = None
                        } else if *index > design_index {
                            *index -= 1;
                        }
                    }
                }

                // delete the design
                fields.designs.remove(design_index);
                debug!("fields (after): {:?}", *fields);
                None
            }

            //
            // design assignments
            //
            ConfigurationTabUiCommand::DesignVariantSelectionChanged(designs_selected_index) => {
                let mut fields = self.fields.lock().unwrap();
                fields.designs_selected_index = designs_selected_index;

                if let Some(designs_selected_index) = designs_selected_index {
                    // update the design name field with the name from the selected entry
                    let design_name = &fields.designs[designs_selected_index].clone();

                    fields.design_name = design_name.to_string();
                }
                None
            }

            ConfigurationTabUiCommand::ApplyRangeClicked(design_index) => {
                let mut fields = self.fields.lock().unwrap();
                let assignment_range_1_based = fields.pcb_unit_range.to_usize_range();
                let assignment_range = (assignment_range_1_based.start() - 1)..=(assignment_range_1_based.end() - 1);

                for assigned_design_index in &mut fields.unit_map[assignment_range] {
                    *assigned_design_index = Some(design_index)
                }
                None
            }
            ConfigurationTabUiCommand::UnassignFromRange(design_index) => {
                let mut fields = self.fields.lock().unwrap();
                let assignment_range_1_based = fields.pcb_unit_range.to_usize_range();
                let assignment_range = (assignment_range_1_based.start() - 1)..=(assignment_range_1_based.end() - 1);

                for assigned_design_index in fields
                    .unit_map
                    [assignment_range]
                    .iter_mut()
                    .filter(|assignment|matches!(assignment, Some(assigned_design_index) if *assigned_design_index == design_index))
                {
                    *assigned_design_index = None
                }
                None
            }
            ConfigurationTabUiCommand::UnassignRange => {
                let mut fields = self.fields.lock().unwrap();
                let assignment_range_1_based = fields.pcb_unit_range.to_usize_range();
                let assignment_range = (assignment_range_1_based.start() - 1)..=(assignment_range_1_based.end() - 1);

                for assigned_design_index in fields.unit_map[assignment_range].iter_mut() {
                    *assigned_design_index = None
                }
                None
            }
            ConfigurationTabUiCommand::ApplyAllClicked(design_index) => {
                let mut fields = self.fields.lock().unwrap();

                for assigned_design_index in fields.unit_map.iter_mut() {
                    *assigned_design_index = Some(design_index)
                }
                None
            }
            ConfigurationTabUiCommand::UnassignAllClicked => {
                let mut fields = self.fields.lock().unwrap();

                for assigned_design_index in fields.unit_map.iter_mut() {
                    *assigned_design_index = None
                }
                None
            }
            ConfigurationTabUiCommand::UnassignSelection(unit_map_selected_indexes) => {
                let mut fields = self.fields.lock().unwrap();

                for (_pcb_unit_index, assigned_design_index) in fields
                    .unit_map
                    .iter_mut()
                    .enumerate()
                    .filter(|(pcb_unit_index, _)| unit_map_selected_indexes.contains(pcb_unit_index))
                {
                    *assigned_design_index = None
                }
                None
            }
            ConfigurationTabUiCommand::AssignSelection(design_index, unit_map_selected_indexes) => {
                let mut fields = self.fields.lock().unwrap();

                for (_pcb_unit_index, assigned_design_index) in fields
                    .unit_map
                    .iter_mut()
                    .enumerate()
                    .filter(|(pcb_unit_index, _)| unit_map_selected_indexes.contains(pcb_unit_index))
                {
                    *assigned_design_index = Some(design_index)
                }
                None
            }
            ConfigurationTabUiCommand::AddDesignClicked => {
                let mut fields = self.fields.lock().unwrap();

                let design_name = DesignName::from(fields.design_name.clone().trim());
                if !fields.designs.contains(&design_name) {
                    fields.designs.push(design_name)
                }
                None
            }

            //
            // form submission
            //
            ConfigurationTabUiCommand::Apply => {
                let fields = self.fields.lock().unwrap();

                let args = fields.as_args();

                Some(ConfigurationTabUiAction::Apply(args))
            }
            ConfigurationTabUiCommand::Reset => Some(ConfigurationTabUiAction::Reset),

            //
            // gerber management
            //
            ConfigurationTabUiCommand::ManageDesignGerbersClicked {
                design_index,
            } => {
                let mode = ManagerGerbersModalMode::Design(design_index);
                self.show_manage_gerbers_modal(mode);
                None
            }
            ConfigurationTabUiCommand::ManagePcbGerbersClicked => {
                let mode = ManagerGerbersModalMode::Panel;
                self.show_manage_gerbers_modal(mode);
                None
            }
            ConfigurationTabUiCommand::ManageGerbersModalUiCommand(command) => {
                if let (Some((mode, modal)), Some(pcb_overview)) = (&mut self.manage_gerbers_modal, &self.pcb_overview)
                {
                    let design = match mode {
                        ManagerGerbersModalMode::Panel => None,
                        ManagerGerbersModalMode::Design(design_index) => {
                            let design = pcb_overview.designs[*design_index].clone();
                            Some(design)
                        }
                    };

                    match modal.update(command, &mut ()) {
                        None => None,
                        Some(ManagerGerberModalAction::CloseDialog) => {
                            self.manage_gerbers_modal = None;
                            None
                        }
                        Some(ManagerGerberModalAction::RemoveGerberFiles {
                            files,
                        }) => {
                            debug!("removing gerber file. mode: {:?}, files: {:?}", mode, files);
                            Some(ConfigurationTabUiAction::RemoveGerberFiles {
                                path: pcb_overview.path.clone(),
                                design,
                                files,
                            })
                        }
                        Some(ManagerGerberModalAction::AddGerberFiles {
                            files,
                        }) => {
                            debug!("gerber files picked. mode: {:?}, picked: {:?}", mode, files);
                            let files = files
                                .into_iter()
                                .map(|file| (file, None))
                                .collect();
                            Some(ConfigurationTabUiAction::UpdateGerberFiles {
                                path: pcb_overview.path.clone(),
                                design,
                                files,
                            })
                        }
                        Some(ManagerGerberModalAction::RefreshGerberFiles) => {
                            debug!("refreshing gerber files. mode: {:?}", mode);
                            Some(ConfigurationTabUiAction::RefreshGerberFiles {
                                path: pcb_overview.path.clone(),
                                design,
                            })
                        }
                        Some(ManagerGerberModalAction::ApplyGerberFileFunctions {
                            file_functions,
                        }) => {
                            debug!("applying gerber file functions. file_functions: {:?}", file_functions);
                            Some(ConfigurationTabUiAction::ApplyGerberFileFunctions {
                                path: pcb_overview.path.clone(),
                                file_functions,
                            })
                        }
                    }
                } else {
                    None
                }
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct ConfigurationTab {}

impl Tab for ConfigurationTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("pcb-configuration-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(
            &state.configuration_tab_ui,
            ui,
            &mut ConfigurationTabUiContext::default(),
        );
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
