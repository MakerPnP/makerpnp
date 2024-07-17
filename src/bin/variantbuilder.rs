use std::fmt::{Display, Formatter};
use std::fs::File;
use std::path::PathBuf;
use anyhow::Error;
use clap::{Args, Parser, Subcommand};
use csv::QuoteStyle;
use termtree::Tree;
use thiserror::Error;
use tracing::{error, info, Level, trace};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{fmt, FmtSubscriber};
use makerpnp::assembly::AssemblyVariantProcessor;
use makerpnp::eda::assembly_variant::AssemblyVariant;
use makerpnp::eda::eda_placement::{DipTracePlacementDetails, EdaPlacementDetails};
use makerpnp::eda::eda_substitution::{EdaSubstitutionResult, EdaSubstitutionRule, EdaSubstitutor};
use makerpnp::loaders::{eda_placements, load_out, part_mappings, parts, substitutions};
use makerpnp::part_mapper::{PartMapper, PartMapperError, PartMappingError, PartMappingResult, PlacementPartMappingResult};

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

// TODO rename 'FILE' to 'SOURCE' for all, and cleanup doc comments
#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
enum Commands {
    /// Build variant
    Build {
        /// Load-out file
        #[arg(long, value_name = "FILE")]
        load_out: Option<String>,

        /// Placements file
        #[arg(long, value_name = "FILE")]
        placements: String,

        /// Parts file
        #[arg(long, value_name = "FILE")]
        parts: String,

        /// Part-mappings file
        #[arg(long, value_name = "FILE")]
        part_mappings: String,

        /// Substitutions files
        #[arg(long, require_equals = true, value_delimiter = ',', num_args = 0.., value_name = "FILE")]
        substitutions: Vec<String>,

        /// Output file
        #[arg(long, value_name = "FILE")]
        output: String,

        #[command(flatten)]
        assembly_variant: AssemblyVariantArgs
    },
}

fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    configure_tracing(&opts)?;

    match &opts.command.unwrap() {
        Commands::Build {
            placements,
            assembly_variant,
            parts,
            part_mappings,
            substitutions,
            load_out,
            output,
        } => {
            build_assembly_variant(placements, assembly_variant, parts, part_mappings, substitutions, load_out, output)?;
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
fn build_assembly_variant(placements_source: &String, assembly_variant_args: &AssemblyVariantArgs, parts_source: &String, part_mappings_source: &String, eda_substitutions_sources: &[String], load_out_source: &Option<String>, output: &String) -> Result<(), Error> {

    let mut original_eda_placements = eda_placements::load_eda_placements(placements_source)?;
    info!("Loaded {} placements", original_eda_placements.len());

    let eda_substitution_rules = eda_substitutions_sources.iter().try_fold(vec![], | mut rules, source | {
        let source_rules = substitutions::load_eda_substitutions(source)?;
        info!("Loaded {} substitution rules from {}", source_rules.len(), source);
        rules.extend(source_rules);

        Ok::<Vec<EdaSubstitutionRule>, anyhow::Error>(rules)
    })?;

    let eda_substitution_results = EdaSubstitutor::substitute(original_eda_placements.as_mut_slice(), eda_substitution_rules.as_slice());
    trace!("eda_substitution_results: {:?}", eda_substitution_results);

    let eda_placements = eda_substitution_results.iter().map(|esr|esr.resulting_placement.clone()).collect();

    let parts = parts::load_parts(parts_source)?;
    info!("Loaded {} parts", parts.len());

    let part_mappings = part_mappings::load_part_mappings(&parts, part_mappings_source)?;
    info!("Loaded {} part mappings", part_mappings.len());

    let load_out_items = match load_out_source {
        Some(source) => load_out::load_items(source),
        None => Ok(vec![]),
    }?;

    info!("Loaded {} load-out items", load_out_items.len());

    let assembly_variant = assembly_variant_args.build_assembly_variant()?;
    info!("Assembly variant: {}", assembly_variant.name);
    info!("Ref_des list: {}", assembly_variant.ref_des_list.join(", "));

    let assembly_variant_processor = AssemblyVariantProcessor::default();

    let result = assembly_variant_processor.process(&eda_placements, assembly_variant)?;
    let variant_placements = result.placements;
    let variant_placements_count = variant_placements.len();

    info!("Matched {} placements for assembly variant", variant_placements_count);

    trace!("{:?}", part_mappings);

    let processing_result = PartMapper::process(&variant_placements, &part_mappings, &load_out_items);

    trace!("{:?}", processing_result);

    let matched_mappings = match &processing_result {
        Ok(mappings) => mappings,
        Err(PartMapperError::MappingErrors(mappings)) => mappings,
    };

    let tree = build_mapping_tree(matched_mappings, eda_substitution_results);
    info!("{}", tree);

    match &processing_result {
        Ok(_) => (),
        Err(PartMapperError::MappingErrors(_)) => {
            error!("Mapping failures")
        }
    }

    write_output_csv(output, matched_mappings)?;

    Ok(())
}

fn write_output_csv(output_file_name: &String, matched_mappings: &Vec<PlacementPartMappingResult>) -> anyhow::Result<()> {

    let output_path = PathBuf::from(output_file_name);

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct Record {
        ref_des: String,
        manufacturer: String,
        mpn: String,
    }

    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_path(output_path)?;

    for matched_mapping in matched_mappings.iter() {
        match matched_mapping {
            PlacementPartMappingResult {
                eda_placement, part, .. } if part.is_some() => {

                let part = part.unwrap();

                writer.serialize(Record {
                    ref_des: eda_placement.ref_des.clone(),
                    manufacturer: part.manufacturer.clone(),
                    mpn: part.mpn.clone(),
                })?;
            },
            _ => (),
        }
    }

    writer.flush()?;

    Ok(())
}

fn build_mapping_tree(matched_mappings: &Vec<PlacementPartMappingResult>, eda_substitution_results: Vec<EdaSubstitutionResult>) -> Tree<String> {
    let mut tree = Tree::new("Mapping Result".to_string());

    for PlacementPartMappingResult { eda_placement, mapping_result: part_mappings_result, .. } in matched_mappings.iter() {

        fn add_error_node(placement_node: &mut Tree<String>, reason: &str) {
            let placement_error_node = Tree::new(format!("ERROR: Unresolved mapping - {}.", reason).to_string());
            placement_node.leaves.push(placement_error_node);
        }

        if let Some(substitution_result) = eda_substitution_results.iter().find(|candidate|{
            candidate.original_placement.ref_des.eq(&eda_placement.ref_des)
        }) {
            let placement_label = format!("{} ({})", eda_placement.ref_des, EdaPlacementTreeFormatter::format(&substitution_result.original_placement.details));
            let mut placement_node = Tree::new(placement_label);

            let mut parent = &mut placement_node;

            for chain_entry in substitution_result.chain.iter() {
                let substitution_label = format!("Substituted ({}), by ({})",
                     chain_entry.rule.format_change(),
                     chain_entry.rule.format_criteria(),
                );

                let substitution_node = Tree::new(substitution_label);
                parent.leaves.push(substitution_node);
                parent = parent.leaves.last_mut().unwrap();
            }

            match part_mappings_result {
                Ok(part_mapping_results) => {
                    add_mapping_nodes(part_mapping_results, parent);
                }
                Err(PartMappingError::ConflictingRules(part_mapping_results)) => {
                    add_mapping_nodes(part_mapping_results, parent);
                    add_error_node(parent, "Conflicting rules");
                },
                Err(PartMappingError::NoRulesApplied(part_mapping_results)) => {
                    add_mapping_nodes(part_mapping_results, parent);
                    add_error_node(parent, "No rules applied");
                },
                Err(PartMappingError::NoMappings) => {
                    add_error_node(parent, "No mappings found");
                },
            }

            tree.leaves.push(placement_node)
        };

    }

    tree
}

fn add_mapping_nodes(part_mapping_results: &Vec<PartMappingResult>, placement_node: &mut Tree<String>) {
    for PartMappingResult { part_mapping, applied_rule } in part_mapping_results.iter() {
        let part_chunk = format!("manufacturer: '{}', mpn: '{}'", part_mapping.part.manufacturer, part_mapping.part.mpn);
        let mut chunks = vec![part_chunk];

        if let Some(rule) = applied_rule {
            let rule_chunk = format!("({})", rule);
            chunks.push(rule_chunk);
        }

        let part_label = chunks.join(" ");

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
