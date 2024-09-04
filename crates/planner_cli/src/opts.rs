use clap::{Parser, Subcommand, ArgGroup};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use cli::args::{PcbKindArg, PcbSideArg, PlacementOperationArg, ProcessOperationArg, ProcessOperationSetArg};
use planner_app::Event;
use planning::design::DesignName;
use planning::reference::Reference;
use planning::placement::PlacementSortingItem;
use planning::process::ProcessName;
use planning::variant::VariantName;
use pnp::object_path::ObjectPath;
use regex::Regex;
use std::path::PathBuf;
use stores::load_out::LoadOutSource;
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(name = "planner_cli")]
#[command(bin_name = "planner_cli")]
#[command(version, about, long_about = None)]
#[command(group(
    ArgGroup::new("requires_project")
        .args(&["project"])
        .required(true)
))]
pub(crate) struct Opts {
    #[command(subcommand)]
    pub(crate) command: Command,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log")]
    pub(crate) trace: Option<PathBuf>,

    /// Path
    #[arg(long, default_value = ".")]
    pub(crate) path: PathBuf,

    /// Project name
    #[arg(long, value_name = "PROJECT_NAME")]
    pub(crate) project: Option<String>,

    #[command(flatten)]
    pub(crate) verbose: Verbosity<InfoLevel>,
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help(true))]
pub(crate) enum Command {
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

// FUTURE consider merging the AssignProcessToParts and AssignLoadOutToParts commands
//        consider making a group for the criteria args (manufacturer/mpn/etc).

#[derive(Error, Debug)]
pub enum EventError {
    #[error("Missing project name")]
    MissingProjectName,
}

impl TryFrom<Opts> for Event {
    type Error = EventError;

    fn try_from(opts: Opts) -> Result<Self, Self::Error> {

        match opts.command {
            Command::Create { } => {
                let project_name = opts.project.ok_or(EventError::MissingProjectName)?;

                Ok(Event::CreateProject { project_name, path: opts.path })
            },
            Command::AddPcb { kind, name } => 
                Ok(Event::AddPcb { kind: kind.into(), name: name.to_string() }),
            Command::AssignVariantToUnit { design, variant, unit } => 
                Ok(Event::AssignVariantToUnit { design, variant, unit }),
            Command::AssignProcessToParts { process, manufacturer, mpn } =>
                Ok(Event::AssignProcessToParts { process, manufacturer, mpn }),
            Command::CreatePhase { process, reference, load_out, pcb_side } =>
                Ok(Event::CreatePhase {
                    process,
                    reference,
                    load_out,
                    pcb_side: pcb_side.into(),
                }),
            Command::AssignPlacementsToPhase { phase, placements } =>
                Ok(Event::AssignPlacementsToPhase {
                    phase,
                    placements,
                }),
            Command::SetPlacementOrdering { phase, placement_orderings } =>
                Ok(Event::SetPlacementOrdering { phase, placement_orderings }),
            Command::GenerateArtifacts { } => 
                Ok(Event::GenerateArtifacts),
            Command::AssignFeederToLoadOutItem { phase, feeder_reference, manufacturer, mpn } =>
                Ok(Event::AssignFeederToLoadOutItem {
                    phase,
                    feeder_reference,
                    manufacturer,
                    mpn,
                }),
            
            Command::RecordPhaseOperation { phase, operation, set } => 
                Ok(Event::RecordPhaseOperation {
                    phase,
                    operation: operation.into(),
                    set: set.into(),
                }),
            Command::RecordPlacementsOperation { object_path_patterns, operation } =>
                Ok(Event::RecordPlacementsOperation { object_path_patterns, operation: operation.into() }),
            Command::ResetOperations { } => 
                Ok(Event::ResetOperations {} ),
        }
    }
}
