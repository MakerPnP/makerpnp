//! What can and can't be changed as a phase progresses, and placements' phases chosen in the Placements table.

use std::fs;

use planner_app::{Event, PlacementOperation, Reference, SetOrClearAction};
use planner_gui_egui::planner_app_core::PlannerCoreService;
use regex::Regex;

use crate::fixture::App;
use crate::workflow::*;

/// `PLACEMENTS-4`
#[test]
fn choosing_a_phase_in_a_placements_phase_cell_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    // "J1" is on the bottom, so isn't offered the top phase.
    app.choose_in_data_table_cell_combo_box("J1", "Phase", "bottom_1", &["top_1"]);
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_placements_assigned_to_a_phase_using_the_core(&mut core, &folder);
    update(&mut core, Event::AssignPlacementsToPhase {
        phase: Reference::from_raw_str("bottom_1"),
        operation: SetOrClearAction::Set,
        placements: Regex::new("^pcb=1::unit=1::ref_des=J1$").unwrap(),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PLACEMENTS-4`
#[test]
fn choosing_none_in_a_placements_phase_cell_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    // "R1" is on the top, so isn't offered the bottom phase.
    app.choose_in_data_table_cell_combo_box("R1", "Phase", "None", &["bottom_1"]);
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_placements_assigned_to_a_phase_using_the_core(&mut core, &folder);
    update(&mut core, Event::AssignPlacementsToPhase {
        phase: Reference::from_raw_str("top_1"),
        operation: SetOrClearAction::Clear,
        placements: Regex::new("^pcb=1::unit=1::ref_des=R1$").unwrap(),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `OPERATIONS-1`
#[test]
fn a_phase_cannot_be_started_until_the_phases_before_it_are_complete() {
    let mut app = App::start();
    let folder = app.folder();

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    app.double_click_text_containing("top_1 (");
    app.wait_for_combo_box_value("load_pcbs: core::load_pcbs", "Pending");

    // its tasks are shown, with no actions.
    app.double_click_text_containing("bottom_1 (");
    app.wait_until("the top phase's tasks were gone", |app| {
        !app.combo_box_labels()
            .iter()
            .any(|label| label.contains("load_pcbs"))
    });
    app.wait_until("its tasks were shown", |app| app.count_text("Pending") > 0);
    let tasks = app
        .combo_box_labels()
        .into_iter()
        .filter(|label| label.contains(": "))
        .collect::<Vec<_>>();
    assert!(tasks.is_empty(), "{tasks:?}");
}

/// `OPERATIONS-2`
#[test]
fn placements_cannot_be_recorded_before_their_phase_allows_it() {
    let mut app = App::start();
    let folder = app.folder();

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    app.double_click_text_containing("top_1 (");
    app.wait_for_combo_box_value("load_pcbs: core::load_pcbs", "Pending");
    app.assert_data_table_cell_is_not_editable("R1", "Placed?");

    // nor after the tasks before the placements task are complete, until it has been started (`ISSUE-12`).
    app.choose_action_from_combo_box("load_pcbs: core::load_pcbs", "Start");
    app.wait_for_combo_box_value("load_pcbs: core::load_pcbs", "Incomplete");
    app.choose_action_from_combo_box("load_pcbs: core::load_pcbs", "Complete");
    app.wait_for_combo_box_value("automated_pnp: core::place_components", "Pending");
    app.wait_until("the operation offered 'Start'", |app| {
        app.combo_box_labels()
            .iter()
            .any(|label| label == "automated_pnp: core::place_components")
    });
    app.assert_data_table_cell_is_not_editable("R1", "Placed?");
}

/// `OPERATIONS-2`
#[test]
fn once_every_placement_has_been_placed_they_can_only_be_reset() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placements_placed(&mut app, &folder);

    app.choose_in_data_table_cell_offering("R1", "Placed?", "Pending", Some(&["Pending", "Placed"]));
    app.wait_until("'R1' was pending", |app| app.count_text("Placed") == 2);
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_placements_placed_using_the_core(&mut core, &folder);
    update(&mut core, Event::RecordPlacementsOperation {
        object_path_patterns: vec![Regex::new("^pcb=1::unit=1::ref_des=R1$").unwrap()],
        operation: PlacementOperation::Reset,
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PHASE-4`, `PLACEMENTS-1`, `PLACEMENTS-4`
#[test]
fn once_a_phase_has_started_it_cannot_be_deleted_and_placements_cannot_change_phase() {
    let mut app = App::start();
    let folder = app.folder();

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    app.double_click_text_containing("top_1 (");
    app.choose_action_from_combo_box("load_pcbs: core::load_pcbs", "Start");
    app.wait_for_combo_box_value("load_pcbs: core::load_pcbs", "Incomplete");

    app.click_tab("Overview");
    app.wait_until_button_in_row_is("top_1", "Delete", false);
    app.wait_until_button_in_row_is("bottom_1", "Delete", true);

    // the placements and the phase are still selected.
    app.click_tab("Placements");
    app.open_combo_box("Actions");
    app.assert_button_is_disabled("Apply");
    app.assert_button_is_disabled("Remove");
    app.close_popup();

    app.assert_data_table_cell_is_not_editable("J1", "Phase");
}
