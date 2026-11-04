use std::collections::{BTreeMap, BTreeSet, HashMap};

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

    let file = std::fs::File::open(&path).with_context(|| format!("Error reading parts. file: {}", path.display()))?;

    read_parts(file).with_context(|| format!("Error reading parts. file: {}", path.display()))
}

/// Reads parts, as [`load_parts`] does, from CSV text, wherever it came from, so that it does no IO of its own, and can
/// be used by any back end, for example one that has the text from a browser's storage.
pub fn read_parts(reader: impl std::io::Read) -> Result<Vec<(Part, HashMap<String, String>)>, Error> {
    let mut csv_reader = csv::ReaderBuilder::new().from_reader(reader);

    let headers = csv_reader
        .headers()
        .with_context(|| "Error reading the headers of the parts.".to_string())?
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

/// The headers of the parts' CSV text, in order, so that the parts can be written with them in the same order.
pub fn read_headers(reader: impl std::io::Read) -> Result<Vec<String>, Error> {
    let mut csv_reader = csv::ReaderBuilder::new().from_reader(reader);
    let headers = csv_reader
        .headers()
        .with_context(|| "Error reading the headers of the parts.".to_string())?;
    Ok(headers
        .iter()
        .map(str::to_string)
        .collect())
}

/// Writes the parts as CSV, every value quoted, as [`read_parts`] reads them: the headers `Manufacturer` and `Mpn`,
/// then the parts' other values' names. Those in `headers`, for example a file's headers before the change, keep their
/// order, so a file changed has its columns as they were, and any others follow, in the order of their names. A part
/// with no value of a name has an empty one.
pub fn write_parts(
    writer: impl std::io::Write,
    headers: &[String],
    parts: &[(Part, BTreeMap<String, String>)],
) -> Result<(), Error> {
    let mut field_names: Vec<String> = headers
        .iter()
        .filter(|header| *header != MANUFACTURER_HEADER && *header != MPN_HEADER)
        .cloned()
        .collect();
    let others: BTreeSet<&String> = parts
        .iter()
        .flat_map(|(_part, fields)| fields.keys())
        .filter(|name| !field_names.contains(name))
        .collect();
    field_names.extend(others.into_iter().cloned());

    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .from_writer(writer);

    let mut record = vec![MANUFACTURER_HEADER.to_string(), MPN_HEADER.to_string()];
    record.extend(field_names.iter().cloned());
    writer.write_record(&record)?;

    for (part, fields) in parts.iter() {
        let mut record = vec![part.manufacturer.clone(), part.mpn.clone()];
        record.extend(field_names.iter().map(|name| {
            fields
                .get(name)
                .cloned()
                .unwrap_or_default()
        }));
        writer.write_record(&record)?;
    }
    writer.flush()?;

    Ok(())
}
