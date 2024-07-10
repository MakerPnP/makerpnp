use anyhow::{bail, Error};
use std::path::PathBuf;
use tracing::trace;
use crate::eda::diptrace::csv::DiptracePlacementRecord;
use crate::eda::eda_placement::EdaPlacement;

#[tracing::instrument]
pub fn load_eda_placements(placements_source: &String) -> Result<Vec<EdaPlacement>, Error> {
    let placements_path_buf = PathBuf::from(placements_source);
    let placements_path = placements_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(placements_path)?;

    let mut placements: Vec<EdaPlacement> = vec![];

    for result in csv_reader.deserialize() {
        let record: DiptracePlacementRecord = result?;
        trace!("{:?}", record);

        if let Ok(placement) = record.build_eda_placement() {
            placements.push(placement);
        } else {
            bail!("todo")
        }
    }
    Ok(placements)
}