use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use clap::{Parser, Subcommand};
use serde::Serialize;
use tracing::info;
use makerpnp::cli;
pub use serde_json::*;

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
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
enum Command {
    /// Create a new job
    Create {
        /// Job name
        #[arg(long, require_equals = true)]
        name: String,
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all(deserialize = "snake_case"))]
struct Project {
    name: String,
}


fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    cli::tracing::configure_tracing(opts.trace)?;

    match &opts.command.unwrap() {
        Command::Create {
            name
        } => {
            let path = PathBuf::from(opts.path);

            let project = Project { name: name.to_string() };

            let mut project_file_path: PathBuf = path.clone();
            project_file_path.push(format!("project-{}.mpnp.json", name));

            let project_file = File::create(project_file_path)?;

            let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
            let mut ser = serde_json::Serializer::with_formatter(project_file, formatter);
            project.serialize(&mut ser).unwrap();

            let mut project_file = ser.into_inner();
            project_file.write(b"\n")?;

            info!("Created job: {}", name);
        }
    }

    Ok(())
}
