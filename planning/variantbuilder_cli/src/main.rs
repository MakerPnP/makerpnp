use std::sync::Arc;

use anyhow::{anyhow, bail};
use clap::Parser;
use cli;
use crossbeam_channel::unbounded;
use tracing::trace;
use variantbuilder_app::{Effect, Event};

use crate::core::Core;
use crate::opts::Opts;

mod core;
mod opts;

fn main() -> anyhow::Result<()> {
    let args = argfile::expand_args(argfile::parse_fromfile, argfile::PREFIX).unwrap();

    let opts = Opts::parse_from(args);

    cli::tracing::configure_tracing(opts.trace.clone(), opts.verbose.clone())?;

    let event = Event::try_from(opts).map_err(|error| anyhow!("{}", error))?;

    let core = core::new();
    run_loop(&core, event)?;

    Ok(())
}

fn run_loop(core: &Core, event: Event) -> Result<(), anyhow::Error> {
    let (tx, rx) = unbounded::<Effect>();

    core::update(&core, event, &Arc::new(tx))?;

    while let Ok(effect) = rx.recv() {
        trace!("run_loop. effect: {:?}", effect);
        match effect {
            _render @ Effect::Render(_) => {
                let view = core.view();

                if let Some(error) = view.error {
                    bail!(error)
                }
            }
            // done by `core::process_effect`, before the effect would be sent here.
            Effect::Stores(_) => {}
        }
    }
    Ok(())
}
