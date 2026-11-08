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

/// `VBGUI-9`, `VBGUI-10`, a design variant has the explorer and the "Overview" panels, and the explorer has a node for
/// each data source, as the "Overview" shows them, in folders named after its groups, and the build's outputs.
#[test]
fn a_design_variant_has_an_explorer_and_an_overview() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);
    app.wait_for_tab("Explorer");
    app.wait_for_tab("Overview");

    // with the changes not yet saved, as the "Overview" shows them.
    choose(&mut app, "Parts", folder.join("parts.csv"));
    add(&mut app, "Global substitutions", folder.join("g1.csv"));

    for text in [
        FILE_NAME,
        "Overview",
        "Placements: board1.csv",
        "Parts: parts.csv",
        "Part mappings: (none)",
        "Assembly rules: (none)",
        "Build output",
        "BOM",
    ] {
        app.wait_for_text(text);
    }
    // a folder for each group, and each list, named as the "Overview" names them.
    for text in [
        "Part matching",
        "Global substitutions",
        "Design specific",
        "Design substitutions",
    ] {
        app.wait_for_text_count(text, 2);
    }
    // in the explorer, the list, and the substitutions as they're applied.
    app.wait_for_text_count("g1.csv", 3);
}

/// `VBGUI-10`, `VBGUI-11`, `STORE-17`, double-clicking a data source shows its table, and double-clicking the variant
/// builder, or "Overview", shows the "Overview" again.
#[test]
fn double_clicking_a_data_source_shows_its_table() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);
    choose(&mut app, "Parts", folder.join("parts.csv"));
    choose(&mut app, "Part mappings", folder.join("part-mappings.csv"));

    app.double_click_text("Part mappings: part-mappings.csv");
    app.wait_for_tab("Part mappings: part-mappings.csv");
    app.wait_for_text("Eda");
    app.wait_for_text("RES_0402");
    // shown in place of the "Overview", beside the explorer.
    app.wait_until_button_is_gone("Save changes");

    app.double_click_text("Overview");
    app.wait_for_enabled_button("Save changes");

    app.double_click_text("Parts: parts.csv");
    app.wait_for_tab("Parts: parts.csv");
    app.wait_for_text("MFR1");
    app.wait_for_text("MPN1");

    app.double_click_text("Placements: board1.csv");
    app.wait_for_tab("Placements: board1.csv");
    app.wait_for_text("RefDes");
    app.wait_for_text("R1");

    app.double_click_text(FILE_NAME);
    app.wait_for_enabled_button("Save changes");

    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: ShowPanel(DataSource { source: PartMappings, path: \"part-mappings.csv\" })",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: OpenTable(PartMappings, ",
        "stores_app: Received event. event: OpenTable { kind: PartMappings, source: ",
        "stores_service: Performing stores operation. operation: ReadTable { kind: PartMappings, source: ",
        "stores_app: Loaded table. kind: PartMappings, source: '",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: ShowPanel(Overview)",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: ShowPanel(DataSource { source: Parts, ",
        "stores_app: Received event. event: OpenParts {",
        "stores_app: Loaded parts. source: '",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: ShowPanel(DataSource { source: Placements, ",
        "stores_app: Loaded table. kind: EdaPlacements { eda_tool: DipTrace }, source: '",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: ShowPanel(Overview)",
    ]);
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-11`, each of a data source's columns can be resized, by dragging the edge on its right.
///
/// Regression test for `ISSUE-24`: dragging the edge between two column headers dragged the header, as if to reorder
/// the columns, rather than resizing the column on its left, in every `egui_deferred_table` table. The header cell
/// sensed drags over its resize handle and, made after it, won egui's hit test; the header's interactive area now
/// leaves out the handles, in `egui_deferred_table`.
#[test]
fn a_data_sources_columns_can_be_resized() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);
    choose(&mut app, "Part mappings", folder.join("part-mappings.csv"));

    app.double_click_text("Part mappings: part-mappings.csv");
    app.wait_for_tab("Part mappings: part-mappings.csv");
    app.wait_for_text("RES_0402");

    for (column, next_column) in [("Eda", "Name"), ("Name", "Value"), ("Value", "Manufacturer")] {
        let before = app.text_position(next_column).x;
        app.drag_column_edge(next_column, 50.0);
        let after = app.text_position(next_column).x;
        assert!(
            (after - before - 50.0).abs() < 5.0,
            "'{column}' is wider by {}",
            after - before
        );
    }
}

/// `VBGUI-11`, `STORE-17`, a part mapping's patterns are shown as the data source has them, an exact value, a valid
/// regular expression and one that isn't, which says why when it's hovered. Their colours are checked by
/// `data_sources::table`'s tests, as the GUI's tests can't see them.
#[test]
fn a_part_mappings_patterns_are_shown_and_an_invalid_one_says_why() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    fs::write(
        folder.join("patterns.csv"),
        "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n\
         \"DipTrace\",\"/RES_.*/\",\"/330R[/\",\"MFR1\",\"MPN1\"\n",
    )
    .unwrap();
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);
    choose(&mut app, "Part mappings", folder.join("patterns.csv"));

    app.double_click_text("Part mappings: patterns.csv");
    app.wait_for_tab("Part mappings: patterns.csv");
    for text in ["DipTrace", "/RES_.*/", "/330R[/", "MFR1", "MPN1"] {
        app.wait_for_text(text);
    }
    // the columns as the data source names them, the EDA tool's fields, then the part.
    let headers = ["Eda", "Name", "Value", "Manufacturer", "Mpn"].map(|header| app.text_position(header).x);
    assert!(headers.is_sorted(), "{headers:?}");

    // red, as DipTrace matches its field, saying why it isn't valid (`VBGUI-13`).
    app.hover_text("/330R[/");
    app.wait_for_text_containing("regex parse error");
    app.wait_for_text_containing("Matched by DipTrace.");
}

/// `VBGUI-13`, a part mapping's status, the first column, a circle, says when it's hovered that the row has no problems,
/// or what they are. Its colours, and the manufacturer's and MPN's, are checked by `data_sources::table`'s tests, as the
/// GUI's tests can't see them.
#[test]
fn a_part_mappings_status_says_what_its_problems_are() {
    let mut app = App::start();
    given_part_mappings_shown(
        &mut app,
        "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n\
         \"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"\n\
         \"DipTrace\",\"/R_[/\",\"\",\"MFR2\",\"\"\n\
         \"KiCad\",\"CAP_0603\",\"\",\"MFR3\",\"MPN3\"\n",
    );
    app.wait_for_text("MFR3");
    // first, before the columns the data source has.
    let headers = ["◑", "Eda", "Name"].map(|header| app.text_position(header).x);
    assert!(headers.is_sorted(), "{headers:?}");

    app.hover_data_table_cell("RES_0402", "◑");
    app.wait_for_text_containing("No problems.");

    app.hover_data_table_cell("MFR2", "◑");
    app.wait_for_text_containing("A pattern isn't valid.");
    app.wait_for_text_containing("The manufacturer, or the MPN, is empty.");
    app.wait_until_no_text_contains("No problems.");

    // KiCad's `package` and `val` have no columns, so neither has a pattern.
    app.hover_data_table_cell("MFR3", "◑");
    app.wait_for_text_containing("The data source has no column for one of KiCad's fields.");
    app.wait_for_text_containing("None of KiCad's fields has a pattern.");
    app.wait_until_no_text_contains("is empty.");
    app.assert_not_logged("Event failed.");
}

const TWO_MAPPINGS: &str = "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n\
                            \"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"\n\
                            \"DipTrace\",\"CAP_0603\",\"10u\",\"MFR2\",\"MPN2\"\n";

/// Creates the design variant, with the part mappings `mappings.csv`, written with `contents`, and shows their panel.
fn given_part_mappings_shown(app: &mut App, contents: &str) -> PathBuf {
    let folder = app.folder();
    given_a_design(&folder);
    let mappings = folder.join("mappings.csv");
    fs::write(&mappings, contents).unwrap();
    create_design_variant(app, &folder);
    app.wait_for_tab(FILE_NAME);
    choose(app, "Part mappings", mappings.clone());

    app.double_click_text("Part mappings: mappings.csv");
    app.wait_for_tab("Part mappings: mappings.csv");
    mappings
}

/// Waits for the file to have the rows, after the headers `TWO_MAPPINGS` has.
fn wait_for_mappings(app: &mut App, path: &Path, rows: &[&str]) {
    let mut expected = "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n".to_string();
    for row in rows {
        expected.push_str(row);
        expected.push('\n');
    }
    app.wait_until(&format!("the file was {expected}"), |_app| {
        fs::read_to_string(path).is_ok_and(|contents| contents == expected)
    });
}

/// `VBGUI-11`, a table too big for its panel is only as tall as the space below what's above it, so its last rows can be
/// scrolled to, rather than being drawn below the panel, where they can't be seen.
///
/// Regression test for `ISSUE-25`: a data source's table, too big to show every row, couldn't be scrolled to its last
/// rows, which were under its horizontal scroll bar, or not drawn at all, unlike the table in `egui_deferred_table`'s
/// 'docking' example's 'Spreadsheet' tab. An `egui_deferred_table` is as big as its `Ui`'s max rect, from where its next
/// widget goes, so below the data source's file, and the row buttons, it went past the bottom of the panel, which
/// doesn't scroll, by as much as they're tall; the example's tab scrolls, so its table can be scrolled into view.
/// `egui_deferred_table` now takes the space left in its `Ui`, from where its next widget goes.
#[test]
fn a_table_too_big_for_its_panel_ends_within_it() {
    let mut app = App::start();
    let mut contents = "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n".to_string();
    for row in 0..200 {
        contents.push_str(&format!("\"DipTrace\",\"R{row}\",\"330R\",\"MFR1\",\"ROW_MPN{row}\"\n"));
    }
    given_part_mappings_shown(&mut app, &contents);
    app.wait_for_text("ROW_MPN0");

    app.scroll_to_the_end_over("ROW_MPN1");

    // the last row, scrolled to, is shown, within the window.
    assert_last_row_is_shown(&mut app, "ROW_MPN199");

    // as the parts are, shown as the parts view shows them.
    let mut parts = "\"Manufacturer\",\"Mpn\"\n".to_string();
    for row in 0..200 {
        parts.push_str(&format!("\"MFR1\",\"PART_MPN{row}\"\n"));
    }
    let folder = app.folder();
    fs::write(folder.join("many-parts.csv"), parts).unwrap();
    app.double_click_text(FILE_NAME);
    choose(&mut app, "Parts", folder.join("many-parts.csv"));
    app.double_click_text("Parts: many-parts.csv");
    app.wait_for_tab("Parts: many-parts.csv");
    app.wait_for_text("PART_MPN0");
    app.scroll_to_the_end_over("PART_MPN1");
    assert_last_row_is_shown(&mut app, "PART_MPN199");

    app.wait_for_logs_in_order(&[
        "stores_app: Loaded table. kind: PartMappings, source: '",
        "stores_app: Loaded parts. source: '",
    ]);
    app.assert_not_logged("Event failed.");
}

/// The table's last row, with the text, is shown, above the table's horizontal scroll bar, within the window. Below what's
/// above the table, the table went past the bottom, by as much as what's above it, its last row then under the scroll
/// bar, or not drawn at all.
fn assert_last_row_is_shown(app: &mut App, text: &str) {
    app.wait_for_text(text);
    let bottom = app.window_rect().max.y - app.scroll_bar_size();
    let last = app.label_rects_starting_with(text);
    assert!(
        last.iter()
            .all(|row| row.max.y <= bottom),
        "the last row is below the table's scroll bar: {last:?}, it starts at {bottom}"
    );
}

/// `VBGUI-13`, a row added to a table too big for its panel is brought into view, and selected.
#[test]
fn a_row_added_is_brought_into_view_and_selected() {
    let mut app = App::start();
    let mut contents = "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n".to_string();
    for row in 0..200 {
        contents.push_str(&format!("\"DipTrace\",\"R{row}\",\"330R\",\"MFR1\",\"ROW_MPN{row}\"\n"));
    }
    let mappings = given_part_mappings_shown(&mut app, &contents);
    app.wait_for_text("ROW_MPN0");
    assert_eq!(
        app.count_text("ROW_MPN199"),
        0,
        "the last row is shown before it's scrolled to"
    );

    app.click_button("Add row");
    app.wait_until("the row was added", |_app| {
        fs::read_to_string(&mappings).is_ok_and(|contents| contents.ends_with("\"DipTrace\",\"\",\"\",\"\",\"\"\n"))
    });

    // the row added, the 201st, shown within the window, and selected, so as the last row, it can't be moved down.
    assert_last_row_is_shown(&mut app, "201");
    app.wait_for_enabled_button("Move up");
    app.assert_button_is_disabled("Move down");
    app.wait_for_logs_in_order(&[
        "ChangeRows(Add {",
        "stores_app: Loaded table. kind: PartMappings, source: '",
        "ChangeShown(Row(200))",
        "ScrolledToRow",
    ]);
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-13`, `STORE-19`, a part mapping's values are edited in their cells, each written once its edit is finished,
/// by clicking another row, or by pressing Enter, and the EDA tool is chosen from a list.
#[test]
fn a_part_mappings_values_are_edited_in_their_cells() {
    let mut app = App::start();
    let mappings = given_part_mappings_shown(&mut app, TWO_MAPPINGS);

    app.edit_data_table_cell("RES_0402", "Name", "/RES_.*/", "CAP_0603");
    wait_for_mappings(&mut app, &mappings, &[
        "\"DipTrace\",\"/RES_.*/\",\"330R\",\"MFR1\",\"MPN1\"",
        "\"DipTrace\",\"CAP_0603\",\"10u\",\"MFR2\",\"MPN2\"",
    ]);
    // shown as the file is now.
    app.wait_for_text("/RES_.*/");

    app.edit_data_table_cell_then_press_enter("CAP_0603", "Mpn", "MPN5");
    wait_for_mappings(&mut app, &mappings, &[
        "\"DipTrace\",\"/RES_.*/\",\"330R\",\"MFR1\",\"MPN1\"",
        "\"DipTrace\",\"CAP_0603\",\"10u\",\"MFR2\",\"MPN5\"",
    ]);
    app.wait_for_text("MPN5");

    // with EasyEDA's column the file didn't have, `Device`, added first (`STORE-21`).
    app.choose_in_data_table_cell_combo_box("CAP_0603", "Eda", "EasyEda", &[]);
    wait_for_file(
        &mut app,
        &mappings,
        "\"Eda\",\"Name\",\"Value\",\"Device\",\"Manufacturer\",\"Mpn\"\n\
         \"DipTrace\",\"/RES_.*/\",\"330R\",\"\",\"MFR1\",\"MPN1\"\n\
         \"EasyEda\",\"CAP_0603\",\"10u\",\"\",\"MFR2\",\"MPN5\"\n",
    );

    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: Table(PartMappings, ",
        "stores_app: Received event. event: ChangeTable { kind: PartMappings, ",
        "stores_service: Performing stores operation. operation: ChangeRows { kind: PartMappings, ",
        "stores_app: Changed table. kind: PartMappings, source: '",
        "stores_app: Loaded table. kind: PartMappings, source: '",
    ]);
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-13`, `STORE-19`, a row is added, filled in, moved up and down, and removed, each change written at once, the
/// row buttons changing the row selected, by clicking one of its cells, or its number, the moves only if one is, the
/// row added, or moved, selected once it's shown, and no row once it's removed, and the other tables, such as the
/// placements, are read-only.
#[test]
fn a_part_mappings_rows_are_added_moved_and_removed() {
    let mut app = App::start();
    let mappings = given_part_mappings_shown(&mut app, TWO_MAPPINGS);
    app.wait_for_text("RES_0402");
    app.assert_button_is_disabled("Remove row");
    app.assert_button_is_disabled("Move up");
    app.assert_button_is_disabled("Move down");

    // a row is selected by clicking one of its cells, and the first row can't be moved up.
    app.click_data_table_cell("RES_0402", "Mpn");
    app.wait_for_enabled_button("Move down");
    app.assert_button_is_disabled("Move up");

    // of the variant builder's EDA tool, to be filled in, and selected, in place of the row selected, so as the last row,
    // it can't be moved down.
    app.click_button("Add row");
    wait_for_mappings(&mut app, &mappings, &[
        "\"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"",
        "\"DipTrace\",\"CAP_0603\",\"10u\",\"MFR2\",\"MPN2\"",
        "\"DipTrace\",\"\",\"\",\"\",\"\"",
    ]);
    app.wait_until_button_is_disabled("Move down");
    app.wait_for_enabled_button("Move up");
    app.edit_data_table_cell_in_row_numbered_then_press_enter(3, "Name", "/LED_.*/");
    wait_for_mappings(&mut app, &mappings, &[
        "\"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"",
        "\"DipTrace\",\"CAP_0603\",\"10u\",\"MFR2\",\"MPN2\"",
        "\"DipTrace\",\"/LED_.*/\",\"\",\"\",\"\"",
    ]);

    // the row moved stays selected, where it's moved to, so it's moved again, to the first row, which can't be moved up.
    app.wait_for_enabled_button("Move up");
    app.click_button("Move up");
    wait_for_mappings(&mut app, &mappings, &[
        "\"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"",
        "\"DipTrace\",\"/LED_.*/\",\"\",\"\",\"\"",
        "\"DipTrace\",\"CAP_0603\",\"10u\",\"MFR2\",\"MPN2\"",
    ]);
    app.wait_for_enabled_button("Move down");
    app.wait_for_enabled_button("Move up");
    app.click_button("Move up");
    wait_for_mappings(&mut app, &mappings, &[
        "\"DipTrace\",\"/LED_.*/\",\"\",\"\",\"\"",
        "\"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"",
        "\"DipTrace\",\"CAP_0603\",\"10u\",\"MFR2\",\"MPN2\"",
    ]);
    app.wait_until_button_is_disabled("Move up");

    // with the second row's number clicked, two are selected, which aren't moved, or removed.
    app.click_row_number(2);
    app.wait_until_button_is_disabled("Move down");
    app.assert_button_is_disabled("Move up");
    app.assert_button_is_disabled("Remove row");

    // clicking a cell selects only its row.
    app.click_data_table_cell("RES_0402", "Mpn");
    app.wait_for_enabled_button("Move down");
    app.click_button("Move down");
    wait_for_mappings(&mut app, &mappings, &[
        "\"DipTrace\",\"/LED_.*/\",\"\",\"\",\"\"",
        "\"DipTrace\",\"CAP_0603\",\"10u\",\"MFR2\",\"MPN2\"",
        "\"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"",
    ]);
    // the last row.
    app.wait_until_button_is_disabled("Move down");

    // and once a row's removed, no row is selected, not the row now in its place.
    app.click_data_table_cell("CAP_0603", "Mpn");
    app.wait_for_enabled_button("Move down");
    app.click_button("Remove row");
    wait_for_mappings(&mut app, &mappings, &[
        "\"DipTrace\",\"/LED_.*/\",\"\",\"\",\"\"",
        "\"DipTrace\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"",
    ]);
    app.wait_until_button_is_disabled("Remove row");
    app.assert_button_is_disabled("Move up");
    app.assert_button_is_disabled("Move down");

    app.wait_for_logs_in_order(&[
        "VariantBuilderTab command. command: Table(PartMappings, ",
        "ChangeRows(Add {",
        "stores_app: Received event. event: ChangeTable { kind: PartMappings, ",
        "stores_app: Changed table. kind: PartMappings, source: '",
        "stores_app: Loaded table. kind: PartMappings, source: '",
        "ChangeShown(Row(2))",
        "ScrolledToRow",
        "ChangeRows(Remove {",
        "ChangeShown(NoRow)",
    ]);
    app.assert_not_logged("Event failed.");

    app.double_click_text("Placements: board1.csv");
    app.wait_for_tab("Placements: board1.csv");
    app.wait_for_text("RefDes");
    app.wait_until_button_is_gone("Add row");
    app.assert_data_table_cell_is_not_editable("R1", "RefDes");
}

/// Waits for the file to be the contents.
fn wait_for_file(app: &mut App, path: &Path, expected: &str) {
    app.wait_until(&format!("the file was {expected}"), |_app| {
        fs::read_to_string(path).is_ok_and(|contents| contents == expected)
    });
}

/// `VBGUI-13`, `STORE-21`, choosing a row's EDA tool whose columns the data source doesn't have adds them first, before
/// "`Manufacturer`", a column shared by tools added once, then changes the row's tool, keeping its patterns, and each
/// pattern's cell says whether the row's tool matches its field when it's hovered.
#[test]
fn choosing_a_rows_eda_tool_adds_its_columns_first() {
    let mut app = App::start();
    let mappings = given_part_mappings_shown(&mut app, TWO_MAPPINGS);

    app.choose_in_data_table_cell_combo_box("CAP_0603", "Eda", "KiCad", &[]);
    wait_for_file(
        &mut app,
        &mappings,
        "\"Eda\",\"Name\",\"Value\",\"Package\",\"Val\",\"Manufacturer\",\"Mpn\"\n\
         \"DipTrace\",\"RES_0402\",\"330R\",\"\",\"\",\"MFR1\",\"MPN1\"\n\
         \"KiCad\",\"CAP_0603\",\"10u\",\"\",\"\",\"MFR2\",\"MPN2\"\n",
    );
    app.wait_for_text("Package");
    app.wait_for_logs_in_order(&[
        "ChangeAfterColumns(PendingChange { eda_tool: KiCad,",
        "stores_app: Received event. event: AddTableColumns { kind: PartMappings, ",
        // the core's "Added table columns." line, which says the revision.
        "columns: ['Package', 'Val'], revision: '",
        "stores_app: Loaded table. kind: PartMappings, source: '",
        "ColumnsShown(Some((Update {",
        "stores_app: Received event. event: ChangeTable { kind: PartMappings, ",
        "stores_app: Changed table. kind: PartMappings, source: '",
    ]);

    // `value` is DipTrace's and EasyEDA's, so only `device` is added.
    app.choose_in_data_table_cell_combo_box("CAP_0603", "Eda", "EasyEda", &[]);
    wait_for_file(
        &mut app,
        &mappings,
        "\"Eda\",\"Name\",\"Value\",\"Package\",\"Val\",\"Device\",\"Manufacturer\",\"Mpn\"\n\
         \"DipTrace\",\"RES_0402\",\"330R\",\"\",\"\",\"\",\"MFR1\",\"MPN1\"\n\
         \"EasyEda\",\"CAP_0603\",\"10u\",\"\",\"\",\"\",\"MFR2\",\"MPN2\"\n",
    );
    app.wait_for_logs_in_order(&["columns: ['Device'], revision: '"]);

    // a tool whose columns are there is changed at once.
    app.choose_in_data_table_cell_combo_box("CAP_0603", "Eda", "KiCad", &[]);
    wait_for_file(
        &mut app,
        &mappings,
        "\"Eda\",\"Name\",\"Value\",\"Package\",\"Val\",\"Device\",\"Manufacturer\",\"Mpn\"\n\
         \"DipTrace\",\"RES_0402\",\"330R\",\"\",\"\",\"\",\"MFR1\",\"MPN1\"\n\
         \"KiCad\",\"CAP_0603\",\"10u\",\"\",\"\",\"\",\"MFR2\",\"MPN2\"\n",
    );

    // every pattern's cell is shown, whichever tool the row has, saying how the tool uses it: blue, not matched.
    app.hover_data_table_cell("CAP_0603", "Name");
    app.wait_for_text_containing("Not matched by KiCad.");
    app.hover_data_table_cell("RES_0402", "Device");
    app.wait_for_text_containing("Not matched by DipTrace.");
    // orange, as KiCad's fields have no patterns in the row.
    app.hover_data_table_cell("CAP_0603", "Package");
    app.wait_for_text_containing("Matched by KiCad. None of its fields has a pattern.");
    // green, not orange, as the row's DipTrace fields have patterns.
    app.hover_data_table_cell("RES_0402", "Value");
    app.wait_for_text_containing("Matched by DipTrace.");
    app.wait_until_no_text_contains("None of its fields has a pattern.");
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-13`, `STORE-21`, "Add row" adds the variant builder's EDA tool's columns first, if the data source doesn't have
/// them, then the row, which is selected once it's shown.
#[test]
fn adding_a_row_adds_its_eda_tools_columns_first() {
    let mut app = App::start();
    let mappings = given_part_mappings_shown(
        &mut app,
        "\"Eda\",\"Package\",\"Val\",\"Manufacturer\",\"Mpn\"\n\
         \"KiCad\",\"R_0402\",\"330R\",\"MFR1\",\"MPN1\"\n",
    );
    app.wait_for_text("R_0402");

    // the variant builder's EDA tool is DipTrace.
    app.click_button("Add row");
    wait_for_file(
        &mut app,
        &mappings,
        "\"Eda\",\"Package\",\"Val\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n\
         \"KiCad\",\"R_0402\",\"330R\",\"\",\"\",\"MFR1\",\"MPN1\"\n\
         \"DipTrace\",\"\",\"\",\"\",\"\",\"\",\"\"\n",
    );
    // selected, as the last row.
    app.wait_for_enabled_button("Move up");
    app.assert_button_is_disabled("Move down");
    app.wait_for_logs_in_order(&[
        "ChangeAfterColumns(PendingChange { eda_tool: DipTrace,",
        // the core's "Added table columns." line, which says the revision.
        "columns: ['Name', 'Value'], revision: '",
        "ColumnsShown(Some((Add {",
        "stores_app: Changed table. kind: PartMappings, source: '",
        "ChangeShown(Row(1))",
    ]);
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-13`, `STORE-21`, if the columns a row's tool needs can't be added, the panel says why, and the row is left as
/// it was.
#[test]
fn a_rows_tool_whose_columns_can_not_be_added_is_left_as_it_was() {
    let mut app = App::start();
    let mappings = given_part_mappings_shown(&mut app, TWO_MAPPINGS);
    app.wait_for_text("CAP_0603");
    let mut permissions = fs::metadata(&mappings)
        .unwrap()
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&mappings, permissions.clone()).unwrap();

    app.choose_in_data_table_cell_combo_box("CAP_0603", "Eda", "KiCad", &[]);

    app.wait_for_text_containing("Unable to change the table. kind: PartMappings, source: '");
    app.wait_for_logs_in_order(&[
        "stores_app: Received event. event: AddTableColumns { kind: PartMappings, ",
        "ColumnsShown(None)",
    ]);
    assert_eq!(fs::read_to_string(&mappings).unwrap(), TWO_MAPPINGS);
    app.assert_not_logged("stores_app: Received event. event: ChangeTable {");
    // the row buttons can be used again.
    app.click_data_table_cell("RES_0402", "Mpn");
    app.wait_for_enabled_button("Move down");

    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&mappings, permissions).unwrap();
}

const SUBSTITUTIONS_HEADERS: &str = "\"Eda\",\"NamePattern\",\"ValuePattern\",\"Name\",\"Value\",\"Comment\"\n";

/// Creates the design variant, adds the substitutions `file_name` to the field, "Design substitutions" or "Global
/// substitutions", written with `contents`, and shows their panel.
fn given_substitutions_shown(app: &mut App, field: &str, file_name: &str, contents: &str) -> PathBuf {
    let folder = app.folder();
    given_a_design(&folder);
    let substitutions = folder.join(file_name);
    fs::write(&substitutions, contents).unwrap();
    create_design_variant(app, &folder);
    app.wait_for_tab(FILE_NAME);
    add(app, field, substitutions.clone());

    // another panel in place of the "Overview", which lists the file too, so only the explorer has it.
    app.double_click_text("Placements: board1.csv");
    app.wait_for_tab("Placements: board1.csv");
    app.double_click_text(file_name);
    app.wait_for_tab(&format!("{field}: {file_name}"));
    substitutions
}

/// Waits for the file to have the rows, after the headers `SUBSTITUTIONS_HEADERS` has.
fn wait_for_substitutions(app: &mut App, path: &Path, rows: &[&str]) {
    let mut expected = SUBSTITUTIONS_HEADERS.to_string();
    for row in rows {
        expected.push_str(row);
        expected.push('\n');
    }
    wait_for_file(app, path, &expected);
}

/// `VBGUI-14`, `STORE-19`, `STORE-21`, a design substitution's pattern, value and comment are edited in their cells, its
/// rows added, moved and removed, and a row's EDA tool chosen, whose columns are added first, each written at once, and
/// a value's cell says whether the row's tool substitutes its field when it's hovered.
#[test]
fn a_design_substitutions_values_are_edited_and_its_rows_changed() {
    let mut app = App::start();
    let mut contents = SUBSTITUTIONS_HEADERS.to_string();
    contents.push_str("\"DipTrace\",\"R0402\",\"\",\"RES_0402\",\"\",\"comment 1\"\n");
    contents.push_str("\"DipTrace\",\"/C.*/\",\"100nF\",\"CAP_0402\",\"\",\"DERATE 50V\"\n");
    let substitutions = given_substitutions_shown(&mut app, "Design substitutions", "d1.csv", &contents);
    app.wait_for_text("RES_0402");

    app.edit_data_table_cell_then_press_enter("RES_0402", "NamePattern", "/R04.*/");
    wait_for_substitutions(&mut app, &substitutions, &[
        "\"DipTrace\",\"/R04.*/\",\"\",\"RES_0402\",\"\",\"comment 1\"",
        "\"DipTrace\",\"/C.*/\",\"100nF\",\"CAP_0402\",\"\",\"DERATE 50V\"",
    ]);
    app.wait_for_text("/R04.*/");
    app.edit_data_table_cell_then_press_enter("RES_0402", "Value", "100R");
    wait_for_substitutions(&mut app, &substitutions, &[
        "\"DipTrace\",\"/R04.*/\",\"\",\"RES_0402\",\"100R\",\"comment 1\"",
        "\"DipTrace\",\"/C.*/\",\"100nF\",\"CAP_0402\",\"\",\"DERATE 50V\"",
    ]);
    app.wait_for_text("100R");
    app.edit_data_table_cell_then_press_enter("RES_0402", "Comment", "comment 2");
    wait_for_substitutions(&mut app, &substitutions, &[
        "\"DipTrace\",\"/R04.*/\",\"\",\"RES_0402\",\"100R\",\"comment 2\"",
        "\"DipTrace\",\"/C.*/\",\"100nF\",\"CAP_0402\",\"\",\"DERATE 50V\"",
    ]);
    app.wait_for_text("comment 2");

    // green, as DipTrace's `value` is substituted, even with no value.
    app.hover_data_table_cell("CAP_0402", "Value");
    app.wait_for_text_containing("Used to substitute DipTrace values.");

    // of the variant builder's EDA tool, with no patterns, values or comment, selected once it's shown, so it's moved.
    app.click_button("Add row");
    wait_for_substitutions(&mut app, &substitutions, &[
        "\"DipTrace\",\"/R04.*/\",\"\",\"RES_0402\",\"100R\",\"comment 2\"",
        "\"DipTrace\",\"/C.*/\",\"100nF\",\"CAP_0402\",\"\",\"DERATE 50V\"",
        "\"DipTrace\",\"\",\"\",\"\",\"\",\"\"",
    ]);
    app.wait_for_enabled_button("Move up");
    app.click_button("Move up");
    wait_for_substitutions(&mut app, &substitutions, &[
        "\"DipTrace\",\"/R04.*/\",\"\",\"RES_0402\",\"100R\",\"comment 2\"",
        "\"DipTrace\",\"\",\"\",\"\",\"\",\"\"",
        "\"DipTrace\",\"/C.*/\",\"100nF\",\"CAP_0402\",\"\",\"DERATE 50V\"",
    ]);

    app.click_data_table_cell("CAP_0402", "Comment");
    app.wait_for_enabled_button("Remove row");
    app.click_button("Remove row");
    wait_for_substitutions(&mut app, &substitutions, &[
        "\"DipTrace\",\"/R04.*/\",\"\",\"RES_0402\",\"100R\",\"comment 2\"",
        "\"DipTrace\",\"\",\"\",\"\",\"\",\"\"",
    ]);
    app.wait_until_text_is_gone("CAP_0402");

    // KiCad's patterns' columns added after the last pattern's, and its values' after the last value's, then the row's
    // tool changed, keeping its patterns and values.
    app.choose_in_data_table_cell_combo_box("RES_0402", "Eda", "KiCad", &[]);
    wait_for_file(
        &mut app,
        &substitutions,
        "\"Eda\",\"NamePattern\",\"ValuePattern\",\"PackagePattern\",\"ValPattern\",\"Name\",\"Value\",\"Package\",\"Val\",\"Comment\"\n\
         \"KiCad\",\"/R04.*/\",\"\",\"\",\"\",\"RES_0402\",\"100R\",\"\",\"\",\"comment 2\"\n\
         \"DipTrace\",\"\",\"\",\"\",\"\",\"\",\"\",\"\",\"\",\"\"\n",
    );
    app.wait_for_text("PackagePattern");
    app.hover_data_table_cell("RES_0402", "Value");
    app.wait_for_text_containing("Not used to substitute KiCad values.");
    app.hover_data_table_cell("RES_0402", "Val");
    app.wait_for_text_containing("Used to substitute KiCad values.");

    app.wait_for_logs_in_order(&[
        "VariantBuilderTab command. command: Table(Substitutions, ",
        "stores_app: Received event. event: ChangeTable { kind: Substitutions, ",
        "stores_app: Changed table. kind: Substitutions, source: '",
        "stores_app: Loaded table. kind: Substitutions, source: '",
        "ChangeRows(Add {",
        "ChangeShown(Row(2))",
        "ChangeRows(Remove {",
        "ChangeShown(NoRow)",
        "ChangeAfterColumns(PendingChange { eda_tool: KiCad,",
        "stores_app: Received event. event: AddTableColumns { kind: Substitutions, ",
        "ColumnsShown(Some((Update {",
        "stores_app: Changed table. kind: Substitutions, source: '",
    ]);
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-13`, `VBGUI-14`, a row's status's and EDA tool's text can't be selected, in a part mappings' table and a
/// substitutions', as they're chosen, not typed, and other cells' text can.
#[test]
fn a_rows_status_and_eda_tool_can_not_be_selected() {
    let mut app = App::start();
    given_part_mappings_shown(&mut app, TWO_MAPPINGS);
    app.wait_for_text("MPN1");
    assert_eq!(app.copy_text_dragged_across_data_table_cell("RES_0402", "Mpn"), "MPN1");
    assert_eq!(app.copy_text_dragged_across_data_table_cell("RES_0402", "Eda"), "");
    assert_eq!(app.copy_text_dragged_across_data_table_cell("RES_0402", "◑"), "");

    let folder = app.folder();
    let substitutions = folder.join("d1.csv");
    let mut contents = SUBSTITUTIONS_HEADERS.to_string();
    contents.push_str("\"DipTrace\",\"R0402\",\"\",\"RES_0402\",\"\",\"comment 1\"\n");
    fs::write(&substitutions, contents).unwrap();
    app.double_click_text(FILE_NAME);
    add(&mut app, "Design substitutions", substitutions);
    app.double_click_text("Placements: board1.csv");
    app.wait_for_tab("Placements: board1.csv");
    app.double_click_text("d1.csv");
    app.wait_for_tab("Design substitutions: d1.csv");
    app.wait_for_text("comment 1");
    assert_eq!(
        app.copy_text_dragged_across_data_table_cell("RES_0402", "Comment"),
        "comment 1"
    );
    assert_eq!(app.copy_text_dragged_across_data_table_cell("RES_0402", "Eda"), "");
    assert_eq!(app.copy_text_dragged_across_data_table_cell("RES_0402", "◑"), "");
}

/// `VBGUI-14`, a global substitutions' panel is changed too, and a substitution's status, first, says what its problems
/// are when it's hovered, or that it has none. Their colours are checked by `data_sources::table`'s tests, as the GUI's
/// tests can't see them.
#[test]
fn a_global_substitutions_status_says_what_its_problems_are() {
    let mut app = App::start();
    let mut contents = SUBSTITUTIONS_HEADERS.to_string();
    contents.push_str("\"DipTrace\",\"R0402\",\"\",\"RES_0402\",\"\",\"good\"\n");
    contents.push_str("\"DipTrace\",\"/R_[/\",\"\",\"RES_0603\",\"\",\"invalid\"\n");
    contents.push_str("\"DipTrace\",\"\",\"\",\"RES_0805\",\"\",\"no patterns\"\n");
    contents.push_str("\"DipTrace\",\"R1206\",\"10K\",\"R1206\",\"10K\",\"the same\"\n");
    contents.push_str("\"DipTrace\",\"R2512\",\"\",\"\",\"\",\"no values\"\n");
    let substitutions = given_substitutions_shown(&mut app, "Global substitutions", "g1.csv", &contents);
    app.wait_for_text("the same");
    let headers = ["◑", "Eda", "NamePattern"].map(|header| app.text_position(header).x);
    assert!(headers.is_sorted(), "{headers:?}");

    app.hover_data_table_cell("good", "◑");
    app.wait_for_text_containing("No problems.");
    app.hover_data_table_cell("invalid", "◑");
    app.wait_for_text_containing("A pattern isn't valid.");
    app.wait_until_no_text_contains("No problems.");
    app.hover_data_table_cell("no patterns", "◑");
    app.wait_for_text_containing("None of DipTrace's fields has a pattern.");
    app.hover_data_table_cell("the same", "◑");
    app.wait_for_text_containing("Matching patterns and values have no effect.");
    app.wait_until_no_text_contains("isn't valid.");
    // red, as neither of DipTrace's fields has a value, which each of its values' cells says too.
    app.hover_data_table_cell("no values", "◑");
    app.wait_for_text_containing("Substitution values not defined.");
    app.wait_until_no_text_contains("have no effect.");
    app.hover_data_table_cell("no values", "Value");
    app.wait_for_text_containing("Used to substitute DipTrace values. Substitution values not defined.");

    // changed, so it changes something.
    app.edit_data_table_cell_then_press_enter("the same", "Value", "10K0");
    wait_for_substitutions(&mut app, &substitutions, &[
        "\"DipTrace\",\"R0402\",\"\",\"RES_0402\",\"\",\"good\"",
        "\"DipTrace\",\"/R_[/\",\"\",\"RES_0603\",\"\",\"invalid\"",
        "\"DipTrace\",\"\",\"\",\"RES_0805\",\"\",\"no patterns\"",
        "\"DipTrace\",\"R1206\",\"10K\",\"R1206\",\"10K0\",\"the same\"",
        "\"DipTrace\",\"R2512\",\"\",\"\",\"\",\"no values\"",
    ]);
    app.wait_for_text("10K0");
    app.hover_data_table_cell("the same", "◑");
    app.wait_for_text_containing("No problems.");

    app.wait_for_logs_in_order(&[
        "VariantBuilderTab command. command: Table(Substitutions, ",
        "stores_app: Received event. event: ChangeTable { kind: Substitutions, ",
        "stores_app: Changed table. kind: Substitutions, source: '",
    ]);
    app.assert_not_logged("Event failed.");
}

const ASSEMBLY_RULES_HEADERS: &str = "\"RefDes\",\"Manufacturer\",\"Mpn\"\n";

/// Waits for the file to have the rows, after the headers `ASSEMBLY_RULES_HEADERS` has.
fn wait_for_assembly_rules(app: &mut App, path: &Path, rows: &[&str]) {
    let mut expected = ASSEMBLY_RULES_HEADERS.to_string();
    for row in rows {
        expected.push_str(row);
        expected.push('\n');
    }
    wait_for_file(app, path, &expected);
}

/// `VBGUI-15`, `STORE-19`, an assembly rule's designator, manufacturer and MPN are edited in their cells, and its rows
/// added, moved and removed, each written at once, and a rule's status, first, says which of its values are empty when
/// it's hovered, or that none is, and can't be selected. Their colours are checked by `data_sources::table`'s tests, as
/// the GUI's tests can't see them.
#[test]
fn an_assembly_rules_values_are_edited_and_its_rows_changed() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    let rules = folder.join("rules.csv");
    let mut contents = ASSEMBLY_RULES_HEADERS.to_string();
    contents.push_str("\"R1\",\"MFR1\",\"MPN1\"\n");
    contents.push_str("\"C1\",\"MFR2\",\"\"\n");
    fs::write(&rules, contents).unwrap();
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);
    choose(&mut app, "Assembly rules", rules.clone());
    // another panel in place of the "Overview", which lists the file too, so only the explorer has it.
    app.double_click_text("Placements: board1.csv");
    app.wait_for_tab("Placements: board1.csv");
    app.double_click_text("Assembly rules: rules.csv");
    app.wait_for_tab("Assembly rules: rules.csv");
    app.wait_for_text("MFR2");
    let headers = ["◑", "RefDes", "Manufacturer", "Mpn"].map(|header| app.text_position(header).x);
    assert!(headers.is_sorted(), "{headers:?}");

    app.hover_data_table_cell("R1", "◑");
    app.wait_for_text_containing("No problems.");
    app.hover_data_table_cell("C1", "◑");
    app.wait_for_text_containing("Empty Mpn.");
    app.wait_until_no_text_contains("No problems.");
    assert_eq!(app.copy_text_dragged_across_data_table_cell("R1", "◑"), "");
    assert_eq!(app.copy_text_dragged_across_data_table_cell("R1", "Mpn"), "MPN1");

    app.edit_data_table_cell_then_press_enter("R1", "RefDes", "R2");
    wait_for_assembly_rules(&mut app, &rules, &["\"R2\",\"MFR1\",\"MPN1\"", "\"C1\",\"MFR2\",\"\""]);
    app.wait_for_text("R2");
    app.edit_data_table_cell_then_press_enter("R2", "Manufacturer", "MFR3");
    wait_for_assembly_rules(&mut app, &rules, &["\"R2\",\"MFR3\",\"MPN1\"", "\"C1\",\"MFR2\",\"\""]);
    app.wait_for_text("MFR3");
    app.edit_data_table_cell_then_press_enter("C1", "Mpn", "MPN2");
    wait_for_assembly_rules(&mut app, &rules, &[
        "\"R2\",\"MFR3\",\"MPN1\"",
        "\"C1\",\"MFR2\",\"MPN2\"",
    ]);
    app.wait_for_text("MPN2");
    app.hover_data_table_cell("C1", "◑");
    app.wait_for_text_containing("No problems.");

    // with no designator, manufacturer or MPN, selected once it's shown, so it's moved.
    app.click_button("Add row");
    wait_for_assembly_rules(&mut app, &rules, &[
        "\"R2\",\"MFR3\",\"MPN1\"",
        "\"C1\",\"MFR2\",\"MPN2\"",
        "\"\",\"\",\"\"",
    ]);
    app.hover_data_table_cell_in_row_numbered(3, "◑");
    app.wait_for_text_containing("Empty RefDes.");
    app.wait_for_text_containing("Empty Manufacturer.");
    app.wait_for_text_containing("Empty Mpn.");
    app.wait_for_enabled_button("Move up");
    app.click_button("Move up");
    wait_for_assembly_rules(&mut app, &rules, &[
        "\"R2\",\"MFR3\",\"MPN1\"",
        "\"\",\"\",\"\"",
        "\"C1\",\"MFR2\",\"MPN2\"",
    ]);

    app.click_data_table_cell("C1", "Mpn");
    app.wait_for_enabled_button("Remove row");
    app.click_button("Remove row");
    wait_for_assembly_rules(&mut app, &rules, &["\"R2\",\"MFR3\",\"MPN1\"", "\"\",\"\",\"\""]);
    app.wait_until_text_is_gone("C1");

    app.wait_for_logs_in_order(&[
        "VariantBuilderTab command. command: Table(AssemblyRules, ",
        "stores_app: Received event. event: ChangeTable { kind: AssemblyRules, ",
        "stores_app: Changed table. kind: AssemblyRules, source: '",
        "stores_app: Loaded table. kind: AssemblyRules, source: '",
        "ChangeRows(Add {",
        "ChangeShown(Row(2))",
        "ChangeRows(Remove {",
        "ChangeShown(NoRow)",
    ]);
    app.assert_not_logged("Event failed.");
}

/// `VBGUI-12`, `STORE-17`, `STORE-9`, the "Build output" panel is a placeholder for the mapping results, and the BOM,
/// which isn't there until the variant is built, is shown once it's built.
#[test]
fn the_bom_is_shown_once_the_variant_is_built() {
    let mut app = App::start();
    let folder = app.folder();
    given_a_design(&folder);
    create_design_variant(&mut app, &folder);
    app.wait_for_tab(FILE_NAME);
    choose(&mut app, "Parts", folder.join("parts.csv"));
    choose(&mut app, "Part mappings", folder.join("part-mappings.csv"));
    app.click_button("Save changes");
    app.wait_for_enabled_button("Build");

    app.double_click_text("Build output");
    app.wait_for_tab("Build output");
    app.wait_for_text("The mapping results are to come.");

    app.double_click_text("BOM");
    app.wait_for_tab("BOM");
    app.wait_for_text_containing("Data source not found. source: '");

    app.click_tab("Overview");
    app.click_button("Build");
    app.wait_for_text("Built.");

    app.click_tab("BOM");
    app.wait_for_text("Designator");
    app.wait_for_text("MPN1 (MFR1)");
    app.wait_for_text("R1");
    app.wait_until_no_text_contains("Data source not found.");
    // the comment is the last column (`VBGUI-11`).
    let headers =
        ["Designator", "Footprint", "JLCPCB Part", "Quantity", "Comment"].map(|header| app.text_position(header).x);
    assert!(headers.is_sorted(), "{headers:?}");

    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: ShowPanel(BuildOutput)",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: ShowPanel(Output(Bom))",
        "stores_app: Received event. event: OpenTable { kind: Bom, source: ",
        "stores_app: Event failed. error: DataSourceNotFound {",
        "planner_gui_egui::ui_app::app_tabs::variant_builder: VariantBuilderTab command. command: BuildClicked",
        "variantbuilder_app: Built variant. path: '",
        "stores_app: Received event. event: TableSourceChanged { kind: Bom, ",
        "stores_app: Loaded table. kind: Bom, source: '",
    ]);
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

    // the "Overview" panel is beside the explorer, so both are narrower (`VBGUI-9`).
    app.resize_window(egui::vec2(1200.0, 1080.0));
    let [placements, narrower, output_bom] = widths(&mut app);
    assert_eq!(placements.0, narrower.0);
    assert_eq!(output_bom, narrower);
    let width = |(left, right): (f32, f32)| right - left;
    assert!(
        width(narrower) < width(exclusion_list) - 400.0,
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
