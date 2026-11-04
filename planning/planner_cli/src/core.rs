use std::sync::{Arc, OnceLock};

use anyhow::anyhow;
use crossbeam_channel::Sender;
use futures::executor::block_on;
use planner_app::{Effect, Event, Planner};
use stores_service::StoresService;
use tracing::trace;

pub type Core = Arc<crux_core::Core<Planner>>;

pub fn new() -> Core {
    Arc::new(crux_core::Core::new())
}

/// The process's stores service, shared by its cores (`STORE-7`).
pub fn stores_service() -> &'static StoresService {
    static SERVICE: OnceLock<StoresService> = OnceLock::new();
    SERVICE.get_or_init(StoresService::new)
}

pub fn update(core: &Core, event: Event, tx: &Arc<Sender<Effect>>) -> anyhow::Result<()> {
    for effect in core.process_event(event) {
        process_effect(core, effect, tx)?;
    }
    Ok(())
}

pub fn process_effect(core: &Core, effect: Effect, tx: &Arc<Sender<Effect>>) -> anyhow::Result<()> {
    trace!("effect: {:?}", effect);

    match effect {
        // `STORE-7`, the CLI does one thing at a time, so it waits for each store operation, and gives the core its result.
        Effect::Stores(mut request) => {
            let output = block_on(stores_service().perform_once(request.operation.clone()))
                .ok_or_else(|| anyhow!("No result from the stores service. operation: {:?}", request.operation))?;
            for effect in core.resolve(&mut request, output)? {
                process_effect(core, effect, tx)?;
            }
        }
        effect => tx
            .send(effect)
            .map_err(|e| anyhow!("{:?}", e))?,
    }

    Ok(())
}
