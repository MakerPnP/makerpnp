use dioxus::core_macro::rsx;
use dioxus::dioxus_core::{Element, use_hook};
use dioxus::hooks::{use_coroutine, use_signal};
use dioxus_sdk::i18n::use_i18;
use dioxus_sdk::translate;
use freya::prelude::*;
use tracing::debug;
use planner_app::ViewModel;
use crate::app_core::CoreService;
use crate::languages;

#[allow(non_snake_case)]
pub fn ToolBar() -> Element {
    let view = use_signal(ViewModel::default);

    let i18n = use_i18();

    let app_core = use_coroutine(|mut rx| {
        let svc = CoreService::new(view);
        async move { svc.run(&mut rx).await }
    });

    let on_click_create = move |_| {
        debug!("create clicked");
        app_core.send(planner_app::Event::CreateProject { 
            project_name: "test".to_string(), 
            path: Default::default() 
        });
    };

    let on_click_save = move |_| {
        debug!("save clicked");
        app_core.send(planner_app::Event::Save );
    };

    let languages_hooked = use_hook(|| {
        let languages_binding = languages::languages();
        let languages = &(*languages_binding);

        // FIXME avoid cloning
        languages.clone()
    });

    let mut change_lang = languages::use_change_language();

    rsx!(
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
        }
    )
} 
