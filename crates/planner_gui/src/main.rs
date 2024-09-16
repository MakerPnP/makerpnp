/// Run as follows:
/// `cargo run --package planner_gui --bin planner_gui`
///
/// To enable logging, set the environment variable appropriately, for example:
/// `RUST_LOG=debug,selectors::matching=info`
use std::path::PathBuf;
use tracing::{info, trace};
use tracing_subscriber::{EnvFilter, fmt};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use vizia::prelude::*;
use crate::app_core::CoreService;
use document::Document;
use crate::language::LanguagePair;
use route::Route;
use tabs::TabKind;
use tabs::document::DocumentTab;
use tabs::home::HomeTab;
use crate::app_data_derived_lenses::popup_window;
use crate::popup_window_state_derived_lenses::kind;
use crate::project::Project;
use crate::tabbed_document_container::{TabbedDocumentContainer, TabbedDocumentEvent};
use crate::tabs::project::ProjectTab;

mod tabs;
mod app_core;
mod document;
mod project;
mod route;
mod language;

mod tabbed_document_container;


enum ApplicationEvent {
    ChangeLanguage { index: usize },
    OpenProject { path: PathBuf },
    CreateProject { name: String, path: PathBuf },
    PopupClosed,
    ShowCreateProject,
}

enum InternalEvent {
    DocumentContainerCreated {}
}

#[derive(Clone, Debug, Data)]
pub enum PopupWindow {
    NewProject (NewProjectPopup)
}

#[derive(Clone, Debug, Default, Data, Lens)]
pub struct PopupWindowState {
    enabled: bool,
    kind: Option<PopupWindow>,
}

enum NewProjectPopupEvent {
    SetName { text: String }
}

#[derive(Clone, Data, Default, Debug)]
struct NewProjectPopup {
    name: String,
    path: PathBuf,
}

impl NewProjectPopup {
    pub fn on_close(self, ecx: &mut EventContext) {
        ecx.emit(ApplicationEvent::CreateProject { name: self.name, path: self.path });
    }

    pub fn on_event(&mut self, ecx: &mut EventContext, event: &mut Event) {
        event.map(|event, _| match event {
            NewProjectPopupEvent::SetName { text } => self.name = text.clone(),
        });
    }
    
    pub fn build<'a>(&self, cx: &'a mut Context, lens: Then<Wrapper<popup_window>, Wrapper<kind>>) -> Handle<'a, Window> {
        Window::popup(cx, true, |cx| {
            VStack::new(cx, |cx: &mut Context| {
                let name_lens = lens.map(|foo| { 
                    match &foo {
                        Some(PopupWindow::NewProject(bar)) => {
                            bar.name.clone()
                        },
                        _ => unreachable!()
                    }
                });
                
                Textbox::new(cx, name_lens)
                    .width(Pixels(300.0))
                    .placeholder("TODO")
                    .on_edit(|cx, text| cx.emit(NewProjectPopupEvent::SetName { text }));

            })
                .child_space(Pixels(20.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .row_between(Pixels(12.0));
        })
            .on_close(|cx| {
                cx.emit(ApplicationEvent::PopupClosed);
            })
            .title("Set color...")
            .inner_size((400, 200))
            //.position((500, 100))
    }
}

impl PopupWindow {
    pub fn build<'a>(&self, cx: &'a mut Context, lens: Then<Wrapper<popup_window>, Wrapper<kind>>) -> Handle<'a, Window> {
        match self {
            PopupWindow::NewProject(popup) => popup.build(cx, lens),
        }
    }

    pub fn on_close(self, ecx: &mut EventContext) {
        match self {
            PopupWindow::NewProject(popup) => popup.on_close(ecx),
        }
    }

    pub fn on_event(&mut self, ecx: &mut EventContext, event: &mut Event) {
        match self {
            PopupWindow::NewProject(popup) => popup.on_event(ecx, event),
        }
    }
}


#[derive(Lens)]
pub struct AppData {
    core: CoreService,
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
        event.map(|event, meta| { match event {
            ApplicationEvent::OpenProject { path } => {
                info!("OpenProject");

                let id = format!("{:?}", path);

                let tab = TabKind::Project(ProjectTab {
                    project: Project { id, name: "TODO".to_string() },
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
                //let popup = PopupWindow::NewProject(NewProjectPopup::default());
                let popup = PopupWindow::NewProject(NewProjectPopup { name: "Test".to_string(), path: Default::default() });
                self.popup_window.kind.replace(popup);
                self.popup_window.enabled = true;
            },
            ApplicationEvent::CreateProject { name, path } => {
                self.core.update(planner_app::Event::CreateProject {
                    project_name: name.to_string(),
                    path: path.clone(),
                }, ecx)
            }
            ApplicationEvent::PopupClosed {} => {
                self.popup_window.enabled = false;
                let popup = self.popup_window.kind.take().unwrap();
                info!("popup closed, popup: {:?}", popup);
                popup.on_close(ecx);
            },
        }});
        event.map(|event, meta| { match event {
            InternalEvent::DocumentContainerCreated {} => {
                self.tab_container_entity.replace(meta.origin.clone());
            }
        }});
        
        if let Some(popup) = self.popup_window.kind.as_mut() {
            popup.on_event(ecx, event);
        }
    }
}

fn main() -> Result<(), ApplicationError> {

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    info!("Started");

    let core = CoreService::new();

    Application::new(|cx| {

        let languages: Vec<LanguagePair> = vec![
            LanguagePair { code: "en-US".to_string(), name: "English (United-States)".to_string() },
            LanguagePair { code: "es-ES".to_string(), name: "Español (España)".to_string() },
        ];

        let selected_language_index = 0;

        language::load_languages(languages.as_slice(), cx);

        let mut app_data = AppData {
            core,
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
    Binding::new(cx, AppData::popup_window.map(move |popup_state|{
        popup_state.kind.clone()
    }), |cx, popup| {
        if let Some(popup) = popup.get(cx) {
            popup.build(cx, AppData::popup_window.then(PopupWindowState::kind));
        }
    });
}

pub fn create_tabs() -> Vec<TabKind>{
    vec![
        TabKind::Home( HomeTab {
            route: Route(None)
        } ),
        TabKind::Document( DocumentTab {
            document: Document { id: "document_1".to_string(), name: "Document 1".to_string() },
            route: Route(None),
        }),
        TabKind::Project( ProjectTab {
            project: Project { id: "project_1".to_string(), name: "Project 1".to_string() },
            route: Route(None),
        }),
    ]
}

