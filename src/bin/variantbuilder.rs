use std::fmt::{Display, Formatter};
use std::fs::File;
use std::path::PathBuf;
use anyhow::Error;
use clap::{Args, Parser, Subcommand};
use termtree::Tree;
use thiserror::Error;
use tracing::{error, info, Level, trace};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{fmt, FmtSubscriber};
use makerpnp::assembly::AssemblyVariantProcessor;
use makerpnp::eda::assembly_variant::AssemblyVariant;
use makerpnp::eda::eda_placement::{DipTracePlacementDetails, EdaPlacementDetails};
use makerpnp::loaders::{eda_placements, part_mappings, parts};
use makerpnp::part_mapper::{PartMapper, PartMapperError, PartMappingError, ProcessingResult};
use makerpnp::part_mapper::part_mapping::PartMapping;

#[derive(Parser)]
#[command(name = "variantbuilder")]
#[command(bin_name = "variantbuilder")]
#[command(version, about, long_about = None)]
struct Opts {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log", require_equals = true)]
    trace: Option<PathBuf>,

}

#[derive(Args, Clone, Debug)]
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

    configure_tracing(&opts)?;

    match &opts.command.unwrap() {
        Commands::Build { placements, assembly_variant, parts, part_mappings } => {
            build_assembly_variant(placements, assembly_variant, parts, part_mappings)?;
        },
    }

    Ok(())
}

fn configure_tracing(opts: &Opts) -> anyhow::Result<()> {
    const SUBSCRIBER_FAILED_MESSAGE: &'static str = "setting default subscriber failed";
    match &opts.trace {
        Some(path) => {
            //println!("using file_subscriber");
            let trace_file: File = File::create(path)?;

            let file_subscriber = FmtSubscriber::builder()
                .with_writer(trace_file)
                .with_max_level(Level::TRACE)
                .finish();

            tracing::subscriber::set_global_default(file_subscriber)
                .expect(SUBSCRIBER_FAILED_MESSAGE);
        },
        _ => {
            // FIXME currently overly verbose
            //println!("using stdout_subscriber");
            let stdout_subscriber = FmtSubscriber::builder()
                .event_format(fmt::format().compact())
                .with_level(false)
                .with_line_number(false)
                .with_span_events(FmtSpan::NONE)
                .without_time()
                .with_max_level(Level::INFO)
                .finish();

            tracing::subscriber::set_global_default(stdout_subscriber)
                .expect(SUBSCRIBER_FAILED_MESSAGE);
        }
    };

    Ok(())
}

#[tracing::instrument]
fn build_assembly_variant(placements_source: &String, assembly_variant_args: &AssemblyVariantArgs, parts_source: &String, part_mappings_source: &String) -> Result<(), Error> {

    let eda_placements = eda_placements::load_eda_placements(placements_source)?;
    info!("Loaded {} placements", eda_placements.len());

    let parts = parts::load_parts(parts_source)?;
    info!("Loaded {} parts", parts.len());

    let part_mappings = part_mappings::load_part_mappings(&parts, part_mappings_source)?;
    info!("Loaded {} part mappings", part_mappings.len());

    let assembly_variant = assembly_variant_args.build_assembly_variant()?;
    info!("Assembly variant: {}", assembly_variant.name);
    info!("Ref_des list: {}", assembly_variant.ref_des_list.join(", "));

    let assembly_variant_processor = AssemblyVariantProcessor::default();

    let result = assembly_variant_processor.process(&eda_placements, assembly_variant)?;
    let variant_placements = result.placements;
    let variant_placements_count = variant_placements.len();

    info!("Matched {} placements for assembly variant", variant_placements_count);

    trace!("{:?}", part_mappings);

    let processing_result = PartMapper::process(&variant_placements, &part_mappings);

    trace!("{:?}", processing_result);

    let matched_mappings = match &processing_result {
        Ok(mappings) => mappings,
        Err(PartMapperError::MappingErrors(mappings)) => mappings,
    };

    let tree = build_mapping_tree(matched_mappings);
    info!("{}", tree);

    match &processing_result {
        Ok(_) => (),
        Err(PartMapperError::MappingErrors(_)) => {
            error!("Mapping failures")
        }
    }

    Ok(())
}

fn build_mapping_tree(matched_mappings: &Vec<ProcessingResult>) -> Tree<String> {
    let mut tree = Tree::new("Mapping Result".to_string());

    for ProcessingResult { eda_placement, part_mappings: part_mappings_result } in matched_mappings.iter() {
        let placement_label = format!("{} ({})", eda_placement.ref_des, EdaPlacementTreeFormatter::format(&eda_placement.details));
        let mut placement_node = Tree::new(placement_label);

        fn add_error_node(placement_node: &mut Tree<String>) {
            let placement_error_node = Tree::new("ERROR: Unresolved mapping conflict.".to_string());
            placement_node.leaves.push(placement_error_node);
        }

        match part_mappings_result {
            Ok(part_mappings) => {
                add_mapping_nodes(part_mappings, &mut placement_node);
            },
            Err(PartMappingError::MultipleMatchingMappings(part_mappings)) => {
                add_mapping_nodes(part_mappings, &mut placement_node);
                add_error_node(&mut placement_node);
            },
            Err(PartMappingError::NoMappings) => {
                add_error_node(&mut placement_node);
            },
        }

        tree.leaves.push(placement_node)
    }

    tree
}

fn add_mapping_nodes(part_mappings: &Vec<&PartMapping>, placement_node: &mut Tree<String>) {
    for part_mapping in part_mappings.iter() {
        let part_label = format!("manufacturer: '{}', mpn: '{}'", part_mapping.part.manufacturer, part_mapping.part.mpn);

        let part_node = Tree::new(part_label);
        placement_node.leaves.push(part_node);
    }
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
