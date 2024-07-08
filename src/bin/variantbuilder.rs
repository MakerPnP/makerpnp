use std::path::PathBuf;
use anyhow::{bail, Error};
use clap::{Args, Parser, Subcommand};
use thiserror::Error;
use makerpnp::assembly::{AssemblyVariant, AssemblyVariantProcessor};
use makerpnp::part::Part;
use makerpnp::part_mapper::criteria::diptrace::ExactMatchCriteria;
use makerpnp::part_mapper::criteria::PlacementMappingCriteria;
use makerpnp::part_mapper::part_mapping::PartMapping;
use makerpnp::part_mapper::PartMapper;
use makerpnp::placement::eda::{DipTracePlacementDetails, EdaPlacement, EdaPlacementDetails};

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

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
struct DiptracePlacementRecord {
    ref_des: String,
    name: String,
    value: String,
}

#[derive(Error, Debug)]
enum DiptracePlacementRecordError {
    #[error("Unknown")]
    Unknown
}

impl DiptracePlacementRecord {
    pub fn build_eda_placement(&self) -> Result<EdaPlacement, DiptracePlacementRecordError> {
        Ok(EdaPlacement {
            ref_des: self.ref_des.to_string(),
            details: EdaPlacementDetails::DipTrace(DipTracePlacementDetails {
                name: self.name.to_string(),
                value: self.value.to_string(),
            })
        })

        // _ => Err(DiptracePlacementRecordError::Unknown)
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
struct PartRecord {
    manufacturer: String,
    mpn: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
struct CSVPartMappingRecord {
    eda: String,
    name: String,
    value: String,
    manufacturer: String,
    mpn: String,
}

enum PartMappingRecord {
    DipTracePartMapping(DipTracePartMappingRecord),
    //KiCadPartMapping(KiCadPartMappingRecord),
}

#[derive(Error, Debug)]
enum CSVPartMappingRecordError {
    #[error("Unknown EDA: '{eda:?}'")]
    UnknownEDA { eda: String }
}

impl TryFrom<CSVPartMappingRecord> for PartMappingRecord {
    type Error = CSVPartMappingRecordError;

    fn try_from(value: CSVPartMappingRecord) -> Result<Self, Self::Error> {
        match value.eda.as_str() {
            "DipTrace" => Ok(PartMappingRecord::DipTracePartMapping(DipTracePartMappingRecord {
                name: value.name.to_string(),
                value: value.value.to_string(),
                manufacturer: value.manufacturer.to_string(),
                mpn: value.mpn.to_string(),
            })),
            _ => Err(CSVPartMappingRecordError::UnknownEDA { eda: value.eda }),
        }
    }
}

impl PartMappingRecord {
    pub fn build_part_mapping<'part>(&self, parts: &'part Vec<Part>) -> Result<PartMapping<'part>, ()> {

        let part_criteria: Part = match self {
            PartMappingRecord::DipTracePartMapping(r) => Ok(Part { manufacturer: r.manufacturer.clone(), mpn: r.mpn.clone() }),
            _ => Err(()) // TODO - unable to build part criteria
        }?;

        let matched_part_ref = parts.iter().find_map(|part| {
            match part.eq(&part_criteria) {
                true => Some(part),
                false => None,
            }
        });

        let part_ref = match matched_part_ref {
            Some(part) => Ok(part),
            _ => Err(()) // TODO
        }?;

        let criterion = match self {
            PartMappingRecord::DipTracePartMapping(record) => {
                Ok(ExactMatchCriteria::new(record.name.clone(), record.value.clone()))
            }
        }?;


        let criteria: Vec<Box<dyn PlacementMappingCriteria>> = vec![Box::new(criterion)];

        let part_mapping = PartMapping::new(part_ref, criteria);

        Ok(part_mapping)
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
struct DipTracePartMappingRecord {
    // from
    name: String,
    value: String,

    // to
    manufacturer: String,
    mpn: String,
}

impl PartRecord {
    pub fn build_part(&self) -> Result<Part, ()> {
        Ok(Part {
            manufacturer: self.manufacturer.clone(),
            mpn: self.mpn.clone(),
        })
    }
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

    let eda_placements = load_eda_placements(placements_source)?;
    println!("Loaded {} placements", eda_placements.len());

    let parts = load_parts(parts_source)?;
    println!("Loaded {} parts", parts.len());

    let part_mappings = load_part_mappings(&parts, part_mappings_source)?;
    println!("Loaded {} part mappings", part_mappings.len());

    let assembly_variant = assembly_variant_args.build_assembly_variant()?;
    println!("Assembly variant: {}", assembly_variant.name);
    println!("Ref_des list: {}", assembly_variant.ref_des_list.join(", "));

    let assembly_variant_processor = AssemblyVariantProcessor::default();

    // when
    let result = assembly_variant_processor.process(&eda_placements, assembly_variant)?;
    let variant_placements = result.placements;
    let variant_placements_count = variant_placements.len();

    println!("Matched {} placements", variant_placements_count);

    println!("{:?}", part_mappings);

    let matched_mappings = PartMapper::process(&variant_placements, &part_mappings);

    println!("{:?}", matched_mappings);

    let matched_placement_count = matched_mappings.len();
    println!("Mapped {} placements to {} parts\n", variant_placements_count, matched_placement_count);

    Ok(())
}

fn load_eda_placements(placements_source: &String) -> Result<Vec<EdaPlacement>, Error> {
    let placements_path_buf = PathBuf::from(placements_source);
    let placements_path = placements_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(placements_path)?;

    let mut placements: Vec<EdaPlacement> = vec![];

    for result in csv_reader.deserialize() {
        let record: DiptracePlacementRecord = result?;
        // TODO output the record in verbose mode
        //println!("{:?}", record);

        if let Ok(placement) = record.build_eda_placement() {
            placements.push(placement);
        } else {
            bail!("todo")
        }
    }
    Ok(placements)
}

fn load_parts(parts_source: &String) -> Result<Vec<Part>, Error> {
    let parts_path_buf = PathBuf::from(parts_source);
    let parts_path = parts_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(parts_path)?;

    let mut parts: Vec<Part> = vec![];

    for result in csv_reader.deserialize() {
        let record: PartRecord = result?;
        // TODO output the record in verbose mode
        //println!("{:?}", record);

        if let Ok(part) = record.build_part() {
            parts.push(part);
        } else {
            bail!("todo")
        }
    }
    Ok(parts)
}

fn load_part_mappings<'part>(parts: &'part Vec<Part>, part_mappings_source: &String) -> Result<Vec<PartMapping<'part>>, Error> {
    let part_mappings_path_buf = PathBuf::from(part_mappings_source);
    let part_mappings_path = part_mappings_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(part_mappings_path)?;

    let mut part_mappings: Vec<PartMapping> = vec![];

    for result in csv_reader.deserialize() {
        let record: CSVPartMappingRecord = result?;
        // TODO output the record in verbose mode
        //println!("{:?}", record);

        let enum_record = PartMappingRecord::try_from(record)?;

        if let Ok(part_mapping) = enum_record.build_part_mapping(parts) {
            part_mappings.push(part_mapping);
        } else {
            bail!("todo")
        }
    }
    Ok(part_mappings)
}
