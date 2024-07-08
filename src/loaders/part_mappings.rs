use anyhow::{bail, Error};
use std::path::PathBuf;
use crate::loaders::csv::{CSVPartMappingRecord, PartMappingRecord};
use crate::pnp::part::Part;
use crate::part_mapper::part_mapping::PartMapping;

pub fn load_part_mappings<'part>(parts: &'part Vec<Part>, part_mappings_source: &String) -> Result<Vec<PartMapping<'part>>, Error> {
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