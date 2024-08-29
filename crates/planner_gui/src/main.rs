#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use freya::prelude::*;
use dioxus_logger::tracing::{debug, Level};
use freya::dioxus_core::AttributeValue;
use planner_app::ViewModel;
use crate::app_core::CoreService;

use dioxus_router::prelude::{Outlet, Routable, Router, use_navigator};

mod app_core;

fn app() -> Element {
    rsx!(Router::<Route> {})
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


    rsx!(
        NativeRouter {
            Sidebar {
                sidebar: rsx!(
                    SidebarItem {
                        onclick: on_click_create,
                        label {
                            "Create"
                        }
                    },
                    SidebarItem {
                        onclick: on_click_save,
                        label {
                            "Save"
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
                    
#[cfg(cruft)]
//#[component]
fn app() -> Element {

    let view = use_signal(ViewModel::default);

    let core = use_coroutine(|mut rx| {
        let svc = CoreService::new(view);
        async move { svc.run(&mut rx).await }
    });

    let onclick = move |_| {
        //core.send(planner_app::Event::CreateProject { project_name: "test".to_string(), path: Default::default() } );
        core.send(planner_app::Event::Save );
    };

    let show_error = move || {
        let view_model = view.read();

        let message = if let Some(error) = &view_model.error {
            format!("{:?}", error)
        } else {
            "No error".to_string()
        };

        message
    };

    rsx!(
        Sidebar {
            
        }
        label {
            onclick,
            //"Create"
            "{show_error()}"
        }
    )
}

fn main() {
    dioxus_logger::init(Level::DEBUG).expect("failed to init logger");
    console_error_panic_hook::set_once();

    launch(app); // Be aware that this will block the thread
}