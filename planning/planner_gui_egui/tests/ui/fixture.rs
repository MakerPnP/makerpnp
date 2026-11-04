use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use egui::accesskit::{Action, ActionData, ActionRequest, Role};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use i18n::I18nConfig;
use planner_app::Event;
use planner_gui_egui::file_picker::FileDialogs;
use planner_gui_egui::planner_app_core::PlannerCoreService;
use planner_gui_egui::ui_app::UiApp;
use tempfile::TempDir;

const WINDOW_SIZE: egui::Vec2 = egui::vec2(1920.0, 1080.0);

/// The simulated time per frame, short enough for the frames of a double-click to count as one.
const STEP: f32 = 0.05;

/// A command sent while drawing a frame is handled at the start of the next (`GARCH-11`), and file dialogs answer on a
/// thread of their own, so tests wait for results rather than counting frames.
const TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Put around the values inserted into text by the translation system.
const ISOLATES: [char; 2] = ['\u{2068}', '\u{2069}'];

/// Answers the file dialogs with prepared answers, in order, panicking if there are none left.
#[derive(Debug, Default)]
pub struct ScriptedFileDialogs {
    answers: Mutex<VecDeque<Option<PathBuf>>>,
}

impl ScriptedFileDialogs {
    fn next_answer(&self) -> Option<PathBuf> {
        self.answers
            .lock()
            .unwrap()
            .pop_front()
            .expect("the test prepared an answer for this file dialog")
    }

    fn is_answered(&self) -> bool {
        self.answers.lock().unwrap().is_empty()
    }
}

impl FileDialogs for ScriptedFileDialogs {
    fn pick_file(&self) -> Option<PathBuf> {
        self.next_answer()
    }

    fn pick_files(&self) -> Option<Vec<PathBuf>> {
        self.next_answer()
            .map(|path| vec![path])
    }

    fn pick_folder(&self) -> Option<PathBuf> {
        self.next_answer()
    }
}

/// What the application logs while a test runs, at debug level and above, in the format `planner_gui_egui --trace`
/// writes, without the time. For example ` INFO planner_app: Created project successfully.`
///
/// Each test runs on its own thread, so each captures only its own logs (`LOG-6`).
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);

impl Logs {
    fn content(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl std::io::Write for Logs {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap()
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The whole application, with a temporary folder for its files and no saved state.
pub struct App {
    harness: Harness<'static, UiApp>,
    file_dialogs: Arc<ScriptedFileDialogs>,
    folder: TempDir,
    logs: Logs,
    /// Where the logs not yet checked by `wait_for_logs_in_order` start.
    logs_checked_to: usize,
    // last, so the logs are captured until everything else is dropped.
    _logs_guard: tracing::subscriber::DefaultGuard,
}

impl Drop for App {
    /// Shows the logs of a test that failed, as they're often what explains it. If `PLANNER_GUI_TEST_LOGS` names a
    /// folder, each test's logs are also written to `<folder>/<test name>.log`.
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("logs:\n{}", self.logs.content());
        }
        if let Some(folder) = std::env::var_os("PLANNER_GUI_TEST_LOGS") {
            let test_name = std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .replace("::", ".");
            let path = Path::new(&folder).join(format!("{test_name}.log"));
            if let Err(error) = std::fs::write(&path, self.logs.content()) {
                eprintln!("failed to write the logs. path: {}, error: {error}", path.display());
            }
        }
    }
}

impl App {
    pub fn start() -> Self {
        // translations are global, and `cargo test` runs in the crate's folder, where the translation files are.
        static TRANSLATIONS: Once = Once::new();
        TRANSLATIONS.call_once(|| {
            i18n::init(I18nConfig {
                languages: vec![String::from("en-US")],
                default: "en-US".to_string(),
                fallback: "en-US".to_string(),
            });
        });

        let logs = Logs::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer({
                let logs = logs.clone();
                move || logs.clone()
            })
            .with_ansi(false)
            .without_time()
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        let logs_guard = tracing::subscriber::set_default(subscriber);

        let file_dialogs = Arc::new(ScriptedFileDialogs::default());
        let harness = Harness::builder()
            .with_size(WINDOW_SIZE)
            .with_step_dt(STEP)
            .build_eframe({
                let file_dialogs = file_dialogs.clone();
                move |cc| UiApp::with_file_dialogs(cc, file_dialogs)
            });

        Self {
            harness,
            file_dialogs,
            folder: TempDir::new().unwrap(),
            logs,
            logs_checked_to: 0,
            _logs_guard: logs_guard,
        }
    }

    /// Waits for each message to be logged, in order, after those found by the previous call, as
    /// `assert_contains_inorder!` checks the CLI's trace. A message is matched by any part of its line, for example
    /// `"planner_app: Event failed."`. Order matters: the first message must be logged before the second, and so on.
    pub fn wait_for_logs_in_order(&mut self, messages: &[&str]) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let logs = self.logs.content();
            match find_in_order(&logs[self.logs_checked_to..], messages) {
                Ok(end) => {
                    self.logs_checked_to += end;
                    return;
                }
                Err(index) if Instant::now() > deadline => {
                    panic!(
                        "timed out waiting for the logs, in order.\nnot found, at index {index}:\n{}\nexpected:\n{:#?}\nlogs since those last checked:\n{}",
                        messages[index],
                        messages,
                        &logs[self.logs_checked_to..]
                    );
                }
                Err(_) => {}
            }
            self.harness.step();
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Checks nothing logged since those found by `wait_for_logs_in_order` contains the text.
    pub fn assert_not_logged(&self, text: &str) {
        let logs = self.logs.content();
        let unchecked = &logs[self.logs_checked_to..];
        assert!(
            !unchecked.contains(text),
            "expected nothing logged to contain '{text}', logs since those last checked:\n{unchecked}"
        );
    }

    /// Saves a new project in the temporary folder, using the core, so the format is always the current one.
    pub fn given_a_project_file(&self, file_name: &str) -> PathBuf {
        let path = self.folder.path().join(file_name);

        let mut core = PlannerCoreService::new();
        core.update(Event::CreateProject {
            name: "test".to_string(),
            path: path.clone(),
            packages: None,
            package_mappings: None,
        })
        .unwrap_or_else(|error| panic!("{error:?}"));
        core.update(Event::Save)
            .unwrap_or_else(|error| panic!("{error:?}"));

        path
    }

    /// Prepares the answer for the next file dialog, `None` being the user cancelling it.
    pub fn when_a_file_dialog_opens_answer(&self, answer: Option<PathBuf>) {
        self.file_dialogs
            .answers
            .lock()
            .unwrap()
            .push_back(answer);
    }

    /// Waits until all the prepared answers have been taken and handled.
    pub fn wait_for_file_dialogs(&mut self) {
        self.wait_until("the file dialogs were answered", |app| {
            app.file_dialogs.is_answered() && !app.harness.state().is_picking_file()
        });
    }

    pub fn click_button(&mut self, label: &str) {
        self.wait_for_button(label);
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::Button, label)
            .click();
        self.harness.step();
    }

    /// Controls move in the frames after they first appear, as their containers are laid out around them, and a click
    /// only hits where a control was in the previous frame.  While a layout is changing, `egui_taffy` asks egui to
    /// discard the pass and do another, so a frame with only one pass has a layout that has stopped changing.
    fn wait_for_the_layout(&mut self) {
        self.wait_until("the layout stopped changing", |app| {
            app.harness
                .output()
                .platform_output
                .num_completed_passes
                == 1
        });
    }

    /// Waits for the text field to contain something, for example the answer to a file dialog.
    pub fn wait_until_field_is_not_empty(&mut self, label: &str) {
        self.wait_until(&format!("text field '{label}' was not empty"), |app| {
            app.field_value(label)
                .is_some_and(|value| !value.is_empty())
        });
    }

    pub fn wait_until_field_is_empty(&mut self, label: &str) {
        self.wait_until(&format!("text field '{label}' was empty"), |app| {
            app.field_value(label)
                .is_some_and(|value| value.is_empty())
        });
    }

    pub fn wait_for_field_value(&mut self, label: &str, value: &str) {
        self.wait_until(&format!("text field '{label}' contained '{value}'"), |app| {
            app.field_value(label)
                .is_some_and(|candidate| candidate == value)
        });
    }

    pub fn wait_until_field_is_gone(&mut self, label: &str) {
        self.wait_until(&format!("text field '{label}' was gone"), |app| {
            !app.is_shown(Role::TextInput, label)
        });
    }

    pub fn click_checkbox(&mut self, label: &str) {
        self.wait_until(&format!("checkbox '{label}' was shown"), |app| {
            app.is_shown(Role::CheckBox, label)
        });
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::CheckBox, label)
            .click();
        self.harness.step();
    }

    pub fn wait_for_checkbox(&mut self, label: &str, checked: bool) {
        let expected = match checked {
            true => egui::accesskit::Toggled::True,
            false => egui::accesskit::Toggled::False,
        };
        self.wait_until(&format!("checkbox '{label}' was {expected:?}"), |app| {
            app.harness
                .query_by_role_and_label(Role::CheckBox, label)
                .is_some_and(|checkbox| checkbox.accesskit_node().toggled() == Some(expected))
        });
    }

    fn field_value(&self, label: &str) -> Option<String> {
        self.harness
            .query_by_role_and_label(Role::TextInput, label)
            .map(|field| {
                field
                    .accesskit_node()
                    .value()
                    .unwrap_or_default()
            })
    }

    pub fn wait_until_button_is_gone(&mut self, label: &str) {
        self.wait_until(&format!("button '{label}' was gone"), |app| {
            !app.is_shown(Role::Button, label)
        });
    }

    /// For a check that is made after the application has handled what it depends on, so waiting isn't needed.
    pub fn assert_button_is_disabled(&mut self, label: &str) {
        self.wait_for_button(label);
        assert!(!self.is_button_enabled(label), "button '{label}' should be disabled");
    }

    pub fn is_button_enabled(&self, label: &str) -> bool {
        self.harness
            .query_by_role_and_label(Role::Button, label)
            .is_some_and(|button| !button.accesskit_node().is_disabled())
    }

    /// Clicks a button that appears several times, the one in the same row as the text.
    pub fn click_button_in_row(&mut self, row_text: &str, label: &str) {
        self.wait_for_text(row_text);
        self.wait_for_the_layout();
        let row = self.text_position(row_text).y;
        self.harness
            .query_all_by_role_and_label(Role::Button, label)
            .find(|button| (button.rect().center().y - row).abs() < 12.0)
            .expect("the row has the button")
            .click();
        self.harness.step();
    }

    /// For a button that appears several times, the one in the same row as the text.
    pub fn wait_until_button_in_row_is(&mut self, row_text: &str, label: &str, enabled: bool) {
        self.wait_for_text(row_text);
        self.wait_for_the_layout();
        let state = if enabled { "enabled" } else { "disabled" };
        self.wait_until(&format!("button '{label}' in row '{row_text}' was {state}"), |app| {
            let row = app.text_position(row_text).y;
            app.harness
                .query_all_by_role_and_label(Role::Button, label)
                .find(|button| (button.rect().center().y - row).abs() < 12.0)
                .is_some_and(|button| button.accesskit_node().is_disabled() != enabled)
        });
    }

    /// The labels of the combo boxes shown.
    pub fn combo_box_labels(&self) -> Vec<String> {
        self.harness
            .query_all(By::new().role(Role::ComboBox))
            .filter_map(|combo_box| combo_box.accesskit_node().label())
            .collect()
    }

    /// Opens a combo box, so that its options can be checked, see `close_popup`.
    pub fn open_combo_box(&mut self, label: &str) {
        self.wait_until(&format!("combo box '{label}' was enabled"), |app| {
            app.harness
                .query_by_role_and_label(Role::ComboBox, label)
                .is_some_and(|combo_box| !combo_box.accesskit_node().is_disabled())
        });
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::ComboBox, label)
            .click();
        self.harness.step();
    }

    pub fn close_popup(&mut self) {
        self.harness
            .key_press(egui::Key::Escape);
        self.harness.step();
    }

    /// Clicks the first of the buttons with this label, the one nearest the top of a tab.
    pub fn click_first_button(&mut self, label: &str) {
        self.wait_for_the_layout();
        self.harness
            .query_all_by_role_and_label(Role::Button, label)
            .next()
            .expect("there is a button")
            .click();
        self.harness.step();
    }

    pub fn wait_until_combo_box_is_disabled(&mut self, label: &str) {
        self.wait_until(&format!("combo box '{label}' was disabled"), |app| {
            app.harness
                .query_by_role_and_label(Role::ComboBox, label)
                .is_some_and(|combo_box| combo_box.accesskit_node().is_disabled())
        });
    }

    pub fn clear_field(&mut self, label: &str) {
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::TextInput, label)
            .click();
        self.harness.step();
        self.harness
            .key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        self.harness.step();
        self.harness
            .key_press(egui::Key::Backspace);
        self.harness.step();
        self.wait_until_field_is_empty(label);
    }

    pub fn wait_until_text_is_gone(&mut self, text: &str) {
        self.wait_until(&format!("text '{text}' was gone"), |app| app.count_text(text) == 0);
    }

    /// Clicks the only tab whose label contains `text`.
    pub fn click_tab_containing(&mut self, text: &str) {
        self.wait_until(&format!("a tab containing '{text}' was shown"), |app| {
            app.count_tabs_containing(text) == 1
        });
        self.wait_for_the_layout();
        self.harness
            .query_all(By::new().role(Role::Tab))
            .find(|tab| {
                let node = tab.accesskit_node();

                node.label()
                    .or_else(|| node.value())
                    .is_some_and(|label| label.contains(text))
            })
            .unwrap()
            .click();
        self.harness.step();
    }

    pub fn count_tabs_containing(&self, text: &str) -> usize {
        self.harness
            .query_all(By::new().role(Role::Tab))
            .filter(|tab| {
                let node = tab.accesskit_node();

                node.label()
                    .or_else(|| node.value())
                    .is_some_and(|label| label.contains(text))
            })
            .count()
    }

    pub fn wait_until_no_tab_contains(&mut self, text: &str) {
        self.wait_until(&format!("no tab contained '{text}'"), |app| {
            app.count_tabs_containing(text) == 0
        });
    }

    pub fn wait_for_enabled_button(&mut self, label: &str) {
        self.wait_until(&format!("button '{label}' was enabled"), |app| {
            app.harness
                .query_by_role_and_label(Role::Button, label)
                .is_some_and(|button| !button.accesskit_node().is_disabled())
        });
    }

    pub fn wait_until_button_is_disabled(&mut self, label: &str) {
        self.wait_until(&format!("button '{label}' was disabled"), |app| {
            app.harness
                .query_by_role_and_label(Role::Button, label)
                .is_some_and(|button| button.accesskit_node().is_disabled())
        });
    }

    /// Clicks a text field, replaces its text, and waits for the application to have accepted the text.
    pub fn type_into(&mut self, label: &str, text: &str) {
        self.wait_until(&format!("text field '{label}' was shown"), |app| {
            app.is_shown(Role::TextInput, label)
        });
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::TextInput, label)
            .click();
        self.harness.step();
        self.harness
            .key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        self.harness.step();
        self.harness
            .get_by_role_and_label(Role::TextInput, label)
            .type_text(text);
        self.harness.step();
        self.wait_until(&format!("text field '{label}' contained '{text}'"), |app| {
            app.harness
                .get_by_role_and_label(Role::TextInput, label)
                .accesskit_node()
                .value()
                .as_deref()
                == Some(text)
        });
    }

    /// Sets a number field, as assistive technology would, and waits for the application to have accepted the number.
    pub fn set_number(&mut self, label: &str, value: f64) {
        self.wait_until(&format!("number field '{label}' was shown"), |app| {
            app.is_shown(Role::SpinButton, label)
        });
        let (target_node, target_tree) = self
            .harness
            .get_by_role_and_label(Role::SpinButton, label)
            .accesskit_node()
            .locate();
        self.harness
            .event(egui::Event::AccessKitActionRequest(ActionRequest {
                action: Action::SetValue,
                target_node,
                target_tree,
                data: Some(ActionData::NumericValue(value)),
            }));
        self.harness.step();
        self.wait_until(&format!("number field '{label}' contained {value}"), |app| {
            app.harness
                .get_by_role_and_label(Role::SpinButton, label)
                .accesskit_node()
                .numeric_value()
                == Some(value)
        });
    }

    pub fn count_text(&self, text: &str) -> usize {
        self.harness
            .query_all_by_role_and_label(Role::Label, text)
            .count()
    }

    /// For text with parts that vary, such as a tree item.
    pub fn wait_for_text_containing(&mut self, text: &str) {
        self.wait_until(&format!("text containing '{text}' was shown"), |app| {
            app.shown().iter().any(|shown| {
                let shown = shown.replace(ISOLATES, "");

                shown.starts_with("Label") && shown.contains(text)
            })
        });
    }

    pub fn wait_until_no_text_contains(&mut self, text: &str) {
        self.wait_until(&format!("no text contained '{text}'"), |app| {
            !app.shown().iter().any(|shown| {
                let shown = shown.replace(ISOLATES, "");

                shown.starts_with("Label") && shown.contains(text)
            })
        });
    }

    /// Clicks the row number at the start of the row containing the text, which selects the row of a data table.
    pub fn click_row_header_of(&mut self, text: &str) {
        let position = self.row_header_position(text);

        self.click_at(position);
    }

    /// Each in its own frame, as they would be for a user.
    fn click_at(&mut self, position: egui::Pos2) {
        self.harness
            .event(egui::Event::PointerMoved(position));
        self.harness.step();
        for pressed in [true, false] {
            self.harness
                .event(egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            self.harness.step();
        }
    }

    /// Where the column with this header crosses the row containing the text.
    fn data_table_cell_position(&mut self, row_text: &str, column: &str) -> egui::Pos2 {
        self.wait_for_text(row_text);
        self.wait_for_text(column);
        self.wait_for_the_layout();

        egui::pos2(self.text_position(column).x, self.text_position(row_text).y)
    }

    /// The text field that a data table shows while one of its cells is being edited.
    fn data_table_cell_editor_at(&self, position: egui::Pos2) -> Option<egui_kittest::Node<'_>> {
        self.harness
            .query_all(By::new().role(Role::TextInput))
            .find(|field| field.rect().contains(position))
    }

    /// Edits a cell of a data table, the first click selects the cell and the second starts editing it.  The change
    /// is only finished by clicking on a different cell, here the same column of another row.
    pub fn edit_data_table_cell(&mut self, row_text: &str, column: &str, value: &str, then_click_row: &str) {
        let position = self.data_table_cell_position(row_text, column);

        self.click_at(position);
        self.click_at(position);
        self.wait_until("the cell's editor was shown", |app| {
            app.data_table_cell_editor_at(position)
                .is_some()
        });
        self.wait_for_the_layout();

        self.data_table_cell_editor_at(position)
            .unwrap()
            .click();
        self.harness.step();
        self.harness
            .key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        self.harness.step();
        if value.is_empty() {
            self.harness
                .key_press(egui::Key::Backspace);
        } else {
            self.data_table_cell_editor_at(position)
                .unwrap()
                .type_text(value);
        }
        self.harness.step();
        self.wait_until(&format!("the cell's editor contained '{value}'"), |app| {
            app.data_table_cell_editor_at(position)
                .is_some_and(|field| {
                    field
                        .accesskit_node()
                        .value()
                        .unwrap_or_default()
                        == value
                })
        });

        let other_position = self.data_table_cell_position(then_click_row, column);
        self.click_at(other_position);
    }

    /// Chooses a radio button in the editor of a cell of a data table, and finishes the change.  The first click
    /// selects the cell and the second starts editing it.  The editor covers the rows below it, so the change is
    /// finished by clicking on the row's text, which is another cell of the same row.
    pub fn choose_in_data_table_cell(&mut self, row_text: &str, column: &str, option: &str) {
        self.choose_in_data_table_cell_offering(row_text, column, option, None);
    }

    /// The same, checking that the cell's editor offers exactly the options in `offered`, in that order.
    pub fn choose_in_data_table_cell_offering(
        &mut self,
        row_text: &str,
        column: &str,
        option: &str,
        offered: Option<&[&str]>,
    ) {
        let position = self.data_table_cell_position(row_text, column);

        self.click_at(position);
        self.click_at(position);
        self.wait_until(&format!("the cell's editor offered '{option}'"), |app| {
            app.is_shown(Role::RadioButton, option)
        });
        if let Some(offered) = offered {
            let labels = self
                .harness
                .query_all(By::new().role(Role::RadioButton))
                .map(|radio| {
                    radio
                        .accesskit_node()
                        .label()
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                labels, offered,
                "the options offered by the '{column}' cell of '{row_text}'"
            );
        }
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::RadioButton, option)
            .click();
        self.harness.step();
        self.wait_until(&format!("'{option}' was chosen"), |app| {
            app.harness
                .get_by_role_and_label(Role::RadioButton, option)
                .accesskit_node()
                .toggled()
                == Some(egui::accesskit::Toggled::True)
        });

        let row = self.text_position(row_text);
        self.click_at(row);
        self.wait_until("the cell's editor was closed", |app| {
            !app.is_shown(Role::RadioButton, option)
        });
    }

    /// Chooses an option in the combo box that a data table shows while one of its cells is being edited, and finishes
    /// the change by clicking on the row's text, which is another cell of the same row.  Checks that the options in
    /// `not_offered` are not offered first.
    pub fn choose_in_data_table_cell_combo_box(
        &mut self,
        row_text: &str,
        column: &str,
        option: &str,
        not_offered: &[&str],
    ) {
        let position = self.data_table_cell_position(row_text, column);

        self.click_at(position);
        self.click_at(position);
        self.wait_until("the cell's editor was shown", |app| {
            app.data_table_cell_combo_box_at(position)
                .is_some()
        });
        self.wait_for_the_layout();
        self.data_table_cell_combo_box_at(position)
            .unwrap()
            .click();
        self.harness.step();
        self.wait_for_button(option);

        for other in not_offered {
            assert!(!self.is_button_shown(other), "'{other}' should not be offered");
        }

        self.harness
            .get_by_role_and_label(Role::Button, option)
            .click();
        self.harness.step();
        self.wait_until(&format!("'{option}' was chosen"), |app| {
            app.data_table_cell_combo_box_at(position)
                .is_some_and(|combo_box| {
                    combo_box
                        .accesskit_node()
                        .value()
                        .is_some_and(|value| value == option)
                })
        });

        let row = self.text_position(row_text);
        self.click_at(row);
        self.wait_until("the cell's editor was closed", |app| {
            app.data_table_cell_combo_box_at(position)
                .is_none()
        });
    }

    fn data_table_cell_combo_box_at(&self, position: egui::Pos2) -> Option<egui_kittest::Node<'_>> {
        self.harness
            .query_all(By::new().role(Role::ComboBox))
            .find(|combo_box| combo_box.rect().contains(position))
    }

    /// Clicks a cell of a data table twice, which would start editing it, and checks no editor is shown.
    pub fn assert_data_table_cell_is_not_editable(&mut self, row_text: &str, column: &str) {
        let position = self.data_table_cell_position(row_text, column);

        self.click_at(position);
        self.click_at(position);
        // an editor is shown as soon as the table handles the click, it doesn't wait for a command to be handled.
        self.harness.run_steps(10);

        let editors = [Role::TextInput, Role::ComboBox, Role::RadioButton]
            .into_iter()
            .flat_map(|role| {
                self.harness
                    .query_all(By::new().role(role))
                    // the radio buttons of an editor are taller than its row, so don't all contain the position.
                    .filter(|editor| role == Role::RadioButton || editor.rect().contains(position))
                    .map(|editor| format!("{role:?} {:?}", editor.accesskit_node().label()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert!(
            editors.is_empty(),
            "the '{column}' cell of '{row_text}' should not be editable, shown: {editors:?}"
        );
    }

    pub fn wait_until_file_exists(&mut self, path: &Path) {
        self.wait_until(&format!("file {} existed", path.display()), |_app| path.exists());
    }

    /// Clicks text that also appears behind a dialog, dialogs are drawn after what's behind them.
    pub fn click_text_in_dialog(&mut self, text: &str) {
        self.wait_for_text_at_least_once(text);
        self.wait_for_the_layout();
        self.harness
            .query_all_by_role_and_label(Role::Label, text)
            .last()
            .unwrap()
            .click();
        self.harness.step();
    }

    fn wait_for_text_at_least_once(&mut self, text: &str) {
        self.wait_until(&format!("text '{text}' was shown"), |app| app.count_text(text) > 0);
    }

    pub fn click_radio_button(&mut self, label: &str) {
        self.wait_until(&format!("radio button '{label}' was shown"), |app| {
            app.is_shown(Role::RadioButton, label)
        });
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::RadioButton, label)
            .click();
        self.harness.step();
    }

    /// The number at the start of the row containing the text.
    fn row_header_position(&mut self, text: &str) -> egui::Pos2 {
        self.wait_for_text(text);
        self.wait_for_the_layout();
        let row = self
            .harness
            .get_by_role_and_label(Role::Label, text)
            .rect()
            .center()
            .y;
        self.harness
            .query_all(By::new().role(Role::Label))
            .filter(|label| {
                let node = label.accesskit_node();

                node.label()
                    .or_else(|| node.value())
                    .is_some_and(|text| !text.is_empty() && text.chars().all(|c| c.is_ascii_digit()))
            })
            .map(|label| label.rect().center())
            .filter(|position| (position.y - row).abs() < 8.0)
            .min_by(|a, b| a.x.total_cmp(&b.x))
            .expect("the row has a number")
    }

    /// Drags a row of a data table, by its number, to where another row is.
    pub fn drag_row(&mut self, text: &str, to_row_containing: &str) {
        let start = self.row_header_position(text);
        let end = self.row_header_position(to_row_containing);

        self.harness
            .event(egui::Event::PointerMoved(start));
        self.harness.step();
        self.harness
            .event(egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
        self.harness.step();
        for step in 1..=5 {
            self.harness
                .event(egui::Event::PointerMoved(start + (end - start) * (step as f32 / 5.0)));
            self.harness.step();
        }
        self.harness
            .event(egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
        self.harness.step();
        self.harness.step();
        self.wait_for_the_layout();
    }

    /// Where some text is, which is unique.
    pub fn text_position(&self, text: &str) -> egui::Pos2 {
        self.harness
            .get_by_role_and_label(Role::Label, text)
            .rect()
            .center()
    }

    pub fn wait_for_text_count(&mut self, text: &str, count: usize) {
        self.wait_until(&format!("text '{text}' was shown {count} times"), |app| {
            app.count_text(text) == count
        });
    }

    /// Waits for a table's resize handle, named as `FORM-6` requires.
    pub fn wait_for_resize_handle(&mut self, name: &str) {
        self.wait_until(&format!("resize handle '{name}' was shown"), |app| {
            app.is_shown(Role::Splitter, name)
        });
    }

    /// Drags a table's resize handle, so that the table is taller by `height`.
    pub fn drag_resize_handle(&mut self, name: &str, height: f32) {
        self.wait_for_resize_handle(name);
        self.wait_for_the_layout();
        let start = self
            .harness
            .get_by_role_and_label(Role::Splitter, name)
            .rect()
            .center();
        let end = start + egui::vec2(0.0, height);

        self.harness
            .event(egui::Event::PointerMoved(start));
        self.harness.step();
        for (position, pressed) in [(start, true), (end, false)] {
            self.harness
                .event(egui::Event::PointerMoved(position));
            self.harness.step();
            self.harness
                .event(egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            self.harness.step();
        }
        self.wait_for_the_layout();
    }

    /// The height of a table, from the top of one of its column headers to the bottom of its resize handle.
    pub fn table_height(&self, column_header: &str, resize_handle: &str) -> f32 {
        let top = self
            .harness
            .get_by_role_and_label(Role::Label, column_header)
            .rect()
            .top();
        let bottom = self
            .harness
            .get_by_role_and_label(Role::Splitter, resize_handle)
            .rect()
            .bottom();

        bottom - top
    }

    /// Opens a combo box and clicks one of its options.
    /// Waits for the choice to be shown.  Showing it can move other controls, for example by removing the field's
    /// error, and commands are handled in a later frame (`GARCH-11`), so without waiting they could move while the next
    /// control is being clicked.
    pub fn choose_from_combo_box(&mut self, label: &str, option: &str) {
        self.choose_action_from_combo_box(label, option);
        self.wait_for_combo_box_value(label, option);
    }

    /// For a combo box that doesn't show what was chosen, such as a menu of actions.
    pub fn choose_action_from_combo_box(&mut self, label: &str, option: &str) {
        self.wait_until(&format!("combo box '{label}' was enabled"), |app| {
            app.harness
                .query_by_role_and_label(Role::ComboBox, label)
                .is_some_and(|combo_box| !combo_box.accesskit_node().is_disabled())
        });
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::ComboBox, label)
            .click();
        self.harness.step();
        self.click_button(option);
    }

    pub fn wait_for_combo_box_value(&mut self, label: &str, value: &str) {
        self.wait_until(&format!("combo box '{label}' showed '{value}'"), |app| {
            app.harness
                .query_by_role_and_label(Role::ComboBox, label)
                .is_some_and(|combo_box| {
                    combo_box
                        .accesskit_node()
                        .value()
                        .is_some_and(|shown| shown == value)
                })
        });
    }

    /// The temporary folder for this application's files.
    pub fn folder(&self) -> PathBuf {
        self.folder.path().to_path_buf()
    }

    /// Clicks plain text, such as a row of a table.
    pub fn click_text(&mut self, text: &str) {
        self.wait_until(&format!("text '{text}' was shown"), |app| {
            app.is_shown(Role::Label, text)
        });
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::Label, text)
            .click();
        self.harness.step();
    }

    /// Double-clicks plain text, such as an item in a tree view.
    pub fn double_click_text(&mut self, text: &str) {
        self.wait_until(&format!("text '{text}' was shown"), |app| {
            app.is_shown(Role::Label, text)
        });
        let position = self
            .harness
            .get_by_role_and_label(Role::Label, text)
            .rect()
            .center();

        self.double_click_at(position);
    }

    /// Double-clicks plain text that contains values, such as an item in a tree view.
    pub fn double_click_text_containing(&mut self, text: &str) {
        self.wait_for_text_containing(text);
        self.wait_for_the_layout();
        let position = self
            .harness
            .query_all(By::new().role(Role::Label))
            .find(|label| {
                let node = label.accesskit_node();

                node.label()
                    .or_else(|| node.value())
                    .is_some_and(|shown| {
                        shown
                            .replace(ISOLATES, "")
                            .contains(text)
                    })
            })
            .unwrap()
            .rect()
            .center();

        self.double_click_at(position);
    }

    fn double_click_at(&mut self, position: egui::Pos2) {
        // pause, as a user would, otherwise egui counts the previous click and sees a triple-click.
        self.harness
            .run_steps((1.0 / STEP) as usize);

        // each in its own frame, as they would be for a user, since `Node::click` does all of this in one frame, which
        // egui_ltreeview does not recognise as a double-click.
        self.harness
            .event(egui::Event::PointerMoved(position));
        self.harness.step();
        for pressed in [true, false, true, false] {
            self.harness
                .event(egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            self.harness.step();
        }
    }

    pub fn is_button_shown(&self, label: &str) -> bool {
        self.is_shown(Role::Button, label)
    }

    pub fn is_tab_shown(&self, label: &str) -> bool {
        self.is_shown(Role::Tab, label)
    }

    pub fn wait_for_button(&mut self, label: &str) {
        self.wait_until(&format!("button '{label}' was shown"), |app| app.is_button_shown(label));
    }

    pub fn wait_for_text(&mut self, text: &str) {
        self.wait_until(&format!("text '{text}' was shown"), |app| {
            app.is_shown(Role::Label, text)
        });
    }

    pub fn click_tab(&mut self, label: &str) {
        self.wait_for_tab(label);
        self.wait_for_the_layout();
        self.harness
            .get_by_role_and_label(Role::Tab, label)
            .click();
        self.harness.step();
    }

    pub fn wait_for_tab(&mut self, label: &str) {
        self.wait_until(&format!("tab '{label}' was shown"), |app| app.is_tab_shown(label));
    }

    fn is_shown(&self, role: Role, label: &str) -> bool {
        self.harness
            .query_by_role_and_label(role, label)
            .is_some()
    }

    /// Runs the application until the condition is true, panics if it never is.
    pub fn wait_until(&mut self, description: &str, condition: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            self.harness.step();
            if condition(self) {
                return;
            }
            if Instant::now() > deadline {
                panic!("timed out waiting until {description}, shown: {:#?}", self.shown());
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Everything with some text, for the failure message.
    fn shown(&self) -> Vec<String> {
        let mut shown: Vec<String> = self
            .harness
            .query_all(By::new())
            .filter_map(|node| {
                let node = node.accesskit_node();
                let text = node.label().or_else(|| node.value())?;
                Some(format!("{:?}: {text}", node.role()))
            })
            .collect();
        shown.sort();
        shown
    }
}

/// Finds each message in the logs, in order, returning where the line of the last one ends, or the index of the first
/// not found. Each message is found after the line of the one before, so two messages can't match the same line.
fn find_in_order(logs: &str, messages: &[&str]) -> Result<usize, usize> {
    let mut end = 0;
    for (index, message) in messages.iter().enumerate() {
        match logs[end..].find(message) {
            Some(start) => {
                let matched = end + start + message.len();
                end = match logs[matched..].find('\n') {
                    Some(newline) => matched + newline + 1,
                    None => logs.len(),
                };
            }
            None => return Err(index),
        }
    }
    Ok(end)
}

/// `LOG-7`
#[test]
fn logs_are_found_only_in_the_order_given() {
    let logs = "DEBUG a: Clicked.\nDEBUG planner_app: Received event. event: Save\n INFO planner_app: Saved project.\n";

    assert!(find_in_order(logs, &["Clicked.", "Received event. event: Save", "Saved project."]).is_ok());
    assert!(find_in_order(logs, &["Clicked.", "Saved project."]).is_ok());
    assert_eq!(find_in_order(logs, &["Saved project.", "Clicked."]), Err(1));
    assert_eq!(find_in_order(logs, &["Clicked.", "Clicked."]), Err(1));
    // each message is found on a line of its own.
    assert_eq!(find_in_order(logs, &["Received event.", "event: Save"]), Err(1));
}
