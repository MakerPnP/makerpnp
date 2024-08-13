use tracing::Level;
use anyhow::{Context, Error};
use std::path::PathBuf;
use tracing::trace;
use crate::stores::csv::PartMappingRecord;
use crate::pnp::part::Part;
use crate::part_mapper::part_mapping::PartMapping;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_part_mappings<'part>(parts: &'part Vec<Part>, part_mappings_source: &String) -> Result<Vec<PartMapping<'part>>, Error> {
    let part_mappings_path_buf = PathBuf::from(part_mappings_source);
    let part_mappings_path = part_mappings_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(part_mappings_path)
        .with_context(|| format!("Error reading part mappings. file: {}", part_mappings_path.to_str().unwrap()))?;

    let mut part_mappings: Vec<PartMapping> = vec![];

    for result in csv_reader.deserialize() {
        let record: PartMappingRecord = result
            .with_context(|| "Deserializing part mapping record".to_string())?;

        trace!("{:?}", record);

        let part_mapping = record.build_part_mapping(parts)
            .with_context(|| format!("Building part mapping from record. record: {:?}", record))?;

        part_mappings.push(part_mapping);
    }
    Ok(part_mappings)
}