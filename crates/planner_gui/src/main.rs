use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use vizia::prelude::*;
use crate::app_core::CoreService;

mod app_core;


enum ProjectEvent {
    Create {},
}

#[derive(Lens)]
pub struct AppData {
    core: CoreService,
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
        })
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
        let app_data = AppData {
            core
        };
        app_data.build(cx);
        
        Button::new(cx, |cx| Label::new(cx, "Create project"))
            .on_press(|ecx|{
                ecx.emit(ProjectEvent::Create {})
            });
        
    })
        .title("Planner")
        .run()
}