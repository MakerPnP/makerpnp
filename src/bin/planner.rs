use std::path::PathBuf;
use clap::{Parser, Subcommand, ArgGroup};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use regex::Regex;
use tracing::{info, trace};
use makerpnp::{cli, planning};
use makerpnp::cli::args::{PcbKindArg, PcbSideArg, PlacementOperationArg, ProcessOperationArg, ProcessOperationSetArg};
use makerpnp::planning::design::{DesignName, DesignVariant};
use makerpnp::planning::reference::Reference;
use makerpnp::planning::placement::PlacementSortingItem;
use makerpnp::planning::process::ProcessName;
use makerpnp::planning::project::{PartStateError, ProcessFactory, Project};
use makerpnp::planning::project;
use makerpnp::planning::phase::PhaseError;
use makerpnp::planning::variant::VariantName;
use makerpnp::pnp::object_path::ObjectPath;
use makerpnp::stores::load_out::LoadOutSource;

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
        #[arg(long, num_args = 0.., value_delimiter = ',', value_parser = makerpnp::cli::parsers::PlacementSortingItemParser::default())]
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

    cli::tracing::configure_tracing(opts.trace, opts.verbose)?;

    let project_name = &opts.project.unwrap();
    let project_file_path = project::build_project_file_path(&project_name, &opts.path);

    match opts.command {
        Command::Create {} => {
            let project = Project::new(project_name.to_string());
            project::save(&project, &project_file_path)?;

            info!("Created job: {}", project.name);
        },
        Command::AddPcb { kind, name } => {
            let mut project = project::load(&project_file_path)?;

            project::add_pcb(&mut project, kind.clone().into(), name)?;

            project::save(&project, &project_file_path)?;
        },
        Command::AssignVariantToUnit { design, variant, unit } => {
            let mut project = project::load(&project_file_path)?;

            project.update_assignment(unit.clone(), DesignVariant { design_name: design.clone(), variant_name: variant.clone() })?;

            project::refresh_from_design_variants(&mut project, &opts.path)?;

            project::save(&project, &project_file_path)?;
        },
        Command::AssignProcessToParts { process: process_name, manufacturer: manufacturer_pattern, mpn: mpn_pattern } => {
            let mut project = project::load(&project_file_path)?;

            let process = project.find_process(&process_name)?.clone();

            let all_parts = project::refresh_from_design_variants(&mut project, &opts.path)?;

            project::update_applicable_processes(&mut project, all_parts.as_slice(), process, manufacturer_pattern, mpn_pattern);

            project::save(&project, &project_file_path)?;
        },
        Command::CreatePhase { process: process_name, reference, load_out, pcb_side: pcb_side_arg } => {
            let mut project = project::load(&project_file_path)?;

            let pcb_side = pcb_side_arg.into();
            
            let process_name_str = process_name.to_string();
            let process = ProcessFactory::by_name(process_name_str.as_str())?;
            
            project.ensure_process(&process)?;

            project.update_phase(reference, process.name.clone(), load_out, pcb_side)?;

            project::save(&project, &project_file_path)?;
        },
        Command::AssignPlacementsToPhase { phase: reference, placements: placements_pattern } => {
            let mut project = project::load(&project_file_path)?;

            project::refresh_from_design_variants(&mut project, &opts.path)?;

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

            planning::load_out::add_parts_to_load_out(&phase.load_out, parts)?;

            project::save(&project, &project_file_path)?;
        },
        Command::SetPlacementOrdering { phase: reference, placement_orderings } => {
            let mut project = project::load(&project_file_path)?;

            project::refresh_from_design_variants(&mut project, &opts.path)?;

            let modified = project::update_placement_orderings(&mut project, &reference, &placement_orderings)?;

            if modified {
                project::save(&project, &project_file_path)?;
            }
        },
        Command::GenerateArtifacts { } => {
            let mut project = project::load(&project_file_path)?;

            let modified = project::update_phase_operation_states(&mut project);

            project::generate_artifacts(&project, &opts.path, &project_name)?;

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
            
            planning::load_out::assign_feeder_to_load_out_item(&phase, &process, &feeder_reference, manufacturer, mpn)?;
        },
    }

    Ok(())
}
