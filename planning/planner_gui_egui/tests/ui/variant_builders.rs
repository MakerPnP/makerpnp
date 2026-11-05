//! Variant builders, created with "New variant builder", or opened with "Open variant builder", whose variant is
//! defined, and data sources chosen, then saved, and built (`VBGUI`, `VARB`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::fixture::App;

const PLACEMENTS: &str = "\"RefDes\",\"Name\",\"Value\",\"Side\",\"X\",\"Y\",\"Rotation\"\n\
                          \"R1\",\"RES_0402\",\"330R\",\"Top\",\"10\",\"20\",\"90\"\n";

const PARTS: &str = "\"Manufacturer\",\"Mpn\"\n\
                     \"MFR1\",\"MPN1\"\n";

const PART_MAPPINGS: &str = "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n\
                             \"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"\n";

/// The data sources of a design with one resistor, in the folder.
fn given_a_design(folder: &Path) {
    fs::write(folder.join("placements.csv"), PLACEMENTS).unwrap();
    fs::write(folder.join("parts.csv"), PARTS).unwrap();
    fs::write(folder.join("part-mappings.csv"), PART_MAPPINGS).unwrap();
}

/// Creates `job1.variant.json` in the folder, with "New variant builder".
fn create_variant_builder(app: &mut App, folder: &Path) {
    app.click_button("New variant builder");
    app.wait_for_tab("New variant builder");
    app.type_into("Variant builder name", "job1");
    app.when_a_file_dialog_opens_answer(Some(folder.to_path_buf()));
    app.click_button("Directory: Choose");
    app.wait_for_file_dialogs();
    app.wait_for_enabled_button("Ok");
    app.click_button("Ok");
}

/// Chooses the file for the field, with its file dialog.
fn choose(app: &mut App, label: &str, path: PathBuf) {
    app.when_a_file_dialog_opens_answer(Some(path));
    app.click_button(&format!("{label}: Choose"));
    app.wait_for_file_dialogs();
}

/// `VBGUI-1` to `VBGUI-4`, `VARB-2`, `VARB-4`, `VARB-5`
#[test]
fn a_new_variant_builder_is_configured_saved_and_built() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);

    create_variant_builder(&mut app, &folder);

    app.wait_for_tab("job1.variant.json");
    app.wait_until_file_exists(&folder.join("job1.variant.json"));
    app.wait_for_field_value("Variant name", "Default");
    app.wait_until_no_tab_contains("New variant builder");

    app.choose_from_combo_box("EDA tool", "DipTrace");
    // `VARB-1`, the files chosen are kept relative to the variant builder's folder.
    choose(&mut app, "Placements", folder.join("placements.csv"));
    app.wait_for_field_value("Placements", "placements.csv");
    choose(&mut app, "Parts", folder.join("parts.csv"));
    app.wait_for_field_value("Parts", "parts.csv");
    choose(&mut app, "Part mappings", folder.join("part-mappings.csv"));
    app.wait_for_field_value("Part mappings", "part-mappings.csv");
    app.type_into("Variant name", "Variant_1");

    // `VBGUI-4`, a variant is built from what its file says, so not until the changes are saved.
    app.assert_button_is_disabled("Build");
    app.click_button("Save changes");
    app.wait_for_enabled_button("Build");
    app.assert_button_is_disabled("Save changes");

    let saved = fs::read_to_string(folder.join("job1.variant.json")).unwrap();
    assert!(saved.contains("\"name\": \"Variant_1\""), "{saved}");
    assert!(saved.contains("\"eda_tool\": \"DipTrace\""), "{saved}");
    assert!(saved.contains("\"placements\": \"placements.csv\""), "{saved}");

    app.click_button("Build");
    app.wait_for_text("Built.");

    let placements = fs::read_to_string(folder.join("job1_placements.csv")).unwrap();
    assert!(placements.contains("\"R1\",\"MFR1\",\"MPN1\""), "{placements}");
    assert!(folder.join("job1_bom.csv").exists());

    app.wait_for_logs_in_order(&[
        "planner_gui_egui::toolbar: Toolbar command. command: NewVariantBuilderClicked",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: Submit",
        "variantbuilder_app: Received event. event: Create {",
        // `STORE-7`, the stores service does the operations, and its results come back as commands.
        "stores_service: Performing stores operation. operation: ReadDocument {",
        "planner_gui_egui::ui_commands: UiApp command. command: VariantBuilderReply(VariantBuilderReply { operation: ReadDocument {",
        "stores_service: Performing stores operation. operation: WriteDocument {",
        "variantbuilder_app: Created variant builder. path: '",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: Changed(EdaTool(Some(DipTrace)))",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: PickClicked(Placements)",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: FilesPicked(Placements, [",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: SaveClicked",
        "variantbuilder_app: Received event. event: Configure {",
        "variantbuilder_app: Configured variant builder. path: '",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: BuildClicked",
        "variantbuilder_app: Received event. event: Build {",
        "variantbuilder_app::variant_builders: Building variant. path: '",
        "variantbuilder_app: Built variant. path: '",
    ]);
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-2`, `VARB-3`
#[test]
fn open_variant_builder_shows_what_its_file_says() {
    let mut app = App::start();
    let folder = app.folder();
    let path = folder.join("job1.variant.json");
    fs::write(
        &path,
        r#"{
            "variant": { "name": "Variant_1", "ref_des_list": ["R1", "R2"] },
            "eda_tool": "KiCad",
            "placements": "placements.csv",
            "substitutions": ["a.csv", "b.csv"],
            "ref_des_exclude_list": ["TP1"]
        }"#,
    )
    .unwrap();

    app.when_a_file_dialog_opens_answer(Some(path));
    app.click_button("Open variant builder");

    app.wait_for_tab("job1.variant.json");
    app.wait_for_field_value("Variant name", "Variant_1");
    app.wait_for_field_value("Ref-des list", "R1, R2");
    app.wait_for_combo_box_value("EDA tool", "KiCad");
    app.wait_for_field_value("Placements", "placements.csv");
    app.wait_for_field_value("Parts", "");
    app.wait_for_field_value("Substitutions", "a.csv, b.csv");
    app.wait_for_field_value("Ref-des excluded", "TP1");

    app.wait_for_logs_in_order(&[
        "planner_gui_egui::toolbar: Toolbar command. command: OpenVariantBuilderClicked",
        "planner_gui_egui::ui_commands: UiApp command. command: OpenVariantBuilderFile(",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: Open",
        "variantbuilder_app: Received event. event: Open {",
        "variantbuilder_app: Opened variant builder. path: '",
    ]);
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-1`, `VARB-2`, a variant builder whose file exists isn't created, and the file is kept.
#[test]
fn a_new_variant_builder_does_not_replace_a_file() {
    let mut app = App::start();
    let folder = app.folder();
    let path = folder.join("job1.variant.json");
    fs::write(&path, "{}").unwrap();

    create_variant_builder(&mut app, &folder);

    app.wait_for_text_containing("Variant builder exists already.");
    assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
    app.wait_for_logs_in_order(&[
        "variantbuilder_app: Received event. event: Create {",
        "variantbuilder_app: Event failed. error: ExistsAlready {",
    ]);
}

/// `VBGUI-3`, `VARB-5`, a variant builder that isn't complete says what it's missing when it's built.
#[test]
fn building_an_incomplete_variant_builder_says_what_is_missing() {
    let mut app = App::start();
    let folder = app.folder();

    create_variant_builder(&mut app, &folder);
    app.wait_for_tab("job1.variant.json");
    app.wait_for_enabled_button("Build");
    app.click_button("Build");

    app.wait_for_text_containing("Variant builder incomplete.");
    app.wait_for_text_containing("\"eda_tool\", \"placements\", \"parts\", \"part_mappings\"");
    assert!(
        !folder
            .join("job1_placements.csv")
            .exists()
    );
}
