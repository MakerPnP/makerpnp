use std::collections::BTreeMap;
use std::error::Error;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::{anyhow, bail};
use clap::{Parser, Subcommand, ArgGroup};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use crossbeam_channel::unbounded;
use regex::Regex;
use tracing::{info, trace};
use {cli, planning};
use cli::args::{PcbKindArg, PcbSideArg, PlacementOperationArg, ProcessOperationArg, ProcessOperationSetArg};
use planner_app::{Effect, Event};
use planning::design::{DesignName, DesignVariant};
use planning::reference::Reference;
use planning::placement::PlacementSortingItem;
use planning::process::ProcessName;
use planning::project::{PartStateError, ProcessFactory, Project};
use planning::project;
use planning::phase::PhaseError;
use planning::variant::VariantName;
use pnp::load_out::LoadOutItem;
use pnp::object_path::ObjectPath;
use stores::load_out::LoadOutSource;
use crate::core::Core;
use crate::opts::{Command, EventError, Opts};

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
        Err(EventError::UnknownEvent { opts }) => {
            let project_name = &opts.project.unwrap();
            let project_file_path = project::build_project_file_path(&project_name, &opts.path);

            match opts.command {
                Command::GenerateArtifacts { } => {
                    let mut project = project::load(&project_file_path)?;

                    let modified = project::update_phase_operation_states(&mut project);

                    let phase_load_out_item_map = project.phases.iter().try_fold(BTreeMap::<Reference, Vec<LoadOutItem>>::new(), |mut map, (reference, phase) | {
                        let load_out_items = stores::load_out::load_items(&LoadOutSource::from_str(&phase.load_out_source).unwrap())?;
                        map.insert(reference.clone(), load_out_items);
                        Ok::<BTreeMap<Reference, Vec<LoadOutItem>>, anyhow::Error>(map)
                    })?;

                    project::generate_artifacts(&project, &opts.path, &project_name, phase_load_out_item_map)?;

                    if modified {
                        project::save(&project, &project_file_path)?;
                    }
                },
                Command::RecordPhaseOperation { phase: reference, operation, set } => {
                    let mut project = project::load(&project_file_path)?;

                    let modified = project::update_phase_operation(&mut project, &opts.path, &reference, operation.into(), set.into())?;

                    if modified {
                        project::save(&project, &project_file_path)?;
                    }
                },
                Command::RecordPlacementsOperation { object_path_patterns, operation } => {
                    let mut project = project::load(&project_file_path)?;

                    let modified = project::update_placements_operation(&mut project, &opts.path, object_path_patterns, operation.into())?;

                    if modified {
                        project::save(&project, &project_file_path)?;
                    }
                },
                Command::AssignFeederToLoadOutItem { phase: reference, feeder_reference, manufacturer, mpn } => {
                    let project = project::load(&project_file_path)?;

                    let phase = project.phases.get(&reference)
                        .ok_or(PhaseError::UnknownPhase(reference))?.clone();

                    let process = project.find_process(&phase.process)?.clone();

                    stores::load_out::assign_feeder_to_load_out_item(&phase, &process, &feeder_reference, manufacturer, mpn)?;
                },
                Command::ResetOperations { } => {
                    let mut project = project::load(&project_file_path)?;

                    project::reset_operations(&mut project)?;

                    project::save(&project, &project_file_path)?;
                },
                _ => unreachable!(),
            }
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
