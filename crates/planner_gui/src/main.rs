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
use crate::project::Project;
use crate::tabbed_document_container::TabbedDocumentContainer;
use crate::tabs::project::ProjectTab;

mod tabs;
mod app_core;
mod document;
mod project;
mod route;
mod language;

mod tabbed_document_container;

enum ProjectEvent {
    Create {},
}


enum ApplicationEvent {
    ChangeLanguage { index: usize }
}

#[derive(Lens)]
pub struct AppData {
    core: CoreService,
    languages: Vec<LanguagePair>,
    selected_language_index: usize
}

impl AppData {
}

impl Model for AppData {

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|event, meta| {
            match event {
                ProjectEvent::Create {} => {
                    self.core.update(planner_app::Event::CreateProject {
                        project_name: "test".to_string(),
                        path: Default::default(),
                    })
                }
            }
        });
        event.map(|event, meta| {
            match event {
                ApplicationEvent::ChangeLanguage { index } => {
                    let language_pair: &LanguagePair = self.languages.get(*index).as_ref().unwrap();
                    info!("change language. index: {}, name: {}, code: {}", index, language_pair.name, language_pair.code);
                    self.selected_language_index = *index;

                    cx.emit(EnvironmentEvent::SetLocale(language_pair.code.parse().unwrap()));
                }
            }
        });
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

        let app_data = AppData {
            core,
            languages,
            selected_language_index,
        };
        app_data.build(cx);

        VStack::new(cx, |cx|{

            //
            // Toolbar
            //

            HStack::new(cx, |cx| {

                Button::new(cx, |cx| Label::new(cx, Localized::new("action-project-create")))
                    .on_press(|ecx|{
                        ecx.emit(ProjectEvent::Create {})
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
            TabbedDocumentContainer::new(cx, create_tabs())
                .width(Percentage(100.0))
                .height(Stretch(1.0));

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

    })
        .title("Planner")
        .run()
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

