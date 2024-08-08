use tracing::{info, Level};
use std::path::PathBuf;
use anyhow::{bail, Error};
use csv::QuoteStyle;
use tracing::trace;
use crate::loaders::csv::LoadOutItemRecord;
use crate::planning::LoadOutSource;
use crate::pnp::load_out_item::LoadOutItem;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_items(load_out_source: &LoadOutSource) -> Result<Vec<LoadOutItem>, Error>  {
    info!("Loading load-out. source: '{}'", load_out_source);
    
    let load_out_path_buf = PathBuf::from(load_out_source.to_string());
    let load_out_path = load_out_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(load_out_path)?;

    let mut items: Vec<LoadOutItem> = vec![];

    for result in csv_reader.deserialize() {
        let record: LoadOutItemRecord = result?;
        trace!("{:?}", record);

        if let Ok(load_out_item) = record.build_load_out_item() {
            items.push(load_out_item);
        } else {
            bail!("todo")
        }
    }
    Ok(items)
}

pub fn store_items(load_out_source: &LoadOutSource, items: &[LoadOutItem]) -> Result<(), Error> {

    let output_path = PathBuf::from(load_out_source.to_string());

    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_path(output_path)?;

    for item in items {
        writer.serialize(
            LoadOutItemRecord {
                reference: item.reference.to_string(),
                manufacturer: item.manufacturer.to_string(),
                mpn: item.mpn.to_string(),
            }
        )?;
    }
    
    writer.flush()?;

    Ok(())
}