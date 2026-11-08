//! The errors a tab shows, each in a frame of its own, which the user can dismiss (`FORM-12`, `FORM-13`).
//!
//! A tab has one [`Errors`], whose [`Errors::show`] shows each of its errors where it belongs, unless it's dismissed. A
//! dismissed error stays hidden, while it's the same, until the user next does something in the tab, when the tab calls
//! [`Errors::clear`], so errors that still apply are shown again, even with the same message.

use std::collections::BTreeSet;

use egui::{Align, Color32, Label, Layout, RichText, Stroke, Ui, Vec2};
use egui_i18n::tr;
use egui_material_icons::icons::{ICON_CLOSE, ICON_ERROR};

/// The frame's border, and the icon's colour, a high saturation red.
pub const ERROR_BORDER: Color32 = Color32::from_rgb(220, 0, 0);
/// The frame's background, a faint red, translucent, so it's faint in a light theme and a dark one.
pub const ERROR_BACKGROUND: Color32 = Color32::from_rgba_premultiplied(28, 0, 0, 28);

/// Identifies an error a tab shows: where it's shown, and what it says, so that a dismissed error stays hidden while it's
/// the same, and a different one, there or elsewhere, is shown.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ErrorReference {
    /// Where the error is shown, unique in the tab, such as the data source a panel shows.
    pub place: String,
    pub message: String,
}

impl ErrorReference {
    pub fn new(place: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            place: place.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ErrorsUiCommand {
    /// The error's "Dismiss" button was clicked.
    Dismissed(ErrorReference),
}

/// The errors a tab has dismissed, hidden until the user next does something in the tab (`FORM-13`).
#[derive(Debug, Default)]
pub struct Errors {
    dismissed: BTreeSet<ErrorReference>,
}

impl Errors {
    pub fn update(&mut self, command: ErrorsUiCommand) {
        match command {
            ErrorsUiCommand::Dismissed(reference) => {
                self.dismissed.insert(reference);
            }
        }
    }

    /// The user did something in the tab, so each error dismissed before is shown again if it's still the tab's, as the
    /// action may have caused it again.
    pub fn clear(&mut self) {
        self.dismissed.clear();
    }

    pub fn is_dismissed(&self, reference: &ErrorReference) -> bool {
        self.dismissed.contains(reference)
    }

    /// Shows the error, unless it's dismissed, in a frame as wide as the space it's in, so as wide as a table below it,
    /// which fills that space, and never wider, so it doesn't make the tab scroll sideways: a red border, a faint red
    /// background, an icon, the message, wrapped, and a button to dismiss it, top right (`FORM-12`, `FORM-13`).
    pub fn show(&self, ui: &mut Ui, place: &str, message: &str, send: &dyn Fn(ErrorsUiCommand)) {
        let reference = ErrorReference::new(place, message);
        if self.is_dismissed(&reference) {
            return;
        }

        egui::Frame::new()
            .stroke(Stroke::new(1.5, ERROR_BORDER))
            .fill(ERROR_BACKGROUND)
            .corner_radius(4.0)
            .inner_margin(6.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    ui.label(
                        RichText::new(ICON_ERROR)
                            .color(ERROR_BORDER)
                            .size(18.0),
                    );
                    // the message takes the width the button leaves, so the button is top right, and the frame only
                    // as tall as the message, leaving the space below for what follows, such as a table.
                    let button_width = ui.spacing().interact_size.y + 2.0 * ui.spacing().button_padding.x;
                    let message_width = (ui.available_width() - button_width - ui.spacing().item_spacing.x).max(0.0);
                    ui.allocate_ui_with_layout(Vec2::new(message_width, 0.0), Layout::top_down(Align::Min), |ui| {
                        ui.set_width(message_width);
                        // wrapped, though the tab's style may extend labels, as a process's editor's does.
                        ui.add(Label::new(message).wrap());
                    });

                    let name = tr!("error-button-dismiss");
                    let response = ui
                        .small_button(ICON_CLOSE)
                        .on_hover_text(&name);
                    // for assistive technology and tests, as the icon has no name (`FORM-4`).
                    response
                        .ctx
                        .accesskit_node_builder(response.id, |node| node.set_label(name));
                    if response.clicked() {
                        send(ErrorsUiCommand::Dismissed(reference.clone()));
                    }
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `FORM-13`, a dismissed error stays dismissed while it's the same, a different one, by its message or its place,
    /// isn't, and clearing shows every one again.
    #[test]
    fn a_dismissed_error_stays_dismissed_until_cleared() {
        let mut errors = Errors::default();
        let error = ErrorReference::new("parts", "Missing header.");
        assert!(!errors.is_dismissed(&error));

        errors.update(ErrorsUiCommand::Dismissed(error.clone()));
        assert!(errors.is_dismissed(&error));
        assert!(!errors.is_dismissed(&ErrorReference::new("parts", "Another error.")));
        assert!(!errors.is_dismissed(&ErrorReference::new("part mappings", "Missing header.")));

        errors.clear();
        assert!(!errors.is_dismissed(&error));
    }
}
