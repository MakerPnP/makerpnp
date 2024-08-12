use std::path::PathBuf;
use anyhow::bail;
use clap::{Parser, Subcommand};
use heck::ToShoutySnakeCase;
use regex::Regex;
use tracing::{info, trace};
use makerpnp::{cli, planning};
use makerpnp::cli::args::{PcbKindArg, PcbSideArg, ProjectArgs};
use makerpnp::planning::design::{DesignName, DesignVariant};
use makerpnp::planning::reference::Reference;
use makerpnp::planning::placement::PlacementSortingItem;
use makerpnp::planning::process::Process;
use makerpnp::planning::project::{PlacementAssignmentError, Project};
use makerpnp::planning::project;
use makerpnp::planning::variant::VariantName;
use makerpnp::pnp::object_path::UnitPath;
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

    // FUTURE find a way to define that project args are required when using a project specific sub-command
    //        without excessive code duplication
    #[command(flatten)]
    project_args: Option<ProjectArgs>,
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
        #[arg(long, require_equals = true, value_parser = clap::value_parser!(UnitPath), value_name = "UNIT_PATH")]
        unit: UnitPath,
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

        /// Placements pattern (regexp)
        #[arg(long, require_equals = true)]
        placements: Regex,
    },
    /// Assign feeder to load-out item
    AssignFeederToLoadOutItem {
        /// Load-out source (e.g. 'load_out_1')
        #[arg(long, require_equals = true)]
        load_out: LoadOutSource,

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
        orderings: Vec<PlacementSortingItem>
    },
    /// Generate artifacts
    GenerateArtifacts {
    }
}

// FUTURE consider merging the AssignProcessToParts and AssignLoadOutToParts commands
//        consider making a group for the criteria args (manufacturer/mpn/etc).

fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    cli::tracing::configure_tracing(opts.trace)?;

    match &opts.project_args {
        Some(ProjectArgs { project: name } ) if name.is_some() => {
            let name = name.as_ref().unwrap();
            let project_file_path = project::build_project_file_path(&name, &opts.path) ;

            match opts.command {
                Command::Create {} => {
                    let project = Project::new(name.to_string());
                    project::project_save(&project, &project_file_path)?;

                    info!("Created job: {}", project.name);
                },
                Command::AddPcb { kind, name } => {
                    let mut project = project::project_load(&project_file_path)?;

                    project::project_add_pcb(&mut project, kind.clone().into(), name)?;
                    
                    project::project_save(&project, &project_file_path)?;
                }
                Command::AssignVariantToUnit { design, variant, unit } => {
                    let mut project = project::project_load(&project_file_path)?;

                    project.update_assignment(unit.clone(), DesignVariant { design_name: design.clone(), variant_name: variant.clone() })?;

                    project::project_refresh_from_design_variants(&mut project, &opts.path)?;

                    project::project_save(&project, &project_file_path)?;
                },
                Command::AssignProcessToParts { process, manufacturer: manufacturer_pattern, mpn: mpn_pattern } => {
                    let mut project = project::project_load(&project_file_path)?;

                    // TODO validate that process is a process used by the project

                    let all_parts = project::project_refresh_from_design_variants(&mut project, &opts.path)?;

                    project::project_update_applicable_processes(&mut project, all_parts.as_slice(), process, manufacturer_pattern, mpn_pattern);

                    project::project_save(&project, &project_file_path)?;
                },
                Command::CreatePhase { process, reference, load_out, pcb_side: pcb_side_arg } => {
                    let mut project = project::project_load(&project_file_path)?;

                    let pcb_side = pcb_side_arg.into();

                    project.update_phase(reference, process, load_out, pcb_side)?;

                    project::project_save(&project, &project_file_path)?;
                },
                Command::AssignPlacementsToPhase { phase: reference, placements: placements_pattern } => {
                    let mut project = project::project_load(&project_file_path)?;

                    project::project_refresh_from_design_variants(&mut project, &opts.path)?;

                    let phase = match project.phases.get(&reference) {
                        Some(phase) => Ok(phase),
                        None => Err(PlacementAssignmentError::UnknownPhase(reference.clone())),
                    }?.clone();

                    let parts = project::assign_placements_to_phase(&mut project, &phase, placements_pattern);
                    trace!("Required load_out parts: {:?}", parts);

                    planning::load_out::add_parts_to_load_out(&phase.load_out, parts)?;

                    project::project_save(&project, &project_file_path)?;
                },
                Command::SetPlacementOrdering { phase: reference, orderings: sort_orderings } => {
                    let mut project = project::project_load(&project_file_path)?;

                    project::project_refresh_from_design_variants(&mut project, &opts.path)?;

                    let phase = match project.phases.get_mut(&reference) {
                        Some(phase) => Ok(phase),
                        None => Err(PlacementAssignmentError::UnknownPhase(reference.clone())),
                    }?;

                    phase.sort_orderings.clone_from(&sort_orderings);

                    trace!("Phase orderings set. phase: '{}', orderings: [{}]", reference, sort_orderings.iter().map(|item|{
                        format!("{}:{}",
                            item.mode.to_string().to_shouty_snake_case(),
                            item.sort_order.to_string().to_shouty_snake_case()
                        )
                    }).collect::<Vec<_>>().join(", "));

                    project::project_save(&project, &project_file_path)?;
                },
                Command::GenerateArtifacts { } => {
                    let project = project::project_load(&project_file_path)?;

                    project::project_generate_artifacts(&project, &opts.path, &name)?;
                },
                _ => {
                    bail!("invalid argument 'project'");
                }
            }
        },
        None => {
            match opts.command {
                Command::AssignFeederToLoadOutItem { load_out, feeder_reference, manufacturer, mpn } => {
                    planning::load_out::assign_feeder_to_load_out_item(load_out, feeder_reference, manufacturer, mpn)?;
                }
                _ => {
                    bail!("using a 'project' argument implies a project specific command should be used");
                }
            }
        },
        Some(_project_args) => {
            bail!("invalid arguments");
        }
    }

    Ok(())
}
