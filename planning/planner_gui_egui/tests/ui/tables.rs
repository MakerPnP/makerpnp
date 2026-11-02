//! Tables that are resized using a handle.

use std::path::Path;

use crate::fixture::App;

/// A header of 20, and two rows of 18, each followed by a spacing of 3.
const HEADER_AND_TWO_ROWS: f32 = 20.0 + 3.0 + 2.0 * (18.0 + 3.0);

fn create_pcb(app: &mut App, folder: &Path) {
    app.click_button("New PCB");
    app.type_into("PCB name", "panel_a");
    app.set_number("Units", 4.0);
    app.when_a_file_dialog_opens_answer(Some(folder.to_path_buf()));
    app.click_button("Directory: Choose");
    app.wait_for_enabled_button("Ok");
    app.click_button("Ok");
    app.wait_for_tab("Configuration");
}

/// `FORM-6`, `FORM-7`
#[test]
fn resizing_a_table_makes_the_rows_that_did_not_fit_reachable() {
    let mut app = App::start();
    let folder = app.folder();
    create_pcb(&mut app, &folder);
    app.type_into("Design name", "design_a");
    app.wait_for_enabled_button("Add");
    app.click_button("Add");
    app.wait_for_text("design_a");
    app.click_text("design_a");
    app.wait_for_enabled_button("Apply range");

    // unit 4 doesn't fit in the table yet, so this does nothing, clicks are handled in the same frame.
    app.click_text("4");
    app.wait_until_button_is_disabled("Assign selected");

    app.drag_resize_handle("Unit mapping: Resize", 100.0);

    app.click_text("4");
    app.wait_for_enabled_button("Assign selected");
}

/// `FORM-8`
#[test]
fn a_table_cannot_be_made_smaller_than_its_header_and_two_rows() {
    let mut app = App::start();
    let folder = app.folder();
    create_pcb(&mut app, &folder);
    app.type_into("Design name", "design_a");
    app.wait_for_enabled_button("Add");
    app.click_button("Add");
    app.wait_for_text("design_a");
    app.click_text("design_a");
    app.wait_for_enabled_button("Apply range");

    app.drag_resize_handle("Unit mapping: Resize", -1000.0);

    let height = app.table_height("PCB Unit", "Unit mapping: Resize");
    assert!(height >= HEADER_AND_TWO_ROWS, "the table's height was {height}");

    // unit 2, there is only one design, so its number appears only in this table.
    app.click_text("2");
    app.wait_for_enabled_button("Assign selected");
}

/// `FORM-6`
#[test]
fn the_tables_of_a_pcb_have_resize_handles() {
    let mut app = App::start();
    let folder = app.folder();
    create_pcb(&mut app, &folder);

    app.wait_for_resize_handle("Designs: Resize");
    app.wait_for_resize_handle("Unit mapping: Resize");

    app.type_into("Design name", "design_a");
    app.wait_for_enabled_button("Add");
    app.click_button("Add");
    app.wait_for_text("design_a");
    app.click_text("design_a");
    app.wait_for_enabled_button("Apply all");
    app.click_button("Apply all");
    app.wait_for_enabled_button("Apply");
    app.click_button("Apply");
    app.wait_for_enabled_button("Save");

    app.double_click_text("Panelization");
    app.wait_for_tab("Panelization");

    app.wait_for_resize_handle("Fiducials: Resize");
    app.wait_for_resize_handle("Design configuration: Resize");
    app.wait_for_resize_handle("Unit positions: Resize");
}

/// `FORM-6`
#[test]
fn the_tables_of_a_project_have_resize_handles() {
    let mut app = App::start();
    let project = app.given_a_project_file("job1.mpnp.json");
    app.when_a_file_dialog_opens_answer(Some(project));
    app.click_button("Open project");
    app.wait_for_tab("Overview");

    // the Issues tab opens on top of it.
    app.click_tab("Overview");

    app.wait_for_resize_handle("Phases: Resize");
    app.wait_for_resize_handle("PCBs: Resize");
}
