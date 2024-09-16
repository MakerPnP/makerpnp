use std::path::PathBuf;
use std::rc::Rc;
use tracing::{debug};
use vizia::context::EventContext;
use vizia::prelude::EmitContext;
use planner_app::{Effect, Event, NavigationOperation, Planner};
use planner_app::view_renderer::ViewRendererOperation;
use crate::ApplicationEvent;

type Core = Rc<planner_app::Core<Effect, Planner>>;

pub struct CoreService {
    core: Core,
}

impl CoreService {
    pub fn new() -> Self {
        debug!("initializing core service");
        Self {
            core: Rc::new(planner_app::Core::new()),
        }
    }

    pub fn update(&self, event: Event, ecx: &mut EventContext) {
        debug!("event: {:?}", event);

        for effect in self.core.process_event(event) {
            process_effect(&self.core, effect, ecx);
        }
    }
}

fn process_effect(_core: &Core, effect: Effect, ecx: &mut EventContext) {
    debug!("core::process_effect. effect: {:?}", effect);
    match effect {
        ref render @ Effect::Render(ref request) => {
            // TODO
        }
        Effect::Navigator(request) => {
            match request.operation {
                NavigationOperation::Navigate { path } => {
                    // TODO use the path
                    let path = PathBuf::from("project-test.mpnp.json");
                    ecx.emit(ApplicationEvent::OpenProject { path })
                }
            }
        }
        
        Effect::ViewRenderer(request) => {
            let ViewRendererOperation::View { reference, view} = request.operation;
            // TODO find the tab with the reference

            // TODO ask the tab to display the view?
        }
    }
}