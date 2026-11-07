//! The data sources, such as the user's parts, which the stores core has, see [`crate::stores_core`].

pub mod parts_table;
pub mod table;

/// Shows the table in the space left in the `Ui`. An `egui_deferred_table` is as big as the `Ui`'s max rect, from where
/// its next widget goes, so below what's shown above it, such as the data source's file, it would go past the bottom of
/// its container, and its last rows couldn't be scrolled to; in a `Ui` of the space left, it ends at the bottom.
fn in_space_left(ui: &mut egui::Ui, table: impl FnOnce(&mut egui::Ui)) {
    let space_left = ui.available_rect_before_wrap();
    ui.scope_builder(egui::UiBuilder::new().max_rect(space_left), table);
}
