//! The same workflow as the planner CLI's `operation_sequence_1` tests, done through the GUI.
//!
//! The GUI has to save the same files as the core does when given the events the CLI gives it.

use std::fs;
use std::path::Path;

use planner_app::{Event, PackageMappingsSource, PackagesSource};
use planner_gui_egui::planner_app_core::PlannerCoreService;

use crate::fixture::App;

fn update(core: &mut PlannerCoreService, event: Event) {
    core.update(event)
        .unwrap_or_else(|error| panic!("{error:?}"));
}

/// What the CLI's `sequence_01_create_job` does.
fn project_saved_by_the_core(path: &Path, packages: &Path, package_mappings: &Path) -> String {
    let mut core = PlannerCoreService::new();
    update(&mut core, Event::CreateProject {
        name: "job1".to_string(),
        path: path.to_path_buf(),
        packages: Some(PackagesSource::File(packages.to_path_buf())),
        package_mappings: Some(PackageMappingsSource::File(package_mappings.to_path_buf())),
    });

    fs::read_to_string(path).unwrap()
}

fn create_project(app: &mut App, folder: &Path) {
    app.click_button("New project");
    app.type_into("Project name", "job1");
    app.when_a_file_dialog_opens_answer(Some(folder.to_path_buf()));
    app.click_button("Directory: Choose");
    app.wait_for_enabled_button("Ok");
    app.click_button("Ok");
    app.wait_for_tab("project-job1.mpnp.json");
}

/// `DOC-1`
#[test]
fn a_new_project_is_saved_as_it_is_created() {
    let mut app = App::start();
    let folder = app.folder();

    create_project(&mut app, &folder);

    // only shown after the core has created the project.
    app.wait_for_tab("Overview");
    assert!(
        folder
            .join("project-job1.mpnp.json")
            .exists()
    );
    app.wait_until_button_is_disabled("Save");
}

/// `DOC-1`, `FORM-1`, `FORM-2`
#[test]
fn a_project_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    let packages = folder.join("packages.csv");
    let package_mappings = folder.join("package_mappings.csv");
    fs::write(&packages, "").unwrap();
    fs::write(&package_mappings, "").unwrap();

    create_project(&mut app, &folder);

    app.click_button("Package sources");
    app.when_a_file_dialog_opens_answer(Some(packages.clone()));
    app.click_button("Packages: Choose");
    app.wait_until_field_is_not_empty("Packages");
    app.when_a_file_dialog_opens_answer(Some(package_mappings.clone()));
    app.click_button("Package mappings: Choose");
    app.wait_until_field_is_not_empty("Package mappings");
    app.click_button("Ok");

    app.wait_for_enabled_button("Save");
    app.click_button("Save");
    app.wait_until_button_is_disabled("Save");

    let saved = fs::read_to_string(&project).unwrap();
    assert_eq!(saved, project_saved_by_the_core(&project, &packages, &package_mappings));
}

/// `FORM-3`
#[test]
fn the_clear_button_of_a_field_clears_its_value() {
    let mut app = App::start();
    let folder = app.folder();

    app.click_button("New project");
    app.type_into("Project name", "job1");
    app.when_a_file_dialog_opens_answer(Some(folder));
    app.click_button("Directory: Choose");
    app.wait_until_field_is_not_empty("Directory");
    app.wait_for_enabled_button("Ok");

    app.click_button("Directory: Clear");

    app.wait_until_field_is_empty("Directory");
    app.wait_until_button_is_disabled("Ok");
}

/// `DOC-1`, `FORM-1`, `FORM-2`
#[test]
fn a_new_pcb_saves_the_same_file_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let pcb = folder.join("panel_a.pcb.json");

    app.click_button("New PCB");
    app.type_into("PCB name", "panel_a");
    app.set_number("Units", 4.0);
    app.when_a_file_dialog_opens_answer(Some(folder.clone()));
    app.click_button("Directory: Choose");
    app.wait_for_enabled_button("Ok");
    app.click_button("Ok");
    app.wait_for_tab("panel_a.pcb.json");

    // only shown after the core has created the PCB.
    app.wait_for_tab("Configuration");

    let saved = fs::read_to_string(&pcb).unwrap();

    let mut core = PlannerCoreService::new();
    update(&mut core, Event::CreatePcb {
        name: "panel_a".to_string(),
        units: 4,
        path: pcb.clone(),
        unit_map: None,
    });

    assert_eq!(saved, fs::read_to_string(&pcb).unwrap());
}
