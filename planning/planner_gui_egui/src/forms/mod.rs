use std::sync::mpsc::Sender;

use egui::{Response, RichText, Ui};
use egui_i18n::translate_fluent;
use egui_taffy::taffy::prelude::{fit_content, fr, length, percent, span};
use egui_taffy::taffy::{AlignItems, AlignSelf, Display, FlexDirection, Style};
use egui_taffy::{Tui, TuiBuilderLogic};
use i18n::fluent_argument_helpers::json::build_fluent_args;
use validator::{ValidateArgs, ValidationError, ValidationErrors};

use crate::sync::{Value, ValueGuard};

pub mod validation;

use crate::forms::transforms::resize_x_transform;

/// transient helper
pub struct Form<F, C> {
    validation_errors: Result<(), ValidationErrors>,
    fields: Value<F>,
    sender: Sender<C>,
}

impl<'v_a, F: ValidateArgs<'v_a>, C> Form<F, C> {
    pub fn new(fields: &Value<F>, sender: &Sender<C>, context: F::Args) -> Self {
        let fields = fields.clone();
        let sender = sender.clone();
        let validation_errors = {
            let fields = fields.lock().unwrap();

            fields.validate_with_args(context)
        };

        Self {
            validation_errors,
            fields,
            sender,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.validation_errors.is_ok()
    }

    pub fn show_fields_vertical(&self, tui: &mut Tui, fields: impl FnOnce(&Self, &mut Tui)) {
        let default_style = Self::form_default_style();

        //
        // form fields container
        //
        tui.style(Style {
            flex_direction: FlexDirection::Row,
            align_self: Some(AlignSelf::Stretch),
            ..default_style()
        })
        .add(|tui| {
            //
            // grid container
            //
            tui.style(Style {
                flex_grow: 1.0,
                display: Display::Grid,
                grid_template_columns: vec![fit_content(percent(1.)), fr(1.)],
                grid_template_rows: vec![fr(1.), fr(1.)],

                // ensure items are centered vertically on rows
                align_items: Some(AlignItems::Center),
                ..default_style()
            })
            .add(|tui| {
                fields(&self, tui);
                // end of grid container content
            });

            // end of form fields container content
        });
    }

    pub fn show_fields_horizontal(&self, tui: &mut Tui, fields: impl FnOnce(&Self, &mut Tui)) {
        let default_style = Self::form_default_style();

        //
        // form fields container
        //
        tui.style(Style {
            flex_direction: FlexDirection::Column,
            align_self: Some(AlignSelf::Stretch),
            ..default_style()
        })
        .add(|tui| {
            //
            // grid container
            //
            tui.style(Style {
                flex_grow: 1.0,
                display: Display::Grid,
                grid_template_columns: vec![fr(1.), fr(1.)],
                grid_template_rows: vec![fit_content(percent(1.)), fr(1.)],

                // ensure items are centered vertically on rows
                align_items: Some(AlignItems::Center),
                ..default_style()
            })
            .add(|tui| {
                fields(&self, tui);
                // end of grid container content
            });

            // end of form fields container content
        });
    }

    fn form_default_style() -> fn() -> Style {
        let style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        style
    }

    pub fn add_field_ui(
        &self,
        field_name: &str,
        label: String,
        tui: &mut Tui,
        mut ui_builder: impl FnMut(&mut Ui, ValueGuard<'_, F>, Sender<C>) -> Response,
    ) {
        let default_style = Self::form_default_style();

        tui.style(Style {
            ..default_style()
        })
        .add(|tui| {
            tui.label(label);
        });

        tui.style(Style {
            flex_grow: 1.0,
            ..default_style()
        })
        .add(|tui| {
            tui.style(Style {
                flex_grow: 1.0,
                ..default_style()
            })
            .ui_add_manual(
                |ui| ui_builder(ui, self.fields.lock().unwrap(), self.sender.clone()),
                resize_x_transform,
            );
        });

        Self::field_error_inner(&self.validation_errors, default_style, tui, field_name);
    }

    pub fn add_field_tui(
        &self,
        field_name: &str,
        label: String,
        tui: &mut Tui,
        mut ui_builder: impl FnMut(&mut Tui, ValueGuard<'_, F>, Sender<C>),
    ) {
        let default_style = Self::form_default_style();

        tui.style(Style {
            ..default_style()
        })
        .add(|tui| {
            tui.label(label);
        });

        tui.style(Style {
            flex_grow: 1.0,
            ..default_style()
        })
        .add(|tui| {
            ui_builder(tui, self.fields.lock().unwrap(), self.sender.clone());
        });

        Self::field_error_inner(&self.validation_errors, default_style, tui, field_name);
    }

    /// Add a named section, and the field errors for the section
    ///
    /// This is useful when nesting forms
    ///
    /// ```plaintext
    /// +--------+---------------------+
    /// | label  | +-----------------+ |
    /// |        | | < nested form > | |
    /// |        | | <             > | |
    /// |        | +-----------------+ |
    /// +--------+---------------------+
    /// | errors |
    /// ```
    ///
    pub fn add_section_tui(
        &self,
        field_name: &str,
        label: String,
        tui: &mut Tui,
        mut ui_builder: impl FnMut(&mut Tui),
    ) {
        let default_style = Self::form_default_style();
        let inner_style = Self::form_default_style();

        tui.style(Style {
            ..default_style()
        })
        .add(|tui| {
            tui.label(label);
        });

        //
        // section container
        //
        tui.style(Style {
            flex_direction: FlexDirection::Row,
            align_self: Some(AlignSelf::Stretch),
            ..default_style()
        })
        .add(|tui| {
            tui.style(Style {
                flex_grow: 1.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                ..inner_style()
            })
            .add(|tui| {
                ui_builder(tui);
            });
        });

        Self::field_error_inner(&self.validation_errors, default_style, tui, field_name);
    }

    pub fn field_error(&self, tui: &mut Tui, field_name: &str) {
        let default_style = Self::form_default_style();

        Self::field_error_inner(&self.validation_errors, default_style, tui, field_name);
    }

    fn field_error_inner(
        validation_errors: &Result<(), ValidationErrors>,
        default_style: fn() -> Style,
        tui: &mut Tui,
        field_name: &str,
    ) {
        if let Err(errors) = validation_errors {
            let errs = errors.field_errors();
            if let Some(field_errors) = errs.get(field_name) {
                tui.style(Style {
                    grid_column: span(2),
                    ..default_style()
                })
                .add(|tui| {
                    for field_error in field_errors.iter() {
                        let code = &field_error.code;
                        let params = &field_error.params;

                        let args = build_fluent_args(params);

                        let message = translate_fluent(code, &args);

                        //trace!("field_error: {}", field_error);

                        tui.label(RichText::new(message).color(colors::ERROR));
                    }
                });
            }
        }
    }

    pub fn field_validation_errors(&self, field_name: &str) -> Option<&Vec<ValidationError>> {
        if let Err(errors) = &self.validation_errors {
            let errs = errors.field_errors();
            if let Some(&field_errors) = errs.get(field_name) {
                return Some(field_errors);
            }
        }

        None
    }
}

pub mod transforms {
    use egui::{Response, Ui};
    use egui_taffy::TuiContainerResponse;

    pub fn no_transform(response: TuiContainerResponse<Response>, _ui: &Ui) -> TuiContainerResponse<Response> {
        response
    }

    pub fn resize_x_transform(mut response: TuiContainerResponse<Response>, ui: &Ui) -> TuiContainerResponse<Response> {
        let available_width = ui.available_width();
        response.min_size.x = 100.0;
        response.max_size.x = available_width;
        response
    }
}

mod colors {
    use egui::Color32;

    pub const ERROR: Color32 = Color32::from_rgb(0xcb, 0x63, 0x5d);
}
