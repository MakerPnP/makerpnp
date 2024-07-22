use anyhow::{bail, Error};
use std::path::PathBuf;
use tracing::trace;
use crate::loaders::csv::PartMappingRecord;
use crate::pnp::part::Part;
use crate::part_mapper::part_mapping::PartMapping;

#[tracing::instrument]
pub fn load_part_mappings<'part>(parts: &'part Vec<Part>, part_mappings_source: &String) -> Result<Vec<PartMapping<'part>>, Error> {
    let part_mappings_path_buf = PathBuf::from(part_mappings_source);
    let part_mappings_path = part_mappings_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(part_mappings_path)?;

    let mut part_mappings: Vec<PartMapping> = vec![];

    for result in csv_reader.deserialize() {
        let record: PartMappingRecord = result?;
        trace!("{:?}", record);

        if let Ok(part_mapping) = record.build_part_mapping(parts) {
            part_mappings.push(part_mapping);
        } else {
            bail!("todo")
        }
    }
    Ok(part_mappings)
}