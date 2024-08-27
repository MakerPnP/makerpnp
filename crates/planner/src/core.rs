use planner_app::{Effect, Event, Planner};

use std::sync::Arc;
use anyhow::anyhow;
use crossbeam_channel::Sender;
use tracing::debug;

pub type Core = Arc<crux_core::Core<Effect, Planner>>;

pub fn new() -> Core {
    Arc::new(crux_core::Core::new())
}

pub fn update(core: &Core, event: Event, tx: &Arc<Sender<Effect>>) -> anyhow::Result<()> {
    debug!("event: {:?}", event);

    for effect in core.process_event(event) {
        process_effect(core, effect, tx)?;
    }
    Ok(())
}

pub fn process_effect(core: &Core, effect: Effect, tx: &Arc<Sender<Effect>>) -> anyhow::Result<()> {
    debug!("effect: {:?}", effect);

    match effect {
        Effect::Render(_) => {
            tx.send(effect)
                .map_err(|e| anyhow!("{:?}", e))?;
        }
    }

    Ok(())
}
