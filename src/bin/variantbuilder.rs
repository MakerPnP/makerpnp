use std::fmt::{Display, Formatter};
use anyhow::Error;
use clap::{Args, Parser, Subcommand};
use termtree::Tree;
use thiserror::Error;

use makerpnp::assembly::AssemblyVariantProcessor;
use makerpnp::eda::assembly_variant::AssemblyVariant;
use makerpnp::eda::eda_placement::{DipTracePlacementDetails, EdaPlacementDetails};
use makerpnp::loaders::{eda_placements, part_mappings, parts};
use makerpnp::part_mapper::{PartMapper, ProcessingResult};

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
        #[arg(long, value_name = "FILE")]
        placements: String,

        /// Parts file
        #[arg(long, value_name = "FILE")]
        parts: String,

        /// Part-mappings file
        #[arg(long, value_name = "FILE")]
        part_mappings: String,

        #[command(flatten)]
        assembly_variant: AssemblyVariantArgs
    },
}

fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    match &opts.command.unwrap() {
        Commands::Build { placements, assembly_variant, parts, part_mappings } => {
            build_assembly_variant(placements, assembly_variant, parts, part_mappings)?;
        },
    }

    Ok(())
}

fn build_assembly_variant(placements_source: &String, assembly_variant_args: &AssemblyVariantArgs, parts_source: &String, part_mappings_source: &String) -> Result<(), Error> {

    let eda_placements = eda_placements::load_eda_placements(placements_source)?;
    println!("Loaded {} placements", eda_placements.len());

    let parts = parts::load_parts(parts_source)?;
    println!("Loaded {} parts", parts.len());

    let part_mappings = part_mappings::load_part_mappings(&parts, part_mappings_source)?;
    println!("Loaded {} part mappings", part_mappings.len());

    let assembly_variant = assembly_variant_args.build_assembly_variant()?;
    println!("Assembly variant: {}", assembly_variant.name);
    println!("Ref_des list: {}", assembly_variant.ref_des_list.join(", "));

    let assembly_variant_processor = AssemblyVariantProcessor::default();

    let result = assembly_variant_processor.process(&eda_placements, assembly_variant)?;
    let variant_placements = result.placements;
    let variant_placements_count = variant_placements.len();

    println!("Matched {} placements", variant_placements_count);

    println!("{:?}", part_mappings);

    let matched_mappings = PartMapper::process(&variant_placements, &part_mappings);

    println!("{:?}", matched_mappings);

    let matched_placement_count = matched_mappings.len();
    println!("Mapped {} placements to {} parts\n", variant_placements_count, matched_placement_count);

    let tree = build_mapping_tree(matched_mappings);
    println!("{}", tree);

    Ok(())
}

fn build_mapping_tree(matched_mappings: Vec<ProcessingResult>) -> Tree<String> {
    let mut tree = Tree::new("Mapping Tree".to_string());

    for ProcessingResult { eda_placement, part_mapping} in matched_mappings.iter() {
        let placement_label = format!("{} ({})", eda_placement.ref_des, EdaPlacementTreeFormatter::format(&eda_placement.details));
        let mut placement_node = Tree::new(placement_label);

        let part_label = format!("manufacturer: '{}', mpn: '{}'", part_mapping.part.manufacturer, part_mapping.part.mpn);
        let part_node= Tree::new(part_label);

        placement_node.leaves.push(part_node);

        tree.leaves.push(placement_node)
    }

    tree
}

struct EdaPlacementTreeFormatter {}

struct DipTracePlacementDetailsLabel<'details>(&'details DipTracePlacementDetails);

impl<'details> Display for DipTracePlacementDetailsLabel<'details> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "name: '{}', value: '{}'", self.0.name, self.0.value)
    }
}

impl EdaPlacementTreeFormatter {
    fn format(details: &EdaPlacementDetails) -> impl Display + '_ {
        match details {
            EdaPlacementDetails::DipTrace(d) => DipTracePlacementDetailsLabel(d)
        }
    }
}
