use tracing::Level;
use anyhow::{bail, Error};
use std::path::PathBuf;
use tracing::trace;
use crate::eda::diptrace::csv::DiptracePlacementRecord;
use crate::eda::eda_placement::EdaPlacement;
use crate::eda::EdaTool;
use crate::eda::kicad::csv::KiCadPlacementRecord;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_eda_placements(eda_tool: EdaTool, placements_source: &String) -> Result<Vec<EdaPlacement>, Error> {
    let placements_path_buf = PathBuf::from(placements_source);
    let placements_path = placements_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(placements_path)?;

    let mut placements: Vec<EdaPlacement> = vec![];

    match eda_tool {
        EdaTool::DipTrace => {
            for result in csv_reader.deserialize() {
                let record: DiptracePlacementRecord = result?;
                trace!("{:?}", record);

                if let Ok(placement) = record.build_eda_placement() {
                    placements.push(placement);
                } else {
                    bail!("todo")
                }
            }
        },
        EdaTool::KiCad => {
            for result in csv_reader.deserialize() {
                let record: KiCadPlacementRecord = result?;
                trace!("{:?}", record);

                if let Ok(placement) = record.build_eda_placement() {
                    placements.push(placement);
                } else {
                    bail!("todo")
                }
            }
        }
    }
    Ok(placements)
}