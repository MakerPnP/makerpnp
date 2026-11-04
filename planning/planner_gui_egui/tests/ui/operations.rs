//! Abandoning a phase's tasks, and skipping placements.

use std::fs;

use planner_app::{Event, PlacementOperation, Reference, TaskAction};
use planner_gui_egui::planner_app_core::PlannerCoreService;
use regex::Regex;

use crate::fixture::App;
use crate::workflow::*;

/// `OPERATIONS-1`, `OPERATIONS-2`, `STORE-13`
#[test]
fn abandoning_a_task_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_phase_operations_recorded(&mut app, &folder);

    app.choose_action_from_combo_box("automated_pnp: place_components", "Abandon");
    app.wait_for_enabled_button("Save");
    // the stores service reads the phase's operation history, then writes it with the task added.
    app.wait_for_logs_in_order(&[
        "planner_app: Received event. event: RecordPhaseOperation {",
        "Marking task as abandoned. phase: top_1, operation: automated_pnp, task: place_components",
        "stores_service: Performing stores operation. operation: ReadDocument { path: ",
        "stores_service: Performing stores operation. operation: WriteDocument { path: ",
        "planner_app: Updated operation history file. path: ",
        "planner_app: Received event. event: OperationHistoryWritten { result: Ok(()) }",
    ]);
    app.assert_not_logged("Event failed.");

    // it no longer offers any actions, and its placements can't be recorded.
    app.wait_until("the task offered no actions", |app| {
        !app.combo_box_labels()
            .iter()
            .any(|label| label.contains("place_components"))
    });
    app.wait_until("the task and its operation were abandoned", |app| {
        app.count_text("Abandoned") == 2
    });
    app.assert_data_table_cell_is_not_editable("R1", "Placed?");

    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_phase_operations_recorded_using_the_core(&mut core, &folder);
    update(&mut core, Event::RecordPhaseOperation {
        phase: Reference::from_raw_str("top_1"),
        operation: Reference::from_raw_str("automated_pnp"),
        task: Reference::from_raw_str("place_components"),
        action: TaskAction::Abandon,
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `OPERATIONS-2`
#[test]
fn skipping_a_placement_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_phase_operations_recorded(&mut app, &folder);

    app.choose_in_data_table_cell("R1", "Placed?", "Skipped");
    app.wait_until("'R1' was skipped", |app| app.count_text("Skipped") == 1);
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_phase_operations_recorded_using_the_core(&mut core, &folder);
    update(&mut core, Event::RecordPlacementsOperation {
        object_path_patterns: vec![Regex::new("^pcb=1::unit=1::ref_des=R1$").unwrap()],
        operation: PlacementOperation::Skip,
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `OPERATIONS-2`
#[test]
fn skipping_the_last_placement_completes_the_task() {
    let mut app = App::start();
    let folder = app.folder();

    create_project_with_phase_operations_recorded(&mut app, &folder);

    app.choose_in_data_table_cell("R1", "Placed?", "Placed");
    app.choose_in_data_table_cell("R2", "Placed?", "Placed");
    app.wait_until("'R1' and 'R2' were placed", |app| app.count_text("Placed") == 2);
    app.choose_in_data_table_cell("R3", "Placed?", "Skipped");

    // the task's actions are gone, and the next operation's task can be started.
    app.wait_until("the task offered no actions", |app| {
        !app.combo_box_labels()
            .iter()
            .any(|label| label.contains("place_components"))
    });
    let tasks = app
        .combo_box_labels()
        .into_iter()
        .filter(|label| label.contains(": "))
        .collect::<Vec<_>>();
    assert_eq!(tasks, ["reflow_oven_soldering: automated_soldering"]);
}
