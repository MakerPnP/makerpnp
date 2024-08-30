#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};
use freya::prelude::*;
use dioxus_logger::tracing::{debug, Level};
use planner_app::ViewModel;
use crate::app_core::CoreService;
use dioxus_sdk::{
    i18n::{
        use_i18,
        use_init_i18n,
        Language,
    },
    translate,
};
use unic_langid::LanguageIdentifier;

use dioxus_router::prelude::{Outlet, Routable, Router};

mod app_core;

static LANGUAGES: Mutex<Option<Vec<LanguagePair>>> = Mutex::new(None);
static SELECTED_LANGUAGE: Mutex<Option<LanguagePair>> = Mutex::new(None);

fn initialise_languages() {

    let languages: Vec<LanguagePair> = vec![
        LanguagePair { code: "en-US".to_string(), name: "English (United-States)".to_string() },
        LanguagePair { code: "es-ES".to_string(), name: "Español (España)".to_string() },
    ];
    
    let first_language: &LanguagePair = languages.first().unwrap();

    let first_language_identifier: LanguageIdentifier = first_language.code.parse().unwrap();
    
    use_init_i18n(first_language_identifier.clone(), first_language_identifier, || {
        languages.iter().map(|LanguagePair { code, name: _name }|{
            match code.as_str() {
                "en-US" => Language::from_str(EN_US).unwrap(),
                "es-ES" => Language::from_str(ES_ES).unwrap(),
                _ => panic!()
            }
        }).collect()
    });

    let mut guard = SELECTED_LANGUAGE.lock().unwrap();
    (*guard).replace(first_language.clone());

    let mut guard = LANGUAGES.lock().unwrap();
    (*guard).replace(languages);
    
}

fn change_language(language_pair: &LanguagePair) {

    let mut i18n = use_i18();

    let mut guard = SELECTED_LANGUAGE.lock().unwrap();
    (*guard).replace(language_pair.clone());

    i18n.set_language(language_pair.code.parse().unwrap());
}

fn languages() -> MutexGuard<'static, Option<Vec<LanguagePair>>> {
    
    let guard = LANGUAGES.lock().expect("not locked");
    
    guard
}

#[derive(Clone)]
struct LanguagePair {
    code: String,
    name: String,
}

// FIXME avoid cloning, return some reference instead, but how!
fn selected_language() -> LanguagePair {

    let guard = SELECTED_LANGUAGE.lock().expect("not locked");

    guard.as_ref().unwrap().clone()
}

fn app() -> Element {

    initialise_languages();

    change_language(&selected_language());


    rsx!(
        rect {
           font_family: "Arimo Nerd",
            Router::<Route> {}
        }
    )
}

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppSidebar)]
    #[route("/")]
    Home,
    #[route("/project/overview")]
    Overview,
    #[end_layout]
    #[route("/..route")]
    PageNotFound { },
}

#[allow(non_snake_case)]
//#[component]
fn Home() -> Element {
    rsx!(
        label {
            "Home"
        }
    )
}

#[allow(non_snake_case)]
//#[component]
fn Overview() -> Element {
    rsx!(
        label {
            "Overview"
        }
    )
}


#[allow(non_snake_case)]
//#[component]
fn PageNotFound() -> Element {
    rsx!(
        label {
            "Not Found"
        }
    )
}
fn use_change_language() -> Signal<Box<(impl FnMut(LanguagePair) + 'static)>> {
    let mut i18n = use_i18();

    let closure = move |language_pair: LanguagePair| {
        let mut guard = SELECTED_LANGUAGE.lock().unwrap();

        i18n.set_language(language_pair.code.parse().unwrap());
        (*guard).replace(language_pair);
    };
    
    use_signal(move || Box::new(closure))
}

#[allow(non_snake_case)]
fn AppSidebar() -> Element {

    let i18n = use_i18();
    
    let view = use_signal(ViewModel::default);

    let app_core = use_coroutine(|mut rx| {
        let svc = CoreService::new(view);
        async move { svc.run(&mut rx).await }
    });

    let on_click_create = move |_| {
        debug!("create clicked");
        app_core.send(planner_app::Event::CreateProject { project_name: "test".to_string(), path: Default::default() } );
    };

    let on_click_save = move |_| {
        debug!("save clicked");
        app_core.send(planner_app::Event::Save );
    };
    
    let current_language_signal = use_signal(|| {
        let selected_language_binding = selected_language();
        let selected_language = selected_language_binding;

        selected_language
    });
    
    let languages_hooked = use_hook(|| {
        let language_set_binding = languages();
        let language_set = language_set_binding.as_ref().unwrap();

        // FIXME avoid cloning
        language_set.clone()
    });

    let mut change_lang = use_change_language();

    rsx!(
        NativeRouter {
            rect {
                width: "100%",
                height: "32px",
                direction: "horizontal",
                background: "#3C3F41",

                rect {
                    width: "60%",
                    direction: "horizontal",
                    Button {
                        onclick: on_click_create,
                        label {
                            {format!("\u{ea7b} {}", translate!(i18n, "messages.toolbar.button.create"))}
                        }
                    },
                    Button {
                        onclick: on_click_save,
                        label {
                            {format!("\u{e27c} {}", translate!(i18n, "messages.toolbar.button.save"))}
                        }
                    },
                },
                // TODO instead of specifying two rects, each with a width, have a spacer element here which takes
                //      up the remaining space so that the first rect is left justified and the second rect is right justified.
                //      and so that the window cannot be resized smaller than the width of the elements in the two rects.
                rect {
                    width: "40%",
                    direction: "horizontal",
                    main_align: "end",
                    // FIXME this additional rect is required because the dropdown inherits the direction from the parent
                    rect {
                       direction: "vertical",
                        Dropdown {
                            value: current_language_signal.read().name.clone(),
                            for language in languages_hooked {
                                DropdownItem {
                                    value: language.code.clone(),
                                    onclick: {
                                        to_owned![language];
                                        move |_| change_lang.write()(language.clone())
                                    },
                                    label { "{language.name}" }
                                }
                            }
                        }
                    }
                }
            },

            Sidebar {
                sidebar: rsx!(
                    SidebarItem {
                        label {
                            "TODO"
                        }
                    },
                ),
                Body {
                    rect {
                        main_align: "center",
                        cross_align: "center",
                        width: "100%",
                        height: "100%",
                        Outlet::<Route> {  }
                    }
                }
            }
        }
    )
}

static ARIMO_NERD_FONT: &[u8] = include_bytes!("../assets/fonts/ArimoNerdFont-Regular.ttf");


static EN_US: &str = include_str!("../assets/i18n/en-US.json");
static ES_ES: &str = include_str!("../assets/i18n/es-ES.json");

fn main() {
    dioxus_logger::init(Level::DEBUG).expect("failed to init logger");
    console_error_panic_hook::set_once();
    
    launch_cfg(
        app,
        LaunchConfig::<()>::builder()
            .with_font("Arimo Nerd", ARIMO_NERD_FONT)
            .build(),
    );
}