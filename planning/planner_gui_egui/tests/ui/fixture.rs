use std::collections::VecDeque;
use std::path::PathBuf;
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

/// Commands are handled on other threads, so tests have to wait for their results.
const TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(5);

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

/// The whole application, with a temporary folder for its files and no saved state.
pub struct App {
    harness: Harness<'static, UiApp>,
    file_dialogs: Arc<ScriptedFileDialogs>,
    folder: TempDir,
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
        }
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
    fn wait_until(&mut self, description: &str, condition: impl Fn(&Self) -> bool) {
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
