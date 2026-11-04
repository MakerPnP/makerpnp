//! The stores service's CSV file back end (`STORE-1`, `STORE-3`, `STORE-6`, `STORE-7`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use futures::executor::block_on;
use stores_protocol::{Part, PartsSource, PartsViewItem, StoresOperation, StoresOutput};
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

fn read_parts(source: &PartsSource) -> Result<Vec<PartsViewItem>, String> {
    match block_on(StoresService::new().perform(StoresOperation::ReadParts {
        source: source.clone(),
    })) {
        StoresOutput::PartsRead(result) => result,
        output => panic!("unexpected output: {:?}", output),
    }
}

fn resolve(source: &PartsSource) -> PartsSource {
    match block_on(StoresService::new().perform(StoresOperation::ResolveSource {
        source: source.clone(),
    })) {
        StoresOutput::SourceResolved {
            source,
        } => source,
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
