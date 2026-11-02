//! The same workflow as the planner CLI's `operation_sequence_1` tests, done through the GUI.
//!
//! The GUI has to save the same files as the core does when given the events the CLI gives it.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::str::FromStr;

use indoc::indoc;
use nalgebra::Vector2;
use planner_app::{
    AddOrRemoveAction, DesignName, DesignSizing, Dimensions, Event, FileReference, ObjectPath, PackageMappingsSource,
    PackagesSource, PcbUnitNumber, PcbUnitPositioning, ProcessReference, VariantName,
};
use planner_gui_egui::planner_app_core::PlannerCoreService;
use regex::Regex;
use rust_decimal::Decimal;

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

fn save(app: &mut App) {
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

/// `PARTS-1`, `PARTS-2`
#[test]
fn assigning_a_process_to_a_part_saves_the_same_file_as_the_cli() {
    let mut app = App::start();
    let folder = app.folder();
    let project = folder.join("project-job1.mpnp.json");
    let placements = folder.join("design_a_variant_a_placements.csv");

    create_project_with_an_assigned_variant(&mut app, &folder);
    fs::write(&placements, DESIGN_A_VARIANT_A_CHANGED_PLACEMENTS).unwrap();
    app.click_tab("Overview");
    app.click_button("Refresh");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    app.double_click_text("Parts");
    app.wait_for_tab("Parts");
    app.choose_from_combo_box("Process", "manual");
    app.click_row_header_of("CONN1");
    app.choose_from_combo_box("Actions", "Add");
    app.wait_for_enabled_button("Save");
    save(&mut app);

    let saved = fs::read_to_string(&project).unwrap();

    // what the CLI's `sequence_07_assign_process_to_parts` does.
    let mut core = PlannerCoreService::new();
    project_with_an_assigned_variant_using_the_core(&mut core, &folder);
    fs::write(&placements, DESIGN_A_VARIANT_A_CHANGED_PLACEMENTS).unwrap();
    update(&mut core, Event::RefreshFromDesignVariants);
    update(&mut core, Event::AssignProcessToParts {
        process: ProcessReference::from_raw_str("manual"),
        operation: AddOrRemoveAction::Add,
        manufacturer: Regex::new("CONN_MFR.*").unwrap(),
        mpn: Regex::new(".*").unwrap(),
    });
    update(&mut core, Event::Save);

    assert_eq!(saved, fs::read_to_string(&project).unwrap());
}
