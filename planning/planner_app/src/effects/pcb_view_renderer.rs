use std::future::Future;

use crux_core::capability::Operation;
use crux_core::command::NotificationBuilder;
use crux_core::{Command, Request};

use crate::PcbView;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum PcbViewRendererOperation {
    View { view: PcbView },
}

impl Operation for PcbViewRendererOperation {
    type Output = ();
}

pub fn view_builder<Effect, Event>(view: PcbView) -> NotificationBuilder<Effect, Event, impl Future<Output = ()>>
where
    Effect: From<Request<PcbViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    Command::notify_shell(PcbViewRendererOperation::View {
        view,
    })
}

pub fn view<Effect, Event>(view: PcbView) -> Command<Effect, Event>
where
    Effect: From<Request<PcbViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    view_builder(view).into()
}
