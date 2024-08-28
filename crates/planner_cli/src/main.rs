use std::sync::Arc;
use anyhow::bail;
use clap::Parser;
use planner_app::{Effect, Event};
use crossbeam_channel::unbounded;

use crate::core::Core;
use crate::opts::{EventError, Opts};

mod core;
mod opts;

fn main() -> anyhow::Result<()>{
    let args = argfile::expand_args(
        argfile::parse_fromfile,
        argfile::PREFIX,
    ).unwrap();

    let opts = Opts::parse_from(args);

    cli::tracing::configure_tracing(opts.trace.clone(), opts.verbose.clone())?;

    let project_name = opts.project.clone().unwrap();
    let path = opts.path.clone();

    let event: Result<Event, _> = Event::try_from(opts);
    
    match event {
        Ok(event) => {
            let core = core::new();

            let should_load_first = match event {
                Event::CreateProject { .. } => false,
                _ => true,
            };
            if should_load_first {
                run_loop(&core, Event::Load { project_name, path })?;
            }
            
            run_loop(&core, event)?;
        },
        // clap configuration prevents this
        Err(EventError::MissingProjectName) => unreachable!(),
    }
    
    Ok(())
}

fn run_loop(core: &Core, event: Event) -> Result<(), anyhow::Error> {
    let (tx, rx) = unbounded::<Effect>();

    core::update(&core, event, &Arc::new(tx))?;

    while let Ok(effect) = rx.recv() {
        match effect {
            _render @ Effect::Render(_) => {
                let view = core.view();
                if let Some(error) = view.error {
                    bail!(error)
                }
            },
        }
    }
    Ok(())
}
