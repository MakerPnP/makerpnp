//! The same workflow as the planner CLI's `operation_sequence_1` tests, done through the GUI.
//!
//! The GUI has to save the same files as the core does when given the events the CLI gives it.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use indoc::indoc;
use nalgebra::Vector2;
use planner_app::{
    AddOrRemoveAction, DesignName, DesignSizing, Dimensions, Event, FileReference, LoadOutSource, ObjectPath,
    PackageMappingsSource, PackagesSource, PcbSide, PcbUnitNumber, PcbUnitPositioning, PlacementOperation,
    PlacementSortingItem, PlacementSortingMode, ProcessReference, Reference, SetOrClearAction, TaskAction, VariantName,
};
use planner_gui_egui::planner_app_core::PlannerCoreService;
use regex::Regex;
use rust_decimal::Decimal;
use util::sorting::SortOrder;

use crate::fixture::App;

pub fn update(core: &mut PlannerCoreService, event: Event) {
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

pub fn create_project(app: &mut App, folder: &Path) {
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

fn assign_units(app: &mut App, units: &[u32]) {
    for unit in units {
        app.set_number("Unit range: End", 4.0);
        app.set_number("Unit range: Start", *unit as f64);
        app.set_number("Unit range: End", *unit as f64);
        app.wait_for_enabled_button("Apply range");
        app.click_button("Apply range");
    }
}

/// What the CLI's `sequence_02_create_pcb` does.
fn create_pcb_with_the_core(core: &mut PlannerCoreService, pcb: &Path) {
    update(core, Event::CreatePcb {
        name: "panel_a".to_string(),
        units: 4,
        path: pcb.to_path_buf(),
        unit_map: Some(BTreeMap::from_iter(
            [(1, "design_a"), (2, "design_b"), (3, "design_a"), (4, "design_b")]
                .map(|(unit, design)| (PcbUnitNumber::try_from(unit).unwrap(), DesignName::from(design))),
        )),
    });
}

fn create_pcb_with_assigned_designs(app: &mut App, folder: &Path) {
    app.click_button("New PCB");
    app.type_into("PCB name", "panel_a");
    app.set_number("Units", 4.0);
    app.when_a_file_dialog_opens_answer(Some(folder.to_path_buf()));
    app.click_button("Directory: Choose");
    app.wait_for_enabled_button("Ok");
    app.click_button("Ok");
    app.wait_for_tab("Configuration");

    for design in ["design_a", "design_b"] {
        app.type_into("Design name", design);
        app.wait_for_enabled_button("Add");
        app.click_button("Add");
        app.wait_for_text(design);
    }

    app.click_text("design_a");
    assign_units(app, &[1, 3]);
    app.click_text("design_b");
    assign_units(app, &[2, 4]);

    app.wait_for_enabled_button("Apply");
    app.click_button("Apply");
    app.wait_for_enabled_button("Save");
}

pub fn save(app: &mut App) {
    app.click_button("Save");
    app.wait_until_button_is_disabled("Save");
}

/// `DOC-1`, `PCBCFG-1`, `PCBCFG-2`, `PCBCFG-3`, `PCBCFG-4`
#[test]
fn a_pcb_with_designs_assigned_to_its_units_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let pcb = folder.join("panel_a.pcb.json");

    create_pcb_with_assigned_designs(&mut app, &folder);
    save(&mut app);

    let saved = fs::read_to_string(&pcb).unwrap();

    create_pcb_with_the_core(&mut PlannerCoreService::new(), &pcb);

    assert_eq!(saved, fs::read_to_string(&pcb).unwrap());
}

/// `PANEL-1`, `PANEL-2`, `PANEL-3`, `PANEL-4`, `PANEL-5`
#[test]
fn a_pcb_with_panel_sizing_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let pcb = folder.join("panel_a.pcb.json");

    create_pcb_with_assigned_designs(&mut app, &folder);
    save(&mut app);

    app.double_click_text("Panelization");
    app.wait_for_tab("Panelization");

    for (field, value) in [
        ("Panel size: X", 122.0),
        ("Panel size: Y", 112.0),
        ("Edge rails: Top", 5.0),
        ("Edge rails: Bottom", 5.0),
        ("Edge rails: Left", 10.0),
        ("Edge rails: Right", 10.0),
        ("design_a: X placement offset", -10.0),
        ("design_a: Y placement offset", -10.0),
        ("design_a: X gerber offset", -20.0),
        ("design_a: Y gerber offset", -20.0),
        ("design_a: X origin", 30.0),
        ("design_a: Y origin", 25.0),
        ("design_a: X size", 60.0),
        ("design_a: Y size", 50.0),
        ("design_b: X placement offset", -10.0),
        ("design_b: Y placement offset", -10.0),
        ("design_b: X gerber offset", -20.0),
        ("design_b: Y gerber offset", -20.0),
        ("design_b: X origin", 20.0),
        ("design_b: Y origin", 25.0),
        ("design_b: X size", 40.0),
        ("design_b: Y size", 50.0),
        ("Unit 1: X", 10.0),
        ("Unit 1: Y", 5.0),
        ("Unit 2: X", 72.0),
        ("Unit 2: Y", 5.0),
        ("Unit 3: X", 10.0),
        ("Unit 3: Y", 57.0),
        ("Unit 4: X", 72.0),
        ("Unit 4: Y", 57.0),
    ] {
        app.set_number(field, value);
    }

    app.wait_for_enabled_button("Apply");
    app.click_button("Apply");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&pcb).unwrap();

    // what the CLI's `sequence_03_configure_panel_sizing` does.
    let mut core = PlannerCoreService::new();
    create_pcb_with_the_core(&mut core, &pcb);
    apply_panel_sizing_with_the_core(&mut core, &pcb);

    assert_eq!(saved, fs::read_to_string(&pcb).unwrap());
}

/// `PROJPCB-1`, `PROJPCB-2`
#[test]
fn a_project_with_a_pcb_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    let pcb = folder.join("panel_a.pcb.json");
    create_pcb_with_the_core(&mut PlannerCoreService::new(), &pcb);

    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");

    app.when_a_file_dialog_opens_answer(Some(pcb.clone()));
    app.click_button("Add PCB");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_04_add_pcb` does.
    let mut core = PlannerCoreService::new();
    update(&mut core, Event::CreateProject {
        name: "job1".to_string(),
        path: project.clone(),
        packages: None,
        package_mappings: None,
    });
    update(&mut core, Event::AddPcb {
        pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PROJPCB-2`
#[test]
fn a_project_keeps_working_after_its_folder_is_moved() {
    let mut app = App::start();
    let folder = app.folder().join("original");
    fs::create_dir(&folder).unwrap();
    create_pcb_with_the_core(&mut PlannerCoreService::new(), &folder.join("panel_a.pcb.json"));

    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");
    app.when_a_file_dialog_opens_answer(Some(folder.join("panel_a.pcb.json")));
    app.click_button("Add PCB");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let moved = app.folder().join("moved");
    fs::rename(&folder, &moved).unwrap();

    let mut app_after_the_move = App::start();
    app_after_the_move.when_a_file_dialog_opens_answer(Some(moved.join("project-job1.mpnp.json")));
    app_after_the_move.click_button("Open project");
    app_after_the_move.wait_for_tab("Overview");
}

/// `PROJPCB-2`
#[test]
fn a_pcb_outside_the_projects_folder_is_stored_with_its_absolute_path() {
    let mut app = App::start();
    let folder = app.folder().join("project");
    fs::create_dir(&folder).unwrap();
    let project = folder.join("project-job1.mpnp.json");
    let pcb = app.folder().join("panel_a.pcb.json");
    create_pcb_with_the_core(&mut PlannerCoreService::new(), &pcb);

    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");
    app.when_a_file_dialog_opens_answer(Some(pcb.clone()));
    app.click_button("Add PCB");
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
    update(&mut core, Event::AddPcb {
        pcb_file: FileReference::Absolute(pcb),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `FORM-5`, `PCBCFG-5`
#[test]
fn reducing_the_units_keeps_their_assignments_for_when_they_are_increased_again() {
    let mut app = App::start();
    let folder = app.folder();
    create_pcb_with_assigned_designs(&mut app, &folder);
    save(&mut app);
    app.wait_until_button_is_disabled("Reset");

    app.set_number("Units", 2.0);
    app.wait_for_enabled_button("Reset");

    app.set_number("Units", 4.0);

    // only disabled when the form is back to what was applied, including the assignments of units 3 and 4.  "Apply" is
    // no use for this, as it's also disabled when a unit has no design.
    app.wait_until_button_is_disabled("Reset");
}

/// `PCBCFG-6`, `FORM-5`
#[test]
fn assign_selected_only_changes_the_units_that_are_shown() {
    let mut app = App::start();
    let folder = app.folder();
    app.click_button("New PCB");
    app.type_into("PCB name", "panel_a");
    app.set_number("Units", 4.0);
    app.when_a_file_dialog_opens_answer(Some(folder));
    app.click_button("Directory: Choose");
    app.wait_for_enabled_button("Ok");
    app.click_button("Ok");
    app.wait_for_tab("Configuration");
    app.type_into("Design name", "design_a");
    app.wait_for_enabled_button("Add");
    app.click_button("Add");
    app.wait_for_text("design_a");
    app.click_text("design_a");
    app.wait_for_enabled_button("Apply range");

    // unit 2, there is only one design, so its number appears only in the units table.
    app.click_text("2");
    app.wait_for_enabled_button("Assign selected");

    // the only selected unit is no longer shown.
    app.set_number("Units", 1.0);
    app.wait_until_button_is_disabled("Assign selected");

    // the selection comes back with the unit, and then applies to it.
    app.set_number("Units", 4.0);
    app.wait_for_enabled_button("Assign selected");
    assert_eq!(app.count_text("design_a"), 1);
    app.click_button("Assign selected");
    app.wait_for_text_count("design_a", 2);
}

const DESIGN_A_VARIANT_A_PLACEMENTS: &str = indoc! {r#"
    "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
    "R3","RES_MFR1","RES1","true","Top","10","15","90"
    "C1","CAP_MFR1","CAP1","true","Bottom","40","45","180"
    "J1","CONN_MFR1","CONN1","true","Bottom","30","35","-90"
    "R1","RES_MFR1","RES1","true","Top","20","25","0"
"#};

/// What the CLI's `sequence_03_configure_panel_sizing` does.
fn apply_panel_sizing_with_the_core(core: &mut PlannerCoreService, pcb: &Path) {
    let design_sizing = |size: (f64, f64), origin: (f64, f64)| DesignSizing {
        size: Vector2::new(size.0, size.1),
        origin: Vector2::new(origin.0, origin.1),
        gerber_offset: Vector2::new(-20.0, -20.0),
        placement_offset: Vector2::new(-10.0, -10.0),
    };
    update(core, Event::ApplyPartialPanelSizing {
        path: pcb.to_path_buf(),
        edge_rails: Some(Dimensions {
            left: 10.0,
            right: 10.0,
            top: 5.0,
            bottom: 5.0,
        }),
        size: Some(Vector2::new(122.0, 112.0)),
        fiducials: None,
        design_sizings: Some(HashMap::from([
            (DesignName::from("design_a"), design_sizing((60.0, 50.0), (30.0, 25.0))),
            (DesignName::from("design_b"), design_sizing((40.0, 50.0), (20.0, 25.0))),
        ])),
        pcb_unit_positionings: Some(HashMap::from(
            [(1, 10.0, 5.0), (2, 72.0, 5.0), (3, 10.0, 57.0), (4, 72.0, 57.0)].map(|(unit, x, y)| {
                (PcbUnitNumber::try_from(unit).unwrap(), PcbUnitPositioning {
                    offset: Vector2::new(x, y),
                    rotation: Decimal::ZERO,
                })
            }),
        )),
    });
    update(core, Event::SavePcb {
        path: pcb.to_path_buf(),
    });
}

/// The PCB, and the placements, as they are by the CLI's `sequence_05_assign_variant_to_unit`.
fn given_a_panel_and_its_placements(folder: &Path) {
    let pcb = folder.join("panel_a.pcb.json");
    let mut core = PlannerCoreService::new();
    create_pcb_with_the_core(&mut core, &pcb);
    apply_panel_sizing_with_the_core(&mut core, &pcb);
    fs::write(
        folder.join("design_a_variant_a_placements.csv"),
        DESIGN_A_VARIANT_A_PLACEMENTS,
    )
    .unwrap();
}

/// `UNITASG-1`, `UNITASG-2`, `UNITASG-3`, `UNITASG-4`
#[test]
fn assigning_a_variant_to_a_unit_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    given_a_panel_and_its_placements(&folder);

    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");
    app.when_a_file_dialog_opens_answer(Some(folder.join("panel_a.pcb.json")));
    app.click_button("Add PCB");
    app.wait_for_enabled_button("Save");

    app.double_click_text("panel_a");
    app.click_button("Create unit assignment");

    app.choose_from_combo_box("Design name", "design_a");
    app.type_into("Variant name", "variant_a");
    app.wait_for_enabled_button("Add");
    app.click_button("Add");
    app.click_text("variant_a");
    app.wait_for_enabled_button("Apply range");
    app.set_number("PCB units: End", 1.0);
    app.click_button("Apply range");
    // the explorer shows the assignment once the core has made it.
    app.wait_for_text_containing("design_a = variant_a");

    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_04_add_pcb` and `sequence_05_assign_variant_to_unit` do.
    let mut core = PlannerCoreService::new();
    update(&mut core, Event::CreateProject {
        name: "job1".to_string(),
        path: project.clone(),
        packages: None,
        package_mappings: None,
    });
    update(&mut core, Event::AddPcb {
        pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
    });
    update(&mut core, Event::AssignVariantToUnit {
        unit: ObjectPath::from_str("pcb=1::unit=1").unwrap(),
        variant: Some(VariantName::from_str("variant_a").unwrap()),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

pub const PLACEMENTS_WITH_TWO_PARTS: &str = indoc! {r#"
    "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
    "R1","RES_MFR1","RES1","true","Top","20","25","0"
    "J1","CONN_MFR1","CONN1","true","Bottom","30","35","-90"
"#};

const DESIGN_A_VARIANT_A_CHANGED_PLACEMENTS: &str = indoc! {r#"
    "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
    "R3","RES_MFR1","RES1","true","Top","40","40","135"
    "R2","RES_MFR2","RES2","true","Top","30","30","45"
    "J1","CONN_MFR1","CONN1","true","Bottom","10","10","0"
    "R1","RES_MFR1","RES1","true","Top","20","20","-45"
"#};

/// Up to the end of the CLI's `sequence_05_assign_variant_to_unit`, saved.
pub fn create_project_with_an_assigned_variant(app: &mut App, folder: &Path) {
    given_a_panel_and_its_placements(folder);

    create_project(app, folder);
    app.wait_for_tab("Overview");
    app.when_a_file_dialog_opens_answer(Some(folder.join("panel_a.pcb.json")));
    app.click_button("Add PCB");
    app.wait_for_enabled_button("Save");

    app.double_click_text("panel_a");
    app.click_button("Create unit assignment");

    app.choose_from_combo_box("Design name", "design_a");
    app.type_into("Variant name", "variant_a");
    app.wait_for_enabled_button("Add");
    app.click_button("Add");
    app.click_text("variant_a");
    app.wait_for_enabled_button("Apply range");
    app.set_number("PCB units: End", 1.0);
    app.click_button("Apply range");
    app.wait_for_text_containing("design_a = variant_a");

    save(app);
}

/// The same, using the core.
fn project_with_an_assigned_variant_using_the_core(core: &mut PlannerCoreService, folder: &Path) {
    fs::write(
        folder.join("design_a_variant_a_placements.csv"),
        DESIGN_A_VARIANT_A_PLACEMENTS,
    )
    .unwrap();
    update(core, Event::CreateProject {
        name: "job1".to_string(),
        path: folder.join("project-job1.mpnp.json"),
        packages: None,
        package_mappings: None,
    });
    update(core, Event::AddPcb {
        pcb_file: FileReference::Relative("panel_a.pcb.json".into()),
    });
    update(core, Event::AssignVariantToUnit {
        unit: ObjectPath::from_str("pcb=1::unit=1").unwrap(),
        variant: Some(VariantName::from_str("variant_a").unwrap()),
    });
}

/// `PLACEREF-1`
#[test]
fn refreshing_from_changed_placements_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    let placements = folder.join("design_a_variant_a_placements.csv");

    create_project_with_an_assigned_variant(&mut app, &folder);
    fs::write(&placements, DESIGN_A_VARIANT_A_CHANGED_PLACEMENTS).unwrap();

    // the Issues tab has a "Refresh" button of its own.
    app.click_tab("Overview");
    app.click_button("Refresh");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_06_refresh_from_design_variants` does.
    let mut core = PlannerCoreService::new();
    project_with_an_assigned_variant_using_the_core(&mut core, &folder);
    fs::write(&placements, DESIGN_A_VARIANT_A_CHANGED_PLACEMENTS).unwrap();
    update(&mut core, Event::RefreshFromDesignVariants);
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// Up to the end of the CLI's `sequence_07_assign_process_to_parts`, saved.
pub fn create_project_with_a_part_assigned_to_a_process(app: &mut App, folder: &Path) {
    create_project_with_an_assigned_variant(app, folder);
    fs::write(
        folder.join("design_a_variant_a_placements.csv"),
        DESIGN_A_VARIANT_A_CHANGED_PLACEMENTS,
    )
    .unwrap();
    app.click_tab("Overview");
    app.click_button("Refresh");
    app.wait_for_enabled_button("Save");
    save(app);

    app.double_click_text("Parts");
    app.wait_for_tab("Parts");
    app.choose_from_combo_box("Process", "manual");
    app.click_row_header_of("CONN1");
    app.choose_action_from_combo_box("Actions", "Add");
    app.wait_for_enabled_button("Save");
    save(app);
}

/// The same, using the core.
pub fn project_with_a_part_assigned_to_a_process_using_the_core(core: &mut PlannerCoreService, folder: &Path) {
    project_with_an_assigned_variant_using_the_core(core, folder);
    fs::write(
        folder.join("design_a_variant_a_placements.csv"),
        DESIGN_A_VARIANT_A_CHANGED_PLACEMENTS,
    )
    .unwrap();
    update(core, Event::RefreshFromDesignVariants);
    update(core, Event::AssignProcessToParts {
        process: ProcessReference::from_raw_str("manual"),
        operation: AddOrRemoveAction::Add,
        manufacturer: Regex::new("CONN_MFR.*").unwrap(),
        mpn: Regex::new(".*").unwrap(),
    });
}

/// `PARTS-1`, `PARTS-2`
#[test]
fn assigning_a_process_to_a_part_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_a_part_assigned_to_a_process(&mut app, &folder);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_07_assign_process_to_parts` does.
    let mut core = PlannerCoreService::new();
    project_with_a_part_assigned_to_a_process_using_the_core(&mut core, &folder);
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// A file dialog can only choose a file that exists (`PHASE-3`), which the CLI's `create-phase` doesn't need.
pub fn given_a_load_out(folder: &Path, name: &str) -> PathBuf {
    let load_out = folder.join(name);
    fs::write(&load_out, "").unwrap();
    load_out
}

fn add_phase(app: &mut App, reference: &str, process: &str, pcb_side: &str, load_out: &Path) {
    // the Parts tab has a "Process" combo box of its own, which is still drawn behind the dialog.
    app.click_tab("Overview");
    app.click_button("Add phase");
    app.type_into("Phase reference", reference);
    app.choose_from_combo_box("PCB Side", pcb_side);
    app.choose_from_combo_box("Process", process);
    app.when_a_file_dialog_opens_answer(Some(load_out.to_path_buf()));
    app.click_button("Loadout source: Choose");
    app.wait_for_enabled_button("Ok");
    app.click_button("Ok");
    app.wait_for_enabled_button("Save");
}

/// `PHASE-1`, `PHASE-2`, `FORM-2`
#[test]
fn creating_a_phase_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    let load_out = given_a_load_out(&folder, "phase_1_top_1_load_out.csv");

    create_project_with_a_part_assigned_to_a_process(&mut app, &folder);
    add_phase(&mut app, "top_1", "pnp", "Top", &load_out);
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_08_create_phase_top` does.
    let mut core = PlannerCoreService::new();
    project_with_a_part_assigned_to_a_process_using_the_core(&mut core, &folder);
    update(&mut core, Event::CreatePhase {
        process: ProcessReference::from_raw_str("pnp"),
        reference: Reference::from_raw_str("top_1"),
        load_out: LoadOutSource::from_absolute_path(load_out).unwrap(),
        pcb_side: PcbSide::Top,
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// Up to the end of the CLI's `sequence_09_create_phase_bottom`, saved.
fn create_project_with_two_phases(app: &mut App, folder: &Path) {
    let top_load_out = given_a_load_out(folder, "phase_1_top_1_load_out.csv");
    let bottom_load_out = given_a_load_out(folder, "phase_2_bottom_1_load_out.csv");

    create_project_with_a_part_assigned_to_a_process(app, folder);
    add_phase(app, "top_1", "pnp", "Top", &top_load_out);
    save(app);
    add_phase(app, "bottom_1", "manual", "Bottom", &bottom_load_out);
    save(app);
}

/// The same, using the core.
fn project_with_two_phases_using_the_core(core: &mut PlannerCoreService, folder: &Path) {
    let top_load_out = given_a_load_out(folder, "phase_1_top_1_load_out.csv");
    let bottom_load_out = given_a_load_out(folder, "phase_2_bottom_1_load_out.csv");

    project_with_a_part_assigned_to_a_process_using_the_core(core, folder);
    update(core, Event::CreatePhase {
        process: ProcessReference::from_raw_str("pnp"),
        reference: Reference::from_raw_str("top_1"),
        load_out: LoadOutSource::from_absolute_path(top_load_out).unwrap(),
        pcb_side: PcbSide::Top,
    });
    update(core, Event::CreatePhase {
        process: ProcessReference::from_raw_str("manual"),
        reference: Reference::from_raw_str("bottom_1"),
        load_out: LoadOutSource::from_absolute_path(bottom_load_out).unwrap(),
        pcb_side: PcbSide::Bottom,
    });
}

/// `PHASE-1`, `PHASE-2`
#[test]
fn creating_a_second_phase_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_two_phases(&mut app, &folder);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_08_create_phase_top` and `sequence_09_create_phase_bottom` do.
    let mut core = PlannerCoreService::new();
    project_with_two_phases_using_the_core(&mut core, &folder);
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// Up to the end of the CLI's `sequence_10_assign_placements_to_phase`, saved.
pub fn create_project_with_placements_assigned_to_a_phase(app: &mut App, folder: &Path) {
    create_project_with_two_phases(app, folder);

    app.double_click_text("Placements");
    app.wait_for_tab("Placements");
    for ref_des in ["R1", "R2", "R3"] {
        app.click_row_header_of(ref_des);
    }
    app.choose_from_combo_box("Phase", "top_1");
    app.choose_action_from_combo_box("Actions", "Apply");
    app.wait_for_enabled_button("Save");
    save(app);
}

/// The same, using the core.
pub fn project_with_placements_assigned_to_a_phase_using_the_core(core: &mut PlannerCoreService, folder: &Path) {
    project_with_two_phases_using_the_core(core, folder);
    update(core, Event::AssignPlacementsToPhase {
        phase: Reference::from_raw_str("top_1"),
        operation: SetOrClearAction::Set,
        placements: Regex::new("pcb=1::unit=1::ref_des=R.*").unwrap(),
    });
}

/// `PLACEMENTS-1`, `PLACEMENTS-2`
#[test]
fn assigning_placements_to_a_phase_saves_the_same_files_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    let load_out = folder.join("phase_1_top_1_load_out.csv");

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    let saved = fs::read_to_string(&project).unwrap();
    let saved_load_out = fs::read_to_string(&load_out).unwrap();
    assert!(saved_load_out.contains("RES1"));

    // what the CLI's `sequence_10_assign_placements_to_phase` does.
    let mut core = PlannerCoreService::new();
    project_with_placements_assigned_to_a_phase_using_the_core(&mut core, &folder);
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
    assert_eq!(saved_load_out, fs::read_to_string(&load_out).unwrap());
}

/// Up to the end of the CLI's `sequence_11_assign_feeder_to_load_out_item`.  Changing a load-out doesn't change the
/// project, so there is nothing to save.
pub fn create_project_with_a_feeder_assigned(app: &mut App, folder: &Path) {
    create_project_with_placements_assigned_to_a_phase(app, folder);

    app.double_click_text_containing("phase_1_top_1_load_out.csv");
    // only the load-out's table has this column. The Placements tab shows "RES1" twice, and stays shown until the
    // load-out's tab has been opened, which happens on another thread (`GARCH-11`).
    app.wait_for_text("Reference");
    app.edit_data_table_cell("RES1", "Reference", "FEEDER_1", "RES2");
    app.wait_for_text("FEEDER_1");
}

/// The same, using the core.
pub fn project_with_a_feeder_assigned_using_the_core(core: &mut PlannerCoreService, folder: &Path) {
    project_with_placements_assigned_to_a_phase_using_the_core(core, folder);
    update(core, Event::AssignFeederToLoadOutItem {
        phase: Reference::from_raw_str("top_1"),
        feeder_reference: Some(Reference::from_raw_str("FEEDER_1")),
        manufacturer: Regex::new(".*").unwrap(),
        mpn: Regex::new("RES1").unwrap(),
    });
}

/// `LOADOUT-1`, `LOADOUT-2`
#[test]
fn assigning_a_feeder_saves_the_same_load_out_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let load_out = folder.join("phase_1_top_1_load_out.csv");

    create_project_with_a_feeder_assigned(&mut app, &folder);

    app.wait_until_button_is_disabled("Save");
    let saved_load_out = fs::read_to_string(&load_out).unwrap();
    assert!(saved_load_out.contains("FEEDER_1"));

    // what the CLI's `sequence_11_assign_feeder_to_load_out_item` does.
    project_with_a_feeder_assigned_using_the_core(&mut PlannerCoreService::new(), &folder);

    assert_eq!(saved_load_out, fs::read_to_string(&load_out).unwrap());
}

/// Up to the end of the CLI's `sequence_12_set_placement_ordering`, saved.
fn create_project_with_placement_orderings(app: &mut App, folder: &Path) {
    create_project_with_a_feeder_assigned(app, folder);

    app.double_click_text_containing("top_1 (");
    app.click_button("Placement orderings.");
    for (mode, order) in [
        ("PCB instance", "Ascending"),
        ("PCB unit", "Ascending"),
        ("Area", "Ascending"),
        ("Height", "Ascending"),
        ("Feeder reference", "Ascending"),
        ("Ref. Des.", "Descending"),
    ] {
        app.click_text_in_dialog(mode);
        app.click_radio_button(order);
        app.wait_for_enabled_button(">");
        app.click_button(">");
        app.wait_for_text_containing(&format!("{mode} ({order})"));
    }
    app.click_button("Ok");
    app.wait_for_enabled_button("Save");
    save(app);
}

/// The same, using the core.
fn project_with_placement_orderings_using_the_core(core: &mut PlannerCoreService, folder: &Path) {
    project_with_a_feeder_assigned_using_the_core(core, folder);
    update(core, Event::SetPlacementOrdering {
        phase: Reference::from_raw_str("top_1"),
        placement_orderings: [
            (PlacementSortingMode::Pcb, SortOrder::Asc),
            (PlacementSortingMode::PcbUnit, SortOrder::Asc),
            (PlacementSortingMode::Area, SortOrder::Asc),
            (PlacementSortingMode::Height, SortOrder::Asc),
            (PlacementSortingMode::FeederReference, SortOrder::Asc),
            (PlacementSortingMode::RefDes, SortOrder::Desc),
        ]
        .map(|(mode, sort_order)| PlacementSortingItem {
            mode,
            sort_order,
        })
        .to_vec(),
    });
}

/// `ORDERING-1`, `ORDERING-2`
#[test]
fn setting_placement_orderings_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placement_orderings(&mut app, &folder);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_12_set_placement_ordering` does.
    let mut core = PlannerCoreService::new();
    project_with_placement_orderings_using_the_core(&mut core, &folder);
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

const PACKAGES: &str = indoc! {r#"
    "Name","SizeX","SizeY","SizeZ"
    "RES_0603_PKG1","1.6","0.85","0.45"
    "RES_0805_PKG1","2.0","1.25","0.5"
"#};

const PACKAGE_MAPPINGS: &str = indoc! {r#"
    "Manufacturer","Mpn","Name"
    "RES_MFR1","RES1","RES_0603_PKG1"
    "RES_MFR2","RES2","RES_0805_PKG1"
"#};

/// Up to the end of the CLI's `sequence_13_generate_artifacts`, saved.  The CLI's project has its package sources from
/// the start, here they are added when they are first needed.
fn create_project_with_artifacts_generated(app: &mut App, folder: &Path) {
    let packages = folder.join("packages.csv");
    let package_mappings = folder.join("package_mappings.csv");
    fs::write(&packages, PACKAGES).unwrap();
    fs::write(&package_mappings, PACKAGE_MAPPINGS).unwrap();

    create_project_with_placement_orderings(app, folder);

    app.click_button("Package sources");
    app.when_a_file_dialog_opens_answer(Some(packages));
    app.click_button("Packages: Choose");
    app.wait_until_field_is_not_empty("Packages");
    app.when_a_file_dialog_opens_answer(Some(package_mappings));
    app.click_button("Package mappings: Choose");
    app.wait_until_field_is_not_empty("Package mappings");
    app.click_button("Ok");
    app.wait_for_enabled_button("Save");
    save(app);

    app.click_button("Generate artifacts");
    app.wait_until_file_exists(&folder.join("job1_report.json"));
    save(app);
}

/// The same, using the core.
fn project_with_artifacts_generated_using_the_core(core: &mut PlannerCoreService, folder: &Path) {
    project_with_placement_orderings_using_the_core(core, folder);
    update(core, Event::ApplyPackageSources {
        packages_source: Some(PackagesSource::File(folder.join("packages.csv"))),
        package_mappings_source: Some(PackageMappingsSource::File(folder.join("package_mappings.csv"))),
    });
    update(core, Event::GenerateArtifacts);
}

/// `ARTIFACTS-1`
#[test]
fn generating_artifacts_saves_the_same_files_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    let artifacts =
        ["top_1_placements.csv", "bottom_1_placements.csv", "job1_report.json"].map(|name| folder.join(name));

    create_project_with_artifacts_generated(&mut app, &folder);

    let saved = fs::read_to_string(&project).unwrap();
    let saved_artifacts = artifacts
        .clone()
        .map(|artifact| fs::read_to_string(artifact).unwrap());

    // the CLI's test expects these, in this order, as set by the placement orderings and package sizes.
    let positions = ["R3", "R1", "R2"].map(|ref_des| {
        saved_artifacts[0]
            .find(&format!("ref_des={ref_des}"))
            .unwrap()
    });
    assert!(
        positions[0] < positions[1] && positions[1] < positions[2],
        "{}",
        saved_artifacts[0]
    );

    for artifact in &artifacts {
        fs::remove_file(artifact).unwrap();
    }

    // what the CLI's `sequence_13_generate_artifacts` does.
    let mut core = PlannerCoreService::new();
    project_with_artifacts_generated_using_the_core(&mut core, &folder);
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
    assert_eq!(
        saved_artifacts,
        artifacts.map(|artifact| fs::read_to_string(artifact).unwrap())
    );
}

/// Up to the end of the CLI's `sequence_14_record_phase_operations`, saved.
pub fn create_project_with_phase_operations_recorded(app: &mut App, folder: &Path) {
    create_project_with_artifacts_generated(app, folder);

    app.choose_action_from_combo_box("load_pcbs: core::load_pcbs", "Start");
    app.wait_for_combo_box_value("load_pcbs: core::load_pcbs", "Incomplete");
    app.choose_action_from_combo_box("load_pcbs: core::load_pcbs", "Complete");
    app.choose_action_from_combo_box("automated_pnp: core::place_components", "Start");
    app.wait_for_combo_box_value("automated_pnp: core::place_components", "Incomplete");
    save(app);
}

/// The same, using the core.
pub fn project_with_phase_operations_recorded_using_the_core(core: &mut PlannerCoreService, folder: &Path) {
    project_with_artifacts_generated_using_the_core(core, folder);
    for (operation, task, action) in [
        ("load_pcbs", "core::load_pcbs", TaskAction::Start),
        ("load_pcbs", "core::load_pcbs", TaskAction::Complete),
        ("automated_pnp", "core::place_components", TaskAction::Start),
    ] {
        update(core, Event::RecordPhaseOperation {
            phase: Reference::from_raw_str("top_1"),
            operation: Reference::from_raw_str(operation),
            task: Reference::from_raw_str(task),
            action,
        });
    }
}

/// `OPERATIONS-1`
#[test]
fn recording_phase_operations_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_phase_operations_recorded(&mut app, &folder);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_14_record_phase_operations` does.
    let mut core = PlannerCoreService::new();
    project_with_phase_operations_recorded_using_the_core(&mut core, &folder);
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// Up to the end of the CLI's `sequence_15_record_placements_operation`, saved.
pub fn create_project_with_placements_placed(app: &mut App, folder: &Path) {
    create_project_with_phase_operations_recorded(app, folder);

    for ref_des in ["R1", "R2", "R3"] {
        app.choose_in_data_table_cell(ref_des, "Placed?", "Placed");
    }
    app.wait_for_text_count("Placed", 3);
    app.wait_for_enabled_button("Save");
    save(app);
}

/// The same, using the core.
pub fn project_with_placements_placed_using_the_core(core: &mut PlannerCoreService, folder: &Path) {
    project_with_phase_operations_recorded_using_the_core(core, folder);
    update(core, Event::RecordPlacementsOperation {
        object_path_patterns: vec![
            Regex::new("pcb=1::unit=1::ref_des=R([1-3])?").unwrap(),
            Regex::new("pcb=1::unit=2::ref_des=.*").unwrap(),
        ],
        operation: PlacementOperation::Place,
    });
}

/// `OPERATIONS-2`
#[test]
fn recording_placements_as_placed_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placements_placed(&mut app, &folder);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_15_record_placements_operation` does.
    let mut core = PlannerCoreService::new();
    project_with_placements_placed_using_the_core(&mut core, &folder);
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `OPERATIONS-3`
#[test]
fn resetting_operations_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");

    create_project_with_placements_placed(&mut app, &folder);

    app.click_button("Reset operations");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_16_reset_operations` does.
    let mut core = PlannerCoreService::new();
    project_with_placements_placed_using_the_core(&mut core, &folder);
    update(&mut core, Event::ResetOperations {});
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}

/// `PHASE-1`, `FORM-2`, `FORM-3`
#[test]
fn the_load_out_of_the_add_phase_dialog_is_required_and_can_be_cancelled_or_cleared() {
    let mut app = App::start();
    let folder = app.folder();
    let load_out = given_a_load_out(&folder, "load_out.csv");
    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");

    app.click_button("Add phase");
    app.type_into("Phase reference", "top_1");
    app.choose_from_combo_box("PCB Side", "Top");
    app.choose_from_combo_box("Process", "pnp");
    app.wait_until_field_is_empty("Loadout source");

    app.when_a_file_dialog_opens_answer(Some(load_out));
    app.click_button("Loadout source: Choose");
    app.wait_until_field_is_not_empty("Loadout source");
    app.wait_for_enabled_button("Ok");

    // cancelling the file dialog leaves the field as it was.
    app.when_a_file_dialog_opens_answer(None);
    app.click_button("Loadout source: Choose");
    app.wait_for_file_dialogs();
    // a later change is handled after any change caused by the file dialog's answer.
    app.choose_from_combo_box("PCB Side", "Bottom");
    app.wait_until_field_is_not_empty("Loadout source");
    app.wait_for_enabled_button("Ok");

    app.click_button("Loadout source: Clear");
    app.wait_until_field_is_empty("Loadout source");
    app.wait_until_button_is_disabled("Ok");
}

/// `PHASE-2`
#[test]
fn cancelling_the_add_phase_dialog_does_not_change_the_project() {
    let mut app = App::start();
    let folder = app.folder();
    let load_out = given_a_load_out(&folder, "load_out.csv");
    create_project(&mut app, &folder);
    app.wait_for_tab("Overview");

    app.click_button("Add phase");
    app.type_into("Phase reference", "top_1");
    app.choose_from_combo_box("PCB Side", "Top");
    app.choose_from_combo_box("Process", "pnp");
    app.when_a_file_dialog_opens_answer(Some(load_out));
    app.click_button("Loadout source: Choose");
    app.wait_for_enabled_button("Ok");
    app.click_button("Cancel");

    app.wait_until_button_is_gone("Cancel");
    app.wait_until_button_is_disabled("Save");
    assert_eq!(app.count_text("top_1"), 0);
}
