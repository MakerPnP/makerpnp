use tracing::{info, Level};
use std::path::PathBuf;
use anyhow::{bail, Error};
use csv::QuoteStyle;
use tracing::trace;
use std::fs::File;
use std::str::FromStr;
use std::fmt::{Display, Formatter};
use crate::stores::csv::LoadOutItemRecord;
use crate::pnp::load_out::LoadOutItem;

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

pub fn ensure_load_out(load_out_source: &LoadOutSource) -> anyhow::Result<()> {
    let load_out_path_buf = PathBuf::from(load_out_source.to_string());
    let load_out_path = load_out_path_buf.as_path();
    if !load_out_path.exists() {
        File::create(&load_out_path)?;    
        info!("Created load-out. source: '{}'", load_out_source);
    }
    
    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadOutSource(String);

impl FromStr for LoadOutSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(LoadOutSource(s.to_string()))
    }
}

impl Display for LoadOutSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}