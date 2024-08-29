use std::rc::Rc;
use dioxus::prelude::{Signal, UnboundedReceiver};
use dioxus::prelude::Writable;
use dioxus_logger::tracing::debug;
use dioxus_router::prelude::use_navigator;
use futures_util::{StreamExt, TryStreamExt};
use planner_app::{Effect, Event, NavigationOperation, Planner, ViewModel};

type Core = Rc<planner_app::Core<Effect, Planner>>;

pub struct CoreService {
    core: Core,
    view: Signal<ViewModel>,
}


impl CoreService {
    pub fn new(view: Signal<ViewModel>) -> Self {
        debug!("initializing core service");
        Self {
            core: Rc::new(planner_app::Core::new()),
            view,
        }
    }

    pub async fn run(&self, rx: &mut UnboundedReceiver<Event>) {
        let mut view = self.view;
        *view.write() = self.core.view();
        while let Some(event) = rx.next().await {
            self.update(event, &mut view);
        }
    }

    fn update(&self, event: Event, view: &mut Signal<ViewModel>) {
        debug!("event: {:?}", event);

        for effect in self.core.process_event(event) {
            process_effect(&self.core, effect, view);
        }
    }
}


fn process_effect(core: &Core, effect: Effect, view: &mut Signal<ViewModel>) {
    debug!("core::process_effect. effect: {:?}", effect);
    match effect {
        Effect::Render(_) => {
            *view.write() = core.view();
        }
        Effect::Navigator(request) => {
            let navigator = use_navigator();
            match request.operation {
                NavigationOperation::Navigate { path } => {
                    navigator.push(path);
                }
            }

            // TODO use the router to navigate
        }
    }
}