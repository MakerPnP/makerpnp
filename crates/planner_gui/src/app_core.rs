use std::rc::Rc;
use tracing::{error, info, debug};
use planner_app::{Effect, Event, NavigationOperation, Planner, ProjectOperationViewModel};

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

    pub fn update(&self, event: Event) {
        debug!("event: {:?}", event);

        for effect in self.core.process_event(event) {
            process_effect(&self.core, effect);
        }
    }
}

fn process_effect(core: &Core, effect: Effect) {
    debug!("core::process_effect. effect: {:?}", effect);
    match &effect {
        render @ Effect::Render(request) => {
            // TODO
            
        }
        Effect::Navigator(request) => {
            // TODO
        }
    }
}