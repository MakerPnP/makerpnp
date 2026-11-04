//! The stores service's document file back end (`STORE-8`, `STORE-10`).

use std::fs;
use std::path::PathBuf;

use futures::executor::block_on;
use stores_protocol::{Document, Revision, StoresOperation, StoresOutput};
use stores_service::StoresService;

fn read(path: &PathBuf) -> Result<Option<Document>, String> {
    match block_on(StoresService::new().perform_once(StoresOperation::ReadDocument {
        path: path.clone(),
    })) {
        Some(StoresOutput::DocumentRead(result)) => result,
        output => panic!("unexpected output: {:?}", output),
    }
}

fn write(path: &PathBuf, contents: &str) -> Result<Revision, String> {
    match block_on(StoresService::new().perform_once(StoresOperation::WriteDocument {
        path: path.clone(),
        contents: contents.to_string(),
    })) {
        Some(StoresOutput::DocumentWritten(result)) => result,
        output => panic!("unexpected output: {:?}", output),
    }
}

/// `STORE-10`, a document is written and read as it is, and its revision is that of the contents written.
#[test]
fn writes_and_reads_a_document_as_it_is() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("project.mpnp.json");

    let revision = write(&path, "{\n    \"name\": \"job1\"\n}\n").expect("written");

    assert_eq!(fs::read_to_string(&path).unwrap(), "{\n    \"name\": \"job1\"\n}\n");
    assert_eq!(
        read(&path),
        Ok(Some(Document {
            revision,
            contents: "{\n    \"name\": \"job1\"\n}\n".to_string(),
        }))
    );
}

/// `STORE-10`, writing replaces the document, and `STORE-8`, its revision changes only if its contents do.
#[test]
fn writing_replaces_the_document() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("project.mpnp.json");

    let first = write(&path, "first\n").unwrap();
    assert_eq!(write(&path, "first\n").unwrap(), first);
    let second = write(&path, "second\n").unwrap();

    assert_ne!(second, first);
    assert_eq!(read(&path).unwrap().unwrap().contents, "second\n");
    assert_eq!(read(&path).unwrap().unwrap().revision, second);
}

/// `STORE-10`, a document that doesn't exist isn't an error, so a core can tell it apart from one it can't read.
#[test]
fn a_document_that_does_not_exist_is_none() {
    let folder = tempfile::tempdir().unwrap();

    assert_eq!(read(&folder.path().join("missing.json")), Ok(None));
}

/// `STORE-10`, a document that can't be written says why, and names its path.
#[test]
fn says_a_document_that_can_not_be_written_is_not_written() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder
        .path()
        .join("missing_folder")
        .join("project.mpnp.json");

    let error = write(&path, "{}\n").expect_err("an error");

    assert!(error.starts_with("Unable to write the document. path: "), "{error}");
    assert!(error.contains("missing_folder"), "{error}");
}
