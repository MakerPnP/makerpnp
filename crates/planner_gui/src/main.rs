#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tracing::info;
use freya::prelude::*;
use dioxus_router::prelude::{Outlet, Routable, Router, use_route};
use dioxus_router::routable::ToRouteSegments;
use tracing_subscriber::{EnvFilter, fmt};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use planner_app::ProjectOperationViewModel;

use crate::toolbar::ToolBar;

mod app_core;
mod toolbar;
mod document_tab;
mod languages;

fn app() -> Element {

    languages::initialise();

    languages::change(&languages::selected());

    rsx!(
        rect {
           font_family: "Arimo Nerd",
            Router::<TabRoute> {}
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
    
    // currently there's compile errors in freya when attempting to use route: Vec<String>
    #[route("/fixme")]
    TabFixme,
    // #[route("/:..route")]
    // TabRouteNotFound { route: Vec<String>  },
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
fn TabRouteNotFound(_route: Vec<String>) -> Element {
    rsx!(
        label {
            "Tab not found"
        }
    )
}

#[allow(non_snake_case)]
fn TabFixme() -> Element {
    rsx!(
        label {
            "FIXME"
        }
    )
}


#[allow(non_snake_case)]
fn AppTabsBar() -> Element {
    rsx!(
        NativeRouter {
            ToolBar {},

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
        DocumentOverview,
    #[end_layout]
    
    // currently there's compile errors in freya when attempting to use route: Vec<String>
    #[route("/Fixme")]
    DocumentFixme,
    // #[route("/:..route")]
    // DocumentRouteNotFound { route: Vec<String>  },
}

#[allow(non_snake_case)]
//#[component]
fn DocumentOverview() -> Element {
    let view = use_signal(ProjectOperationViewModel::default);

    rsx!(
        label {
            { format!("Name: {}", view.read().project.as_ref().unwrap().name) }
        }
    )
}


#[allow(non_snake_case)]
//#[component]
fn DocumentRouteNotFound(_route: Vec<String>) -> Element {
    rsx!(
        label {
            "Not Found"
        }
    )
}

#[allow(non_snake_case)]
fn DocumentFixme() -> Element {
    rsx!(
        label {
            "FIXME"
        }
    )
}


#[allow(non_snake_case)]
fn DocumentRouteLayout() -> Element {

    rsx!(
        Router::<DocumentRoute> {}
        NativeRouter {
            DocumentLayout {}
        }
    )
}

#[allow(non_snake_case)]
fn DocumentLayout() -> Element {
    // TODO get the current document?
    // FIXME broken, causes panic
    // let document_path: DocumentRoute = use_route();
    // info!("document_path: {}", document_path);
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