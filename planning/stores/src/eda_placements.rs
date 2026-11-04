use anyhow::{anyhow, Context, Error};
use eda::diptrace::csv::DiptracePlacementRecord;
use eda::easyeda::csv::EasyEdaPlacementRecord;
use eda::kicad::csv::KiCadPlacementRecord;
use eda::placement::EdaPlacement;
use eda::EdaTool;
use tracing::Level;
use tracing::{info, trace};
use util::source::Source;

pub type EdaPlacementsSource = Source;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_eda_placements(eda_tool: EdaTool, source: &EdaPlacementsSource) -> Result<Vec<EdaPlacement>, Error> {
    info!("Loading eda placements. source: {}", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let file =
        std::fs::File::open(&path).with_context(|| format!("Error reading placements. file: {}", path.display()))?;

    read_eda_placements(eda_tool, file).with_context(|| format!("Error reading placements. file: {}", path.display()))
}

/// The placements, from the EDA tool's CSV, whose delimiter depends on the tool.
pub fn read_eda_placements(eda_tool: EdaTool, reader: impl std::io::Read) -> Result<Vec<EdaPlacement>, Error> {
    let mut csv_reader_builder = csv::ReaderBuilder::new();

    // TODO consider moving the creation of the CSV reader builder into the EdaTool specific modules.
    let csv_reader_builder = match eda_tool {
        EdaTool::EasyEda => {
            csv_reader_builder
                //.flexible(true)
                .delimiter(b'\t')
        }
        _ => &mut csv_reader_builder,
    };

    let mut csv_reader = csv_reader_builder.from_reader(reader);

    let mut placements: Vec<EdaPlacement> = vec![];

    match eda_tool {
        EdaTool::DipTrace => {
            for result in csv_reader.deserialize() {
                let record: DiptracePlacementRecord =
                    result.with_context(|| "Deserializing placement record".to_string())?;

                trace!("{:?}", record);

                let placement = record
                    .build_eda_placement()
                    .with_context(|| format!("Building placement from record. record: {:?}", record))?;

                placements.push(placement);
            }
        }
        EdaTool::KiCad => {
            for result in csv_reader.deserialize() {
                let record: KiCadPlacementRecord =
                    result.with_context(|| "Deserializing placement record".to_string())?;

                trace!("{:?}", record);

                let placement = record
                    .build_eda_placement()
                    .with_context(|| format!("Building placement from record. record: {:?}", record))?;

                placements.push(placement);
            }
        }
        EdaTool::EasyEda => {
            for result in csv_reader.deserialize() {
                let record: EasyEdaPlacementRecord =
                    result.with_context(|| "Deserializing placement record".to_string())?;

                trace!("{:?}", record);

                let placement = record
                    .build_eda_placement()
                    .with_context(|| format!("Building placement from record. record: {:?}", record))?;

                placements.push(placement);
            }
        }
    }
    Ok(placements)
}
