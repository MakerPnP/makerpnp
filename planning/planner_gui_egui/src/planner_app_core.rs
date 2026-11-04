use std::sync::Arc;

use planner_app::effects::pcb_view_renderer::PcbViewRendererOperation;
use planner_app::effects::process_view_renderer::ProcessViewRendererOperation;
use planner_app::effects::project_view_renderer::ProjectViewRendererOperation;
use planner_app::{Effect, Event, PcbView, Planner, ProcessView, ProjectView};
use tracing::{error, trace};

type Core = Arc<planner_app::Core<Planner>>;

pub struct PlannerCoreService {
    core: Core,
    generation: u64,
}

#[derive(Debug, Clone)]
pub enum PlannerAction {
    SetModifiedState {
        project_modified: bool,
        pcbs_modified: bool,
        processes_modified: bool,
        /// the core's generation when this state was taken. An older state must not replace a newer one. Until commands were
        /// handled in order (`GARCH-11`), actions could arrive out of order.
        generation: u64,
    },
    ProjectView(ProjectView),
    PcbView(PcbView),
    ProcessView(ProcessView),
}

#[derive(Debug, Clone)]
pub enum PlannerError {
    CoreError((chrono::DateTime<chrono::Utc>, String)),
    Other((chrono::DateTime<chrono::Utc>, String)),
}

impl PlannerCoreService {
    pub fn new() -> Self {
        Self {
            core: Arc::new(planner_app::Core::new()),
            generation: 0,
        }
    }

    /// Incremented by every [`Self::update`].  When this changes, data previously received from the core may be stale.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Process an event that may change the core's state.
    ///
    /// The generation changes even if the event fails, since a failing event may have partially changed the state.
    #[must_use]
    pub fn update(&mut self, event: Event) -> Result<Vec<PlannerAction>, PlannerError> {
        self.generation = self.generation.wrapping_add(1);

        self.process(event)
    }

    /// Process an event that only reads the core's state, e.g. requesting a view.
    #[must_use]
    pub fn query(&mut self, event: Event) -> Result<Vec<PlannerAction>, PlannerError> {
        self.process(event)
    }

    fn process(&mut self, event: Event) -> Result<Vec<PlannerAction>, PlannerError> {
        trace!("event: {:?}", event);

        let mut actions: Vec<PlannerAction> = Vec::new();

        for effect in self.core.process_event(event) {
            let action = Self::process_effect(&self.core, effect, self.generation);
            match action {
                Ok(action) => {
                    actions.push(action);
                }
                Err(error) => return Err(error),
            }
        }

        Ok(actions)
    }

    pub fn process_effect(core: &Core, effect: Effect, generation: u64) -> Result<PlannerAction, PlannerError> {
        trace!("effect: {:?}", effect);

        match effect {
            Effect::Render(_) => {
                let mut view = core.view();
                let task = match view.error.take() {
                    Some(error) => {
                        error!("core error: {:?}", error);
                        Err(PlannerError::CoreError(error))
                    }
                    None => Ok(PlannerAction::SetModifiedState {
                        project_modified: view.project_modified,
                        pcbs_modified: view.pcbs_modified,
                        processes_modified: view.processes_modified,
                        generation,
                    }),
                };

                task
            }
            Effect::ProjectView(request) => {
                let ProjectViewRendererOperation::View {
                    view,
                } = request.operation;

                Ok(PlannerAction::ProjectView(view))
            }
            Effect::PcbView(request) => {
                let PcbViewRendererOperation::View {
                    view,
                } = request.operation;

                Ok(PlannerAction::PcbView(view))
            }
            Effect::ProcessView(request) => {
                let ProcessViewRendererOperation::View {
                    view,
                } = request.operation;

                Ok(PlannerAction::ProcessView(view))
            }
        }
    }
}
