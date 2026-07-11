use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use cli::args::EdaToolArg;
use thiserror::Error;
use variantbuilder_app::{
    AssemblyRuleSource, AssemblyVariant, EdaSubstitutionsSource, Event, LoadOutSource, PartsSource, PlacementsSource,
};

#[derive(Parser)]
#[command(name = "variantbuilder_cli")]
#[command(bin_name = "variantbuilder_cli")]
#[command(version, about, long_about = None)]
pub struct Opts {
    #[command(subcommand)]
    pub command: Command,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log")]
    pub trace: Option<PathBuf>,

    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

#[derive(Args, Clone, Debug)]
pub struct AssemblyVariantArgs {
    /// Name of assembly variant
    #[arg(long, default_value = "Default")]
    name: String,

    /// List of reference designators for assembly variant (defaults to all if not specified)
    #[arg(long, num_args = 0.., value_delimiter = ',')]
    ref_des_list: Vec<String>,
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum AssemblyVariantError {
    #[error("Unknown error")]
    Unknown,
}

impl AssemblyVariantArgs {
    pub fn build_assembly_variant(&self) -> Result<AssemblyVariant, AssemblyVariantError> {
        Ok(AssemblyVariant::new(self.name.clone(), self.ref_des_list.clone()))
    }
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
pub enum Command {
    /// Build variant
    Build {
        /// EDA tool
        #[arg(long)]
        eda: EdaToolArg,

        /// Load-out source
        #[arg(long, value_name = "SOURCE")]
        load_out: Option<LoadOutSource>,

        /// Placements source
        #[arg(long, value_name = "SOURCE")]
        placements: PlacementsSource,

        /// Parts source
        #[arg(long, value_name = "SOURCE")]
        parts: PartsSource,

        /// Part-mappings source
        #[arg(long, value_name = "SOURCE")]
        part_mappings: PartsSource,

        /// Substitution sources
        #[arg(long, value_delimiter = ',', num_args = 0.., value_name = "SOURCE")]
        substitutions: Vec<EdaSubstitutionsSource>,

        /// List of reference designators to exclude (test-pads, fiducials, etc)
        #[arg(long, num_args = 0.., value_delimiter = ',')]
        ref_des_exclude_list: Vec<String>,

        /// List of reference designators to disable (use for do-not-fit, no-place, etc)
        #[arg(long, num_args = 0.., value_delimiter = ',')]
        ref_des_disable_list: Vec<String>,

        /// Assembly rules source
        #[arg(long, value_name = "SOURCE")]
        assembly_rules: Option<AssemblyRuleSource>,

        // FIXME rename to output_placements
        /// Output CSV file
        #[arg(long, value_name = "FILE")]
        output: String,

        /// Output BOM CSV file (Currently JLCPCB format only)
        #[arg(long, value_name = "FILE")]
        output_bom: Option<String>,

        /// Optional assembly variant
        #[command(flatten)]
        assembly_variant_args: Option<AssemblyVariantArgs>,
    },
}

#[derive(Error, Debug)]
pub enum EventError {
    #[error("None")]
    None,
    #[error("Assembly variant error. Cause: {0}")]
    AssemblyVariantError(AssemblyVariantError),
}

impl TryFrom<Opts> for Event {
    type Error = EventError;

    fn try_from(ops: Opts) -> Result<Self, Self::Error> {
        match ops.command {
            Command::Build {
                eda,
                placements,
                assembly_variant_args,
                parts,
                part_mappings,
                substitutions,
                load_out,
                assembly_rules,
                output,
                output_bom,
                ref_des_exclude_list,
                ref_des_disable_list,
            } => {
                let eda_tool = eda.build();
                let assembly_variant = assembly_variant_args
                    .as_ref()
                    .map_or_else(|| Ok(AssemblyVariant::default()), |args| args.build_assembly_variant())
                    .map_err(|error| EventError::AssemblyVariantError(error))?;

                let event = Event::Build {
                    eda_tool,
                    placements,
                    assembly_variant,
                    parts,
                    part_mappings,
                    substitutions,
                    load_out,
                    assembly_rules,
                    output,
                    output_bom,
                    ref_des_exclude_list,
                    ref_des_disable_list,
                };

                Ok(event)
            }
        }
    }
}
