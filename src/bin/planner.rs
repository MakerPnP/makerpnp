use std::fs::File;
use std::io::Write;
use std::path::{PathBuf};
use clap::{Parser, Subcommand, ValueEnum};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};
use makerpnp::cli;
pub use serde_json::*;
use makerpnp::loaders::placements::PlacementRecord;
use makerpnp::planning::{DesignName, DesignVariant, LoadOutName, Process, ProcessAssignment, Project, Reference, UnitPath, VariantName};
use makerpnp::pnp::part::Part;

#[derive(Parser)]
#[command(name = "planner")]
#[command(bin_name = "planner")]
#[command(version, about, long_about = None)]
struct Opts {
    #[command(subcommand)]
    command: Option<Command>,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log", require_equals = true)]
    trace: Option<PathBuf>,

    /// Path
    #[arg(long, require_equals = true, default_value = ".")]
    path: PathBuf,

    /// Job name
    #[arg(long, require_equals = true)]
    name: String,
}

#[derive(Clone, ValueEnum)]
enum ProcessArg {
    Pnp,
}

impl From<ProcessArg> for Process {
    fn from(value: ProcessArg) -> Self {
        match value {
            ProcessArg::Pnp => Process::Pnp
        }
    }
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
enum Command {
    /// Create a new job
    Create {
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
    /// Assign process to parts
    AssignProcessToParts {
        /// Process name
        #[arg(long, require_equals = true)]
        process: ProcessArg,

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
        process: ProcessArg,
        
        /// Reference
        #[arg(long, require_equals = true)]
        reference: Reference,
        
        /// Load-out name
        #[arg(long, require_equals = true)]
        load_out: Option<LoadOutName>,
    },
}


fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    cli::tracing::configure_tracing(opts.trace)?;

    let project_file_path = build_project_file_path(&opts.name, &opts.path);

    // TODO print help if no command specified, currently this panics
    match opts.command.unwrap() {
        Command::Create {} => {
            let project = Project::new(opts.name.to_string());
            project_save(&project, &project_file_path)?;

            info!("Created job: {}", project.name);
        },
        Command::AssignVariantToUnit { design, variant, unit } => {
            let mut project = project_load(&project_file_path)?;
            
            project.update_assignment(unit.clone(), DesignVariant { design_name: design.clone(), variant_name: variant.clone() })?;

            let unique_design_variants = build_unique_design_variants(&project);
            let all_parts = load_all_parts(unique_design_variants.as_slice(), &opts.path)?;

            project_refresh_assignments(&mut project, all_parts.as_slice());

            project_save(&project, &project_file_path)?;
        },
        Command::AssignProcessToParts { process: process_arg, manufacturer: manufacturer_pattern, mpn: mpn_pattern } => {
            let mut project = project_load(&project_file_path)?;

            let process: Process = process_arg.into();

            let unique_design_variants = build_unique_design_variants(&project);
            let all_parts = load_all_parts(unique_design_variants.as_slice(), &opts.path)?;

            project_refresh_assignments(&mut project, all_parts.as_slice());

            project_update_assignments(&mut project, all_parts.as_slice(), process, manufacturer_pattern, mpn_pattern);

            project_save(&project, &project_file_path)?;
        },
        Command::CreatePhase { process: process_arg, reference, load_out } => {
            let mut project = project_load(&project_file_path)?;

            let process: Process = process_arg.into();
            
            project.update_phase(reference, process, load_out)?;

            project_save(&project, &project_file_path)?;
        },
    }

    Ok(())
}

#[derive(Debug)]
enum Change {
    New,
    Existing,
    Unused,
}

fn project_refresh_assignments(project: &mut Project, all_parts: &[Part]) {
    let changes = find_changes(project, all_parts);

    for change_item in changes.iter() {
        match change_item {
            (Change::New, part) => {
                debug!("new unassigned part. part: {:?}", part);
                let _ = project.process_part_assignments.entry(part.clone()).or_insert(ProcessAssignment::Unassigned);
            }
            (Change::Existing, _) => {}
            (Change::Unused, part) => {
                debug!("removing previously assigned part. part: {:?}", part);
                let _ = project.process_part_assignments.remove(&part);
            }
        }
    }
}

fn find_changes(project: &mut Project, all_parts: &[Part]) -> Vec<(Change, Part)> {
    let mut changes: Vec<(Change, Part)> = vec![];

    for part in all_parts.iter() {
        match project.process_part_assignments.contains_key(part) {
            true => changes.push((Change::Existing, part.clone())),
            false => changes.push((Change::New, part.clone())),
        }
    }

    for (part, _process) in project.process_part_assignments.iter() {
        if !all_parts.contains(part) {
            changes.push((Change::Unused, part.clone()))
        }
    }

    info!("changes:\n{:?}", changes);

    changes
}

fn project_update_assignments(project: &mut Project, all_parts: &[Part], process: Process, manufacturer_pattern: Regex, mpn_pattern: Regex) {

    let changes = find_changes(project, all_parts);

    for change in changes.iter() {
        match change {
            (Change::Existing, part) => {
                if manufacturer_pattern.is_match(part.manufacturer.as_str()) && mpn_pattern.is_match(part.mpn.as_str()) {
                    project.process_part_assignments.entry(part.clone())
                        .and_modify(|v| {

                            let should_change = match v {
                                ProcessAssignment::Unassigned => true,
                                ProcessAssignment::Assigned(current_process) => current_process != &process,
                            };

                            if should_change {
                                info!("changing process. part: {:?}, old_process: {:?}, new_process: {:?}", part, v, process);
                                *v = ProcessAssignment::Assigned(process.clone())
                            }
                        });
                }
            },
            _ => {
                panic!("unexpected change. change: {:?}", change);
            }
        }
    }
}

fn build_unique_design_variants(project: &Project) -> Vec<DesignVariant> {
    let unique_design_variants: Vec<DesignVariant> = project.unit_assignments.iter().fold(vec![], |mut acc, (_path, design_variant)| {
        if !acc.contains(design_variant) {
            acc.push(design_variant.clone())
        }

        acc
    });

    unique_design_variants
}

fn load_all_parts(unique_design_variants: &[DesignVariant], path: &PathBuf) -> anyhow::Result<Vec<Part>> {
    let mut all_parts: Vec<Part> = vec![];

    for DesignVariant { design_name: design, variant_name: variant } in unique_design_variants {
        let mut placements_path = PathBuf::from(path);
        placements_path.push(format!("{}_{}_placements.csv", design, variant) );

        let mut csv_reader = csv::ReaderBuilder::new().from_path(placements_path)?;
        for result in csv_reader.deserialize() {
            let record: PlacementRecord = result?;
            trace!("{:?}", record);

            let part = Part { manufacturer: record.manufacturer, mpn: record.mpn };
            if !all_parts.contains(&part) {
                all_parts.push(part);
            }
        }
    }

    Ok(all_parts)
}

fn build_project_file_path(name: &str, path: &PathBuf) -> PathBuf {
    let mut project_file_path: PathBuf = path.clone();
    project_file_path.push(format!("project-{}.mpnp.json", name));
    project_file_path
}

fn project_load(project_file_path: &PathBuf) -> anyhow::Result<Project> {
    let project_file = File::open(project_file_path.clone())?;
    let mut de = serde_json::Deserializer::from_reader(project_file);
    let project = Project::deserialize(&mut de)?;
    Ok(project)
}

fn project_save(project: &Project, project_file_path: &PathBuf) -> anyhow::Result<()> {
    let project_file = File::create(project_file_path)?;
    let formatter = ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = Serializer::with_formatter(project_file, formatter);
    project.serialize(&mut ser)?;

    let mut project_file = ser.into_inner();
    project_file.write(b"\n")?;

    Ok(())
}
