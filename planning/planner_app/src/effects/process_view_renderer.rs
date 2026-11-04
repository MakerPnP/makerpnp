use std::future::Future;

use crux_core::capability::Operation;
use crux_core::command::NotificationBuilder;
use crux_core::{Command, Request};

use crate::ProcessView;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum ProcessViewRendererOperation {
    View { view: ProcessView },
}

impl Operation for ProcessViewRendererOperation {
    type Output = ();
}

pub fn view_builder<Effect, Event>(view: ProcessView) -> NotificationBuilder<Effect, Event, impl Future<Output = ()>>
where
    Effect: From<Request<ProcessViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    Command::notify_shell(ProcessViewRendererOperation::View {
        view,
    })
}

pub fn view<Effect, Event>(view: ProcessView) -> Command<Effect, Event>
where
    Effect: From<Request<ProcessViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    view_builder(view).into()
}
