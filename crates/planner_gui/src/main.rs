/// Run as follows:
/// `cargo run --package planner_gui --bin planner_gui`
///
/// To enable logging, set the environment variable appropriately, for example:
/// `RUST_LOG=debug,selectors::matching=info`
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use tracing::{info, trace};
use tracing_subscriber::{EnvFilter, fmt};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use vizia::prelude::*;
use crate::app_core::CoreService;
use regex::{Captures, Regex};
use crate::language::LanguagePair;
use route::Route;
use tabs::TabKind;
use tabs::home::HomeTab;
use crate::popups::{PopupWindow, PopupWindowState};
use crate::popups::new_project::NewProjectPopup;
use crate::project::Project;
use crate::tabbed_document_container::{TabbedDocumentContainer, TabbedDocumentEvent};
use crate::tabs::project::ProjectTab;

mod tabs;
mod app_core;
mod project;
mod route;
mod language;

mod popups;

mod tabbed_document_container;


enum ApplicationEvent {
    ChangeLanguage { index: usize },
    OpenProject { path: PathBuf },
    CreateProject { name: String, path: PathBuf },
    PopupClosed,
    ShowCreateProject,
    Navigate { path: String },
}

enum InternalEvent {
    DocumentContainerCreated {}
}


#[derive(Lens)]
pub struct AppData {
    languages: Vec<LanguagePair>,
    selected_language_index: usize,
    tab_container_entity: Option<Entity>,
    popup_window: PopupWindowState,
}

impl AppData {
}

impl Model for AppData {

    fn event(&mut self, ecx: &mut EventContext, event: &mut Event) {
        trace!("event: {:?}", &event);
        event.map(|event, _meta| { match event {
            ApplicationEvent::OpenProject { path } => {
                info!("OpenProject. path: {:?}", &path);

                let id = path.to_str().unwrap().to_string();
                let name = Localized::new("spinner-loading").to_string_local(ecx);

                let tab = TabKind::Project(ProjectTab {
                    project: Project { id, name },
                    route: Route(None),
                });

                ecx.emit_to(self.tab_container_entity.unwrap(), TabbedDocumentEvent::AddTab { tab })
            },
            ApplicationEvent::ChangeLanguage { index } => {
                let language_pair: &LanguagePair = self.languages.get(*index).as_ref().unwrap();
                info!("change language. index: {}, name: {}, code: {}", index, language_pair.name, language_pair.code);
                self.selected_language_index = *index;

                ecx.emit(EnvironmentEvent::SetLocale(language_pair.code.parse().unwrap()));
            },
            ApplicationEvent::ShowCreateProject {} => {
                let popup = PopupWindow::NewProject(NewProjectPopup { name: "Test Name".to_string(), path: ".".to_string() });
                self.popup_window.kind.replace(popup);
                self.popup_window.enabled = true;
            },
            ApplicationEvent::CreateProject { name, path } => {
                CORE_SERVICE.update(planner_app::Event::CreateProject {
                    project_name: name.to_string(),
                    directory_path: path.clone(),
                }, ecx)
            }
            ApplicationEvent::PopupClosed {} => {
                self.popup_window.enabled = false;
                let popup = self.popup_window.kind.take().unwrap();
                info!("popup closed, popup: {:?}", popup);
            },
            ApplicationEvent::Navigate { path } => {

                // TODO implement some form of router, for now this will suffice
                let pattern = Regex::new(r"/project/load/(?<path>.*)").unwrap();
                if let Some(captures) = pattern.captures(path) {
                    let encoded_path = captures.name("path").unwrap().as_str();
                    let path = String::from_utf8(BASE64_STANDARD.decode(encoded_path).unwrap()).unwrap();
                    let path = PathBuf::from(path);
                    
                    ecx.emit(ApplicationEvent::OpenProject { path })
                } else {
                    // unimplemented/bad path
                    unreachable!()
                }
            },
        }});
        event.map(|event, meta| match event {
            InternalEvent::DocumentContainerCreated {} => {
                self.tab_container_entity.replace(meta.origin.clone());
            }
        });
        
        if let Some(popup) = self.popup_window.kind.as_mut() {
            popup.on_event(ecx, event);
        }
    }
}

static CORE_SERVICE: LazyLock<CoreService> = LazyLock::new(||{
    CoreService::new()
});

fn main() -> Result<(), ApplicationError> {

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    info!("Started");

    Application::new(|cx| {

        let languages: Vec<LanguagePair> = vec![
            LanguagePair { code: "en-US".to_string(), name: "English (United-States)".to_string() },
            LanguagePair { code: "es-ES".to_string(), name: "Español (España)".to_string() },
        ];

        let selected_language_index = 0;

        language::load_languages(languages.as_slice(), cx);

        let app_data = AppData {
            languages,
            selected_language_index,
            tab_container_entity: None,
            popup_window: PopupWindowState::default(),
        };
        app_data.build(cx);

        VStack::new(cx, |cx|{

            //
            // Toolbar
            //

            HStack::new(cx, move |cx| {

                Button::new(cx, |cx| Label::new(cx, Localized::new("action-project-create")))
                    .on_press(|ecx|{
                        ecx.emit(ApplicationEvent::ShowCreateProject {})
                    })
                    .width(Stretch(2.0)); // FIXME if this is too small, content overflows

                Element::new(cx)
                    .width(Stretch(7.0));

                PickList::new(
                    cx,
                    AppData::languages.map(|languages| {
                        languages.iter().map(|language|language.name.clone()).collect::<Vec<_>>()
                    }),
                    AppData::selected_language_index,
                    true
                )
                    .on_select(|ecx, index|{
                        ecx.emit(ApplicationEvent::ChangeLanguage { index })
                    })
                    .width(Stretch(3.0)); // FIXME if this is too small, content overflows

            })
                .background_color(Color::rgb(0xee, 0xee, 0xee))
                .width(Stretch(1.0))
                .height(Pixels(32.0));

            //
            // Tab container
            //
            let _handle = TabbedDocumentContainer::new(cx, create_tabs())
                .width(Percentage(100.0))
                .height(Stretch(1.0))
                .on_build(|ecx|{
                    // we need an event here, so we can record the Entity from the handle in 'AppData', which we can't borrow or mutate in the calling method.
                    ecx.emit(InternalEvent::DocumentContainerCreated {} )
                });

            // i.e. this is not workable due to compile errors.
            //app_data.tab_container_entity.replace(_handle.entity());

            //
            // Status bar
            //
            VStack::new(cx, |cx|{
                HStack::new(cx,|cx|{
                    Label::new(cx, "");
                });
            })
                .background_color(Color::darkgray())
                .width(Stretch(1.0))
                .height(Pixels(32.0));
        })
            .height(Stretch(1.0));


        make_popup(cx);

    })
        .title("Planner")
        .run()
}

fn make_popup(cx: &mut Context) {
    Binding::new(cx, AppData::popup_window.map(|s| s.enabled), |cx, enabled| {
        if enabled.get(cx) {
            let popup = AppData::popup_window.get(cx);

            if let Some(kind) = popup.kind {
                kind.build(cx, AppData::popup_window.then(PopupWindowState::kind));
            }
        }
    });
}

pub fn create_tabs() -> Vec<TabKind>{
    vec![
        TabKind::Home( HomeTab {
            route: Route(None)
        } ),
    ]
}

