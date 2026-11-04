//! Reversing and rejecting the changes that the workflow makes, which the CLI's sequence doesn't cover.

use std::fs;
use std::path::Path;

use planner_app::{AddOrRemoveAction, Event, ProcessReference, Reference, SetOrClearAction};
use planner_gui_egui::planner_app_core::PlannerCoreService;
use regex::Regex;

use crate::fixture::App;
use crate::workflow::*;

/// The phase assigned to a placement in a saved project, if any.
fn saved_phase_of(project: &Path, ref_des: &str) -> Option<String> {
    let saved = fs::read_to_string(project).unwrap();
    let placement = saved
        .find(&format!("\"pcb=1::unit=1::ref_des={ref_des}\""))
        .expect("the placement is in the project");
    let after_the_key = &saved[placement + 1..];
    let block = match after_the_key.find("\"pcb=1::unit=1::ref_des=") {
        Some(next) => &after_the_key[..next],
        None => after_the_key,
    };

    block
        .lines()
        .find(|line| line.contains("\"phase\":"))
        .filter(|line| !line.contains("null"))
        .map(|line| line.trim().to_string())
}

/// `PARTS-2`
#[test]
fn removing_a_process_from_a_part_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_a_part_assigned_to_a_process(&mut app, &folder);

    // the part is still selected, and the process still chosen.
    app.choose_action_from_combo_box("Actions", "Remove");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_a_part_assigned_to_a_process_using_the_core(&mut core, &folder);
    update(&mut core, Event::AssignProcessToParts {
        process: ProcessReference::from_raw_str("manual"),
        operation: AddOrRemoveAction::Remove,
        manufacturer: Regex::new("CONN_MFR.*").unwrap(),
        mpn: Regex::new(".*").unwrap(),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PARTS-1`, `PARTS-2`
#[test]
fn a_process_is_only_added_to_the_parts_still_selected() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_a_part_assigned_to_a_process(&mut app, &folder);

    // leaves "CONN1" selected, as it was, and selects "RES1" and "RES2".
    app.click_row_header_of("RES1");
    app.click_row_header_of("RES2");
    // deselects "CONN1".
    app.click_row_header_of("CONN1");
    app.choose_from_combo_box("Process", "pnp");
    app.choose_action_from_combo_box("Actions", "Add");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_a_part_assigned_to_a_process_using_the_core(&mut core, &folder);
    update(&mut core, Event::AssignProcessToParts {
        process: ProcessReference::from_raw_str("pnp"),
        operation: AddOrRemoveAction::Add,
        manufacturer: Regex::new("RES_MFR.*").unwrap(),
        mpn: Regex::new(".*").unwrap(),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PARTS-1`
#[test]
fn the_parts_actions_are_disabled_once_nothing_is_selected() {
    let mut app = App::start();
    let folder = app.folder();

    create_project_with_a_part_assigned_to_a_process(&mut app, &folder);

    app.click_row_header_of("CONN1");
    app.wait_until_combo_box_is_disabled("Actions");
}

/// `PLACEMENTS-1`, `PLACEMENTS-2`
#[test]
fn removing_a_phase_from_a_placement_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    // they are all still selected, as is the phase.
    app.click_row_header_of("R2");
    app.click_row_header_of("R3");
    app.choose_action_from_combo_box("Actions", "Remove");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();
    assert_eq!(saved_phase_of(&project, "R1"), None);
    assert!(
        saved_phase_of(&project, "R2")
            .unwrap()
            .contains("top_1")
    );

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

/// `PLACEMENTS-3`
#[test]
fn a_phase_can_be_removed_straight_after_it_was_applied() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    // the same rows and phase are still selected.
    app.choose_action_from_combo_box("Actions", "Remove");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();
    for ref_des in ["R1", "R2", "R3"] {
        assert_eq!(saved_phase_of(&project, ref_des), None);
    }

    let mut core = PlannerCoreService::new();
    project_with_placements_assigned_to_a_phase_using_the_core(&mut core, &folder);
    update(&mut core, Event::AssignPlacementsToPhase {
        phase: Reference::from_raw_str("top_1"),
        operation: SetOrClearAction::Clear,
        placements: Regex::new("pcb=1::unit=1::ref_des=R.*").unwrap(),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PLACEMENTS-1`
#[test]
fn the_actions_are_disabled_once_nothing_is_selected() {
    let mut app = App::start();
    let folder = app.folder();

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    for ref_des in ["R1", "R2", "R3"] {
        app.click_row_header_of(ref_des);
    }
    app.wait_until_combo_box_is_disabled("Actions");
}

/// `LOADOUT-1`, `FORM-11`
#[test]
fn clearing_a_feeder_writes_the_same_load_out_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let load_out = folder.join("phase_1_top_1_load_out.csv");

    create_project_with_a_feeder_assigned(&mut app, &folder);

    app.edit_data_table_cell("RES1", "Reference", "", "RES2");
    app.wait_until_text_is_gone("FEEDER_1");

    let saved_load_out = fs::read_to_string(&load_out).unwrap();
    assert!(!saved_load_out.contains("FEEDER_1"), "{saved_load_out}");

    let mut core = PlannerCoreService::new();
    project_with_a_feeder_assigned_using_the_core(&mut core, &folder);
    update(&mut core, Event::AssignFeederToLoadOutItem {
        phase: Reference::from_raw_str("top_1"),
        feeder_reference: None,
        manufacturer: Regex::new("RES_MFR1").unwrap(),
        mpn: Regex::new("RES1").unwrap(),
    });

    assert_eq!(saved_load_out, fs::read_to_string(&load_out).unwrap());
}

/// `LOADOUT-3`, `FORM-11`
#[test]
fn an_invalid_feeder_does_not_change_the_load_out() {
    let mut app = App::start();
    let folder = app.folder();
    let load_out = folder.join("phase_1_top_1_load_out.csv");

    create_project_with_a_feeder_assigned(&mut app, &folder);
    let before = fs::read_to_string(&load_out).unwrap();

    app.edit_data_table_cell("RES1", "Reference", "not valid", "RES2");
    // a later change is handled after the one being tested.
    app.edit_data_table_cell("RES2", "Reference", "FEEDER_2", "RES1");
    app.wait_for_text("FEEDER_2");

    app.wait_for_text("FEEDER_1");
    assert_eq!(
        fs::read_to_string(&load_out).unwrap(),
        before.replace("\"\",\"RES_MFR2\",\"RES2\"", "\"FEEDER_2\",\"RES_MFR2\",\"RES2\"")
    );
}

/// `PHASE-4`
#[test]
fn deleting_a_phase_closes_its_tabs_and_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    let load_out = folder.join("phase_1_top_1_load_out.csv");

    create_project_with_a_feeder_assigned(&mut app, &folder);
    assert!(app.count_tabs_containing("load_out") > 0);

    app.click_tab("Overview");
    app.click_button_in_row("top_1", "Delete");
    app.wait_for_enabled_button("Save");
    app.wait_until_no_tab_contains("load_out");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();
    // the file isn't the project's, the phase only refers to it.
    assert!(load_out.exists());

    let mut core = PlannerCoreService::new();
    project_with_a_feeder_assigned_using_the_core(&mut core, &folder);
    update(&mut core, Event::DeletePhase {
        reference: Reference::from_raw_str("top_1"),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `ORDERING-1`, `ORDERING-2`
#[test]
fn an_ordering_can_be_moved_back_before_ok_is_clicked() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);
    app.double_click_text_containing("top_1 (");
    app.click_button("Placement orderings.");

    for mode in ["Area", "Height"] {
        app.click_text_in_dialog(mode);
        app.wait_for_enabled_button(">");
        app.click_button(">");
        app.wait_for_text(&format!("{mode} (Ascending)"));
    }
    app.click_text_in_dialog("Area (Ascending)");
    app.wait_for_enabled_button("<");
    app.click_button("<");
    app.wait_until_text_is_gone("Area (Ascending)");
    app.click_button("Ok");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_placements_assigned_to_a_phase_using_the_core(&mut core, &folder);
    update(&mut core, Event::SetPlacementOrdering {
        phase: Reference::from_raw_str("top_1"),
        placement_orderings: vec![planner_app::PlacementSortingItem {
            mode: planner_app::PlacementSortingMode::Height,
            sort_order: util::sorting::SortOrder::Asc,
        }],
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `ORDERING-2`
#[test]
fn cancelling_the_placement_orderings_dialog_does_not_change_the_project() {
    let mut app = App::start();
    let folder = app.folder();

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);
    app.double_click_text_containing("top_1 (");
    app.click_button("Placement orderings.");

    app.click_text_in_dialog("Area");
    app.wait_for_enabled_button(">");
    app.click_button(">");
    app.wait_for_text("Area (Ascending)");
    app.click_button("Cancel");

    app.wait_until_button_is_gone("Cancel");
    app.wait_until_button_is_disabled("Save");
}

/// `PHASE-1`
#[test]
fn the_add_button_of_the_overview_tab_opens_the_add_phase_dialog() {
    let mut app = App::start();
    let folder = app.folder();

    create_project(&mut app, &folder);
    app.click_tab("Overview");
    app.wait_for_tab("Overview");
    // the PCBs have one too, further down.
    app.click_first_button("Add");

    app.wait_for_text_containing("Add phase - ");
    app.click_button("Cancel");
    app.wait_until_button_is_gone("Cancel");
}

/// `PHASE-1`
#[test]
fn the_add_phase_dialog_needs_a_valid_reference_a_side_and_a_process() {
    let mut app = App::start();
    let folder = app.folder();
    let load_out = given_a_load_out(&folder, "load_out.csv");
    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");
    add_processes(&mut app, &folder);

    app.click_button("Add phase");
    app.when_a_file_dialog_opens_answer(Some(load_out));
    app.click_button("Loadout source: Choose");
    app.wait_until_field_is_not_empty("Loadout source");

    // nothing but the load-out.
    app.assert_button_is_disabled("Ok");

    // no process.
    app.type_into("Phase reference", "top_1");
    app.choose_from_combo_box("PCB Side", "Top");
    app.assert_button_is_disabled("Ok");

    // valid.
    app.choose_from_combo_box("Process", "pnp");
    app.wait_for_enabled_button("Ok");

    // an invalid reference, and then none.
    app.type_into("Phase reference", "not valid");
    app.wait_until_button_is_disabled("Ok");
    app.wait_for_text("Reference invalid");
    app.type_into("Phase reference", "top_1");
    app.wait_for_enabled_button("Ok");
    app.clear_field("Phase reference");
    app.wait_until_button_is_disabled("Ok");
}

/// `PLACEMENTS-2`, `PLACEMENTS-3`
#[test]
fn removing_a_phase_leaves_the_placements_of_other_phases_as_they_were() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    // "R1", "R2" and "R3" are still selected, they're on the top.
    app.click_row_header_of("J1");
    app.choose_from_combo_box("Phase", "bottom_1");
    app.choose_action_from_combo_box("Actions", "Apply");
    app.wait_for_text_containing("placements were not changed, they're on the other side of the PCB to the phase.");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    app.choose_from_combo_box("Phase", "top_1");
    app.wait_until_no_text_contains("placements were not changed");
    app.choose_action_from_combo_box("Actions", "Remove");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    assert_eq!(saved_phase_of(&project, "R1"), None);

    assert!(
        saved_phase_of(&project, "J1")
            .unwrap()
            .contains("bottom_1")
    );
    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_placements_assigned_to_a_phase_using_the_core(&mut core, &folder);
    update(&mut core, Event::AssignPlacementsToPhase {
        phase: Reference::from_raw_str("bottom_1"),
        operation: SetOrClearAction::Set,
        placements: Regex::new("^pcb=1::unit=1::ref_des=J1$").unwrap(),
    });
    update(&mut core, Event::AssignPlacementsToPhase {
        phase: Reference::from_raw_str("top_1"),
        operation: SetOrClearAction::Clear,
        placements: Regex::new("pcb=1::unit=1::ref_des=R.*").unwrap(),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PLACEMENTS-2`
#[test]
fn applying_a_phase_to_a_placement_on_the_other_side_says_so() {
    let mut app = App::start();
    let folder = app.folder();

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    for ref_des in ["R1", "R2", "R3"] {
        app.click_row_header_of(ref_des);
    }
    app.click_row_header_of("J1");
    // "top_1" is still chosen.
    app.choose_action_from_combo_box("Actions", "Apply");

    app.wait_for_text_containing("1 placement was not changed, it's on the other side of the PCB to the phase.");
    app.assert_button_is_disabled("Save");

    // and the message goes once the selection changes.
    app.click_row_header_of("J1");
    app.wait_until_no_text_contains("placement was not changed");
}
