//! Adding a phase's parts to its load-out.

use std::fs;

use planner_app::{Event, Reference};
use planner_gui_egui::planner_app_core::PlannerCoreService;
use regex::Regex;

use crate::fixture::App;
use crate::workflow::*;

/// `LOADOUT-2`, `LOADOUT-4`
#[test]
fn adding_parts_to_a_load_out_writes_the_same_load_out_as_the_core() {
    let mut app = App::start();
    let folder = app.folder();
    let load_out = folder.join("phase_1_top_1_load_out.csv");

    create_project_with_placements_assigned_to_a_phase(&mut app, &folder);

    // the phase's parts were added when its placements were assigned, so start again with an empty load-out.
    fs::write(&load_out, "").unwrap();
    app.double_click_text_containing("phase_1_top_1_load_out.csv");
    app.wait_for_tab("phase_1_top_1_load_out.csv");
    app.wait_until("the load-out was empty", |app| app.count_text("RES1") == 0);

    app.double_click_text_containing("top_1 (");
    app.click_button("Add parts to load-out.");
    app.wait_until("the parts were added to the load-out", |_app| {
        fs::read_to_string(&load_out)
            .unwrap()
            .contains("RES1")
    });
    app.wait_until_button_is_disabled("Save");

    // the load-out's tab shows them.
    app.click_tab_containing("phase_1_top_1_load_out.csv");
    app.wait_for_text("RES1");

    let saved_load_out = fs::read_to_string(&load_out).unwrap();

    let mut core = PlannerCoreService::new();
    project_with_placements_assigned_to_a_phase_using_the_core(&mut core, &folder);
    fs::write(&load_out, "").unwrap();
    update(&mut core, Event::AddPartsToLoadout {
        phase: Reference::from_raw_str("top_1"),
        manufacturer: Regex::new("^.*$").unwrap(),
        mpn: Regex::new("^.*$").unwrap(),
    });

    assert_eq!(saved_load_out, fs::read_to_string(&load_out).unwrap());
}

/// `LOADOUT-4`
#[test]
fn adding_parts_to_a_load_out_keeps_the_parts_already_in_it() {
    let mut app = App::start();
    let folder = app.folder();
    let load_out = folder.join("phase_1_top_1_load_out.csv");

    create_project_with_a_feeder_assigned(&mut app, &folder);

    let remove_res2 = |load_out: &str| {
        load_out
            .lines()
            .filter(|line| !line.contains("\"RES2\""))
            .map(|line| format!("{line}\n"))
            .collect::<String>()
    };
    let without_res2 = remove_res2(&fs::read_to_string(&load_out).unwrap());
    assert!(without_res2.contains("FEEDER_1"), "{without_res2}");
    fs::write(&load_out, &without_res2).unwrap();

    app.double_click_text_containing("top_1 (");
    app.click_button("Add parts to load-out.");
    app.wait_until("the missing part was added to the load-out", |_app| {
        fs::read_to_string(&load_out)
            .unwrap()
            .contains("RES2")
    });

    let saved_load_out = fs::read_to_string(&load_out).unwrap();
    assert!(saved_load_out.contains("FEEDER_1"), "{saved_load_out}");

    let mut core = PlannerCoreService::new();
    project_with_a_feeder_assigned_using_the_core(&mut core, &folder);
    fs::write(&load_out, remove_res2(&fs::read_to_string(&load_out).unwrap())).unwrap();
    update(&mut core, Event::AddPartsToLoadout {
        phase: Reference::from_raw_str("top_1"),
        manufacturer: Regex::new("^.*$").unwrap(),
        mpn: Regex::new("^.*$").unwrap(),
    });

    assert_eq!(saved_load_out, fs::read_to_string(&load_out).unwrap());
}
