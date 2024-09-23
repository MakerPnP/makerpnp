use crux_core::capability::{CapabilityContext, Operation};
use crux_core::macros::Capability;
use thiserror::Error;
use crate::ProjectView;

#[derive(Capability)]
pub struct ViewRenderer<Ev> {
    context: CapabilityContext<ViewRendererOperation, Ev>,
}

impl<Ev> ViewRenderer<Ev> {
    pub fn new(context: CapabilityContext<ViewRendererOperation, Ev>) -> Self {
        Self {
            context,
        }
    }
}
impl<Ev: 'static> ViewRenderer<Ev> {

    pub fn view<F>(&self, view: ProjectView, make_event: F)
    where
        F: FnOnce(Result<(), ViewRendererError>) -> Ev + Send + Sync + 'static,
    {
        self.context.spawn({
            let context = self.context.clone();
            async move {
                let response = run_view(&context, view).await;
                context.update_app(make_event(response))
            }
        });
    }
}


async fn run_view<Ev: 'static>(
    context: &CapabilityContext<ViewRendererOperation, Ev>,
    view: ProjectView,
) -> Result<(), ViewRendererError> {
    context
        .request_from_shell(ViewRendererOperation::View { view })
        .await
        .unwrap_set()
}


#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub enum ViewRendererResult {
    Ok { response: ViewRendererResponse },
    Err { error: ViewRendererError },
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub enum ViewRendererResponse {
    Ok
}

impl ViewRendererResult {
    fn unwrap_set(self) -> Result<(), ViewRendererError> {
        match self {
            ViewRendererResult::Ok { response } => match response {
                ViewRendererResponse::Ok => Ok(()),
            },
            ViewRendererResult::Err { error } => Err(error.clone()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub enum ViewRendererOperation {
    View { 
        view: ProjectView
    }
}

impl Operation for ViewRendererOperation {
    type Output = ViewRendererResult;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Error)]
#[serde(rename_all = "camelCase")]
pub enum ViewRendererError {
    #[error("other error: {message}")]
    Other { message: String },
}