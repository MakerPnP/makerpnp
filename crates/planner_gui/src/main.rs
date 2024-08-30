#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::ops::Index;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};
use freya::prelude::*;
use dioxus_logger::tracing::{debug, Level};
use freya::dioxus_core::AttributeValue;
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

use dioxus_router::prelude::{Outlet, Routable, Router, use_navigator};
use indexmap::IndexMap;

mod app_core;

static LANGUAGE_SET: Mutex<Option<IndexMap<String, String>>> = Mutex::new(None);
static SELECTED_LANGUAGE: Mutex<Option<String>> = Mutex::new(None);

fn load_languages() {

    let language_set: IndexMap<String, String> = IndexMap::from([
        ("en-US".to_string(), "English (United-States)".to_string()),
        ("es-ES".to_string(), "Español (España)".to_string()),
    ]);
    
    let keys = language_set.keys();
    let first_language = keys.into_iter().next().unwrap();
    
    change_language(first_language);
    
    let mut guard = LANGUAGE_SET.lock().unwrap();
    (*guard).replace(language_set);
    
}

fn change_language(language_code: &String) {

    let mut i18 = use_i18();

    let mut guard = SELECTED_LANGUAGE.lock().unwrap();
    (*guard).replace(language_code.clone());

    i18.set_language(language_code.parse().unwrap());
}

fn language_set() -> MutexGuard<'static, Option<IndexMap<String, String>>> {
    
    let guard = LANGUAGE_SET.lock().expect("not locked");
    
    guard
}

// FIXME avoid cloning, return some reference instead, but how!
fn selected_language() -> String {

    let guard = SELECTED_LANGUAGE.lock().expect("not locked");

    guard.as_ref().unwrap().clone()
}

fn app() -> Element {

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
#[component]
fn Home() -> Element {
    rsx!(
        label {
            "Home"
        }
    )
}

#[allow(non_snake_case)]
#[component]
fn Overview() -> Element {
    rsx!(
        label {
            "Overview"
        }
    )
}


#[allow(non_snake_case)]
#[component]
fn PageNotFound() -> Element {
    rsx!(
        label {
            "Not Found"
        }
    )
}

#[allow(non_snake_case)]
fn AppSidebar() -> Element {

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
    
    let current_language_thing = use_signal(|| {
        // WTF goes here ???

        let selected_language_binding = selected_language();
        let selected_language = selected_language_binding;

        selected_language
    });

    
    let language_set_thing = use_hook(|| {
        let language_set_binding = language_set();
        let language_set = language_set_binding.as_ref().unwrap();

        // FIXME avoid cloning
        language_set.clone()
    });

    rsx!(
        NativeRouter {
            rect {
                width: "100%",
                height: "32px",
                direction: "horizontal",
                background: "#3C3F41",

                Button {
                    onclick: on_click_create,
                    label {
                        "\u{ea7b} Create"
                    }
                },
                Button {
                    onclick: on_click_save,
                    label {
                        "\u{e27c} Save"
                    }
                },
                // TODO some divider
                Dropdown {
                    value: current_language_thing.read().clone(),
                    for (value, name) in language_set_thing {
                        DropdownItem {
                            value: value.clone(),
                            onclick: {
                                to_owned![value];
                                move |_| change_language(&value)
                            },
                            label { "{name}" }
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

    load_languages();
    
    launch_cfg(
        app,
        LaunchConfig::<()>::builder()
            .with_font("Arimo Nerd", ARIMO_NERD_FONT)
            .build(),
    );
}