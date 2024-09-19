use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{debug};
use vizia::context::EventContext;
use vizia::events::Propagation;
use vizia::prelude::EmitContext;
use vizia::view;
use planner_app::{Effect, Event, NavigationOperation, Planner, ProjectView};
use planner_app::view_renderer::ViewRendererOperation;
use crate::ApplicationEvent;

type Core = Arc<planner_app::Core<Effect, Planner>>;

pub struct CoreService {
    core: Core,
}

impl CoreService {
    pub fn new() -> Self {
        debug!("initializing core service");
        Self {
            core: Arc::new(planner_app::Core::new()),
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
                    ecx.emit(CoreEvent::Navigate { path })
                }
            }
        }
        
        Effect::ViewRenderer(request) => {
            let ViewRendererOperation::View { view} = request.operation;
            ecx.emit(CoreEvent::RenderView { view })
        }
    }
}

#[derive(Debug)]
pub enum CoreEvent {
    RenderView { view: ProjectView },
    Navigate { path: String },
}