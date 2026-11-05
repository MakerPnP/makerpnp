//! Design variants, each a variant builder, created with "New design variant", from an EDA tool's placements, or
//! opened with "Open design variant", whose data sources are chosen, then saved, and built (`VBGUI`, `VARB`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::fixture::App;

const PLACEMENTS: &str = "\"RefDes\",\"Name\",\"Value\",\"Side\",\"X\",\"Y\",\"Rotation\"\n\
                          \"R1\",\"RES_0402\",\"330R\",\"Top\",\"10\",\"20\",\"90\"\n";

const PARTS: &str = "\"Manufacturer\",\"Mpn\"\n\
                     \"MFR1\",\"MPN1\"\n";

const PART_MAPPINGS: &str = "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n\
                             \"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"\n";

/// The data sources of a design with one resistor, `board1`, in the folder.
fn given_a_design(folder: &Path) {
    fs::write(folder.join("board1.csv"), PLACEMENTS).unwrap();
    fs::write(folder.join("parts.csv"), PARTS).unwrap();
    fs::write(folder.join("part-mappings.csv"), PART_MAPPINGS).unwrap();
}

/// The variant builder [`create_design_variant`] creates, in the folder.
const FILE_NAME: &str = "board1_default.variant.json";

/// Chooses DipTrace and the placements `board1.csv` in the folder, in the "New design variant" form.
fn choose_eda_tool_and_placements(app: &mut App, folder: &Path) {
    app.click_button("New design variant");
    app.wait_for_tab("New design variant");
    app.choose_from_combo_box("EDA tool", "DipTrace");
    choose(app, "Placements", folder.join("board1.csv"));
}

/// Creates `board1_default.variant.json` in the folder, with "New design variant".
fn create_design_variant(app: &mut App, folder: &Path) {
    choose_eda_tool_and_placements(app, folder);
    app.wait_for_field_value("Design name", "board1");
    app.wait_for_enabled_button("Ok");
    app.click_button("Ok");
}

/// Chooses the file for the field, with its file dialog.
fn choose(app: &mut App, label: &str, path: PathBuf) {
    app.when_a_file_dialog_opens_answer(Some(path));
    app.click_button(&format!("{label}: Choose"));
    app.wait_for_file_dialogs();
}

/// Adds the file to the field's list, with its file dialog.
fn add(app: &mut App, label: &str, path: PathBuf) {
    app.when_a_file_dialog_opens_answer(Some(path));
    app.click_button(&format!("{label}: Add"));
    app.wait_for_file_dialogs();
}

/// `VBGUI-1` to `VBGUI-4`, `VARB-2`, `VARB-4`, `VARB-5`
#[test]
fn a_new_design_variant_is_configured_saved_and_built() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);

    create_design_variant(&mut app, &folder);

    app.wait_for_tab(FILE_NAME);
    app.wait_until_file_exists(&folder.join(FILE_NAME));
    app.wait_until_no_tab_contains("New design variant");
    // `VBGUI-1`, what it's of is shown, and can't be changed.
    app.wait_for_field_value("EDA tool", "DipTrace");
    app.wait_for_field_value("Placements", "board1.csv");
    app.wait_for_field_value("Design name", "board1");
    app.wait_for_field_value("Variant name", "default");
    assert!(!app.is_button_shown("Placements: Choose"));

    // `VARB-1`, the files chosen are kept relative to the variant builder's folder.
    choose(&mut app, "Parts", folder.join("parts.csv"));
    app.wait_for_field_value("Parts", "parts.csv");
    choose(&mut app, "Part mappings", folder.join("part-mappings.csv"));
    app.wait_for_field_value("Part mappings", "part-mappings.csv");
    app.type_into("No-place list", "R2");

    // `VBGUI-4`, a variant is built from what its file says, so not until the changes are saved.
    app.assert_button_is_disabled("Build");
    app.click_button("Save changes");
    app.wait_for_enabled_button("Build");
    app.assert_button_is_disabled("Save changes");

    let saved = fs::read_to_string(folder.join(FILE_NAME)).unwrap();
    assert!(saved.contains("\"design_name\": \"board1\""), "{saved}");
    assert!(saved.contains("\"variant_name\": \"default\""), "{saved}");
    assert!(saved.contains("\"eda_tool\": \"DipTrace\""), "{saved}");
    assert!(saved.contains("\"placements\": \"board1.csv\""), "{saved}");
    assert!(saved.contains("\"parts\": \"parts.csv\""), "{saved}");

    app.click_button("Build");
    app.wait_for_text("Built.");

    let placements = fs::read_to_string(folder.join("board1_default_placements.csv")).unwrap();
    assert!(placements.contains("\"R1\",\"MFR1\",\"MPN1\""), "{placements}");
    assert!(
        folder
            .join("board1_default_bom.csv")
            .exists()
    );

    app.wait_for_logs_in_order(&[
        "planner_gui_egui::toolbar: Toolbar command. command: NewDesignVariantClicked",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: EdaToolChanged(Some(DipTrace))",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: PickPlacementsClicked",
        // `VBGUI-1`, the placements are chosen from the EDA tool's files.
        "planner_gui_egui::file_picker: Picking a file. extensions: [\"csv\"]",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: PlacementsPicked(",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: Submit",
        "variantbuilder_app: Received event. event: Create {",
        // `STORE-7`, the stores service does the operations, and its results come back as commands.
        "stores_service: Performing stores operation. operation: ReadDocument {",
        "planner_gui_egui::ui_commands: UiApp command. command: VariantBuilderReply(VariantBuilderReply { operation: ReadDocument {",
        "stores_service: Performing stores operation. operation: WriteDocument {",
        "variantbuilder_app: Created variant builder. path: '",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: PickClicked(Parts)",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: FilePicked(Parts, ",
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

/// `VBGUI-1`, the placements are chosen once there's an EDA tool, and again if it's changed, and the design is named
/// after them unless the user has named it.
#[test]
fn a_new_design_variant_is_named_after_its_placements_unless_named() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);

    app.click_button("New design variant");
    app.wait_for_tab("New design variant");
    assert!(!app.is_button_shown("Placements: Choose"));
    app.wait_for_field_value("Design name", "");
    app.wait_for_field_value("Variant name", "default");
    // `VBGUI-8`, the fields are as wide as each other, filling the width.
    let design_name = app.field_rect("Design name");
    let variant_name = app.field_rect("Variant name");
    assert_eq!(design_name.x_range(), variant_name.x_range());
    assert!(design_name.width() > 1500.0, "{design_name:?}");

    app.choose_from_combo_box("EDA tool", "DipTrace");
    choose(&mut app, "Placements", folder.join("board1.csv"));
    app.wait_for_field_value("Design name", "board1");
    choose(&mut app, "Placements", folder.join("board2.csv"));
    app.wait_for_field_value("Design name", "board2");

    app.type_into("Design name", "design1");
    choose(&mut app, "Placements", folder.join("board1.csv"));
    app.wait_for_field_value(
        "Placements",
        &folder
            .join("board1.csv")
            .display()
            .to_string(),
    );
    app.wait_for_field_value("Design name", "design1");

    app.choose_from_combo_box("EDA tool", "KiCad");
    app.wait_for_field_value("Placements", "");
    app.wait_until_button_is_disabled("Ok");

    choose(&mut app, "Placements", folder.join("board1.csv"));
    app.type_into("Variant name", "variant1");
    app.wait_for_enabled_button("Ok");
    app.click_button("Ok");

    app.wait_for_tab("design1_variant1.variant.json");
    app.wait_until_file_exists(&folder.join("design1_variant1.variant.json"));
    app.wait_for_field_value("EDA tool", "KiCad");
}

/// `VBGUI-2`, `VARB-3`
#[test]
fn open_design_variant_shows_what_its_file_says() {
    let mut app = App::start();
    let folder = app.folder();
    let path = folder.join("job1.variant.json");
    fs::write(
        &path,
        r#"{
            "design_name": "board1",
            "variant_name": "Variant_1",
            "eda_tool": "KiCad",
            "placements": "placements.csv",
            "global_substitutions": ["a.csv", "b.csv"],
            "design_substitutions": ["c.csv"],
            "ref_des_list": ["R1", "R2"],
            "ref_des_exclusion_list": ["TP1"]
        }"#,
    )
    .unwrap();

    app.when_a_file_dialog_opens_answer(Some(path));
    app.click_button("Open design variant");

    app.wait_for_tab("job1.variant.json");
    app.wait_for_field_value("Design name", "board1");
    app.wait_for_field_value("Variant name", "Variant_1");
    app.wait_for_field_value("EDA tool", "KiCad");
    app.wait_for_field_value("Placements", "placements.csv");
    app.wait_for_field_value("Parts", "");
    app.wait_for_button("Global substitutions: Remove a.csv");
    app.wait_for_button("Global substitutions: Remove b.csv");
    app.wait_for_button("Design substitutions: Remove c.csv");
    app.wait_for_field_value("Ref-des list", "R1, R2");
    app.wait_for_field_value("Exclusion list", "TP1");
    app.wait_for_field_value("No-place list", "");

    app.wait_for_logs_in_order(&[
        "planner_gui_egui::toolbar: Toolbar command. command: OpenDesignVariantClicked",
        "planner_gui_egui::ui_commands: UiApp command. command: OpenVariantBuilderFile(",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: Open",
        "variantbuilder_app: Received event. event: Open {",
        "variantbuilder_app: Opened variant builder. path: '",
    ]);
    app.assert_not_logged("Event failed.");
}

/// The table of the substitutions as they're applied, its header and its rows (`VBGUI-7`).
fn applied(app: &mut App) -> Vec<Vec<String>> {
    app.rows_below("Substitutions, in the order they're applied")
        .into_iter()
        .take_while(|row| row.len() == 3)
        .collect()
}

/// `VBGUI-2`, `VBGUI-8`, each field is as wide as the others, filling the width, resized with the window, and the
/// buttons of a list of files are lined up by its table.
#[test]
fn the_fields_fill_the_width_and_the_buttons_of_a_list_line_up() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);
    add(&mut app, "Design substitutions", folder.join("d1.csv"));
    add(&mut app, "Design substitutions", folder.join("a-longer-name.csv"));

    let widths = |app: &mut App| {
        ["Placements", "Exclusion list", "Output BOM"]
            .map(|label| app.field_rect(label))
            .map(|rect| (rect.left(), rect.right()))
    };
    let [placements, exclusion_list, output_bom] = widths(&mut app);
    assert_eq!(placements.0, exclusion_list.0);
    assert_eq!(placements.0, output_bom.0);
    assert_eq!(exclusion_list.1, output_bom.1);

    for action in ["Move up", "Move down", "Remove"] {
        let first = app.button_rect(&format!("Design substitutions: {action} d1.csv"));
        let second = app.button_rect(&format!("Design substitutions: {action} a-longer-name.csv"));
        assert_eq!(first.center().x, second.center().x, "{action}");
    }

    app.resize_window(egui::vec2(1200.0, 1080.0));
    let [_, narrower, _] = widths(&mut app);
    assert_eq!(narrower.0, exclusion_list.0);
    assert!(
        narrower.1 < exclusion_list.1 - 600.0,
        "{narrower:?}, {exclusion_list:?}"
    );
}

/// `VBGUI-2`, `VBGUI-7`, `VARB-5`, the substitutions are reordered, and shown in the order they're applied, the design's
/// first.
#[test]
fn substitutions_are_reordered_and_shown_in_the_order_they_are_applied() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);
    app.wait_for_text("(none)");

    add(&mut app, "Global substitutions", folder.join("g1.csv"));
    add(&mut app, "Design substitutions", folder.join("d1.csv"));
    add(&mut app, "Design substitutions", folder.join("d2.csv"));
    app.wait_until_text_is_gone("(none)");
    assert_eq!(applied(&mut app), [
        ["Order", "File", "Source"],
        ["1", "d1.csv", "Design"],
        ["2", "d2.csv", "Design"],
        ["3", "g1.csv", "Global"],
    ]);
    app.assert_button_is_disabled("Design substitutions: Move up d1.csv");
    app.assert_button_is_disabled("Design substitutions: Move down d2.csv");
    app.assert_button_is_disabled("Global substitutions: Move up g1.csv");
    app.assert_button_is_disabled("Global substitutions: Move down g1.csv");

    app.click_button("Design substitutions: Move down d1.csv");
    app.wait_for_enabled_button("Design substitutions: Move up d1.csv");
    assert_eq!(applied(&mut app), [
        ["Order", "File", "Source"],
        ["1", "d2.csv", "Design"],
        ["2", "d1.csv", "Design"],
        ["3", "g1.csv", "Global"],
    ]);

    app.click_button("Design substitutions: Move up d1.csv");
    app.wait_until_button_is_disabled("Design substitutions: Move up d1.csv");
    app.click_button("Design substitutions: Move down d1.csv");
    app.wait_until_button_is_disabled("Design substitutions: Move down d1.csv");

    app.click_button("Save changes");
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: MoveDownClicked(DesignSubstitutions, 0)",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: MoveUpClicked(DesignSubstitutions, 1)",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: MoveDownClicked(DesignSubstitutions, 0)",
        "variantbuilder_app: Received event. event: Configure {",
        "variantbuilder_app: Configured variant builder. path: '",
    ]);

    let saved = fs::read_to_string(folder.join(FILE_NAME)).unwrap();
    assert!(
        saved.contains("\"design_substitutions\": [\n        \"d2.csv\",\n        \"d1.csv\"\n    ],"),
        "{saved}"
    );
}

/// `VBGUI-2`, files are added to a list, and removed from it, until the changes are saved.
#[test]
fn substitutions_are_added_and_removed() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);

    add(&mut app, "Global substitutions", folder.join("a.csv"));
    add(&mut app, "Global substitutions", folder.join("b.csv"));
    add(&mut app, "Design substitutions", folder.join("c.csv"));
    app.wait_for_button("Design substitutions: Remove c.csv");
    app.click_button("Global substitutions: Remove a.csv");
    app.wait_until_button_is_gone("Global substitutions: Remove a.csv");

    app.click_button("Save changes");
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: AddClicked(GlobalSubstitutions)",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: FilesPicked(GlobalSubstitutions, [",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: RemoveClicked(GlobalSubstitutions, 0)",
        "variantbuilder_app: Received event. event: Configure {",
        "variantbuilder_app: Configured variant builder. path: '",
    ]);

    let saved = fs::read_to_string(folder.join(FILE_NAME)).unwrap();
    assert!(
        saved.contains("\"global_substitutions\": [\n        \"b.csv\"\n    ],"),
        "{saved}"
    );
    assert!(
        saved.contains("\"design_substitutions\": [\n        \"c.csv\"\n    ],"),
        "{saved}"
    );
}

/// `VBGUI-6`, `VARB-8`
#[test]
fn new_assembly_rules_are_created_and_chosen() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);

    app.when_a_file_dialog_opens_answer(Some(folder.join("rules.csv")));
    app.click_button("Assembly rules: New");
    app.wait_for_file_dialogs();

    app.wait_for_field_value("Assembly rules", "rules.csv");
    app.wait_until_file_exists(&folder.join("rules.csv"));
    assert_eq!(
        fs::read_to_string(folder.join("rules.csv")).unwrap(),
        "\"RefDes\",\"Manufacturer\",\"Mpn\"\n"
    );
    let saved = fs::read_to_string(folder.join(FILE_NAME)).unwrap();
    assert!(saved.contains("\"assembly_rules\": \"rules.csv\""), "{saved}");

    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: NewAssemblyRulesClicked",
        "planner_gui_egui::file_picker: Picking a file to save. file_name: 'board1_default_assembly_rules.csv'",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: NewAssemblyRulesPicked(",
        "variantbuilder_app: Received event. event: CreateAssemblyRules {",
        "variantbuilder_app: Created assembly rules. path: '",
    ]);
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-1`, `VARB-2`, a variant builder whose file exists isn't created, and the file is kept.
#[test]
fn a_new_design_variant_does_not_replace_a_file() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    let path = folder.join(FILE_NAME);
    fs::write(&path, "{}").unwrap();

    create_design_variant(&mut app, &folder);

    app.wait_for_text_containing("Variant builder exists already.");
    assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
    app.wait_for_logs_in_order(&[
        "variantbuilder_app: Received event. event: Create {",
        "variantbuilder_app: Event failed. error: ExistsAlready {",
    ]);
}

/// `VBGUI-3`, `VARB-5`, a variant builder that isn't complete says what it's missing when it's built.
#[test]
fn building_an_incomplete_design_variant_says_what_is_missing() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);

    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);
    app.wait_for_enabled_button("Build");
    app.click_button("Build");

    app.wait_for_text_containing("Variant builder incomplete.");
    app.wait_for_text_containing("\"parts\", \"part_mappings\"");
    assert!(
        !folder
            .join("board1_default_placements.csv")
            .exists()
    );
}
