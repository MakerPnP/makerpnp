//! Process documents, and a project's processes, beyond the steps of the CLI's sequence that `workflow.rs` covers.

use std::fs;
use std::path::{Path, PathBuf};

use planner_app::{
    Event, FileReference, OperationReference, ProcessDefinition, ProcessEdit, ProcessPresetFactory, ProcessReference,
    Reference, TaskKind, TaskReference,
};
use planner_gui_egui::planner_app_core::PlannerCoreService;

use crate::fixture::App;
use crate::workflow::*;

const UNIQUE_FEEDER_REFERENCES: &str = "core::unique_feeder_references";

fn preset(name: &str) -> ProcessDefinition {
    ProcessPresetFactory::by_preset_name(name).unwrap()
}

/// The file the core saves for the definition.
fn saved_by_the_core(folder: &Path, definition: ProcessDefinition) -> String {
    let path = folder.join("expected.process.json");
    let mut core = PlannerCoreService::new();
    update(&mut core, Event::CreateProcess {
        path: path.clone(),
        reference: definition.reference.clone(),
        preset: None,
    });
    update(&mut core, Event::ApplyProcessDefinition {
        path: path.clone(),
        process_definition: definition,
    });
    update(&mut core, Event::SaveProcess {
        path: path.clone(),
    });

    let content = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();
    content
}

/// Opens `pnp.process.json`, made from the `pnp` preset, using "Open process".
fn open_pnp_process(app: &mut App, folder: &Path) -> PathBuf {
    given_the_processes(folder);
    let process = folder.join("pnp.process.json");

    app.when_a_file_dialog_opens_answer(Some(process.clone()));
    app.click_button("Open process");
    app.wait_for_tab("pnp.process.json");
    app.wait_for_field_value("Operation 1", "load_pcbs");

    process
}

fn apply_and_save(app: &mut App) {
    app.wait_for_enabled_button("Apply");
    app.click_button("Apply");
    app.wait_until_button_is_disabled("Apply");
    app.wait_for_enabled_button("Save");
    save(app);
}

//
// process documents
//

/// `PROC-1`, `FORM-1`
#[test]
fn a_new_process_needs_a_valid_reference_and_a_directory() {
    let mut app = App::start();
    let folder = app.folder();

    app.click_button("New process");
    app.assert_button_is_disabled("Ok");

    app.type_into("Process reference", "pnp");
    app.assert_button_is_disabled("Ok");

    app.when_a_file_dialog_opens_answer(Some(folder.clone()));
    app.click_button("Directory: Choose");
    app.wait_until_field_is_not_empty("Directory");
    app.wait_for_enabled_button("Ok");

    // a reference can't contain whitespace.
    app.type_into("Process reference", "glue and wave");
    app.wait_until_button_is_disabled("Ok");
    app.wait_for_text("Reference invalid");

    app.type_into("Process reference", "glue_and_wave");
    app.wait_for_enabled_button("Ok");

    // "Preset" offers no preset, and each of the presets.
    app.choose_from_combo_box("Preset", "manual");
    app.choose_from_combo_box("Preset", "pnp");
    app.choose_from_combo_box("Preset", "None");

    app.click_button("Ok");
    app.wait_for_tab("glue_and_wave.process.json");
    app.wait_for_button("Add operation");

    let expected = ProcessDefinition::new(ProcessReference::from_raw_str("glue_and_wave"));
    assert_eq!(
        fs::read_to_string(folder.join("glue_and_wave.process.json")).unwrap(),
        saved_by_the_core(&folder, expected)
    );
}

/// `PROC-1`, `PROC-2`
#[test]
fn opening_a_process_shows_its_definition() {
    let mut app = App::start();
    let folder = app.folder();

    open_pnp_process(&mut app, &folder);

    app.wait_for_field_value("Process reference", "pnp");
    for (operation, reference, task, task_reference, kind) in [
        (
            "Operation 1",
            "load_pcbs",
            "Operation 1, task 1",
            "load_pcbs",
            "Load PCBs",
        ),
        (
            "Operation 2",
            "automated_pnp",
            "Operation 2, task 1",
            "place_components",
            "Place components",
        ),
        (
            "Operation 3",
            "reflow_oven_soldering",
            "Operation 3, task 1",
            "automated_soldering",
            "Generic",
        ),
    ] {
        app.wait_for_field_value(operation, reference);
        app.wait_for_field_value(task, task_reference);
        app.wait_for_combo_box_value(&format!("{task}: Kind"), kind);
    }
    app.wait_for_checkbox(UNIQUE_FEEDER_REFERENCES, true);

    // nothing has changed.
    app.assert_button_is_disabled("Apply");
    app.assert_button_is_disabled("Reset");
    app.assert_button_is_disabled("Save");
}

/// `PROC-2`, `PROC-3`
#[test]
fn moving_and_deleting_operations_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let process = open_pnp_process(&mut app, &folder);

    // the first can't move up, and the last can't move down.
    app.assert_button_is_disabled("Operation 1: Move up");
    app.assert_button_is_disabled("Operation 3: Move down");

    app.click_button("Operation 3: Move up");
    app.wait_for_field_value("Operation 2", "reflow_oven_soldering");
    app.click_button("Operation 1: Move down");
    app.wait_for_field_value("Operation 1", "reflow_oven_soldering");
    app.wait_for_field_value("Operation 2", "load_pcbs");

    app.click_button("Operation 3: Delete");
    app.wait_until_field_is_gone("Operation 3");

    apply_and_save(&mut app);

    let mut expected = preset("pnp");
    let [load_pcbs, _automated_pnp, reflow] = <[_; 3]>::try_from(expected.operations).unwrap();
    expected.operations = vec![reflow, load_pcbs];

    assert_eq!(
        fs::read_to_string(&process).unwrap(),
        saved_by_the_core(&folder, expected)
    );
}

/// `PROC-2`, `PROC-3`
#[test]
fn editing_tasks_and_rules_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let process = open_pnp_process(&mut app, &folder);

    app.type_into("Operation 2", "place_by_machine");
    app.type_into("Operation 2, task 1", "place");
    app.choose_from_combo_box("Operation 1, task 1: Kind", "Generic");
    app.click_button("Operation 3, task 1: Delete");
    app.wait_until_field_is_gone("Operation 3, task 1");
    app.click_checkbox(UNIQUE_FEEDER_REFERENCES);
    app.wait_for_checkbox(UNIQUE_FEEDER_REFERENCES, false);

    apply_and_save(&mut app);

    let mut expected = preset("pnp");
    expected.operations[0].tasks[0].kind = TaskKind::Generic;
    expected.operations[1].reference = OperationReference::from_raw_str("place_by_machine");
    expected.operations[1].tasks[0].reference = TaskReference::from_raw_str("place");
    expected.operations[2].tasks.clear();
    expected.rules.clear();

    assert_eq!(
        fs::read_to_string(&process).unwrap(),
        saved_by_the_core(&folder, expected)
    );

    // and the rule can be added again.
    app.click_checkbox(UNIQUE_FEEDER_REFERENCES);
    app.wait_for_checkbox(UNIQUE_FEEDER_REFERENCES, true);
    app.wait_for_enabled_button("Apply");
}

/// `PROC-3`
#[test]
fn an_invalid_process_cannot_be_applied() {
    let mut app = App::start();
    let folder = app.folder();
    open_pnp_process(&mut app, &folder);

    // two operations with the same reference.
    app.type_into("Operation 3", "load_pcbs");
    app.wait_until_button_is_disabled("Apply");
    app.wait_for_text_containing("Duplicate operation");
    app.wait_for_enabled_button("Reset");

    // a reference that isn't valid.
    app.type_into("Operation 3", "reflow oven");
    app.wait_until_button_is_disabled("Apply");
    app.wait_for_text_containing("Operation 3: reference invalid");

    // a task's reference that isn't valid.
    app.type_into("Operation 3", "reflow_oven_soldering");
    app.type_into("Operation 3, task 1", "solder it");
    app.wait_until_button_is_disabled("Apply");
    app.wait_for_text_containing("Operation 3, task 1: reference invalid");

    // two tasks with the same reference in an operation.
    app.type_into("Operation 3, task 1", "automated_soldering");
    app.type_into("Operation 3: New task", "automated_soldering");
    app.click_button("Operation 3: Add task");
    app.wait_until_button_is_disabled("Apply");
    app.wait_for_text_containing("Duplicate task");

    app.click_button("Reset");
    app.wait_until_button_is_disabled("Reset");
    app.wait_until_field_is_gone("Operation 3, task 2");
    app.assert_button_is_disabled("Save");
}

/// `PROC-2`
#[test]
fn a_task_or_operation_cannot_be_added_without_a_valid_reference() {
    let mut app = App::start();
    let folder = app.folder();
    open_pnp_process(&mut app, &folder);

    app.assert_button_is_disabled("Operation 1: Add task");
    app.type_into("Operation 1: New task", "inspect");
    app.wait_for_enabled_button("Operation 1: Add task");
    app.type_into("Operation 1: New task", "inspect visually");
    app.wait_until_button_is_disabled("Operation 1: Add task");

    app.type_into("New operation", "wave solder");
    app.wait_until_button_is_disabled("Add operation");
    app.type_into("New operation", "wave_solder");
    app.wait_for_enabled_button("Add operation");
    app.click_button("Add operation");
    app.wait_for_field_value("Operation 4", "wave_solder");
}

//
// a project's processes
//

/// `PROJPROC-1`
#[test]
fn cancelling_add_process_does_not_change_the_project() {
    let mut app = App::start();
    let folder = app.folder();
    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");

    app.when_a_file_dialog_opens_answer(None);
    app.click_button("Add process");
    app.wait_for_file_dialogs();

    app.assert_button_is_disabled("Save");
}

/// `PROJPROC-1`, `PROJPCB-2`
#[test]
fn a_process_outside_the_projects_folder_is_stored_with_its_absolute_path() {
    let mut app = App::start();
    let folder = app.folder().join("project");
    fs::create_dir(&folder).unwrap();
    let project = folder.join("project-job1.mpnp.json");
    given_the_processes(&app.folder());
    let process = app.folder().join("pnp.process.json");

    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");
    app.when_a_file_dialog_opens_answer(Some(process.clone()));
    app.click_button("Add process");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    update(&mut core, Event::CreateProject {
        name: "job1".to_string(),
        path: project.clone(),
        packages: None,
        package_mappings: None,
    });
    update(&mut core, Event::AddProcess {
        process_file: FileReference::Absolute(process),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PROJPROC-2`
#[test]
fn a_projects_process_tab_shows_its_copy_and_opens_its_file() {
    let mut app = App::start();
    let folder = app.folder();
    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");
    add_processes(&mut app, &folder);

    app.double_click_text("manual");
    app.wait_for_text_containing("File: manual.process.json");
    app.wait_for_text_containing("Reference: manual");
    app.wait_for_text("manually_solder_components");
    app.wait_for_text_containing("place_components (Place components)");
    app.wait_for_text_containing("manual_soldering (Generic)");

    app.click_button("Open file");
    app.wait_for_tab("manual.process.json");
    app.wait_for_field_value("Operation 2", "manually_solder_components");

    // opening it again shows the same tab.
    app.click_tab("project-job1.mpnp.json");
    app.click_button("Open file");
    app.wait_for_field_value("Operation 2", "manually_solder_components");
    assert_eq!(app.count_tabs_containing("manual.process.json"), 1);
}

/// `PROJPROC-3`, `LOG-1`, `LOG-3`
#[test]
fn removing_a_process_saves_the_same_file_as_the_cli_and_closes_its_tab() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");
    add_processes(&mut app, &folder);

    app.double_click_text("manual");
    app.click_button("Remove process");
    app.wait_until_button_is_gone("Remove process");

    // the click, as the project and the tab received it, then the event the core received, in order, and no failure.
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::project: Project command. command: (ProjectKey(1v1), ProcessTabUiCommand { process: Reference(\"manual\"), command: RemoveClicked })",
        "planner_gui_egui::project::tabs::process_tab: ProcessTabUi command. command: RemoveClicked",
        "planner_app: Received event. event: RemoveProcess { process_reference: Reference(\"manual\") }",
    ]);
    app.assert_not_logged("Event failed.");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    let mut core = PlannerCoreService::new();
    update(&mut core, Event::CreateProject {
        name: "job1".to_string(),
        path: project.clone(),
        packages: None,
        package_mappings: None,
    });
    update(&mut core, Event::AddProcess {
        process_file: FileReference::Relative("pnp.process.json".into()),
    });
    update(&mut core, Event::AddProcess {
        process_file: FileReference::Relative("manual.process.json".into()),
    });
    update(&mut core, Event::RemoveProcess {
        process_reference: ProcessReference::from_raw_str("manual"),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
    // the file is not deleted.
    assert!(
        folder
            .join("manual.process.json")
            .exists()
    );
}

/// `PROJPROC-3`, `PROCDEF-7`, `LOG-2`, `LOG-3`, `LOG-8`
#[test]
fn a_process_used_by_a_phase_cannot_be_removed() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    let load_out = given_a_load_out(&folder, "load_out.csv");
    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");
    add_processes(&mut app, &folder);
    add_phase(&mut app, "top_1", "manual", "Top", &load_out);
    save(&mut app);
    let saved = fs::read_to_string(&project).unwrap();

    app.double_click_text("manual");
    app.click_button("Remove process");
    app.wait_for_text_containing("ProcessInUse");

    // the click, the event, and why the core refused it, in order.
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::project::tabs::process_tab: ProcessTabUi command. command: RemoveClicked",
        "planner_app: Received event. event: RemoveProcess { process_reference: Reference(\"manual\") }",
        "planner_app: Event failed. error: ProcessError(ProcessInUse { process_reference: Reference(\"manual\") })",
    ]);
    // then the project receives the error to show it, and the error isn't logged again (`LOG-8`).
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::project: Project command. command: (ProjectKey(1v1), Error(CoreError((",
    ]);
    app.assert_not_logged("ProcessInUse");

    app.click_button("Ok");

    app.wait_for_button("Remove process");
    app.assert_button_is_disabled("Save");
    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PROJPROC-3`, `PROCDEF-6`, `LOG-2`, `LOG-3`
#[test]
fn a_process_cannot_be_refreshed_once_a_phase_using_it_has_started() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    create_project_with_phase_operations_recorded(&mut app, &folder);
    let saved = fs::read_to_string(&project).unwrap();

    // `top_1` uses `pnp`, and its first task is complete.
    let process = folder.join("pnp.process.json");
    let mut core = PlannerCoreService::new();
    update(&mut core, Event::LoadProcess {
        path: process.clone(),
    });
    update(&mut core, Event::EditProcess {
        path: process.clone(),
        edit: ProcessEdit::RemoveRule {
            rule: Reference::from_raw_str(UNIQUE_FEEDER_REFERENCES),
        },
    });
    update(&mut core, Event::SaveProcess {
        path: process,
    });

    app.double_click_text("pnp");
    app.click_button("Refresh processes");
    app.wait_for_text_containing("ProcessInProgress");

    // the click, the event, and why the core refused it, in order.
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::project::tabs::process_tab: ProcessTabUi command. command: RefreshClicked",
        "planner_app: Received event. event: RefreshProcesses",
        "planner_app: Event failed. error: ProcessError(ProcessInProgress {",
    ]);

    app.click_button("Ok");

    app.assert_button_is_disabled("Save");
    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PROJPROC-3`, `PROCDEF-6`
#[test]
fn refreshing_unchanged_processes_does_not_change_the_project() {
    let mut app = App::start();
    let folder = app.folder();
    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");
    add_processes(&mut app, &folder);

    app.double_click_text("pnp");
    app.click_button("Refresh processes");
    app.wait_for_button("Refresh processes");

    app.assert_button_is_disabled("Save");
}
