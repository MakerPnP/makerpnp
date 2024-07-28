use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tracing::info;
use makerpnp::cli;
pub use serde_json::*;
use makerpnp::planning::{DesignName, Project, UnitAssignment, UnitPath, VariantName};

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
    path: String,

    /// Job name
    #[arg(long, require_equals = true)]
    name: String,
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
    }
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
            project.add_assignment(UnitAssignment {
                unit_path: unit.clone(),
                design_name: design.clone(),
                variant_name: variant.clone(),
            });

            project_save(&project, &project_file_path)?;

            info!("Assignment created. unit: {}, design: {}, variant: {}",
                unit,
                design,
                variant,
            );
        },
    }

    Ok(())
}

fn build_project_file_path(name: &str, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    let name = name;

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
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(project_file, formatter);
    project.serialize(&mut ser).unwrap();

    let mut project_file = ser.into_inner();
    project_file.write(b"\n")?;

    Ok(())
}
