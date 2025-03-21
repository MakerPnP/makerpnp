#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use i18n::I18nConfig;
use planner_gui_egui::ui_app::UiApp;

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 440.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    i18n::init(I18nConfig {
        languages: vec![String::from("es-ES"), String::from("en-US")],
        default: "en-US".to_string(),
        fallback: "en-US".to_string(),
    });

    if let Err(e) = eframe::run_native(
        "MakerPnP - Planner",
        options,
        Box::new(|cc| Ok(Box::new(UiApp::new(cc)))),
    ) {
        eprintln!("Failed to run eframe: {:?}", e);
    }
}
