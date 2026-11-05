//! Tables that are resized using a handle.

use std::fs;
use std::path::Path;

use crate::fixture::App;
use crate::workflow;

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

/// A project with the three parts in the placements file, and its parts tab showing.
fn open_parts_table(app: &mut App) {
    let folder = app.folder();
    workflow::create_project_with_an_assigned_variant(app, &folder);
    app.double_click_text("Parts");
    app.wait_for_tab("Parts");
    app.wait_for_text("CAP1");
}

/// `FORM-9`
///
/// Regression test for `ISSUE-4`: dragging a row of the Parts, Placements or load-out table moved the table's
/// columns, not the row.
#[test]
fn dragging_a_row_of_a_data_table_moves_the_row_and_not_the_columns() {
    let mut app = App::start();
    open_parts_table(&mut app);
    assert!(app.text_position("CAP1").y < app.text_position("RES1").y);
    let manufacturer_x = app.text_position("CAP_MFR1").x;
    let mpn_x = app.text_position("CAP1").x;
    assert!(manufacturer_x < mpn_x);

    app.drag_row("CAP1", "RES1");

    assert!(
        app.text_position("CAP1").y > app.text_position("RES1").y,
        "the row should have moved"
    );
    assert!(
        app.text_position("CAP_MFR1").x < app.text_position("CAP1").x,
        "the columns should not have moved"
    );
}

/// `FORM-10`
#[test]
fn a_data_table_shows_each_row_once_after_a_row_was_moved_and_rows_were_removed() {
    let mut app = App::start();
    open_parts_table(&mut app);
    app.drag_row("CAP1", "RES1");

    let placements = app
        .folder()
        .join("design_a_variant_a_placements.csv");
    fs::write(placements, workflow::PLACEMENTS_WITH_TWO_PARTS).unwrap();
    app.click_button("Refresh");

    app.wait_for_text_count("CAP1", 0);
    assert_eq!(app.count_text("CONN1"), 1);
    assert_eq!(app.count_text("RES1"), 1);
}
