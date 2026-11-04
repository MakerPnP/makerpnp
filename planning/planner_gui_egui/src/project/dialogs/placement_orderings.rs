use egui::{Modal, Ui};
use egui_i18n::tr;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Size, Style};
use egui_taffy::{Tui, tui};
use indexmap::IndexMap;
use planner_app::{PlacementSortingItem, PlacementSortingMode, Reference};
use tracing::debug;
use util::sorting::SortOrder;
use validator::Validate;

use crate::forms::Form;
use crate::i18n::conversions::{placement_sorting_mode_to_i18n_key, sort_order_to_i18n_key};
use crate::sync::Value;
use crate::ui_component::{ComponentState, UiComponent};
use crate::widgets::augmented_list_selector;

#[derive(Debug)]
pub struct PlacementOrderingsModal {
    phase_reference: Reference,

    fields: Value<PlacementOrderingFields>,

    pub component: ComponentState<PlacementOrderingsModalUiCommand>,
}

impl PlacementOrderingsModal {
    pub fn new(phase_reference: Reference, orderings: &[PlacementSortingItem]) -> Self {
        let orderings = orderings
            .iter()
            .map(|item| (item.mode.clone(), item.sort_order.clone()))
            .collect::<Vec<(PlacementSortingMode, SortOrder)>>();

        let orderings: IndexMap<PlacementSortingMode, SortOrder> = IndexMap::from_iter(orderings);

        let fields = PlacementOrderingFields {
            orderings,
        };

        Self {
            phase_reference,
            fields: Value::new(fields),
            component: ComponentState::default(),
        }
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<PlacementOrderingFields, PlacementOrderingsModalUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("placement_orderings_form"))
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
                    form.add_field_tui(
                        "orderings",
                        tr!("form-phase-placement-orderings-input-orderings"),
                        tui,
                        {
                            move |tui: &mut Tui, fields, sender| {
                                let all_items = IndexMap::from([
                                    (
                                        PlacementSortingMode::Area,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::Area)),
                                    ),
                                    (
                                        PlacementSortingMode::FeederReference,
                                        tr!(placement_sorting_mode_to_i18n_key(
                                            &PlacementSortingMode::FeederReference
                                        )),
                                    ),
                                    (
                                        PlacementSortingMode::Height,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::Height)),
                                    ),
                                    (
                                        PlacementSortingMode::Part,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::Part)),
                                    ),
                                    (
                                        PlacementSortingMode::Pcb,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::Pcb)),
                                    ),
                                    (
                                        PlacementSortingMode::PcbUnit,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::PcbUnit)),
                                    ),
                                    (
                                        PlacementSortingMode::PcbUnitXY,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::PcbUnitXY)),
                                    ),
                                    (
                                        PlacementSortingMode::PcbUnitYX,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::PcbUnitYX)),
                                    ),
                                    (
                                        PlacementSortingMode::RefDes,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::RefDes)),
                                    ),
                                ]);

                                fn selected_item_mapper(
                                    (k, v): (&PlacementSortingMode, &SortOrder),
                                ) -> (PlacementSortingMode, (String, SortOrder)) {
                                    let label = format!(
                                        "{} ({})",
                                        tr!(placement_sorting_mode_to_i18n_key(k)),
                                        tr!(sort_order_to_i18n_key(v))
                                    )
                                    .to_string();
                                    (k.clone(), (label, v.clone()))
                                }

                                let added_fn = {
                                    let sender = sender.clone();
                                    move |k: &PlacementSortingMode, v: &SortOrder| {
                                        debug!("add. k: {:?}, v: {:?}", k, v);
                                        sender
                                            .send(PlacementOrderingsModalUiCommand::AddOrdering(k.clone(), v.clone()))
                                            .expect("sent");
                                    }
                                };

                                let removed_fn = {
                                    let sender = sender.clone();
                                    move |k: &PlacementSortingMode, v: &SortOrder| {
                                        debug!("remove. k: {:?}, v: {:?}", k, v);
                                        sender
                                            .send(PlacementOrderingsModalUiCommand::RemoveOrdering(
                                                k.clone(),
                                                v.clone(),
                                            ))
                                            .expect("sent");
                                    }
                                };

                                augmented_list_selector::AugmentedListSelector::show(
                                    tui,
                                    default_style,
                                    &fields.orderings,
                                    &all_items,
                                    selected_item_mapper,
                                    SortOrder::Asc,
                                    Self::sort_order_buttons,
                                    added_fn,
                                    removed_fn,
                                );

                                // end of field
                            }
                        },
                    );
                    // end of fields
                });
                // end of form
            });
    }

    fn sort_order_buttons(ui: &mut Ui, v: SortOrder) -> Option<SortOrder> {
        let mut result = None;

        if ui
            .add(egui::RadioButton::new(
                v == SortOrder::Asc,
                tr!(sort_order_to_i18n_key(&SortOrder::Asc)),
            ))
            .clicked()
        {
            result = Some(SortOrder::Asc);
        }
        if ui
            .add(egui::RadioButton::new(
                v == SortOrder::Desc,
                tr!(sort_order_to_i18n_key(&SortOrder::Desc)),
            ))
            .clicked()
        {
            result = Some(SortOrder::Desc);
        }

        result
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
pub struct PlacementOrderingFields {
    orderings: IndexMap<PlacementSortingMode, SortOrder>,
}

#[derive(Debug, Clone)]
pub enum PlacementOrderingsModalUiCommand {
    Submit,
    Cancel,
    AddOrdering(PlacementSortingMode, SortOrder),
    RemoveOrdering(PlacementSortingMode, SortOrder),
}

#[derive(Debug, Clone)]
pub enum PlacementOrderingsModalAction {
    Submit(PlacementOrderingsArgs),
    CloseDialog,
}

/// Value object
#[derive(Debug, Clone)]
pub struct PlacementOrderingsArgs {
    pub orderings: Vec<PlacementSortingItem>,
}

impl UiComponent for PlacementOrderingsModal {
    type UiContext<'context> = ();
    type UiCommand = PlacementOrderingsModalUiCommand;
    type UiAction = PlacementOrderingsModalAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let modal_id = ui
            .id()
            .with("phase_placement_orderings_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_min_width(400.0);
            ui.set_max_width(ui.ctx().content_rect().width() * 0.5);

            ui.heading(tr!("modal-phase-placement-orderings-title", { phase: self.phase_reference.to_string() }));

            let form = Form::new(&self.fields, &self.component.sender, ());

            self.show_form(ui, &form);

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .button(tr!("form-common-button-cancel"))
                        .clicked()
                    {
                        self.component
                            .send(PlacementOrderingsModalUiCommand::Cancel);
                    }

                    if ui
                        .add_enabled(form.is_valid(), egui::Button::new(tr!("form-common-button-ok")))
                        .clicked()
                    {
                        self.component
                            .send(PlacementOrderingsModalUiCommand::Submit);
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
        debug!("PlacementOrderingsModal command. command: {:?}", command);
        match command {
            PlacementOrderingsModalUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let orderings = fields
                    .orderings
                    .iter()
                    .map(|k_v| k_v.into())
                    .collect::<Vec<_>>();
                let args = PlacementOrderingsArgs {
                    orderings,
                };
                Some(PlacementOrderingsModalAction::Submit(args))
            }
            PlacementOrderingsModalUiCommand::Cancel => Some(PlacementOrderingsModalAction::CloseDialog),
            PlacementOrderingsModalUiCommand::AddOrdering(item, order) => {
                let mut fields = self.fields.lock().unwrap();
                fields.orderings.insert(item, order);

                None
            }
            PlacementOrderingsModalUiCommand::RemoveOrdering(item, _order) => {
                let mut fields = self.fields.lock().unwrap();
                fields.orderings.shift_remove(&item);

                None
            }
        }
    }
}
