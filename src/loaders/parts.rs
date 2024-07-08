use anyhow::{bail, Error};
use std::path::PathBuf;
use crate::loaders::csv::PartRecord;
use crate::pnp::part::Part;
pub fn load_parts(parts_source: &String) -> Result<Vec<Part>, Error> {
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