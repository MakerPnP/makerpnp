//! The Variant Builder's core, which creates, opens, configures and builds variant builders (`VARB`), shared by every
//! variant builder tab, so that a variant builder shown by more than one tab is shown the same by each.
//!
//! The store operations it asks for are done by the process's one stores service, on a runtime of their own, so the GUI
//! doesn't wait for them. Each result is sent back as a command, [`UiCommand::VariantBuilderReply`], which is handled,
//! in order with the others, on the thread that draws the frames (`GARCH-11`), where it's given to the core.

use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use stores_app::{StoresOperation, StoresOutput};
use tracing::error;
use tracing::instrument::WithSubscriber;
use variantbuilder_app::{Core, Effect, Event, OperationViewModel, VariantBuilder};

use crate::stores_core::stores_service;
use crate::sync::Enqueue;
use crate::ui_commands::UiCommand;

pub struct VariantBuilderCoreService {
    core: Core<VariantBuilder>,
    runtime: tokio::runtime::Runtime,
    sender: Enqueue<UiCommand>,
    ui_context: egui::Context,
}

impl Debug for VariantBuilderCoreService {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VariantBuilderCoreService")
            .finish()
    }
}

/// A result of a store operation, for the core that asked for it.
#[derive(Clone)]
pub struct VariantBuilderReply {
    operation: StoresOperation,
    output: StoresOutput,
    request: Arc<Mutex<crux_core::Request<StoresOperation>>>,
}

/// Says what the operation and its output were, as the command is logged.
impl Debug for VariantBuilderReply {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VariantBuilderReply")
            .field("operation", &self.operation)
            .field("output", &self.output)
            .finish()
    }
}

impl VariantBuilderCoreService {
    pub fn new(sender: Enqueue<UiCommand>, ui_context: egui::Context) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("variant-builder")
            .build()
            .expect("a runtime");

        Self {
            core: Core::new(),
            runtime,
            sender,
            ui_context,
        }
    }

    /// Processes an event, having the stores service do the operations the core asks for.
    pub fn process(&mut self, event: Event) {
        let effects = self.core.process_event(event);
        self.handle_effects(effects);
    }

    /// Gives the core a result of an operation it asked for.
    pub fn resolve(&mut self, reply: VariantBuilderReply) {
        let result = {
            let mut request = reply.request.lock().unwrap();
            self.core
                .resolve(&mut *request, reply.output)
        };
        match result {
            Ok(effects) => self.handle_effects(effects),
            Err(cause) => error!("Unable to give the variant builder core a result. cause: {:?}", cause),
        }
    }

    /// What the core knows of the variant builders, read as each frame is drawn.
    pub fn view(&self) -> OperationViewModel {
        self.core.view()
    }

    fn handle_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                // what the tabs show is read from the core's view model as each frame is drawn.
                Effect::Render(_) => {}
                Effect::Stores(request) => self.perform(request),
            }
        }
    }

    /// Has the stores service do the operation, and sends its result back as a command.
    fn perform(&self, request: crux_core::Request<StoresOperation>) {
        let sender = self.sender.clone();
        let ui_context = self.ui_context.clone();
        let operation = request.operation.clone();
        let mut outputs = stores_service().perform(operation.clone());
        let request = Arc::new(Mutex::new(request));
        self.runtime.spawn(
            async move {
                while let Some(output) = outputs.next().await {
                    let reply = VariantBuilderReply {
                        operation: operation.clone(),
                        output,
                        request: request.clone(),
                    };
                    // the GUI may be closing, in which case there's nothing to give the result to.
                    if sender
                        .send(UiCommand::VariantBuilderReply(reply))
                        .is_err()
                    {
                        return;
                    }
                    ui_context.request_repaint();
                }
            }
            // logged where the core asked for it, for example to a test's logs.
            .with_current_subscriber(),
        );
    }
}
