//! The stores service's CSV file back end (`STORE-1`, `STORE-3`, `STORE-6` to `STORE-8`, `STORE-15`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use futures::executor::block_on;
use stores_protocol::{
    Part, Parts, PartsSource, PartsViewItem, PartsWriteError, Revision, StoresOperation, StoresOutput,
};
use stores_service::StoresService;

fn csv_file(folder: &Path, name: &str, content: &str) -> PartsSource {
    let path = folder.join(name);
    fs::write(&path, content).unwrap();
    PartsSource::File(path)
}

fn item(manufacturer: &str, mpn: &str, fields: &[(&str, &str)]) -> PartsViewItem {
    PartsViewItem {
        part: Part::new(manufacturer.to_string(), mpn.to_string()),
        fields: fields
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// The parts read, without their revision.
fn read_parts(source: &PartsSource) -> Result<Vec<PartsViewItem>, String> {
    read(source).map(|parts| parts.parts)
}

fn read(source: &PartsSource) -> Result<Parts, String> {
    match block_on(StoresService::new().perform_once(StoresOperation::ReadParts {
        source: source.clone(),
    })) {
        Some(StoresOutput::PartsRead(result)) => result,
        output => panic!("unexpected output: {:?}", output),
    }
}

fn resolve(source: &PartsSource) -> PartsSource {
    match block_on(StoresService::new().perform_once(StoresOperation::ResolveSource {
        source: source.clone(),
    })) {
        Some(StoresOutput::SourceResolved {
            source,
        }) => source,
        output => panic!("unexpected output: {:?}", output),
    }
}

/// `STORE-1`
#[test]
fn reads_the_parts_in_order_with_their_other_values() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\",\"LCSC\",\"Notes\"\n\
         \"RESCO\",\"RES1\",\"C1234\",\"\"\n\
         \"CAPCO\",\"CAP1\",\"C5678\",\"note\"\n",
    );

    assert_eq!(
        read_parts(&source),
        Ok(vec![
            item("RESCO", "RES1", &[("LCSC", "C1234"), ("Notes", "")]),
            item("CAPCO", "CAP1", &[("LCSC", "C5678"), ("Notes", "note")]),
        ])
    );
}

/// `STORE-1`
#[test]
fn a_file_with_only_the_headers_has_no_parts() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");

    assert_eq!(read_parts(&source), Ok(vec![]));
}

/// `STORE-1`, `ISSUE-23`, values that look like numbers are kept as they are in the file.
#[test]
fn keeps_values_that_look_like_numbers() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\",\"Package\"\n\"RESCO\",\"1234\",\"0402\"\n",
    );

    assert_eq!(
        read_parts(&source),
        Ok(vec![item("RESCO", "1234", &[("Package", "0402")])])
    );
}

/// `STORE-3`, the error names the missing header, so the user can fix the file.
#[test]
fn says_which_header_is_missing() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Maker\",\"Mpn\"\n\"RESCO\",\"RES1\"\n");

    let error = read_parts(&source).expect_err("an error");

    assert!(error.contains("Missing header. header: 'Manufacturer'"), "{error}");
}

/// `STORE-3`
#[test]
fn says_a_file_that_does_not_exist_can_not_be_read() {
    let folder = tempfile::tempdir().unwrap();
    let source = PartsSource::File(folder.path().join("missing.csv"));

    let error = read_parts(&source).expect_err("an error");

    assert!(error.starts_with("Error reading parts. file: "), "{error}");
}

/// `STORE-6`, one file is one data source, however its path is written.
#[test]
fn resolves_a_file_to_its_canonical_path() {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("other")).unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let other_spelling = PartsSource::File(
        folder
            .path()
            .join("other")
            .join("..")
            .join("parts.csv"),
    );

    assert_ne!(source, other_spelling);
    assert_eq!(resolve(&other_spelling), resolve(&source));
    let PartsSource::File(path) = resolve(&source) else {
        panic!()
    };
    // without the `\\?\` prefix Windows adds.
    assert!(
        !path
            .display()
            .to_string()
            .starts_with(r"\\?\"),
        "{}",
        path.display()
    );
}

/// `STORE-6`, on Windows, file names that differ only in case are the same file.
#[cfg(windows)]
#[test]
fn resolves_a_file_whatever_the_case_of_its_path() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let other_case = PartsSource::File(folder.path().join("PARTS.CSV"));

    assert_eq!(resolve(&other_case), resolve(&source));
}

/// `STORE-6`, a data source whose file doesn't exist is kept as it is, so an error names it as the user did.
#[test]
fn keeps_a_data_source_whose_file_does_not_exist_as_it_is() {
    let folder = tempfile::tempdir().unwrap();
    let source = PartsSource::File(folder.path().join("missing.csv"));

    assert_eq!(resolve(&source), source);
}

fn revision(source: &PartsSource) -> Revision {
    read(source).expect("parts").revision
}

/// `STORE-8`, a file's revision is the same while its contents are, however its path is written, and changes when they
/// do.
#[test]
fn a_files_revision_changes_only_when_its_contents_do() {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("other")).unwrap();
    let content = "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n";
    let source = csv_file(folder.path(), "parts.csv", content);
    let other_spelling = PartsSource::File(
        folder
            .path()
            .join("other")
            .join("..")
            .join("parts.csv"),
    );

    let first = revision(&source);
    assert_eq!(revision(&source), first);
    assert_eq!(revision(&other_spelling), first);

    // written again, unchanged.
    csv_file(folder.path(), "parts.csv", content);
    assert_eq!(revision(&source), first);

    csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES2\"\n",
    );
    assert_ne!(revision(&source), first);

    // the same contents in another file have the same revision.
    let copy = csv_file(folder.path(), "copy.csv", content);
    assert_eq!(revision(&copy), first);
}

fn write(source: &PartsSource, revision: &Revision, parts: Vec<PartsViewItem>) -> Result<Revision, PartsWriteError> {
    match block_on(StoresService::new().perform_once(StoresOperation::WriteParts {
        source: source.clone(),
        revision: revision.clone(),
        parts,
    })) {
        Some(StoresOutput::PartsWritten(result)) => result,
        output => panic!("unexpected output: {:?}", output),
    }
}

/// `STORE-15`, the parts are written with the file's columns in the order they were, new ones after them, every value
/// quoted, and the revision written is the file's, as it's read back.
#[test]
fn writes_the_parts_keeping_the_order_of_the_columns() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Mpn\",\"Notes\",\"Manufacturer\",\"LCSC\"\n\"RES1\",\"\",\"RESCO\",\"C1\"\n",
    );
    let based_on = revision(&source);

    let written = write(&source, &based_on, vec![
        item("RESCO", "RES1", &[("LCSC", "C1"), ("Notes", ""), ("Package", "")]),
        item("CAPCO", "1234", &[
            ("LCSC", "C2"),
            ("Notes", "X7R"),
            ("Package", "0402"),
        ]),
    ])
    .unwrap();

    let PartsSource::File(path) = &source else {
        unreachable!()
    };
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "\"Manufacturer\",\"Mpn\",\"Notes\",\"LCSC\",\"Package\"\n\
         \"RESCO\",\"RES1\",\"\",\"C1\",\"\"\n\
         \"CAPCO\",\"1234\",\"X7R\",\"C2\",\"0402\"\n"
    );
    assert_eq!(written, revision(&source));
    assert_eq!(read_parts(&source).unwrap(), vec![
        item("RESCO", "RES1", &[("LCSC", "C1"), ("Notes", ""), ("Package", "")]),
        item("CAPCO", "1234", &[
            ("LCSC", "C2"),
            ("Notes", "X7R"),
            ("Package", "0402")
        ]),
    ]);
}

/// `STORE-15`, a file changed since the revision the change was based on isn't written, and the refusal says the
/// file's revision now.
#[test]
fn refuses_to_write_a_file_changed_since() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n",
    );
    let based_on = revision(&source);
    let changed = "\"Manufacturer\",\"Mpn\"\n\"OTHERCO\",\"OTHER1\"\n";
    csv_file(folder.path(), "parts.csv", changed);

    let result = write(&source, &based_on, vec![item("CAPCO", "CAP1", &[])]);

    assert_eq!(
        result,
        Err(PartsWriteError::Changed {
            revision: Some(revision(&source)),
        })
    );
    let PartsSource::File(path) = &source else {
        unreachable!()
    };
    assert_eq!(fs::read_to_string(path).unwrap(), changed);
}

/// `STORE-15`, a file removed since isn't written again.
#[test]
fn refuses_to_write_a_file_removed_since() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let based_on = revision(&source);
    let PartsSource::File(path) = &source else {
        unreachable!()
    };
    fs::remove_file(path).unwrap();

    let result = write(&source, &based_on, vec![item("CAPCO", "CAP1", &[])]);

    assert_eq!(
        result,
        Err(PartsWriteError::Changed {
            revision: None,
        })
    );
    assert!(!path.exists());
}
