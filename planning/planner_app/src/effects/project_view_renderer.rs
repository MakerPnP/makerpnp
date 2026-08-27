use std::future::Future;

use crux_core::capability::Operation;
use crux_core::command::NotificationBuilder;
use crux_core::{Command, Request};

use crate::ProjectView;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum ProjectViewRendererOperation {
    View { view: ProjectView },
}

impl Operation for ProjectViewRendererOperation {
    type Output = ();
}

pub fn view_builder<Effect, Event>(view: ProjectView) -> NotificationBuilder<Effect, Event, impl Future<Output = ()>>
where
    Effect: From<Request<ProjectViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    Command::notify_shell(ProjectViewRendererOperation::View {
        view,
    })
}

pub fn view<Effect, Event>(view: ProjectView) -> Command<Effect, Event>
where
    Effect: From<Request<ProjectViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    view_builder(view).into()
}
