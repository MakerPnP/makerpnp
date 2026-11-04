//! The back end for documents that are files, on a native file system (`STORE-10`).
//!
//! A document is read and written as it is: the core that keeps it knows its format. The files are read and written with
//! `std::fs`, which blocks the thread the future is polled on, as the files are small.

use std::io::ErrorKind;
use std::path::Path;

use stores_protocol::{Document, Revision};

use crate::csv_file::revision;

/// The document, or `None` if there's no file at the path.
pub async fn read(path: &Path) -> Result<Option<Document>, String> {
    let contents = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Unable to read the document. path: {:?}, cause: '{}'",
                path, error
            ))
        }
    };
    // read once, so the revision is of the contents read.
    let revision = revision(&contents);
    let contents = String::from_utf8(contents)
        .map_err(|error| format!("Unable to read the document. path: {:?}, cause: '{}'", path, error))?;

    Ok(Some(Document {
        revision,
        contents,
    }))
}

/// Writes the document, replacing the file if there is one, and returns the revision written.
pub async fn write(path: &Path, contents: &str) -> Result<Revision, String> {
    std::fs::write(path, contents)
        .map_err(|error| format!("Unable to write the document. path: {:?}, cause: '{}'", path, error))?;

    Ok(revision(contents.as_bytes()))
}
