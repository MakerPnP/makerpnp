use std::collections::HashMap;

use anyhow::{anyhow, Context, Error};
use pnp::part::Part;
use tracing::Level;
use tracing::{info, trace};
use util::source::Source;

pub type PartsSource = Source;

const MANUFACTURER_HEADER: &str = "Manufacturer";
const MPN_HEADER: &str = "Mpn";

/// Loads the parts, and each part's other values by header, for example `"LCSC"`.
///
/// Every value is read as text, as it is in the file, so that a value that looks like a number, such as an MPN of
/// `"1234"` or a package of `"0402"`, is kept as it is (`ISSUE-23`).
#[tracing::instrument(level = Level::DEBUG)]
pub fn load_parts(source: &PartsSource) -> Result<Vec<(Part, HashMap<String, String>)>, Error> {
    info!("Loading parts. source: '{}'", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(path.clone())
        .with_context(|| format!("Error reading parts. file: {}", path.display()))?;

    let headers = csv_reader
        .headers()
        .with_context(|| format!("Error reading the headers of the parts. file: {}", path.display()))?
        .clone();

    let index_of = |name: &str| {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| {
                anyhow!(
                    "Missing header. header: '{}', headers: {:?}",
                    name,
                    headers.iter().collect::<Vec<_>>()
                )
            })
    };
    let manufacturer_index = index_of(MANUFACTURER_HEADER)?;
    let mpn_index = index_of(MPN_HEADER)?;

    let mut parts: Vec<(Part, HashMap<String, String>)> = vec![];

    for result in csv_reader.records() {
        let record = result.with_context(|| "Reading part record".to_string())?;

        trace!("{:?}", record);

        let value = |index: usize| {
            record
                .get(index)
                .unwrap_or_default()
                .to_string()
        };

        let part = Part::new(value(manufacturer_index), value(mpn_index));
        let meta_data: HashMap<String, String> = headers
            .iter()
            .enumerate()
            .filter(|(index, _header)| *index != manufacturer_index && *index != mpn_index)
            .map(|(index, header)| (header.to_string(), value(index)))
            .collect();

        parts.push((part, meta_data));
    }
    Ok(parts)
}
