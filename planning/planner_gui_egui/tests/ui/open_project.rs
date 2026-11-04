use crate::fixture::App;

const PROJECT_FILE: &str = "job1.mpnp.json";

/// A button that's only on a project's toolbar, so it's only shown once the project has opened.
const PROJECT_TOOLBAR_BUTTON: &str = "Generate artifacts";

#[test]
fn the_toolbar_is_shown_on_startup() {
    let mut app = App::start();

    for button in [
        "Home",
        "New project",
        "Open project",
        "New PCB",
        "Open PCB",
        "Save",
        "Close all",
    ] {
        app.wait_for_button(button);
    }
}

/// `STORE-10`, the project is read by the stores service.
#[test]
fn opening_a_project_shows_its_tab_and_its_toolbar() {
    let mut app = App::start();
    let project = app.given_a_project_file(PROJECT_FILE);
    app.when_a_file_dialog_opens_answer(Some(project));

    app.click_button("Open project");

    app.wait_for_tab(PROJECT_FILE);
    app.wait_for_button(PROJECT_TOOLBAR_BUTTON);
    app.wait_for_logs_in_order(&[
        "planner_app: Received event. event: Load {",
        "stores_service: Performing stores operation. operation: ReadDocument { path: ",
        "planner_app: Received event. event: ProjectLoaded {",
    ]);
    app.assert_not_logged("Event failed.");
}

#[test]
fn double_clicking_an_item_in_the_project_explorer_opens_its_tab() {
    let mut app = App::start();
    let project = app.given_a_project_file(PROJECT_FILE);
    app.when_a_file_dialog_opens_answer(Some(project));
    app.click_button("Open project");
    app.wait_for_tab(PROJECT_FILE);
    assert!(!app.is_tab_shown("Parts"));

    app.double_click_text("Parts");

    app.wait_for_tab("Parts");
}

#[test]
fn a_project_can_be_opened_after_cancelling_the_file_dialog() {
    let mut app = App::start();
    let project = app.given_a_project_file(PROJECT_FILE);
    app.when_a_file_dialog_opens_answer(None);

    app.click_button("Open project");
    app.wait_for_file_dialogs();
    assert!(!app.is_tab_shown(PROJECT_FILE));

    app.when_a_file_dialog_opens_answer(Some(project));
    app.click_button("Open project");

    app.wait_for_tab(PROJECT_FILE);
}
