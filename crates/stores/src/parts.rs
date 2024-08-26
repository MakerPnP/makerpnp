use tracing::Level;
use anyhow::{Context, Error};
use std::path::PathBuf;
use tracing::trace;
use pnp::part::Part;
use crate::csv::PartRecord;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_parts(parts_source: &String) -> Result<Vec<Part>, Error> {
    let parts_path_buf = PathBuf::from(parts_source);
    let parts_path = parts_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(parts_path)
        .with_context(|| format!("Error reading parts. file: {}", parts_path.to_str().unwrap()))?;

    let mut parts: Vec<Part> = vec![];

    for result in csv_reader.deserialize() {
        let record: PartRecord = result
            .with_context(|| "Deserializing part record".to_string())?;

        trace!("{:?}", record);

        let part = record.build_part()
            .with_context(|| format!("Building part from record. record: {:?}", record))?;

        parts.push(part);
    }
    Ok(parts)
}