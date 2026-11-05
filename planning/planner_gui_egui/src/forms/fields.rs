//! Forms laid out in two columns, without `Tui`: the labels, as wide as the widest label, and the fields, filling the
//! rest of the width, so that each field is as wide as the others, and they're resized with their container.
//!
//! Several grids, such as one for each group of a form's fields, share their labels' width when they're given the same
//! labels, see [`FieldGrid::label_width`], so their fields line up.

use egui::{Align, Grid, Layout, RichText, TextEdit, Ui, Widget, WidgetText};
use egui_i18n::tr;

use crate::forms::NamedResponse;
use crate::forms::colors;

/// A two column grid of labels and fields.
pub struct FieldGrid {
    id: egui::Id,
    label_width: f32,
}

impl FieldGrid {
    /// A grid whose labels' column is `label_width` wide, or wider if a label is.
    pub fn new(id: egui::Id, label_width: f32) -> Self {
        Self {
            id,
            label_width,
        }
    }

    /// The width of the widest of the labels, as shown in the Ui.
    pub fn label_width(ui: &Ui, labels: &[String]) -> f32 {
        labels
            .iter()
            .map(|label| {
                WidgetText::from(label.as_str())
                    .into_galley(
                        ui,
                        Some(egui::TextWrapMode::Extend),
                        f32::INFINITY,
                        egui::TextStyle::Body,
                    )
                    .size()
                    .x
            })
            .fold(0.0, f32::max)
    }

    /// Shows the grid, its rows added with [`FieldRows`].
    pub fn show(self, ui: &mut Ui, add_rows: impl FnOnce(&mut FieldRows)) {
        Grid::new(self.id)
            // with the number of columns, the last fills the rest of the width.
            .num_columns(2)
            .min_col_width(self.label_width)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                let mut rows = FieldRows {
                    ui,
                };
                add_rows(&mut rows);
            });
    }
}

/// The rows of a [`FieldGrid`].
pub struct FieldRows<'ui> {
    ui: &'ui mut Ui,
}

impl FieldRows<'_> {
    /// A row with the label, and the field, filling the rest of the width, which the field, or fields, such as a table
    /// and a button below it, are laid out in from top to bottom.
    pub fn row<R>(&mut self, label: &str, add_field: impl FnOnce(&mut Ui) -> R) -> R {
        self.ui.label(label);
        let inner = self
            .ui
            .vertical(|ui| {
                ui.set_width(ui.available_width());
                add_field(ui)
            })
            .inner;
        self.ui.end_row();
        inner
    }

    /// A row with the messages under the field, such as why its value isn't valid.
    pub fn errors(&mut self, messages: &[String]) {
        if messages.is_empty() {
            return;
        }
        self.ui.label("");
        self.ui.vertical(|ui| {
            for message in messages {
                ui.label(RichText::new(message).color(colors::ERROR));
            }
        });
        self.ui.end_row();
    }

    /// A row with a field that can't be changed, named after its label.
    pub fn read_only(&mut self, label: &str, value: &str) {
        self.row(label, |ui| {
            let mut value = value.to_string();
            TextEdit::singleline(&mut value)
                .interactive(false)
                .desired_width(ui.available_width())
                .ui(ui)
                .named(label.to_string());
        });
    }

    /// A row with a field whose value is chosen with a dialog, see [`picker`].
    pub fn picker(&mut self, label: &str, value: &str) -> Option<PickerAction> {
        self.row(label, |ui| picker(ui, label, value, &[]))
    }
}

/// What the user did with a [`picker`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickerAction {
    Clear,
    Choose,
    /// One of the other actions, by its index.
    Other(usize),
}

/// A field that can't be typed in, whose value is chosen with a dialog, filling the width, with buttons to clear it, to
/// choose it, and the other actions, on the right. The field is named after the label, and each button
/// "`<label>: <action>`", for assistive technology and tests.
pub fn picker(ui: &mut Ui, label: &str, value: &str, others: &[String]) -> Option<PickerAction> {
    let mut action = None;
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        // right to left, so the last first.
        for (index, other) in others.iter().enumerate().rev() {
            if ui
                .button(other)
                .named(format!("{}: {}", label, other))
                .clicked()
            {
                action = Some(PickerAction::Other(index));
            }
        }
        if ui
            .button("...")
            .named(format!("{}: {}", label, tr!("form-common-button-choose")))
            .clicked()
        {
            action = Some(PickerAction::Choose);
        }
        if ui
            .button("x")
            .named(format!("{}: {}", label, tr!("form-common-button-clear")))
            .clicked()
        {
            action = Some(PickerAction::Clear);
        }
        let mut value = value.to_string();
        TextEdit::singleline(&mut value)
            .interactive(false)
            .desired_width(ui.available_width())
            .ui(ui)
            .named(label.to_string());
    });
    action
}
