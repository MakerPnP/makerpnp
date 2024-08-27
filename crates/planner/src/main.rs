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

#[derive(Parser)]
#[command(name = "planner")]
#[command(bin_name = "planner")]
#[command(version, about, long_about = None)]
#[command(group(
    ArgGroup::new("requires_project")
        .args(&["project"])
            .required(true)
))]
struct Opts {
    #[command(subcommand)]
    command: Command,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log")]
    trace: Option<PathBuf>,

    /// Path
    #[arg(long, default_value = ".")]
    path: PathBuf,

    // See also "Reference: CLAP-1" below. 
    /// Project name
    #[arg(long, value_name = "PROJECT_NAME")]
    pub project: Option<String>,

    #[command(flatten)]
    verbose: Verbosity<InfoLevel>,
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
enum Command {
    /// Create a new job
    Create {
    },
    /// Add a PCB
    AddPcb {
        /// PCB kind
        #[arg(long)]
        kind: PcbKindArg,
        
        /// Name of the PCB, e.g. 'panel_1'
        #[arg(long)]
        name: String,
    },
    /// Assign a design variant to a PCB unit
    AssignVariantToUnit {
        /// Name of the design
        #[arg(long, value_parser = clap::value_parser!(DesignName), value_name = "DESIGN_NAME")]
        design: DesignName,

        /// Variant of the design
        #[arg(long, value_parser = clap::value_parser!(VariantName), value_name = "VARIANT_NAME")]
        variant: VariantName,

        /// PCB unit path
        #[arg(long, value_parser = clap::value_parser!(ObjectPath), value_name = "OBJECT_PATH")]
        unit: ObjectPath,
    },
    /// Assign a process to parts
    AssignProcessToParts {
        /// Process name
        #[arg(long)]
        process: ProcessName,

        /// Manufacturer pattern (regexp)
        #[arg(long)]
        manufacturer: Regex,

        /// Manufacturer part number (regexp)
        #[arg(long)]
        mpn: Regex,
    },
    /// Create a phase
    CreatePhase {
        /// Process name
        #[arg(long)]
        process: ProcessName,
        
        /// Phase reference (e.g. 'top_1')
        #[arg(long)]
        reference: Reference,
        
        /// Load-out source (e.g. 'load_out_1')
        #[arg(long)]
        load_out: LoadOutSource,

        /// PCB side
        #[arg(long)]
        pcb_side: PcbSideArg,
    },
    /// Assign placements to a phase
    AssignPlacementsToPhase {
        /// Phase reference (e.g. 'top_1')
        #[arg(long)]
        phase: Reference,

        /// Placements object path pattern (regexp)
        #[arg(long)]
        placements: Regex,
    },
    /// Assign feeder to load-out item
    AssignFeederToLoadOutItem {
        /// Phase reference (e.g. 'top_1')
        #[arg(long)]
        phase: Reference,

        /// Feeder reference (e.g. 'FEEDER_1')
        #[arg(long)]
        feeder_reference: Reference,

        /// Manufacturer pattern (regexp)
        #[arg(long)]
        manufacturer: Regex,

        /// Manufacturer part number (regexp)
        #[arg(long)]
        mpn: Regex,
    },
    /// Set placement ordering for a phase
    SetPlacementOrdering {
        /// Phase reference (e.g. 'top_1')
        #[arg(long)]
        phase: Reference,

        /// Orderings (e.g. 'PCB_UNIT:ASC,FEEDER_REFERENCE:ASC')
        #[arg(long, num_args = 0.., value_delimiter = ',', value_parser = cli::parsers::PlacementSortingItemParser::default())]
        placement_orderings: Vec<PlacementSortingItem>
    },
    
    // FUTURE consider adding a command to allow the phase ordering to be changed, currently phase ordering is determined by the order of phase creation.
    
    /// Generate artifacts
    GenerateArtifacts {
    },
    /// Record phase operation
    RecordPhaseOperation {
        /// Phase reference (e.g. 'top_1')
        #[arg(long)]
        phase: Reference,

        /// The operation to update
        #[arg(long)]
        operation: ProcessOperationArg,

        /// The process operation to set
        #[arg(long)]
        set: ProcessOperationSetArg,
    },   
    /// Record placements operation
    RecordPlacementsOperation {
        /// List of reference designators to apply the operation to
        #[arg(long, required = true, num_args = 1.., value_delimiter = ',')]
        object_path_patterns: Vec<Regex>,
        
        /// The completed operation to apply
        #[arg(long)]
        operation: PlacementOperationArg,
    },
    /// Reset operations
    ResetOperations {
    }
}

use crux_core::App;
use crux_core::macros::Effect;
use crux_core::render::Render;
use thiserror::Error;

#[derive(Default)]
pub struct Planner;

#[derive(Default)]
pub struct Model {
    project: Option<Project>,
    project_file_path: Option<PathBuf>,

    error: Option<anyhow::Error>

}

#[cfg_attr(feature = "typegen", derive(crux_core::macros::Export))]
#[derive(Effect)]
pub struct Capabilities {
    render: Render<Event>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct ViewModel {
    error: Option<String>
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Event {
    None, // we can't instantiate an empty enum, so let's have a dummy variant for now
    CreateProject { 
        project_name: String,
        project_file_path: PathBuf,
    },
    Save,
}

#[derive(Error, Debug)]
pub enum EventError {
    #[error("Unknown event")]
    UnknownEvent
}


impl TryFrom<&Opts> for Event {
    type Error = EventError;

    fn try_from(opts: &Opts) -> Result<Self, Self::Error> {
        let project_name = opts.project.as_ref().unwrap();
        let project_file_path = project::build_project_file_path(project_name, &opts.path);

        match opts.command {
            Command::Create { } => Ok(Event::CreateProject { project_name: project_name.clone(), project_file_path }),
            _ => Err(EventError::UnknownEvent)
        }
    }
}

impl App for Planner {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Capabilities = Capabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            Event::None => {}
            Event::CreateProject { project_name, project_file_path} => {
                let project = Project::new(project_name.to_string());
                model.project.replace(project);
                model.project_file_path.replace(project_file_path);
                
                self.update(Event::Save {}, model, caps);
            },
            Event::Save => {
                if let (Some(project), Some(profile_file_path)) = (&model.project, &model.project_file_path) {
                    match project::save(project, profile_file_path) {
                        Ok(_) => {
                            info!("Created job: {}", project.name);
                        },
                        Err(e) => { 
                           model.error.replace(e); 
                        }
                    }
                } else {
                    model.error.replace(anyhow!("Attempt to save without project and path"));
                }
            }
        }

        caps.render.render();
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        
        let error: Option<String> = match &model.error {
            None => None,
            Some(error) => Some(format!("{:?}", error)),
        };
        
        ViewModel {
            error
        }
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;
    use crux_core::{assert_effect, testing::AppTester};

    #[test]
    fn minimal() {
        let hello = AppTester::<Planner, _>::default();
        let mut model = Model::default();

        // Call 'update' and request effects
        let update = hello.update(Event::None, &mut model);

        // Check update asked us to `Render`
        assert_effect!(update, Effect::Render(_));

        // Make sure the view matches our expectations
        let actual_view = &hello.view(&model);
        let expected_view = ViewModel::default();
        assert_eq!(actual_view, &expected_view);
    }
}

mod core {
    use std::sync::Arc;
    use anyhow::anyhow;
    use crossbeam_channel::Sender;
    use tracing::debug;
    use crate::{Effect, Event, Planner};

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
}

// FUTURE consider merging the AssignProcessToParts and AssignLoadOutToParts commands
//        consider making a group for the criteria args (manufacturer/mpn/etc).

fn main() -> anyhow::Result<()>{
    let args = argfile::expand_args(
        argfile::parse_fromfile,
        argfile::PREFIX,
    ).unwrap();

    let opts = Opts::parse_from(args);

    cli::tracing::configure_tracing(opts.trace.clone(), opts.verbose.clone())?;
    
    let event: Result<Event, _> = Event::try_from(&opts);
    
    match event {
        Ok(event) => {
            let core = core::new();

            let (tx, rx) = unbounded::<Effect>();

            core::update(&core, event, &Arc::new(tx))?;

            while let Ok(effect) = rx.recv() {
                if let Effect::Render(_) = effect {
                    let view = core.view();
                    if let Some(error) = view.error {
                        bail!(error)
                    }
                }
            }
        },
        Err(_) => {
            let project_name = &opts.project.unwrap();
            let project_file_path = project::build_project_file_path(&project_name, &opts.path);

            match opts.command {
                Command::AddPcb { kind, name } => {
                    let mut project = project::load(&project_file_path)?;

                    project::add_pcb(&mut project, kind.clone().into(), name)?;

                    project::save(&project, &project_file_path)?;
                },
                Command::AssignVariantToUnit { design, variant, unit } => {
                    let mut project = project::load(&project_file_path)?;

                    project.update_assignment(unit.clone(), DesignVariant { design_name: design.clone(), variant_name: variant.clone() })?;

                    let unique_design_variants = project.unique_design_variants();
                    let design_variant_placement_map = stores::placements::load_all_placements(&unique_design_variants, &opts.path)?;
                    let _all_parts = project::refresh_from_design_variants(&mut project, design_variant_placement_map);

                    project::save(&project, &project_file_path)?;
                },
                Command::AssignProcessToParts { process: process_name, manufacturer: manufacturer_pattern, mpn: mpn_pattern } => {
                    let mut project = project::load(&project_file_path)?;

                    let process = project.find_process(&process_name)?.clone();

                    let unique_design_variants = project.unique_design_variants();
                    let design_variant_placement_map = stores::placements::load_all_placements(&unique_design_variants, &opts.path)?;
                    let all_parts = project::refresh_from_design_variants(&mut project, design_variant_placement_map);

                    project::update_applicable_processes(&mut project, all_parts.as_slice(), process, manufacturer_pattern, mpn_pattern);

                    project::save(&project, &project_file_path)?;
                },
                Command::CreatePhase { process: process_name, reference, load_out, pcb_side: pcb_side_arg } => {
                    let mut project = project::load(&project_file_path)?;

                    let pcb_side = pcb_side_arg.into();

                    let process_name_str = process_name.to_string();
                    let process = ProcessFactory::by_name(process_name_str.as_str())?;

                    project.ensure_process(&process)?;

                    stores::load_out::ensure_load_out(&load_out)?;

                    project.update_phase(reference, process.name.clone(), load_out.to_string(), pcb_side)?;

                    project::save(&project, &project_file_path)?;
                },
                Command::AssignPlacementsToPhase { phase: reference, placements: placements_pattern } => {
                    let mut project = project::load(&project_file_path)?;

                    let unique_design_variants = project.unique_design_variants();
                    let design_variant_placement_map = stores::placements::load_all_placements(&unique_design_variants, &opts.path)?;
                    let _all_parts = project::refresh_from_design_variants(&mut project, design_variant_placement_map);

                    let phase = project.phases.get(&reference)
                        .ok_or(PhaseError::UnknownPhase(reference))?.clone();

                    let parts = project::assign_placements_to_phase(&mut project, &phase, placements_pattern);
                    trace!("Required load_out parts: {:?}", parts);

                    let _modified = project::update_phase_operation_states(&mut project);

                    for part in parts.iter() {
                        let part_state = project.part_states.get_mut(&part)
                            .ok_or_else(|| PartStateError::NoPartStateFound { part: part.clone() })?;

                        project::add_process_to_part(part_state, part, phase.process.clone());
                    }

                    stores::load_out::add_parts_to_load_out(&LoadOutSource::from_str(&phase.load_out_source).unwrap(), parts)?;

                    project::save(&project, &project_file_path)?;
                },
                Command::SetPlacementOrdering { phase: reference, placement_orderings } => {
                    let mut project = project::load(&project_file_path)?;

                    let unique_design_variants = project.unique_design_variants();
                    let design_variant_placement_map = stores::placements::load_all_placements(&unique_design_variants, &opts.path)?;
                    let _all_parts = project::refresh_from_design_variants(&mut project, design_variant_placement_map);

                    let modified = project::update_placement_orderings(&mut project, &reference, &placement_orderings)?;

                    if modified {
                        project::save(&project, &project_file_path)?;
                    }
                },
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
        }
    }
    
    Ok(())
}
