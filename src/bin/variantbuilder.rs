use std::path::PathBuf;
use anyhow::{bail, Error};
use clap::{Args, Parser, Subcommand};
use thiserror::Error;
use makerpnp::assembly::{AssemblyVariant, AssemblyVariantProcessor, Placement};

#[derive(Parser)]
#[command(name = "variantbuilder")]
#[command(bin_name = "variantbuilder")]
#[command(version, about, long_about = None)]
struct Opts {
    // /// Show version information
    // #[arg(short = 'V', long)]
    // version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Args)]
#[derive(Clone)]
struct AssemblyVariantArgs {
    /// Name of assembly variant
    #[arg(long, default_value = "Default")]
    name: String,

    /// List of reference designators
    #[arg(long, num_args = 0.., value_delimiter = ',')]
    ref_des_list: Vec<String>
}

#[allow(dead_code)]
#[derive(Error, Debug)]
enum AssemblyVariantError {
    #[error("Unknown error")]
    Unknown
}

impl AssemblyVariantArgs {
    pub fn build_assembly_variant(&self) -> Result<AssemblyVariant, AssemblyVariantError> {
        // TODO add all placements to the refdes list if the ref_des_list is empty
        Ok(AssemblyVariant::new(
            self.name.clone(),
            self.ref_des_list.clone(),
        ))
    }
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
enum Commands {
    /// Build variant
    Build {
        /// Placements file
        #[arg(short = 'p', long, value_name = "FILE")]
        placements: String,

        #[command(flatten)]
        assembly_variant: AssemblyVariantArgs
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
struct DiptracePlacementRecord {
    ref_des: String,
    name: String,
    value: String,
}

impl DiptracePlacementRecord {
    pub fn build_placement(&self) -> Result<Placement, ()> {
        Ok(Placement {
            ref_des: self.ref_des.clone(),
        })
    }
}

fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    match &opts.command.unwrap() {
        Commands::Build { placements, assembly_variant } => {
            build_assembly_variant(placements, assembly_variant)?;
        },
    }

    Ok(())
}

fn build_assembly_variant(placements: &String, assembly_variant_args: &AssemblyVariantArgs) -> Result<(), Error> {
    let placements_path_buf = PathBuf::from(placements);
    let placements_path = placements_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(placements_path)?;

    let mut placements: Vec<Placement> = vec![];

    for result in csv_reader.deserialize() {
        let record: DiptracePlacementRecord = result?;
        // TODO output the record in verbose mode
        //println!("{:?}", record);

        if let Ok(placement) = record.build_placement() {
            placements.push(placement);
        } else {
            bail!("todo")
        }
    }

    let assembly_variant = assembly_variant_args.build_assembly_variant()?;

    println!("Loaded {} placements", placements.len());
    println!("Assembly variant: {}", assembly_variant.name);
    println!("Ref_des list: {}", assembly_variant.ref_des_list.join(", "));

    let assembly_variant_processor = AssemblyVariantProcessor::default();

    // when
    let result = assembly_variant_processor.process(placements, assembly_variant)?;

    println!("Matched {} placements", result.placements.len());

    Ok(())
}
