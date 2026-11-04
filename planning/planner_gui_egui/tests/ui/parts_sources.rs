//! Parts data sources, opened with "Open parts", whose parts the stores core loads (`PARTSRC`, `STORE`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::fixture::App;

const PARTS: &str = "\"Manufacturer\",\"Mpn\",\"Package\",\"LCSC\"\n\
                     \"RESCO\",\"1234\",\"0402\",\"C1234\"\n\
                     \"CAPCO\",\"CAP1\",\"0603\",\"\"\n";

fn given_a_parts_file(folder: &Path, name: &str, content: &str) -> PathBuf {
    let path = folder.join(name);
    fs::write(&path, content).unwrap();
    path
}

/// Chooses a CSV file as the data source, using the file dialog, without clicking "Ok".
fn choose_csv_file(app: &mut App, path: &Path) {
    app.click_button("Open parts");
    app.wait_for_tab("Open parts");

    app.choose_from_combo_box("Data source", "CSV file");
    app.when_a_file_dialog_opens_answer(Some(path.to_path_buf()));
    app.click_button("File: Choose");
    app.wait_for_file_dialogs();
    app.wait_for_field_value("File", &path.display().to_string());
}

/// `PARTSRC-1`, `PARTSRC-2`, `PARTSRC-3`, `STORE-1`, `STORE-2`, `ISSUE-23`
#[test]
fn open_parts_shows_the_parts_of_a_csv_file() {
    let mut app = App::start();
    let parts = given_a_parts_file(&app.folder(), "parts.csv", PARTS);

    choose_csv_file(&mut app, &parts);
    app.click_button("Ok");

    app.wait_for_tab("parts.csv");
    for text in ["Manufacturer", "MPN", "LCSC", "Package"] {
        app.wait_for_text(text);
    }
    // values that look like numbers are kept as they are in the file.
    for text in ["RESCO", "1234", "0402", "C1234", "CAPCO", "CAP1", "0603"] {
        app.wait_for_text(text);
    }
    app.wait_for_text_containing("CSV file: ");
    app.wait_for_text_containing("parts.csv");
    app.wait_until_no_tab_contains("Open parts");

    app.wait_for_logs_in_order(&[
        "planner_gui_egui::toolbar: Toolbar command. command: OpenPartsClicked",
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: SourceKindChanged(CsvFile)",
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: PickCsvFileClicked",
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: CsvFilePicked(",
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: Submit",
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: Open",
        "stores_app: Received event. event: OpenParts {",
        // `STORE-7`, the stores service does the operations, and its results come back as commands.
        "stores_service: Performing stores operation. operation: ResolveSource {",
        "planner_gui_egui::ui_commands: UiApp command. command: StoresReply(StoresReply { operation: ResolveSource {",
        "stores_app: Received event. event: SourceResolved {",
        "stores_service: Performing stores operation. operation: ReadParts {",
        "planner_gui_egui::ui_commands: UiApp command. command: StoresReply(StoresReply { operation: ReadParts {",
        "stores_app: Received event. event: PartsRead {",
        "stores_app: Loaded parts. source: '",
    ]);
    app.assert_not_logged("Event failed.");

    // `STORE-8`, the file hasn't changed, so its revision hasn't, and the parts shown are kept.
    app.click_button("Reload");
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: ReloadClicked",
        "stores_app: Received event. event: LoadParts {",
        "stores_app: Received event. event: PartsRead {",
        "stores_app: Parts unchanged. source: '",
    ]);
    app.wait_for_text("RESCO");
    app.assert_not_logged("Loaded parts.");
}

/// `PARTSRC-1`, `FORM-5`, "Ok" needs a data source, and choosing none keeps the file for when a CSV file is chosen
/// again.
#[test]
fn ok_needs_a_data_source_and_a_file() {
    let mut app = App::start();
    let parts = given_a_parts_file(&app.folder(), "parts.csv", PARTS);

    app.click_button("Open parts");
    app.wait_for_combo_box_value("Data source", "--choose source--");
    app.assert_button_is_disabled("Ok");
    assert!(!app.is_button_shown("File: Choose"));

    app.choose_from_combo_box("Data source", "CSV file");
    app.wait_for_text("CSV file");
    app.wait_until_field_is_empty("File");
    app.assert_button_is_disabled("Ok");

    // cancelling the dialog leaves the field empty.
    app.when_a_file_dialog_opens_answer(None);
    app.click_button("File: Choose");
    app.wait_for_file_dialogs();
    app.wait_until_field_is_empty("File");
    app.assert_button_is_disabled("Ok");

    app.when_a_file_dialog_opens_answer(Some(parts.clone()));
    app.click_button("File: Choose");
    app.wait_for_file_dialogs();
    app.wait_for_field_value("File", &parts.display().to_string());
    app.wait_for_enabled_button("Ok");

    app.choose_from_combo_box("Data source", "--choose source--");
    app.wait_until_field_is_gone("File");
    app.wait_until_button_is_disabled("Ok");

    app.choose_from_combo_box("Data source", "CSV file");
    app.wait_for_field_value("File", &parts.display().to_string());
    app.wait_for_enabled_button("Ok");

    app.click_button("File: Clear");
    app.wait_until_field_is_empty("File");
    app.wait_until_button_is_disabled("Ok");
}

/// `PARTSRC-3`, `STORE-3`, `STORE-9`, the error says which header is missing, and the file is shown once it's fixed,
/// without "Reload", which then finds it unchanged.
#[test]
fn a_file_without_the_headers_shows_why_and_is_shown_once_fixed() {
    let mut app = App::start();
    let parts = given_a_parts_file(&app.folder(), "parts.csv", "\"Maker\",\"Mpn\"\n\"RESCO\",\"RES1\"\n");

    choose_csv_file(&mut app, &parts);
    app.click_button("Ok");
    app.wait_for_tab("parts.csv");
    app.wait_for_text_containing("Missing header. header: 'Manufacturer'");
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: Open",
        "stores_app: Received event. event: OpenParts {",
        "stores_app: Event failed. error: UnableToLoadParts {",
    ]);
    assert_eq!(app.count_text("RES1"), 0);

    // `STORE-9`, the file was watched once identified, though it couldn't be read.
    fs::write(&parts, PARTS).unwrap();
    app.wait_for_text("RESCO");
    app.wait_until_no_text_contains("Missing header.");
    app.wait_for_logs_in_order(&[
        "stores_app: Data source changed. source: '",
        "stores_app: Loaded parts. source: '",
    ]);

    app.click_button("Reload");
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: ReloadClicked",
        "stores_app: Received event. event: LoadParts {",
        "stores_app: Parts unchanged. source: '",
    ]);
    app.wait_for_text("RESCO");
}

/// `PARTSRC-3`, `FORM-8`, a file with only the headers has no parts, and no table is drawn.
#[test]
fn a_file_with_no_parts_says_so() {
    let mut app = App::start();
    let parts = given_a_parts_file(&app.folder(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");

    choose_csv_file(&mut app, &parts);
    app.click_button("Ok");

    app.wait_for_text("There are no parts.");
    assert_eq!(app.count_text("Manufacturer"), 0);
}

/// `PARTSRC-5`, `STORE-5`, `STORE-6`, `STORE-9`, a data source shown in two tabs is loaded once, even when its path is
/// written differently, and a change to its file, by anyone, is shown in both, without "Reload".
#[test]
fn a_data_source_can_be_shown_in_two_tabs() {
    let mut app = App::start();
    let parts = given_a_parts_file(&app.folder(), "parts.csv", PARTS);
    let other_parts = given_a_parts_file(
        &app.folder(),
        "other_parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"DIODECO\",\"D1\"\n",
    );

    choose_csv_file(&mut app, &parts);
    app.click_button("Ok");
    app.wait_for_text("RESCO");

    choose_csv_file(&mut app, &other_parts);
    app.click_button("Ok");
    app.wait_for_tab("other_parts.csv");
    app.wait_for_text("DIODECO");
    // each tab shows its own data source's parts.
    assert_eq!(app.count_text("RESCO"), 0);
    // "stores_app: Loaded parts. source: '<path>', part_count: <n>", for each.
    app.wait_for_logs_in_order(&["parts.csv', part_count: 2", "other_parts.csv', part_count: 1"]);

    // the same file, written differently.
    fs::create_dir(app.folder().join("other")).unwrap();
    let other_spelling = app
        .folder()
        .join("other")
        .join("..")
        .join("parts.csv");
    choose_csv_file(&mut app, &other_spelling);
    app.click_button("Ok");
    app.wait_for_text("RESCO");
    // "parts.csv" twice, and "other_parts.csv".
    assert_eq!(app.count_tabs_containing("parts.csv"), 3);
    // shown as the file's canonical path.
    app.wait_until_no_text_contains(&format!("other{}..", std::path::MAIN_SEPARATOR));
    // used as it was loaded, not loaded again.
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: Open",
        "stores_app: Received event. event: OpenParts {",
        "stores_app: Parts already loaded. source: '",
    ]);
    app.assert_not_logged("Loaded parts.");
    // `STORE-9`, and watched once.
    app.assert_not_logged("operation: WatchSource {");
    app.assert_not_logged("Event failed.");

    // changed by another process, as the CLI or an editor would.
    fs::write(&parts, "\"Manufacturer\",\"Mpn\"\n\"NEWCO\",\"NEW1\"\n").unwrap();
    app.wait_for_text("NEWCO");
    app.wait_until_text_is_gone("RESCO");
    app.wait_for_logs_in_order(&[
        "stores_app: Received event. event: SourceChanged {",
        "stores_app: Data source changed. source: '",
        "stores_app: Loaded parts. source: '",
    ]);

    // the first tab shows the change too. "other_parts.csv" is shown first, so that the parts are seen to change with
    // the tab.
    app.click_tab_containing("other_parts.csv");
    app.wait_for_text("DIODECO");
    assert_eq!(app.count_text("NEWCO"), 0);
    app.click_nth_tab_containing("parts.csv", 0);
    app.wait_for_text("NEWCO");
    assert_eq!(app.count_text("RESCO"), 0);
    // without "Reload".
    app.assert_not_logged("ReloadClicked");
    app.assert_not_logged("event: LoadParts");
}

/// `PARTSRC-3`, `PARTSRC-5`, `STORE-3`, a refused reload is the data source's, so every tab showing it says why, and
/// keeps showing the parts loaded before.
#[test]
fn a_refused_reload_is_shown_by_every_tab_showing_the_data_source() {
    let mut app = App::start();
    let parts = given_a_parts_file(&app.folder(), "parts.csv", PARTS);
    let other_parts = given_a_parts_file(
        &app.folder(),
        "other_parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"DIODECO\",\"D1\"\n",
    );

    choose_csv_file(&mut app, &parts);
    app.click_button("Ok");
    app.wait_for_text("RESCO");
    choose_csv_file(&mut app, &other_parts);
    app.click_button("Ok");
    app.wait_for_text("DIODECO");
    choose_csv_file(&mut app, &parts);
    app.click_button("Ok");
    app.wait_for_text("RESCO");

    fs::write(&parts, "\"Maker\",\"Mpn\"\n").unwrap();
    app.click_button("Reload");
    app.wait_for_text_containing("Missing header. header: 'Manufacturer'");
    app.wait_for_text("RESCO");
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: ReloadClicked",
        "stores_app: Received event. event: LoadParts {",
        "stores_app: Event failed. error: UnableToLoadParts {",
    ]);

    // the other data source's tab has no error, the first tab showing this one has.
    app.click_tab_containing("other_parts.csv");
    app.wait_for_text("DIODECO");
    app.wait_until_no_text_contains("Missing header.");
    app.click_nth_tab_containing("parts.csv", 0);
    app.wait_for_text_containing("Missing header. header: 'Manufacturer'");
    app.wait_for_text("RESCO");
}

/// `PARTSRC-6`, `STORE-15`, a part typed is added, and removed, by the stores core, each change based on the revision
/// of the parts shown, the buttons enabled only for a part that isn't, or is, shown.
#[test]
fn adding_and_removing_a_part_changes_the_file() {
    let mut app = App::start();
    let parts = given_a_parts_file(&app.folder(), "parts.csv", PARTS);

    choose_csv_file(&mut app, &parts);
    app.click_button("Ok");
    app.wait_for_text("RESCO");

    // no part typed.
    app.assert_button_is_disabled("Add part");
    app.assert_button_is_disabled("Remove part");

    // a part that's shown can only be removed.
    app.type_into("Manufacturer", "RESCO");
    app.type_into("MPN", "1234");
    app.wait_for_enabled_button("Remove part");
    app.assert_button_is_disabled("Add part");

    app.type_into("Manufacturer", "LEDCO");
    app.type_into("MPN", "LED1");
    app.wait_for_enabled_button("Add part");
    app.assert_button_is_disabled("Remove part");

    app.click_button("Add part");
    // now shown, so it can only be removed.
    app.wait_for_enabled_button("Remove part");
    app.wait_until_button_is_disabled("Add part");
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: AddPartClicked",
        "stores_app: Received event. event: ChangeParts {",
        "stores_service: Performing stores operation. operation: WriteParts {",
        "planner_gui_egui::ui_commands: UiApp command. command: StoresReply(StoresReply { operation: WriteParts {",
        "stores_app: Received event. event: PartsWritten {",
        "stores_app: Changed parts. source: '",
    ]);
    app.assert_not_logged("Event failed.");
    assert_eq!(
        fs::read_to_string(&parts).unwrap(),
        format!("{PARTS}\"LEDCO\",\"LED1\",\"\",\"\"\n")
    );

    app.click_button("Remove part");
    app.wait_for_enabled_button("Add part");
    app.wait_until_button_is_disabled("Remove part");
    app.wait_for_logs_in_order(&[
        "planner_gui_egui::ui_app::app_tabs::parts: PartsTab command. command: RemovePartClicked",
        "stores_app: Received event. event: ChangeParts {",
        "stores_app: Changed parts. source: '",
    ]);
    app.assert_not_logged("Event failed.");
    assert_eq!(fs::read_to_string(&parts).unwrap(), PARTS);
}
