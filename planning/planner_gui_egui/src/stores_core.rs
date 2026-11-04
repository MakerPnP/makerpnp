//! The stores core, which has the data sources, such as the user's parts, and the stores service, which does the store
//! operations the core asks for (`STORE-7`).
//!
//! The application has one of each, shared by every tab, so that a data source used by more than one tab is loaded once,
//! and so that each tab sees the same data (`PARTSRC-5`).
//!
//! The stores service is async, so its operations are done on a runtime of their own, and the GUI doesn't wait for them.
//! Each result is sent back as a command, [`UiCommand::StoresReply`], which is handled, in order with the others, on the
//! thread that draws the frames (`GARCH-11`), where it's given to the core.

use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

use stores_app::{
    Core, Effect, Event, PartsSource, PartsSourceStatus, PartsView, PartsViewRendererOperation, Stores,
    StoresOperation, StoresOutput,
};
use stores_service::StoresService;
use tracing::error;
use tracing::instrument::WithSubscriber;

use crate::sync::Enqueue;
use crate::ui_commands::UiCommand;

pub struct StoresCoreService {
    core: Core<Stores>,
    service: Arc<StoresService>,
    runtime: tokio::runtime::Runtime,
    replies: Replies,

    /// The latest view of each data source's parts, from the core, shared by every tab that shows them, so that the
    /// tabs don't each keep a copy, and so that each tab shows a data source's parts as soon as they change.
    parts_views: BTreeMap<PartsSource, Arc<PartsView>>,
}

/// Where the results of the store operations are sent, and how the GUI is woken to handle them.
#[derive(Clone)]
struct Replies {
    sender: Enqueue<UiCommand>,
    ui_context: egui::Context,
}

impl Debug for StoresCoreService {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoresCoreService")
            .finish()
    }
}

/// The result of a store operation, for the core that asked for it.
///
/// A command, so cloned, though only the first clone handled gives the result to the core.
#[derive(Clone)]
pub struct StoresReply {
    operation: StoresOperation,
    output: StoresOutput,
    request: Arc<Mutex<Option<crux_core::Request<StoresOperation>>>>,
}

/// Says what the operation and its output were, as the command is logged.
impl Debug for StoresReply {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoresReply")
            .field("operation", &self.operation)
            .field("output", &self.output)
            .finish()
    }
}

impl StoresCoreService {
    pub fn new(sender: Enqueue<UiCommand>, ui_context: egui::Context) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("stores")
            .build()
            .expect("a runtime");

        Self {
            core: Core::new(),
            service: Arc::new(StoresService::new()),
            runtime,
            replies: Replies {
                sender,
                ui_context,
            },
            parts_views: Default::default(),
        }
    }

    /// Processes an event, keeping the views the core sends, so that every tab showing their data source shows them,
    /// and having the stores service do the operations the core asks for.
    pub fn process(&mut self, event: Event) {
        let effects = self.core.process_event(event);
        self.handle_effects(effects);
    }

    /// Gives the core the result of an operation it asked for.
    pub fn resolve(&mut self, reply: StoresReply) {
        let Some(mut request) = reply.request.lock().unwrap().take() else {
            return;
        };
        match self
            .core
            .resolve(&mut request, reply.output)
        {
            Ok(effects) => self.handle_effects(effects),
            Err(cause) => error!("Unable to give the stores core a result. cause: {:?}", cause),
        }
    }

    fn handle_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                // what the tabs show is read from the core's view model as each frame is drawn.
                Effect::Render(_) => {}
                Effect::PartsView(request) => {
                    let PartsViewRendererOperation::View {
                        view,
                    } = request.operation;
                    self.parts_views
                        .insert(view.source.clone(), Arc::new(view));
                }
                Effect::Stores(request) => self.perform(request),
            }
        }
    }

    fn perform(&self, request: crux_core::Request<StoresOperation>) {
        let service = self.service.clone();
        let replies = self.replies.clone();
        let operation = request.operation.clone();
        self.runtime.spawn(
            async move {
                let output = service.perform(operation.clone()).await;
                let reply = StoresReply {
                    operation,
                    output,
                    request: Arc::new(Mutex::new(Some(request))),
                };
                // the GUI may be closing, in which case there's nothing to give the result to.
                let _ = replies
                    .sender
                    .send(UiCommand::StoresReply(reply));
                replies.ui_context.request_repaint();
            }
            // logged where the core asked for it, for example to a test's logs.
            .with_current_subscriber(),
        );
    }

    /// What the core knows of the data source, by the data source as it was asked for, or as the stores service
    /// identified it.
    pub fn parts_source_status(&self, source: &PartsSource) -> Option<PartsSourceStatus> {
        self.core
            .view()
            .parts_sources
            .remove(source)
    }

    /// The latest view of the data source's parts, if they're loaded, by the data source as the stores service
    /// identified it (`STORE-6`).
    pub fn parts_view(&self, source: &PartsSource) -> Option<Arc<PartsView>> {
        self.parts_views.get(source).cloned()
    }
}
