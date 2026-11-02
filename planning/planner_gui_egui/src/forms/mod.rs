use std::sync::mpsc::Sender;

use egui::{Button, Frame, Response, RichText, TextEdit, Ui, Widget, WidgetInfo, WidgetType};
use egui_i18n::{tr, translate_fluent};
use egui_taffy::taffy::prelude::{fit_content, fr, length, percent, span};
use egui_taffy::taffy::{AlignContent, AlignItems, AlignSelf, Display, FlexDirection, Style};
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

        let label_id = tui
            .style(Style {
                ..default_style()
            })
            .add(|tui| tui.label(label).id);

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
                |ui| ui_builder(ui, self.fields.lock().unwrap(), self.sender.clone()).labelled_by(label_id),
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

/// A button in a `Tui` layout, filling its layout cell.
pub trait TuiTextButton<'r>: TuiBuilderLogic<'r> {
    /// As `TuiBuilderLogic::button`, whose response has no accessibility information, so assistive technology and tests
    /// can't find it.
    fn text_button(self, text: String) -> Response {
        let response = self
            .button(|tui| tui.label(text.clone()))
            .response;

        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, response.enabled(), text.clone()));

        response
    }
}

impl<'r, T: TuiBuilderLogic<'r>> TuiTextButton<'r> for T {}

/// For controls with no label of their own, for example the number fields in a table.
pub trait NamedResponse {
    /// Sets the control's name, for assistive technology and tests.
    fn named(self, name: String) -> Self;
}

impl NamedResponse for Response {
    fn named(self, name: String) -> Self {
        self.ctx
            .accesskit_node_builder(self.id, |node| node.set_label(name));

        self
    }
}

/// What the user did with a [`picker_field`].
pub enum PickerFieldAction {
    Clear,
    Pick,
}

/// A read-only field whose value is chosen using a dialog, with buttons inside the field's frame to clear the value and
/// to choose one.
///
/// The buttons are named after the field for assistive technology, as their text alone doesn't say what they are for.
pub fn picker_field(tui: &mut Tui, label: &str, value: &str) -> Option<PickerFieldAction> {
    let default_style = || Style {
        padding: length(2.),
        gap: length(2.),
        ..Default::default()
    };
    let name_button = |response: &Response, action: String| {
        let name = format!("{label}: {action}");
        response
            .ctx
            .accesskit_node_builder(response.id, |node| node.set_label(name));
    };

    let mut action = None;

    tui.style(Style {
        display: Display::Flex,
        align_content: Some(AlignContent::Stretch),
        flex_grow: 1.0,
        ..default_style()
    })
    .add_with_border(|tui| {
        tui.style(Style {
            flex_grow: 1.0,
            ..default_style()
        })
        .ui_add_manual(
            |ui| {
                // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                let mut value = value.to_string();

                let response = TextEdit::singleline(&mut value)
                    .desired_width(ui.available_width())
                    .interactive(false)
                    .frame(Frame::NONE)
                    .ui(ui);

                response
                    .ctx
                    .accesskit_node_builder(response.id, |node| node.set_label(label));

                response
            },
            resize_x_transform,
        );

        let clear = tui
            .style(Style {
                flex_grow: 0.0,
                ..default_style()
            })
            .ui_add(Button::new("x"));
        name_button(&clear, tr!("form-common-button-clear"));
        if clear.clicked() {
            action = Some(PickerFieldAction::Clear);
        }

        let pick = tui
            .style(Style {
                flex_grow: 0.0,
                ..default_style()
            })
            .ui_add(Button::new("..."));
        name_button(&pick, tr!("form-common-button-choose"));
        if pick.clicked() {
            action = Some(PickerFieldAction::Pick);
        }
    });

    action
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
