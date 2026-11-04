//! The back end for data sources that are CSV files, on a native file system.
//!
//! The files are read with `std::fs`, which blocks the thread the future is polled on, as the files are small.

use anyhow::anyhow;
use stores_protocol::{PartsSource, PartsViewItem, Source};

/// For a file that exists, its canonical path, without the `\\?\` prefix Windows adds where it isn't needed, otherwise
/// the source as it is, so that an error names it as the user did (`STORE-6`).
///
/// For example `d:\parts\Parts.csv` and `D:\parts\..\parts\parts.csv` are the same file on Windows, so the same data
/// source.
pub fn resolve(source: &Source) -> Source {
    match source {
        Source::File(path) => dunce::canonicalize(path)
            .map(Source::File)
            .unwrap_or_else(|_| source.clone()),
        _ => source.clone(),
    }
}

pub async fn read_parts(source: &PartsSource) -> anyhow::Result<Vec<PartsViewItem>> {
    let parts = match source {
        Source::File(_) => stores::parts::load_parts(source)?,
        _ => return Err(anyhow!("Unsupported source. source: {:?}", source)),
    };

    Ok(parts
        .into_iter()
        .map(|(part, fields)| PartsViewItem {
            part,
            fields: fields.into_iter().collect(),
        })
        .collect())
}
