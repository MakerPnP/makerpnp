use std::path::PathBuf;
use anyhow::{bail, Error};
use tracing::trace;
use crate::loaders::csv::LoadOutItemRecord;
use crate::pnp::load_out_item::LoadOutItem;

#[tracing::instrument]
pub fn load_items(load_out_source: &String) -> Result<Vec<LoadOutItem>, Error>  {
    let load_out_path_buf = PathBuf::from(load_out_source);
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