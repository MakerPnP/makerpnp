use std::collections::BTreeSet;
use std::fs::File;

use anyhow::{anyhow, Context, Error};
use csv::QuoteStyle;
use planning::process::{ProcessDefinition, ProcessReference, ProcessRuleReference};
use pnp::load_out::LoadOutItem;
use pnp::part::Part;
use pnp::reference::Reference;
use regex::Regex;
use thiserror::Error;
use tracing::trace;
use tracing::{info, Level};
use util::source::Source;

use crate::csv::LoadOutItemRecord;

pub type LoadOutSource = Source;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_items(source: &LoadOutSource) -> Result<Vec<LoadOutItem>, Error> {
    info!("Loading load-out. source: '{}'", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let file = File::open(&path).with_context(|| format!("Error reading load-out. file: {}", path.display()))?;
    read_items(file).with_context(|| format!("Error reading load-out. file: {}", path.display()))
}

/// A load-out's items, from its CSV.
pub fn read_items(reader: impl std::io::Read) -> Result<Vec<LoadOutItem>, Error> {
    let mut csv_reader = csv::ReaderBuilder::new().from_reader(reader);

    let mut items: Vec<LoadOutItem> = vec![];

    for result in csv_reader.deserialize() {
        let record: LoadOutItemRecord = result.with_context(|| "Deserializing load-out record".to_string())?;

        trace!("{:?}", record);

        let load_out_item = record
            .build_load_out_item()
            .with_context(|| format!("Building load-out from record. record: {:?}", record))?;

        items.push(load_out_item);
    }
    Ok(items)
}

/// A load-out's items, as its CSV.
pub fn write_items(writer: impl std::io::Write, items: &[LoadOutItem]) -> Result<(), Error> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_writer(writer);

    for item in items {
        writer.serialize(LoadOutItemRecord {
            reference: item.reference.clone(),
            manufacturer: item.manufacturer.to_string(),
            mpn: item.mpn.to_string(),
        })?;
    }

    writer.flush()?;

    Ok(())
}

/// Adds each part that isn't in the load-out, with no feeder.
pub fn add_parts(load_out_items: &mut Vec<LoadOutItem>, parts: &BTreeSet<Part>) {
    for part in parts.iter() {
        trace!("Checking for part in load_out. part: {:?}", part);

        let matched = pnp::load_out::find_load_out_item_by_part(load_out_items, part);

        if matched.is_some() {
            continue;
        }

        let load_out_item = LoadOutItem {
            reference: None,
            manufacturer: part.manufacturer.clone(),
            mpn: part.mpn.clone(),
        };

        info!("Adding part to load_out. part: {:?}", part);
        load_out_items.push(load_out_item)
    }
}

#[derive(Error, Debug)]
pub enum FeederAssignmentError {
    #[error("No matching part; patterns must match exactly one part. manufacturer: {manufacturer}, mpn: {mpn}")]
    NoMatchingPart { manufacturer: Regex, mpn: Regex },

    #[error("Multiple matching parts; patterns must match exactly one part for the process. process: {process}, manufacturer: {manufacturer}, mpn: {mpn}")]
    MultipleMatchingParts {
        process: ProcessReference,
        manufacturer: Regex,
        mpn: Regex,
    },
}

/// Assigns the feeder to the items whose parts match the patterns, or removes theirs, and returns their parts.
pub fn assign_feeder(
    load_out_items: &mut [LoadOutItem],
    process: &ProcessDefinition,
    feeder_reference: &Option<Reference>,
    manufacturer: &Regex,
    mpn: &Regex,
) -> Result<Vec<Part>, FeederAssignmentError> {
    let mut parts: Vec<Part> = vec![];

    let mut items: Vec<_> = load_out_items
        .iter_mut()
        .filter(|item| manufacturer.is_match(&item.manufacturer) && mpn.is_match(&item.mpn))
        .collect();

    if items.is_empty() {
        return Err(FeederAssignmentError::NoMatchingPart {
            manufacturer: manufacturer.clone(),
            mpn: mpn.clone(),
        });
    }

    if process.has_rule(&ProcessRuleReference::from_raw_str("core::unique_feeder_references")) && items.len() > 1 {
        return Err(FeederAssignmentError::MultipleMatchingParts {
            process: process.reference.clone(),
            manufacturer: manufacturer.clone(),
            mpn: mpn.clone(),
        });
    }

    for item in items.iter_mut() {
        let part = Part {
            manufacturer: item.manufacturer.clone(),
            mpn: item.mpn.clone(),
        };

        item.reference = feeder_reference.clone();

        parts.push(part);
    }

    for part in parts.iter() {
        match &feeder_reference {
            Some(reference) => {
                info!(
                    "Assigned feeder to load-out item. feeder: {}, part: {:?}",
                    reference, part
                );
            }
            None => {
                info!("Removed feeder from load-out item. part: {:?}", part);
            }
        }
    }

    Ok(parts)
}
