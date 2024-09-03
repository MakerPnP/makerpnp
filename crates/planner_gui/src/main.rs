#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};
use tracing::{info, debug};
use freya::prelude::*;
use dioxus_router::prelude::{Outlet, Routable, Router, use_route};
use dioxus_sdk::{
    i18n::{
        use_i18,
        use_init_i18n,
        Language,
    },
    translate,
};
use unic_langid::LanguageIdentifier;
use tracing_subscriber::{EnvFilter, fmt};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use planner_app::ViewModel;
use crate::app_core::CoreService;

mod app_core;

static LANGUAGES: Mutex<Vec<LanguagePair>> = Mutex::new(Vec::new());
static SELECTED_LANGUAGE: Mutex<Option<LanguagePair>> = Mutex::new(None);

// TODO make loading languages dynamic so that translators don't have to re-compile to test.
//      the code is prepared for this by using the two mutexes above instead of using
//      static constants.
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
    (*guard).extend(languages);
    
}

fn change_language(language_pair: &LanguagePair) {

    let mut i18n = use_i18();

    let mut guard = SELECTED_LANGUAGE.lock().unwrap();
    (*guard).replace(language_pair.clone());

    i18n.set_language(language_pair.code.parse().unwrap());
}

fn languages() -> MutexGuard<'static, Vec<LanguagePair>> {
    
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
            Router::<TabRoute> {}
            Router::<DocumentRoute> {}
        }
    )
}


#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum TabRoute {
    #[layout(AppTabsBar)]
    #[route("/")]
    EmptyTab,
    #[route("/:tab")]
    DocumentTab { tab: String },
    #[end_layout]
    #[route("/..route")]
    TabNotFound { },
}

#[allow(non_snake_case)]
#[component]
fn DocumentTab(tab: String) -> Element {
    let tab_path: TabRoute = use_route();
    info!("tab_path: {}", tab_path);

    DocumentRouteLayout()
}

#[allow(non_snake_case)]
fn EmptyTab() -> Element {
    let tab_path: TabRoute = use_route();
    info!("tab_path: {}", tab_path);

    rsx!(
        label {
            "Empty tab"
        }
    )
}

#[allow(non_snake_case)]
fn TabNotFound() -> Element {
    rsx!(
        label {
            "Tab not found"
        }
    )
}

#[allow(non_snake_case)]
fn AppTabsBar() -> Element {
    let view = use_signal(ViewModel::default);

    let i18n = use_i18();

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

    let languages_hooked = use_hook(|| {
        let languages_binding = languages();
        let languages = &(*languages_binding);

        // FIXME avoid cloning
        languages.clone()
    });

    let mut change_lang = use_change_language();

    rsx!(
        NativeRouter {
            rect {
                width: "100%",
                height: "44",
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
                            value: format!("\u{f1ab}"),
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

            Tabsbar {
                Link {
                    to: TabRoute::EmptyTab,
                    ActivableRoute {
                        route: TabRoute::EmptyTab,
                        exact: true,
                        Tab {
                            label {
                                "Empty tab"
                            }
                        }
                    }
                },
                Link {
                    to: TabRoute::DocumentTab { tab: "document_1".into() },
                    ActivableRoute {
                        route: TabRoute::DocumentTab { tab: "document_1".into() },
                        exact: true,
                        Tab {
                            label {
                                "Document 1"
                            }
                        }
                    }
                },
                Link {
                    to: TabRoute::DocumentTab { tab: "document_2".into() },
                    ActivableRoute {
                        route: TabRoute::DocumentTab { tab: "document_2".into() },
                        exact: true,
                        Tab {
                            label {
                                "Document 2"
                            }
                        }
                    }
                },
            },

            Body {
                rect {
                    main_align: "center",
                    cross_align: "center",
                    width: "100%",
                    height: "100%",
                    Outlet::<TabRoute> {  }
                }
            }
        }
    )
}



#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum DocumentRoute {
    #[layout(DocumentRouteLayout)]
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
fn DocumentRouteLayout() -> Element {
    
    rsx!(
        NativeRouter {
            DocumentLayout {}
        }
    )
}

#[allow(non_snake_case)]
fn DocumentLayout() -> Element {

    let view = use_signal(ViewModel::default);

    // TODO get the current document?
    let document_path: DocumentRoute = use_route();
    info!("document_path: {}", document_path);
    rsx!(

        Sidebar {
            sidebar: rsx!(
                SidebarItem {
                    label {
                        "TODO_1"
                    }
                },
                SidebarItem {
                    label {
                        "TODO_2"
                    }
                },
            ),
            Body {
                rect {
                    main_align: "center",
                    cross_align: "center",
                    width: "100%",
                    height: "100%",
                    Outlet::<DocumentRoute> {  }
                }
            }
        }
    )
}

static ARIMO_NERD_FONT: &[u8] = include_bytes!("../assets/fonts/ArimoNerdFont-Regular.ttf");


static EN_US: &str = include_str!("../assets/i18n/en-US.json");
static ES_ES: &str = include_str!("../assets/i18n/es-ES.json");

fn main() {

    // run with environment variable `RUST_LOG=info,dioxus_core::virtual_dom=warn` to suppress the spammy dioxus_core info messages but keep the main logging.
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
    
    info!("Started");
    
    console_error_panic_hook::set_once();
    
    launch_cfg(
        app,
        LaunchConfig::<()>::new()
            .with_font("Arimo Nerd", ARIMO_NERD_FONT),
    );
}