use std::path::PathBuf;
use clap::{Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use heck::ToShoutySnakeCase;
use regex::Regex;
use tracing::{info, trace};
use makerpnp::{cli, planning};
use makerpnp::cli::args::{PcbKindArg, PcbSideArg, PlacementOperationArg};
use makerpnp::planning::design::{DesignName, DesignVariant};
use makerpnp::planning::reference::Reference;
use makerpnp::planning::placement::PlacementSortingItem;
use makerpnp::planning::process::Process;
use makerpnp::planning::project::{PartStateError, Project};
use makerpnp::planning::{process, project};
use makerpnp::planning::phase::PhaseError;
use makerpnp::planning::variant::VariantName;
use makerpnp::pnp::object_path::ObjectPath;
use makerpnp::stores::load_out::LoadOutSource;

#[derive(Parser)]
#[command(name = "planner")]
#[command(bin_name = "planner")]
#[command(version, about, long_about = None)]
struct Opts {
    #[command(subcommand)]
    command: Command,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log", require_equals = true)]
    trace: Option<PathBuf>,

    /// Path
    #[arg(long, require_equals = true, default_value = ".")]
    path: PathBuf,

    /// Project name
    #[arg(long, require_equals = true, value_name = "PROJECT_NAME")]
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
        #[arg(long, require_equals = true)]
        kind: PcbKindArg,
        
        /// Name of the PCB, e.g. 'panel_1'
        #[arg(long, require_equals = true)]
        name: String,
    },
    /// Assign a design variant to a PCB unit
    AssignVariantToUnit {
        /// Name of the design
        #[arg(long, require_equals = true, value_parser = clap::value_parser!(DesignName), value_name = "DESIGN_NAME")]
        design: DesignName,

        /// Variant of the design
        #[arg(long, require_equals = true, value_parser = clap::value_parser!(VariantName), value_name = "VARIANT_NAME")]
        variant: VariantName,

        /// PCB unit path
        #[arg(long, require_equals = true, value_parser = clap::value_parser!(ObjectPath), value_name = "OBJECT_PATH")]
        unit: ObjectPath,
    },
    /// Assign a process to parts
    AssignProcessToParts {
        /// Process name
        #[arg(long, require_equals = true)]
        process: Process,

        /// Manufacturer pattern (regexp)
        #[arg(long, require_equals = true)]
        manufacturer: Regex,

        /// Manufacturer part number (regexp)
        #[arg(long, require_equals = true)]
        mpn: Regex,
    },
    /// Create a phase
    CreatePhase {
        /// Process name
        #[arg(long, require_equals = true)]
        process: Process,
        
        /// Phase reference (e.g. 'top_1')
        #[arg(long, require_equals = true)]
        reference: Reference,
        
        /// Load-out source (e.g. 'load_out_1')
        #[arg(long, require_equals = true)]
        load_out: LoadOutSource,

        /// PCB side
        #[arg(long, require_equals = true)]
        pcb_side: PcbSideArg,
    },
    /// Assign placements to a phase
    AssignPlacementsToPhase {
        /// Phase reference (e.g. 'top_1')
        #[arg(long, require_equals = true)]
        phase: Reference,

        /// Placements object path pattern (regexp)
        #[arg(long, require_equals = true)]
        placements: Regex,
    },
    /// Assign feeder to load-out item
    AssignFeederToLoadOutItem {
        /// Phase reference (e.g. 'top_1')
        #[arg(long, require_equals = true)]
        phase: Reference,

        /// Feeder reference (e.g. 'FEEDER_1')
        #[arg(long, require_equals = true)]
        feeder_reference: Reference,

        /// Manufacturer pattern (regexp)
        #[arg(long, require_equals = true)]
        manufacturer: Regex,

        /// Manufacturer part number (regexp)
        #[arg(long, require_equals = true)]
        mpn: Regex,
    },
    /// Set placement ordering for a phase
    SetPlacementOrdering {
        /// Phase reference (e.g. 'top_1')
        #[arg(long, require_equals = true)]
        phase: Reference,

        /// Orderings (e.g. 'PCB_UNIT:ASC,FEEDER_REFERENCE:ASC')
        #[arg(long, num_args = 0.., require_equals = true, value_delimiter = ',', value_parser = makerpnp::cli::parsers::PlacementSortingItemParser::default())]
        placement_orderings: Vec<PlacementSortingItem>
    },
    
    // FUTURE consider adding a command to allow the phase ordering to be changed, currently phase ordering is determined by the order of phase creation.
    
    /// Generate artifacts
    GenerateArtifacts {
    },
    /// Record placements operation
    RecordPlacementsOperation {
        /// List of reference designators to apply the operation to
        #[arg(long, required = true, num_args = 1.., value_delimiter = ',')]
        object_path_patterns: Vec<Regex>,
        
        /// The completed operation to apply
        #[arg(long, require_equals = true)]
        operation: PlacementOperationArg,
    }
}

// FUTURE consider merging the AssignProcessToParts and AssignLoadOutToParts commands
//        consider making a group for the criteria args (manufacturer/mpn/etc).

fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    cli::tracing::configure_tracing(opts.trace, opts.verbose)?;

    if let Some(project_name) = &opts.project {
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
            }
            Command::AssignVariantToUnit { design, variant, unit } => {
                let mut project = project::load(&project_file_path)?;

                project.update_assignment(unit.clone(), DesignVariant { design_name: design.clone(), variant_name: variant.clone() })?;

                project::refresh_from_design_variants(&mut project, &opts.path)?;

                project::save(&project, &project_file_path)?;
            },
            Command::AssignProcessToParts { process, manufacturer: manufacturer_pattern, mpn: mpn_pattern } => {
                let mut project = project::load(&project_file_path)?;

                process::assert_process(&process, &project.processes)?;

                let all_parts = project::refresh_from_design_variants(&mut project, &opts.path)?;

                project::update_applicable_processes(&mut project, all_parts.as_slice(), process, manufacturer_pattern, mpn_pattern);

                project::save(&project, &project_file_path)?;
            },
            Command::CreatePhase { process, reference, load_out, pcb_side: pcb_side_arg } => {
                let mut project = project::load(&project_file_path)?;

                let pcb_side = pcb_side_arg.into();

                project.ensure_process(&process)?;

                project.update_phase(reference, process, load_out, pcb_side)?;

                project::save(&project, &project_file_path)?;
            },
            Command::AssignPlacementsToPhase { phase: reference, placements: placements_pattern } => {
                let mut project = project::load(&project_file_path)?;

                project::refresh_from_design_variants(&mut project, &opts.path)?;

                let phase = project.phases.get(&reference)
                    .ok_or(PhaseError::UnknownPhase(reference))?.clone();

                let parts = project::assign_placements_to_phase(&mut project, &phase, placements_pattern);
                trace!("Required load_out parts: {:?}", parts);

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

                let phase = project.phases.get_mut(&reference)
                    .ok_or(PhaseError::UnknownPhase(reference.clone()))?;

                phase.placement_orderings.clone_from(&placement_orderings);

                info!("Phase placement orderings set. phase: '{}', orderings: [{}]", reference, placement_orderings.iter().map(|item|{
                    format!("{}:{}",
                        item.mode.to_string().to_shouty_snake_case(),
                        item.sort_order.to_string().to_shouty_snake_case()
                    )
                }).collect::<Vec<_>>().join(", "));

                project::save(&project, &project_file_path)?;
            }
            Command::GenerateArtifacts { } => {
                let project = project::load(&project_file_path)?;

                project::generate_artifacts(&project, &opts.path, &project_name)?;
            },
            Command::RecordPlacementsOperation { object_path_patterns, operation } => {
                let mut project = project::load(&project_file_path)?;

                let modified = project::update_placements_operation(&mut project, object_path_patterns, operation.into())?;

                if modified {
                    project::save(&project, &project_file_path)?;
                }
            }
            Command::AssignFeederToLoadOutItem { phase: reference, feeder_reference, manufacturer, mpn } => {
                let project = project::load(&project_file_path)?;

                let phase = project.phases.get(&reference)
                    .ok_or(PhaseError::UnknownPhase(reference))?.clone();

                planning::load_out::assign_feeder_to_load_out_item(&phase, &feeder_reference, manufacturer, mpn)?;
            }
        }
    }

    Ok(())
}
