//! The back end for data sources that are CSV files, on a native file system.
//!
//! The files are read with `std::fs`, which blocks the thread the future is polled on, as the files are small.

use anyhow::{anyhow, Context};
use sha2::{Digest, Sha256};
use stores_protocol::{Parts, PartsSource, PartsViewItem, Revision, Source};
use tracing::info;

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

/// The parts, and the revision of the file they were read from (`STORE-8`).
pub async fn read_parts(source: &PartsSource) -> anyhow::Result<Parts> {
    let Source::File(path) = source else {
        return Err(anyhow!("Unsupported source. source: {:?}", source));
    };

    info!("Loading parts. source: '{}'", source);
    // read once, so the revision is of the contents the parts were read from.
    let contents = std::fs::read(path).with_context(|| format!("Error reading parts. file: {}", path.display()))?;
    let revision = revision(&contents);

    let parts = stores::parts::read_parts(contents.as_slice())
        .with_context(|| format!("Error reading parts. file: {}", path.display()))?
        .into_iter()
        .map(|(part, fields)| PartsViewItem {
            part,
            fields: fields.into_iter().collect(),
        })
        .collect();

    Ok(Parts {
        revision,
        parts,
    })
}

/// The revision of a file's contents: their SHA-256 hash, so it's the same in every process, and changes if they do.
pub fn revision(contents: &[u8]) -> Revision {
    Revision(format!("{:x}", Sha256::digest(contents)))
}
