#![deny(missing_docs)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use cli::args::{
    AddOrRemoveOperationArg, PcbSideArg, PlacementOperationArg, SetOrClearOperationArg, TaskActionArg, TaskKindArg,
};
use cli::parsers::{dimensions_decimal_parser, vector2_decimal_parser};
use nalgebra::Vector2;
use planner_app::{Event, ProcessEdit};
use planning::design::DesignName;
use planning::file::FileReference;
use planning::placement::PlacementSortingItem;
use planning::process::ProcessReference;
use planning::variant::VariantName;
use pnp::object_path::ObjectPath;
use pnp::panel::{DesignSizing, Dimensions, PcbUnitPositioning};
use pnp::pcb::PcbUnitNumber;
use pnp::reference::Reference;
use regex::Regex;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use stores::load_out::LoadOutSource;
use stores::package_mappings::PackageMappingsSource;
use stores::packages::PackagesSource;
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(name = "planner_cli")]
#[command(bin_name = "planner_cli")]
#[command(version, about, long_about = None)]
pub(crate) struct Opts {
    #[command(subcommand)]
    pub(crate) command: ModeCommand,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log")]
    pub(crate) trace: Option<PathBuf>,

    #[command(flatten)]
    pub(crate) verbose: Verbosity<InfoLevel>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ModeCommand {
    /// Project mode
    Project(ProjectArgs),

    /// PCB mode
    Pcb(PcbCommandArgs),

    /// Process mode, to define a process's operations and tasks
    Process(ProcessCommandArgs),
}

#[derive(Debug, Parser)]
pub(crate) struct ProcessCommandArgs {
    /// Specify a process context, e.g. 'glue_and_wave.process.json'
    #[arg(long, value_name = "PROCESS_FILE")]
    pub(crate) process_file: PathBuf,

    #[command(subcommand)]
    pub(crate) command: ProcessCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProcessCommand {
    /// Create a process file
    Create {
        /// Process reference, e.g. 'glue_and_wave'
        #[arg(long)]
        reference: ProcessReference,

        /// The process to start from, otherwise the process has no operations
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(["pnp", "manual"]))]
        preset: Option<String>,
    },
    /// Change the process's reference
    SetReference {
        /// Process reference, e.g. 'glue_and_wave'
        #[arg(long)]
        reference: ProcessReference,
    },
    /// Add an operation, after the others
    AddOperation {
        /// Operation reference, e.g. 'apply_glue'
        #[arg(long)]
        operation: Reference,
    },
    /// Remove an operation
    RemoveOperation {
        /// Operation reference
        #[arg(long)]
        operation: Reference,
    },
    /// Add a task to an operation, after its other tasks
    AddTask {
        /// Operation reference
        #[arg(long)]
        operation: Reference,

        /// Task reference, e.g. 'wave_solder'
        #[arg(long)]
        task: Reference,

        /// What the task does, as far as the planner needs to know
        #[arg(long)]
        kind: TaskKindArg,
    },
    /// Remove a task from an operation
    RemoveTask {
        /// Operation reference
        #[arg(long)]
        operation: Reference,

        /// Task reference
        #[arg(long)]
        task: Reference,
    },
    /// Add a rule
    AddRule {
        /// Rule reference
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(planner_app::PROCESS_RULES))]
        rule: String,
    },
    /// Remove a rule
    RemoveRule {
        /// Rule reference
        #[arg(long)]
        rule: Reference,
    },
}

#[derive(Debug, Parser)]
pub(crate) struct ProjectArgs {
    /// Path
    #[arg(long, default_value = ".")]
    pub(crate) path: PathBuf,

    /// Project name
    #[arg(long, value_name = "PROJECT_NAME")]
    pub(crate) project: String,

    #[command(subcommand)]
    pub(crate) command: ProjectCommand,
}

#[derive(Debug, Parser)]
pub(crate) struct PcbCommandArgs {
    /// Specify a PCB context
    #[arg(long, value_name = "PCB_FILE")]
    pub(crate) pcb_file: PathBuf,

    #[command(subcommand)]
    pub(crate) command: PcbCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PcbCommand {
    /// Create a PCB file
    Create {
        /// Name of the PCB, e.g. 'panel_1'
        #[arg(long)]
        name: String,

        /// The number of individual PCB units. 1 = single, >1 = panel
        #[arg(long)]
        units: u16,

        /// The mapping of designs to units e.g. '1=design_a,2=design_b,3=design_a,4=design_b'. unit is 1-based.
        #[arg(long, required = true, value_parser = parse_design_kv, num_args = 1.., value_delimiter = ',')]
        design: Vec<(PcbUnitNumber, DesignName)>,
    },
    /// Configure a PCB
    ConfigurePanelSizing {
        /// Edge rails (left,right,top,bottom) e.g. 'left=5,right=5,top=10,bottom=10'.
        #[arg(long, value_name = "DIMENSIONS", value_parser = dimensions_decimal_parser)]
        edge_rails: Option<Dimensions<Decimal>>,

        /// PCB size (x,y) e.g. 'x=100,y=100'.
        #[arg(long, value_name = "VECTOR2", value_parser = vector2_decimal_parser)]
        size: Option<Vector2<Decimal>>,

        // TODO fiducials
        /// Design sizing (e.g. 'design_a:origin=x=15.25,y=15.25:offset=x=-10.0,y=-10.0:size=x=30.5,y=30.5')
        #[arg(long, value_name = "DESIGN_SIZING", action = clap::ArgAction::Append)]
        design_sizing: Vec<DesignSizingArgs>,

        /// PCB unit positioning (e.g. '1:offset=x=10,y=10:rotation=90')
        #[arg(long, value_name = "PCB_UNIT_POSITIONING", action = clap::ArgAction::Append)]
        pcb_unit_position: Vec<PcbUnitPositioningArgs>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PcbUnitPositioningArgs {
    unit: PcbUnitNumber,
    offset: Vector2<Decimal>,
    /// positive anti-clockwise rotation in degrees
    rotation: Decimal,
}

// FIXME It would be better if this mod was deleted and handled by clap after adding appropriate attributes to `DesignSizingArgs`
mod pcb_unit_positioning {
    use std::num::ParseIntError;
    use std::str::FromStr;

    use cli::parsers::vector2_decimal_parser;
    use pnp::pcb::PcbUnitNumber;
    use rust_decimal::Decimal;
    use thiserror::Error;

    use crate::opts::PcbUnitPositioningArgs;

    impl FromStr for PcbUnitPositioningArgs {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            pcb_unit_position_parser(s).map_err(|e| e.to_string())
        }
    }

    #[derive(Error, Debug)]
    enum PcbUnitPositioningParserError {
        #[error("Invalid format, expected '<unit>:offset=x=<x>,y=<y>':rotation=<degrees>")]
        InvalidFormat(String),
        #[error("Invalid unit: '{0}', expected a number > 0, cause: {1}")]
        InvalidUnit(String, ParseIntError),
        #[error("Invalid origin: '{0}'")]
        InvalidOrigin(String),
        #[error("Missing offset")]
        MissingOffset,
        #[error("Invalid rotation: '{0}', cause: {1}")]
        InvalidRotation(String, rust_decimal::Error),
        #[error("Missing rotation")]
        MissingRotation,
    }

    /// e.g. '1:offset=x=12,y=7:rotation=0'
    fn pcb_unit_position_parser(s: &str) -> Result<PcbUnitPositioningArgs, PcbUnitPositioningParserError> {
        let (pcb_unit_str, rest) = s
            .split_once(':')
            .ok_or(PcbUnitPositioningParserError::InvalidFormat(s.to_string()))?;

        let unit: PcbUnitNumber = pcb_unit_str
            .parse::<PcbUnitNumber>()
            .map_err(|error| PcbUnitPositioningParserError::InvalidUnit(pcb_unit_str.to_string(), error))?;

        let parts = rest.split(':').peekable();

        let required_keys = ["offset", "rotation"];

        let mut offset = None;
        let mut rotation = None;

        for label_chunk in parts {
            let (label, rest) = label_chunk
                .split_once('=')
                .ok_or_else(|| PcbUnitPositioningParserError::InvalidFormat(label_chunk.to_string()))?;

            if !required_keys.contains(&label) {
                return Err(PcbUnitPositioningParserError::InvalidFormat(s.to_string()));
            }

            println!("label: '{:?}' rest: '{:?}'", label, rest);
            match label {
                "offset" => {
                    let value = vector2_decimal_parser(rest).map_err(PcbUnitPositioningParserError::InvalidOrigin)?;
                    offset = Some(value);
                }
                "rotation" => {
                    let value = rest
                        .parse::<Decimal>()
                        .map_err(|e| PcbUnitPositioningParserError::InvalidRotation(rest.to_string(), e))?;
                    rotation = Some(value)
                }
                _ => return Err(PcbUnitPositioningParserError::InvalidFormat(s.to_string())),
            }
        }

        Ok(PcbUnitPositioningArgs {
            unit,
            offset: offset.ok_or(PcbUnitPositioningParserError::MissingOffset)?,
            rotation: rotation.ok_or(PcbUnitPositioningParserError::MissingRotation)?,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DesignSizingArgs {
    name: DesignName,
    origin: Vector2<Decimal>,
    gerber_offset: Vector2<Decimal>,
    placement_offset: Vector2<Decimal>,
    size: Vector2<Decimal>,
}

// FIXME It would be better if this mod was deleted and handled by clap after adding appropriate attributes to `DesignSizingArgs`
mod design_sizing {
    use std::collections::HashMap;
    use std::str::FromStr;

    use cli::parsers::vector2_decimal_parser;
    use nalgebra::Vector2;
    use planning::design::{DesignName, DesignNameError};
    use rust_decimal::Decimal;
    use thiserror::Error;

    use crate::opts::DesignSizingArgs;

    #[derive(Error, Debug)]
    enum DesignSizingParserError {
        #[error("Invalid name: {0}")]
        InvalidName(DesignNameError),

        #[error("Missing origin")]
        MissingOrigin,
        #[error("Invalid origin: {0}")]
        InvalidOrigin(String),

        #[error("Missing gerber offset")]
        MissingGerberOffset,
        #[error("Invalid gerber offset: {0}")]
        InvalidGerberOffset(String),

        #[error("Missing placement offset")]
        MissingPlacementOffset,
        #[error("Invalid placement offset: {0}")]
        InvalidPlacementOffset(String),

        #[error("Missing size")]
        MissingSize,
        #[error("Invalid size: {0}")]
        InvalidSize(String),

        #[error("Invalid format, expected '<name>:origin=x=<x>,y=<y>:g_offset=x=<x>,y=<y>:p_offset=x=<x>,y=<y>:size=x=<x>,y=<y>'")]
        InvalidFormat(String),
    }

    impl FromStr for DesignSizingArgs {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            design_sizing_parser(s).map_err(|e| e.to_string())
        }
    }

    /// e.g. 'design_b:origin=x=20,y=25:offset=x=-10,y=-10:size=x=40,y=50'
    fn design_sizing_parser(s: &str) -> Result<DesignSizingArgs, DesignSizingParserError> {
        let (design_name, rest) = s
            .split_once(':')
            .ok_or(DesignSizingParserError::InvalidFormat(s.to_string()))?;

        let name: DesignName =
            DesignName::try_from(design_name.to_string()).map_err(DesignSizingParserError::InvalidName)?;

        let parts = rest.split(':').peekable();

        let required_keys = ["origin", "g_offset", "p_offset", "size"];

        let mut values: HashMap<&str, Vector2<Decimal>> = HashMap::with_capacity(required_keys.len());

        for label_chunk in parts {
            let (label, rest) = label_chunk
                .split_once('=')
                .ok_or_else(|| DesignSizingParserError::InvalidFormat(label_chunk.to_string()))?;

            if !required_keys.contains(&label) {
                return Err(DesignSizingParserError::InvalidFormat(s.to_string()));
            }

            let vector = vector2_decimal_parser(rest).map_err(|e| match label {
                "origin" => DesignSizingParserError::InvalidOrigin(e),
                "g_offset" => DesignSizingParserError::InvalidGerberOffset(e),
                "p_offset" => DesignSizingParserError::InvalidPlacementOffset(e),
                "size" => DesignSizingParserError::InvalidSize(e),
                _ => unreachable!(),
            })?;

            if let Some(_existing_key) = values.insert(label, vector) {
                // same key specified twice
                return Err(DesignSizingParserError::InvalidFormat(s.to_string()));
            }
        }

        Ok(DesignSizingArgs {
            name,
            origin: values
                .remove("origin")
                .ok_or(DesignSizingParserError::MissingOrigin)?,
            gerber_offset: values
                .remove("g_offset")
                .ok_or(DesignSizingParserError::MissingGerberOffset)?,
            placement_offset: values
                .remove("p_offset")
                .ok_or(DesignSizingParserError::MissingPlacementOffset)?,
            size: values
                .remove("size")
                .ok_or(DesignSizingParserError::MissingSize)?,
        })
    }
}

fn parse_design_kv(s: &str) -> Result<(u16, DesignName), String> {
    let mut split = s.splitn(2, '=');
    let key = split
        .next()
        .ok_or_else(|| format!("Missing key in '{}'", s))?;
    let value = split
        .next()
        .ok_or_else(|| format!("Missing value in '{}'", s))?;

    let key_parsed = key
        .parse::<u16>()
        .map_err(|e| format!("Invalid number '{}': {}", key, e))?;

    Ok((key_parsed, value.into()))
}

#[derive(Subcommand, Debug)]
//#[command(arg_required_else_help(true))]
pub(crate) enum ProjectCommand {
    /// Create a new job
    Create {
        /// The source for packages
        #[arg(long)]
        packages: Option<PackagesSource>,

        /// The source for part to package mappings
        #[arg(long)]
        package_mappings: Option<PackageMappingsSource>,
    },
    /// Add a PCB file to the project
    AddPcb {
        /// The path of the PCB, e.g. 'relative:<some_relative_path>' or '<some_absolute_path>'
        /// paths can be prefixed with `relative:` to make them relative to the project path.
        #[arg(long, value_parser = cli::parsers::FileReferenceParser::default(), value_name = "FILE_REFERENCE")]
        file: FileReference,
    },
    /// Remove a PCB from the project
    RemovePcb {
        /// The zero-based index of the PCB
        #[arg(long)]
        index: u16,
    },
    /// Assign a design variant to a PCB unit
    AssignVariantToUnit {
        /// PCB unit path
        #[arg(long, value_parser = clap::value_parser!(ObjectPath), value_name = "OBJECT_PATH")]
        unit: ObjectPath,

        /// Variant of the design
        #[arg(long, value_parser = clap::value_parser!(VariantName), value_name = "VARIANT_NAME")]
        variant: Option<VariantName>,
    },
    /// Refresh from design variants
    RefreshFromDesignVariants,
    /// Add a copy of a process, from its file, to the project
    AddProcess {
        /// The path of the process file, e.g. 'relative=<some_relative_path>' or '<some_absolute_path>'
        /// paths can be prefixed with `relative=` to make them relative to the project path.
        #[arg(long, value_parser = cli::parsers::FileReferenceParser::default(), value_name = "FILE_REFERENCE")]
        file: FileReference,
    },
    /// Remove a process from the project, its file is not deleted
    RemoveProcess {
        /// Process reference
        #[arg(long)]
        process: ProcessReference,
    },
    /// Update the project's copy of each process from its file
    RefreshProcesses,
    /// Assign a process to parts
    AssignProcessToParts {
        /// Process name
        #[arg(long)]
        process: ProcessReference,

        /// Operation
        #[arg(long)]
        operation: AddOrRemoveOperationArg,

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
        process: ProcessReference,

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

        /// Operation
        #[arg(long)]
        operation: SetOrClearOperationArg,

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
        feeder_reference: Option<Reference>,

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

        /// Orderings (e.g. 'PCB_UNIT:ASC,FEEDER_REFERENCE:ASC,REF_DES:ASC')
        #[arg(long, required = true, num_args = 0.., value_delimiter = ',', value_parser = cli::parsers::PlacementSortingItemParser::default())]
        placement_orderings: Vec<PlacementSortingItem>,
    },

    // FUTURE consider adding a command to allow the phase ordering to be changed, currently phase ordering is determined by the order of phase creation.
    /// Generate artifacts
    GenerateArtifacts {},
    /// Record phase operation
    RecordPhaseOperation {
        /// Phase reference (e.g. 'top_1')
        #[arg(long)]
        phase: Reference,

        /// Operation reference
        #[arg(long)]
        operation: Reference,

        /// The task to update
        #[arg(long)]
        task: Reference,

        /// The task action to apply
        #[arg(long)]
        action: TaskActionArg,
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
    ResetOperations {},
}

// FUTURE consider merging the AssignProcessToParts and AssignLoadOutToParts commands
//        consider making a group for the criteria args (manufacturer/mpn/etc).

#[derive(Error, Debug)]
pub enum EventError {
    #[error("Missing project name")]
    MissingProjectName,
    #[error("Missing command")]
    MissingCommand,
}

impl TryFrom<Opts> for Event {
    type Error = EventError;

    fn try_from(opts: Opts) -> Result<Self, Self::Error> {
        match opts.command {
            ModeCommand::Pcb(pcb_args) => match pcb_args.command {
                PcbCommand::Create {
                    name,
                    units,
                    design,
                } => {
                    let unit_map = design
                        .into_iter()
                        .collect::<BTreeMap<_, _>>();

                    Ok(Event::CreatePcb {
                        name: name.to_string(),
                        units,
                        unit_map: Some(unit_map),
                        path: pcb_args.pcb_file.to_path_buf(),
                    })
                }
                PcbCommand::ConfigurePanelSizing {
                    edge_rails,
                    size,
                    design_sizing,
                    pcb_unit_position,
                } => {
                    let design_sizings = design_sizing
                        .into_iter()
                        .map(|args| {
                            (args.name, DesignSizing {
                                size: Vector2::new(args.size.x.to_f64().unwrap(), args.size.y.to_f64().unwrap()),
                                gerber_offset: Vector2::new(
                                    args.gerber_offset.x.to_f64().unwrap(),
                                    args.gerber_offset.y.to_f64().unwrap(),
                                ),
                                placement_offset: Vector2::new(
                                    args.placement_offset
                                        .x
                                        .to_f64()
                                        .unwrap(),
                                    args.placement_offset
                                        .y
                                        .to_f64()
                                        .unwrap(),
                                ),
                                origin: Vector2::new(args.origin.x.to_f64().unwrap(), args.origin.y.to_f64().unwrap()),
                            })
                        })
                        .collect::<HashMap<_, _>>();

                    Ok(Event::ApplyPartialPanelSizing {
                        path: pcb_args.pcb_file.to_path_buf(),
                        edge_rails: edge_rails.map(|edge_rails| Dimensions {
                            top: edge_rails.top.to_f64().unwrap(),
                            bottom: edge_rails.bottom.to_f64().unwrap(),
                            left: edge_rails.left.to_f64().unwrap(),
                            right: edge_rails.right.to_f64().unwrap(),
                        }),
                        size: size.map(|size| Vector2::new(size.x.to_f64().unwrap(), size.y.to_f64().unwrap())),
                        // TODO
                        fiducials: None,
                        design_sizings: Some(design_sizings),
                        pcb_unit_positionings: Some(
                            pcb_unit_position
                                .into_iter()
                                .map(|args| {
                                    (args.unit, PcbUnitPositioning {
                                        offset: Vector2::new(
                                            args.offset.x.to_f64().unwrap(),
                                            args.offset.y.to_f64().unwrap(),
                                        ),
                                        rotation: args.rotation,
                                    })
                                })
                                .collect::<HashMap<_, _>>(),
                        ),
                    })
                }
            },
            ModeCommand::Process(process_args) => {
                let path = process_args.process_file;
                let edit = |edit: ProcessEdit| {
                    Ok(Event::EditProcess {
                        path: path.clone(),
                        edit,
                    })
                };

                match process_args.command {
                    ProcessCommand::Create {
                        reference,
                        preset,
                    } => Ok(Event::CreateProcess {
                        path: path.clone(),
                        reference,
                        preset,
                    }),
                    ProcessCommand::SetReference {
                        reference,
                    } => edit(ProcessEdit::SetReference {
                        reference,
                    }),
                    ProcessCommand::AddOperation {
                        operation,
                    } => edit(ProcessEdit::AddOperation {
                        operation,
                    }),
                    ProcessCommand::RemoveOperation {
                        operation,
                    } => edit(ProcessEdit::RemoveOperation {
                        operation,
                    }),
                    ProcessCommand::AddTask {
                        operation,
                        task,
                        kind,
                    } => edit(ProcessEdit::AddTask {
                        operation,
                        task,
                        kind: kind.into(),
                    }),
                    ProcessCommand::RemoveTask {
                        operation,
                        task,
                    } => edit(ProcessEdit::RemoveTask {
                        operation,
                        task,
                    }),
                    ProcessCommand::AddRule {
                        rule,
                    } => edit(ProcessEdit::AddRule {
                        rule: Reference::from_raw(rule),
                    }),
                    ProcessCommand::RemoveRule {
                        rule,
                    } => edit(ProcessEdit::RemoveRule {
                        rule,
                    }),
                }
            }
            ModeCommand::Project(project_args) => match project_args.command {
                ProjectCommand::Create {
                    packages,
                    package_mappings,
                } => {
                    let name = project_args.project;
                    let directory = project_args.path.clone();

                    let path = build_project_file_path(&name, &directory);

                    Ok(Event::CreateProject {
                        name,
                        path,
                        packages,
                        package_mappings,
                    })
                }
                ProjectCommand::AddPcb {
                    file,
                } => Ok(Event::AddPcb {
                    pcb_file: file,
                }),
                ProjectCommand::RemovePcb {
                    index,
                } => Ok(Event::RemovePcb {
                    index,
                }),
                ProjectCommand::AssignVariantToUnit {
                    unit,
                    variant,
                } => Ok(Event::AssignVariantToUnit {
                    unit,
                    variant,
                }),
                ProjectCommand::RefreshFromDesignVariants => Ok(Event::RefreshFromDesignVariants),
                ProjectCommand::AddProcess {
                    file,
                } => Ok(Event::AddProcess {
                    process_file: file,
                }),
                ProjectCommand::RemoveProcess {
                    process,
                } => Ok(Event::RemoveProcess {
                    process_reference: process,
                }),
                ProjectCommand::RefreshProcesses => Ok(Event::RefreshProcesses),
                ProjectCommand::AssignProcessToParts {
                    process,
                    operation,
                    manufacturer,
                    mpn,
                } => Ok(Event::AssignProcessToParts {
                    process,
                    operation: operation.into(),
                    manufacturer,
                    mpn,
                }),
                ProjectCommand::CreatePhase {
                    process,
                    reference,
                    load_out,
                    pcb_side,
                } => Ok(Event::CreatePhase {
                    process,
                    reference,
                    load_out,
                    pcb_side: pcb_side.into(),
                }),
                ProjectCommand::AssignPlacementsToPhase {
                    phase,
                    operation,
                    placements,
                } => Ok(Event::AssignPlacementsToPhase {
                    phase,
                    operation: operation.into(),
                    placements,
                }),
                ProjectCommand::SetPlacementOrdering {
                    phase,
                    placement_orderings,
                } => Ok(Event::SetPlacementOrdering {
                    phase,
                    placement_orderings,
                }),
                ProjectCommand::GenerateArtifacts {} => Ok(Event::GenerateArtifacts),
                ProjectCommand::AssignFeederToLoadOutItem {
                    phase,
                    feeder_reference,
                    manufacturer,
                    mpn,
                } => Ok(Event::AssignFeederToLoadOutItem {
                    phase,
                    feeder_reference,
                    manufacturer,
                    mpn,
                }),
                ProjectCommand::RecordPhaseOperation {
                    phase,
                    operation,
                    task,
                    action,
                } => Ok(Event::RecordPhaseOperation {
                    phase,
                    operation: operation.into(),
                    task: task.into(),
                    action: action.into(),
                }),
                ProjectCommand::RecordPlacementsOperation {
                    object_path_patterns,
                    operation,
                } => Ok(Event::RecordPlacementsOperation {
                    object_path_patterns,
                    operation: operation.into(),
                }),
                ProjectCommand::ResetOperations {} => Ok(Event::ResetOperations {}),
            },
        }
    }
}

pub fn build_project_file_path(name: &str, directory: &Path) -> PathBuf {
    let mut project_file_path: PathBuf = PathBuf::from(directory);
    project_file_path.push(format!("project-{}.mpnp.json", name));
    project_file_path
}
