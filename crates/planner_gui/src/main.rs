use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use vizia::prelude::*;
use crate::app_core::CoreService;
use crate::language::LanguagePair;

mod app_core;
mod language {
    use vizia::prelude::*;

    #[derive(Clone)]
    pub struct LanguagePair {
        pub code: String,
        pub name: String,
    }

    pub fn load_languages(languages: &[LanguagePair], cx: &mut Context) {
        for pair in languages {
            match pair.code.as_str() {
                "en-US" => {
                    cx.add_translation(
                        "en-US".parse().unwrap(),
                        include_str!("../resources/translations/en-US/planner.ftl").to_owned(),
                    );
                },
                "es-ES" => {
                    cx.add_translation(
                        "es-ES".parse().unwrap(),
                        include_str!("../resources/translations/es-ES/planner.ftl").to_owned(),
                    );
                },
                _ => unreachable!()
            }
        }

    }
}

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

                    EnvironmentEvent::SetLocale(language_pair.code.parse().unwrap());
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
            .width(Stretch(1.0));
    })
        .title("Planner")
        .run()
}