//! The stores service's CSV file back end (`STORE-1`, `STORE-3`, `STORE-6` to `STORE-8`, `STORE-15`, `STORE-17`,
//! `STORE-19`, `STORE-21`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use futures::executor::block_on;
use stores_protocol::{
    AddColumnsError, ColumnPosition, ColumnRef, EdaTool, FieldChange, FieldPattern, FieldValue, Part, Parts,
    PartsSource, PartsViewItem, PartsWriteError, Pattern, RefDes, Revision, RowKey, RowOrdering, RowRef, RowsChange,
    Source, StoresOperation, StoresOutput, Table, TableKind, TableRecords,
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

fn read_table(kind: TableKind, source: &Source) -> Result<Option<TableRecords>, String> {
    match block_on(StoresService::new().perform_once(StoresOperation::ReadTable {
        kind,
        source: source.clone(),
    })) {
        Some(StoresOutput::TableRead(result)) => result,
        output => panic!("unexpected output: {:?}", output),
    }
}

/// The table read, which there must be.
fn table_of(kind: TableKind, source: &Source) -> Table {
    read_table(kind, source)
        .unwrap()
        .expect("a table")
        .table
}

/// The table's column names, in order.
fn names(table: &Table) -> Vec<String> {
    table
        .columns()
        .into_iter()
        .map(|column| column.name)
        .collect()
}

/// Each row's values, as its columns show them.
fn rows(table: &Table) -> Vec<Vec<String>> {
    let columns = table.columns();
    (0..table.len())
        .map(|row| {
            columns
                .iter()
                .map(|column| {
                    table
                        .cell(row, &column.value)
                        .to_string()
                })
                .collect()
        })
        .collect()
}

fn strings(values: &[&str]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.to_string())
        .collect()
}

/// `STORE-17`, part mappings are read as the part mapper reads them: the part, and each of the EDA tool's fields'
/// patterns, a regular expression between `/`s parsed as it's read, and kept if it isn't valid.
#[test]
fn reads_part_mappings_with_their_patterns() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "part-mappings.csv",
        "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n\
         \"DipTrace\",\"RES_0402\",\"/330R?/\",\"MFR1\",\"MPN1\"\n\
         \"DipTrace\",\"/RES_[/\",\"1\",\"MFR2\",\"MPN2\"\n",
    );

    let records = read_table(TableKind::PartMappings, &source)
        .unwrap()
        .expect("a table");

    let Table::PartMappings(items) = &records.table else {
        panic!("part mappings");
    };
    assert_eq!(items[0].eda_tool, EdaTool::DipTrace);
    assert_eq!(items[0].part, Part::new("MFR1".to_string(), "MPN1".to_string()));
    assert_eq!(items[0].criteria, vec![
        FieldPattern {
            field_name: "name".to_string(),
            pattern: Pattern::Exact("RES_0402".to_string()),
        },
        FieldPattern {
            field_name: "value".to_string(),
            pattern: Pattern::parse("/330R?/"),
        },
    ]);
    assert!(matches!(&items[0].criteria[1].pattern, Pattern::Regex { regex: Ok(regex), .. } if regex.is_match("330")));
    assert!(matches!(&items[1].criteria[0].pattern, Pattern::Regex {
        regex: Err(_),
        ..
    }));

    assert_eq!(
        names(&records.table),
        strings(&["Eda", "Name", "Value", "Manufacturer", "Mpn"])
    );
    assert_eq!(rows(&records.table), vec![
        strings(&["DipTrace", "RES_0402", "/330R?/", "MFR1", "MPN1"]),
        strings(&["DipTrace", "/RES_[/", "1", "MFR2", "MPN2"]),
    ]);
    assert_eq!(
        records.revision,
        stores_service::csv_file::revision(&fs::read(folder.path().join("part-mappings.csv")).unwrap())
    );
}

/// `STORE-17`, a part mapping has a pattern for each field of any EDA tool the file has a column for, not only its own
/// tool's, in the EDA tools' order, whatever the columns' order, so every value is shown whichever tool the row has.
#[test]
fn reads_part_mappings_with_every_eda_tools_fields_the_file_has() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "part-mappings.csv",
        "\"Eda\",\"Package\",\"Val\",\"Name\",\"Value\",\"Device\",\"Manufacturer\",\"Mpn\"\n\
         \"DipTrace\",\"R_0402\",\"330R\",\"RES_0402\",\"330\",\"\",\"MFR1\",\"MPN1\"\n\
         \"KiCad\",\"C_0402\",\"100nF\",\"\",\"\",\"\",\"MFR2\",\"MPN2\"\n\
         \"EasyEda\",\"\",\"\",\"\",\"10k\",\"RES\",\"MFR3\",\"MPN3\"\n",
    );

    let records = read_table(TableKind::PartMappings, &source)
        .unwrap()
        .expect("a table");

    let Table::PartMappings(items) = &records.table else {
        panic!("part mappings");
    };
    let fields = |index: usize| -> Vec<(String, Pattern)> {
        items[index]
            .criteria
            .iter()
            .map(|criterion| (criterion.field_name.clone(), criterion.pattern.clone()))
            .collect()
    };
    let exact = |name: &str, value: &str| (name.to_string(), Pattern::Exact(value.to_string()));
    assert_eq!(fields(0), vec![
        exact("name", "RES_0402"),
        exact("value", "330"),
        exact("package", "R_0402"),
        exact("val", "330R"),
        exact("device", ""),
    ]);
    assert_eq!(fields(1), vec![
        exact("name", ""),
        exact("value", ""),
        exact("package", "C_0402"),
        exact("val", "100nF"),
        exact("device", ""),
    ]);
    assert_eq!(items[2].eda_tool, EdaTool::EasyEda);
    assert_eq!(fields(2)[1], exact("value", "10k"));
    assert_eq!(fields(2)[4], exact("device", "RES"));

    assert_eq!(
        names(&records.table),
        strings(&[
            "Eda",
            "Name",
            "Value",
            "Package",
            "Val",
            "Device",
            "Manufacturer",
            "Mpn"
        ])
    );
    assert_eq!(
        rows(&records.table)[1],
        strings(&["KiCad", "", "", "C_0402", "100nF", "", "MFR2", "MPN2"])
    );
}

/// `STORE-17`, a part mapping whose EDA tool isn't known is refused, saying why.
#[test]
fn part_mappings_with_an_unknown_eda_tool_are_refused() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "part-mappings.csv",
        "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n\"Unknown\",\"RES_0402\",\"330R\",\"MFR1\",\"MPN1\"\n",
    );

    let error = read_table(TableKind::PartMappings, &source).unwrap_err();

    assert!(error.contains("Unknown EDA. value: \"Unknown\""), "{error}");
}

/// `STORE-17`, substitutions have a pattern and a value for each of the EDA tool's fields, and their comment, last.
#[test]
fn reads_substitutions_with_their_patterns_and_comment_last() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "substitutions.csv",
        "\"Eda\",\"NamePattern\",\"ValuePattern\",\"Name\",\"Value\",\"Comment\"\n\
         \"DipTrace\",\"/RES_.*/\",\"330R\",\"RES_0603\",\"330\",\"DERATE 50V\"\n",
    );

    let table = table_of(TableKind::Substitutions, &source);

    let Table::Substitutions(items) = &table else {
        panic!("substitutions");
    };
    assert!(matches!(&items[0].criteria[0].pattern, Pattern::Regex {
        regex: Ok(_),
        ..
    }));
    assert_eq!(
        names(&table),
        strings(&["Eda", "NamePattern", "Name", "ValuePattern", "Value", "Comment"])
    );
    assert_eq!(rows(&table), vec![strings(&[
        "DipTrace",
        "/RES_.*/",
        "RES_0603",
        "330R",
        "330",
        "DERATE 50V"
    ])]);
}

/// `STORE-17`, a substitution has a pattern and a value for each field of any EDA tool the file has a column for, not
/// only its own tool's, in the EDA tools' order, whatever the columns' order, and one whose tool's field has no column
/// is read without it, so it can be shown, and given the column.
#[test]
fn reads_substitutions_with_every_eda_tools_fields_the_file_has() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "substitutions.csv",
        "\"Eda\",\"Comment\",\"PackagePattern\",\"Name\",\"NamePattern\",\"Package\",\"ValuePattern\",\"Value\"\n\
         \"DipTrace\",\"c1\",\"\",\"RES\",\"R1\",\"\",\"/1.*/\",\"10k\"\n\
         \"KiCad\",\"\",\"R_0402\",\"\",\"\",\"R_0603\",\"\",\"\"\n",
    );

    let records = read_table(TableKind::Substitutions, &source)
        .unwrap()
        .expect("a table");

    let Table::Substitutions(items) = &records.table else {
        panic!("substitutions");
    };
    let patterns = |index: usize| -> Vec<(&str, &str)> {
        items[index]
            .criteria
            .iter()
            .map(|criterion| (criterion.field_name.as_str(), criterion.pattern.text()))
            .collect()
    };
    let values = |index: usize| -> Vec<(&str, &str)> {
        items[index]
            .transforms
            .iter()
            .map(|transform| (transform.field_name.as_str(), transform.field_value.as_str()))
            .collect()
    };
    // the file has neither of `val`'s columns, so no row has it, KiCad's included, nor `device`'s.
    assert_eq!(patterns(0), vec![("name", "R1"), ("value", "/1.*/"), ("package", "")]);
    assert_eq!(values(0), vec![("name", "RES"), ("value", "10k"), ("package", "")]);
    assert_eq!(items[0].comment.as_deref(), Some("c1"));
    assert_eq!(items[1].eda_tool, EdaTool::KiCad);
    assert_eq!(patterns(1), vec![("name", ""), ("value", ""), ("package", "R_0402")]);
    assert_eq!(values(1), vec![("name", ""), ("value", ""), ("package", "R_0603")]);
    // an empty comment is one, as the file has its column.
    assert_eq!(items[1].comment.as_deref(), Some(""));

    // `STORE-19`, their keys, references, the headers as they're written, and their ordering.
    assert_eq!(records.keys, vec![RowKey("0".to_string()), RowKey("1".to_string())]);
    assert_eq!(records.ordering, RowOrdering::Position);
    assert_eq!(references(&records), vec![
        ("Eda", Some("Eda")),
        ("NamePattern", Some("NamePattern")),
        ("Name", Some("Name")),
        ("ValuePattern", Some("ValuePattern")),
        ("Value", Some("Value")),
        ("PackagePattern", Some("PackagePattern")),
        ("Package", Some("Package")),
        ("Comment", Some("Comment")),
    ]);
}

/// `STORE-17`, `STORE-21`, substitutions have a pattern's and a value's column for each field the file has a column
/// for, even with no rows, only those it has, and no comment's column if it has none.
#[test]
fn substitutions_with_no_rows_have_their_fields_columns() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "substitutions.csv",
        "\"Eda\",\"ValPattern\",\"Package\",\"Notes\"\n",
    );

    let records = read_table(TableKind::Substitutions, &source)
        .unwrap()
        .expect("a table");

    assert_eq!(references(&records), vec![
        ("Eda", Some("Eda")),
        ("Package", Some("Package")),
        ("ValPattern", Some("ValPattern")),
    ]);
}

/// The table's columns' names and references, in order.
fn references(records: &TableRecords) -> Vec<(&str, Option<&str>)> {
    records
        .columns
        .iter()
        .map(|column| {
            (
                column.name.as_str(),
                column
                    .reference
                    .as_ref()
                    .map(|reference| reference.0.as_str()),
            )
        })
        .collect()
}

/// `STORE-17`, assembly rules and a variant's placements are read as the Variant Builder writes them.
#[test]
fn reads_assembly_rules_and_placements() {
    let folder = tempfile::tempdir().unwrap();
    let rules = csv_file(
        folder.path(),
        "rules.csv",
        "\"RefDes\",\"Manufacturer\",\"Mpn\"\n\"R1\",\"MFR1\",\"MPN1\"\n",
    );
    let placements = csv_file(
        folder.path(),
        "placements.csv",
        "\"RefDes\",\"Manufacturer\",\"Mpn\",\"Place\",\"PcbSide\",\"X\",\"Y\",\"Rotation\"\n\
         \"R1\",\"MFR1\",\"MPN1\",\"true\",\"Bottom\",\"10\",\"20\",\"-90\"\n",
    );

    let rules = table_of(TableKind::AssemblyRules, &rules);
    assert_eq!(names(&rules), strings(&["RefDes", "Manufacturer", "Mpn"]));
    assert_eq!(rows(&rules), vec![strings(&["R1", "MFR1", "MPN1"])]);

    let placements = table_of(TableKind::Placements, &placements);
    assert_eq!(
        names(&placements),
        strings(&[
            "RefDes",
            "Manufacturer",
            "Mpn",
            "Place",
            "PcbSide",
            "X",
            "Y",
            "Rotation"
        ])
    );
    assert_eq!(rows(&placements), vec![strings(&[
        "R1", "MFR1", "MPN1", "true", "bottom", "10", "20", "-90"
    ])]);
}

/// `STORE-17`, a BOM is written without headers, as JLCPCB's are, so every row is read, by the order of its values,
/// with its designators, and the comment last.
#[test]
fn reads_a_bom_without_headers_with_the_comment_last() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "bom.csv",
        "\"MPN1 (MFR1)\",\"R1, R2\",\"RES_0402\",\"C1234\",\"2\"\n\"MPN2 (MFR2)\",\"C1\",\"CAP_0402\",\"\",\"1\"\n",
    );

    let table = table_of(TableKind::Bom, &source);

    let Table::Bom(items) = &table else {
        panic!("a BOM");
    };
    assert_eq!(items[0].designators, vec![
        RefDes::from("R1".to_string()),
        RefDes::from("R2".to_string())
    ]);
    assert_eq!(items[1].jlcpcb_part, None);
    assert_eq!(
        names(&table),
        strings(&["Designator", "Footprint", "JLCPCB Part", "Quantity", "Comment"])
    );
    assert_eq!(rows(&table), vec![
        strings(&["R1, R2", "RES_0402", "C1234", "2", "MPN1 (MFR1)"]),
        strings(&["C1", "CAP_0402", "", "1", "MPN2 (MFR2)"]),
    ]);
}

/// `STORE-17`, EDA placements are read in the EDA tool's format, with the fixed columns, then each field's name.
#[test]
fn reads_eda_placements_as_a_table() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "board1.csv",
        "\"RefDes\",\"Name\",\"Value\",\"Side\",\"X\",\"Y\",\"Rotation\"\n\
         \"R1\",\"RES_0402\",\"330R\",\"Top\",\"10\",\"20\",\"90\"\n",
    );

    let table = table_of(
        TableKind::EdaPlacements {
            eda_tool: EdaTool::DipTrace,
        },
        &source,
    );

    assert_eq!(
        names(&table),
        strings(&["RefDes", "Place", "Side", "X", "Y", "Rotation", "name", "value"])
    );
    assert_eq!(rows(&table), vec![strings(&[
        "R1", "true", "top", "10", "20", "90", "RES_0402", "330R"
    ])]);
}

/// `STORE-17`, a data source that doesn't exist is none, not an error, such as a BOM not yet built.
#[test]
fn a_table_whose_file_does_not_exist_is_none() {
    let folder = tempfile::tempdir().unwrap();
    let source = Source::File(folder.path().join("bom.csv"));

    assert_eq!(read_table(TableKind::Bom, &source), Ok(None));
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

/// `STORE-1`, values that look like numbers are kept as they are in the file.
///
/// Regression test for `ISSUE-23`: a parts CSV file couldn't be loaded if any value looked like a number, for example an
/// MPN of `1234`, or a package of `0402` in a column of its own. `variantbuilder_cli build` failed, as did anything
/// else that loaded parts, with an error that didn't say which value or why. With `#[serde(flatten)]`, serde read every
/// field through `deserialize_any`, and the csv crate guessed each value's type, reading `0402` as an integer.
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

fn change_rows(source: &Source, change: RowsChange) -> Result<Revision, PartsWriteError> {
    change_rows_of(TableKind::PartMappings, source, change)
}

fn change_rows_of(kind: TableKind, source: &Source, change: RowsChange) -> Result<Revision, PartsWriteError> {
    match block_on(StoresService::new().perform_once(StoresOperation::ChangeRows {
        kind,
        source: source.clone(),
        change,
    })) {
        Some(StoresOutput::RowsChanged(result)) => result,
        output => panic!("unexpected output: {:?}", output),
    }
}

fn contents(source: &Source) -> String {
    let Source::File(path) = source else { unreachable!() };
    fs::read_to_string(path).unwrap()
}

fn value(column: &str, value: &str) -> FieldValue {
    FieldValue {
        column: ColumnRef(column.to_string()),
        value: value.to_string(),
    }
}

/// A row, by its key, expected to have the values, by column.
fn row_ref(key: &str, expected: &[(&str, &str)]) -> RowRef {
    RowRef {
        key: RowKey(key.to_string()),
        expected: expected
            .iter()
            .map(|(column, text)| value(column, text))
            .collect(),
    }
}

/// Part mappings with a column the table doesn't show, `Notes`, a KiCad row whose DipTrace fields' columns it doesn't
/// show either, and columns in an order of their own.
const MAPPINGS: &str = "\"Mpn\",\"Name\",\"Eda\",\"Notes\",\"Manufacturer\",\"Value\",\"Package\"\n\
                        \"MPN1\",\"RES_0402\",\"DipTrace\",\"keep me\",\"MFR1\",\"/330R?/\",\"\"\n\
                        \"MPN2\",\"n\",\"KiCad\",\"\",\"MFR2\",\"v\",\"R_0402\"\n\
                        \"MPN3\",\"CAP_0603\",\"DipTrace\",\"\",\"MFR3\",\"10u\",\"\"\n";

/// `STORE-19`, part mappings' rows' keys are their places in the file, their columns' references the file's headers,
/// as they're written, and their rows are ordered by their place.
#[test]
fn reads_part_mappings_keys_references_and_ordering() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "part-mappings.csv", MAPPINGS);

    let records = read_table(TableKind::PartMappings, &source)
        .unwrap()
        .expect("a table");

    assert_eq!(records.keys, vec![
        RowKey("0".to_string()),
        RowKey("1".to_string()),
        RowKey("2".to_string())
    ]);
    assert_eq!(records.ordering, RowOrdering::Position);
    assert_eq!(
        records
            .columns
            .iter()
            .map(|column| (
                column.name.as_str(),
                column
                    .reference
                    .as_ref()
                    .map(|reference| reference.0.as_str())
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Eda", Some("Eda")),
            ("Name", Some("Name")),
            ("Value", Some("Value")),
            ("Package", Some("Package")),
            ("Manufacturer", Some("Manufacturer")),
            ("Mpn", Some("Mpn")),
        ]
    );

    // a table that can't be changed has no references, and its rows can't be moved.
    let bom = csv_file(folder.path(), "bom.csv", "\"330R\",\"R1\",\"RES_0402\",\"\",\"1\"\n");
    let records = read_table(TableKind::Bom, &bom)
        .unwrap()
        .expect("a table");
    assert!(records
        .columns
        .iter()
        .all(|column| column.reference.is_none()));
    assert_eq!(records.ordering, RowOrdering::Unsupported);
}

/// `STORE-19`, updating a row changes only the columns named, so the columns the table doesn't show are as they were,
/// every value quoted, and the revision is the file's.
#[test]
fn updates_only_the_columns_named() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "part-mappings.csv", MAPPINGS);

    let revision = change_rows(&source, RowsChange::Update {
        key: RowKey("1".to_string()),
        changes: vec![
            FieldChange {
                column: ColumnRef("Package".to_string()),
                old: "R_0402".to_string(),
                new: "/R_0[46]0[23]/".to_string(),
            },
            FieldChange {
                column: ColumnRef("Mpn".to_string()),
                old: "MPN2".to_string(),
                new: "MPN4".to_string(),
            },
        ],
    })
    .unwrap();

    assert_eq!(
        contents(&source),
        "\"Mpn\",\"Name\",\"Eda\",\"Notes\",\"Manufacturer\",\"Value\",\"Package\"\n\
         \"MPN1\",\"RES_0402\",\"DipTrace\",\"keep me\",\"MFR1\",\"/330R?/\",\"\"\n\
         \"MPN4\",\"n\",\"KiCad\",\"\",\"MFR2\",\"v\",\"/R_0[46]0[23]/\"\n\
         \"MPN3\",\"CAP_0603\",\"DipTrace\",\"\",\"MFR3\",\"10u\",\"\"\n"
    );
    assert_eq!(revision, self::revision(&source));
}

/// `STORE-19`, a row is added after the others, with an empty value of each column not named, and a row is removed.
#[test]
fn adds_and_removes_rows() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "part-mappings.csv", MAPPINGS);

    change_rows(&source, RowsChange::Add {
        values: vec![
            value("Eda", "DipTrace"),
            value("Name", "/LED_.*/"),
            value("Value", "/R[/"),
            value("Manufacturer", "MFR4"),
            value("Mpn", "MPN4"),
        ],
    })
    .unwrap();
    change_rows(&source, RowsChange::Remove {
        row: row_ref("1", &[("Eda", "KiCad"), ("Package", "R_0402"), ("Mpn", "MPN2")]),
    })
    .unwrap();

    assert_eq!(
        contents(&source),
        "\"Mpn\",\"Name\",\"Eda\",\"Notes\",\"Manufacturer\",\"Value\",\"Package\"\n\
         \"MPN1\",\"RES_0402\",\"DipTrace\",\"keep me\",\"MFR1\",\"/330R?/\",\"\"\n\
         \"MPN3\",\"CAP_0603\",\"DipTrace\",\"\",\"MFR3\",\"10u\",\"\"\n\
         \"MPN4\",\"/LED_.*/\",\"DipTrace\",\"\",\"MFR4\",\"/R[/\",\"\"\n"
    );
}

/// `STORE-19`, a row is moved to before another, whether it's before or after it, or to the end.
#[test]
fn moves_rows() {
    let mpns = |source: &Source| -> Vec<String> {
        let Table::PartMappings(items) = table_of(TableKind::PartMappings, source) else {
            panic!("part mappings")
        };
        items
            .into_iter()
            .map(|item| item.part.mpn)
            .collect()
    };
    let folder = tempfile::tempdir().unwrap();
    let mpn = |key: &str, mpn: &str| row_ref(key, &[("Mpn", mpn)]);
    let cases = [
        (mpn("2", "MPN3"), Some(mpn("0", "MPN1")), ["MPN3", "MPN1", "MPN2"]),
        (mpn("0", "MPN1"), Some(mpn("2", "MPN3")), ["MPN2", "MPN1", "MPN3"]),
        (mpn("0", "MPN1"), None, ["MPN2", "MPN3", "MPN1"]),
    ];

    for (row, before, expected) in cases {
        let source = csv_file(folder.path(), "part-mappings.csv", MAPPINGS);

        change_rows(&source, RowsChange::Move {
            row: row.clone(),
            before: before.clone(),
        })
        .unwrap();

        assert_eq!(mpns(&source), strings(&expected), "{:?} before {:?}", row, before);
    }
    // the columns the table doesn't show move with their rows.
    let source = csv_file(folder.path(), "part-mappings.csv", MAPPINGS);
    change_rows(&source, RowsChange::Move {
        row: mpn("0", "MPN1"),
        before: None,
    })
    .unwrap();
    assert!(contents(&source).ends_with("\"MPN1\",\"RES_0402\",\"DipTrace\",\"keep me\",\"MFR1\",\"/330R?/\",\"\"\n"));
}

/// `STORE-19`, a change whose values aren't as they were, or whose row isn't there, changes nothing, and the refusal
/// says the file's revision now, as the file has changed since the change was made.
#[test]
fn refuses_a_change_whose_values_are_not_as_they_were() {
    let folder = tempfile::tempdir().unwrap();
    let changes = [
        RowsChange::Update {
            key: RowKey("1".to_string()),
            changes: vec![FieldChange {
                column: ColumnRef("Mpn".to_string()),
                old: "MPN1".to_string(),
                new: "MPN4".to_string(),
            }],
        },
        RowsChange::Remove {
            row: row_ref("3", &[("Mpn", "MPN4")]),
        },
        RowsChange::Move {
            row: row_ref("0", &[("Mpn", "MPN1")]),
            before: Some(row_ref("2", &[("Mpn", "MPN2")])),
        },
    ];

    for change in changes {
        let source = csv_file(folder.path(), "part-mappings.csv", MAPPINGS);

        let result = change_rows(&source, change.clone());

        assert_eq!(
            result,
            Err(PartsWriteError::Changed {
                revision: Some(revision(&source)),
            }),
            "{:?}",
            change
        );
        assert_eq!(contents(&source), MAPPINGS, "{:?}", change);
    }
}

/// Substitutions with a column the table doesn't show, `Notes`, in pairs, each field's pattern's then its value's.
const SUBSTITUTIONS: &str = "\"Eda\",\"NamePattern\",\"Name\",\"ValuePattern\",\"Value\",\"Notes\",\"Comment\"\n\
                             \"DipTrace\",\"R1\",\"RES\",\"/1.*/\",\"10k\",\"keep me\",\"c1\"\n\
                             \"DipTrace\",\"C1\",\"CAP\",\"\",\"\",\"\",\"\"\n";

/// `STORE-19`, a substitution's tool, pattern, value and comment are updated, and rows added, removed and moved, as
/// part mappings' are, the columns the table doesn't show as they were.
#[test]
fn changes_substitutions() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "substitutions.csv", SUBSTITUTIONS);
    let change = |change: RowsChange| change_rows_of(TableKind::Substitutions, &source, change).unwrap();
    let field = |column: &str, old: &str, new: &str| FieldChange {
        column: ColumnRef(column.to_string()),
        old: old.to_string(),
        new: new.to_string(),
    };

    change(RowsChange::Update {
        key: RowKey("1".to_string()),
        changes: vec![
            field("Eda", "DipTrace", "EasyEda"),
            field("ValuePattern", "", "/10[un]/"),
            field("Value", "", "10u"),
            field("Comment", "", "derate 16V"),
        ],
    });
    change(RowsChange::Add {
        values: vec![
            value("Eda", "DipTrace"),
            value("NamePattern", "L1"),
            value("Name", "IND"),
        ],
    });
    change(RowsChange::Move {
        row: row_ref("2", &[("NamePattern", "L1")]),
        before: Some(row_ref("0", &[("NamePattern", "R1")])),
    });
    change(RowsChange::Remove {
        row: row_ref("2", &[("Name", "CAP"), ("Comment", "derate 16V")]),
    });

    assert_eq!(
        contents(&source),
        "\"Eda\",\"NamePattern\",\"Name\",\"ValuePattern\",\"Value\",\"Notes\",\"Comment\"\n\
         \"DipTrace\",\"L1\",\"IND\",\"\",\"\",\"\",\"\"\n\
         \"DipTrace\",\"R1\",\"RES\",\"/1.*/\",\"10k\",\"keep me\",\"c1\"\n"
    );
}

/// `STORE-21`, an EDA tool's columns are added to a substitutions file in two groups: its patterns' after the last
/// pattern's column, and its values' after the last value's column, whatever columns are among them, or, with no
/// pattern's column, before the first value's column, with no value's column, after the last pattern's column, and,
/// with neither, before its comment's column, or, if it has none, after its last column.
#[test]
fn adds_an_eda_tools_substitutions_columns_after_the_last_of_their_group() {
    let columns = ["PackagePattern", "ValPattern", "Package", "Val"];
    let cases = [
        // in pairs, as the Variant Builder's files are.
        (
            "\"Eda\",\"NamePattern\",\"Name\",\"ValuePattern\",\"Value\",\"Comment\"",
            "\"Eda\",\"NamePattern\",\"Name\",\"ValuePattern\",\"PackagePattern\",\"ValPattern\",\"Value\",\"Package\",\"Val\",\"Comment\"",
        ),
        // in two groups, with columns of the user's among them.
        (
            "\"Eda\",\"NamePattern\",\"Notes\",\"ValuePattern\",\"Comment\",\"Name\",\"Value\",\"Owner\"",
            "\"Eda\",\"NamePattern\",\"Notes\",\"ValuePattern\",\"PackagePattern\",\"ValPattern\",\"Comment\",\"Name\",\"Value\",\"Package\",\"Val\",\"Owner\"",
        ),
        // the values' group first.
        (
            "\"Eda\",\"Name\",\"Value\",\"NamePattern\",\"ValuePattern\"",
            "\"Eda\",\"Name\",\"Value\",\"Package\",\"Val\",\"NamePattern\",\"ValuePattern\",\"PackagePattern\",\"ValPattern\"",
        ),
        // no pattern's column.
        (
            "\"Eda\",\"Notes\",\"Name\",\"Value\",\"Comment\"",
            "\"Eda\",\"Notes\",\"PackagePattern\",\"ValPattern\",\"Name\",\"Value\",\"Package\",\"Val\",\"Comment\"",
        ),
        // no value's column.
        (
            "\"Eda\",\"NamePattern\",\"ValuePattern\",\"Notes\"",
            "\"Eda\",\"NamePattern\",\"ValuePattern\",\"PackagePattern\",\"ValPattern\",\"Package\",\"Val\",\"Notes\"",
        ),
        // neither, with a comment's column.
        (
            "\"Eda\",\"Comment\",\"Notes\"",
            "\"Eda\",\"PackagePattern\",\"ValPattern\",\"Package\",\"Val\",\"Comment\",\"Notes\"",
        ),
        // neither, and no comment's column.
        (
            "\"Eda\",\"Notes\"",
            "\"Eda\",\"Notes\",\"PackagePattern\",\"ValPattern\",\"Package\",\"Val\"",
        ),
    ];

    for (headers, expected) in cases {
        let folder = tempfile::tempdir().unwrap();
        let source = csv_file(folder.path(), "substitutions.csv", &format!("{headers}\n"));

        add_columns_of(
            TableKind::Substitutions,
            &source,
            &columns,
            ColumnPosition::EdaToolColumns,
        )
        .unwrap();

        assert_eq!(contents(&source), format!("{expected}\n"), "{headers}");
    }

    // each row has an empty value of each.
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "substitutions.csv", SUBSTITUTIONS);
    add_columns_of(
        TableKind::Substitutions,
        &source,
        &columns,
        ColumnPosition::EdaToolColumns,
    )
    .unwrap();
    assert!(contents(&source)
        .contains("\"DipTrace\",\"R1\",\"RES\",\"/1.*/\",\"\",\"\",\"10k\",\"\",\"\",\"keep me\",\"c1\"\n"));
}

fn add_columns(source: &Source, columns: &[&str], position: ColumnPosition) -> Result<Revision, AddColumnsError> {
    add_columns_of(TableKind::PartMappings, source, columns, position)
}

fn add_columns_of(
    kind: TableKind,
    source: &Source,
    columns: &[&str],
    position: ColumnPosition,
) -> Result<Revision, AddColumnsError> {
    match block_on(StoresService::new().perform_once(StoresOperation::AddColumns {
        kind,
        source: source.clone(),
        columns: strings(columns),
        position,
    })) {
        Some(StoresOutput::ColumnsAdded(result)) => result,
        output => panic!("unexpected output: {:?}", output),
    }
}

const KICAD_MAPPINGS: &str = "\"Eda\",\"Package\",\"Val\",\"Manufacturer\",\"Mpn\",\"Notes\"\n\
                              \"KiCad\",\"R_0402\",\"330R\",\"MFR1\",\"MPN1\",\"first\"\n\
                              \"KiCad\",\"C_0402\",\"100nF\",\"MFR2\",\"MPN2\",\"\"\n";

/// `STORE-21`, columns are added where they're to go, together, in the order given, with an empty value in each row,
/// every other column, value and row as it was, every value quoted, and the revision is the file's.
#[test]
fn adds_columns_where_they_are_to_go() {
    let cases = [
        (
            ColumnPosition::Before("Manufacturer".to_string()),
            "\"Eda\",\"Package\",\"Val\",\"Device\",\"Value\",\"Manufacturer\",\"Mpn\",\"Notes\"\n\
             \"KiCad\",\"R_0402\",\"330R\",\"\",\"\",\"MFR1\",\"MPN1\",\"first\"\n\
             \"KiCad\",\"C_0402\",\"100nF\",\"\",\"\",\"MFR2\",\"MPN2\",\"\"\n",
        ),
        (
            ColumnPosition::After("Eda".to_string()),
            "\"Eda\",\"Device\",\"Value\",\"Package\",\"Val\",\"Manufacturer\",\"Mpn\",\"Notes\"\n\
             \"KiCad\",\"\",\"\",\"R_0402\",\"330R\",\"MFR1\",\"MPN1\",\"first\"\n\
             \"KiCad\",\"\",\"\",\"C_0402\",\"100nF\",\"MFR2\",\"MPN2\",\"\"\n",
        ),
        (
            ColumnPosition::First,
            "\"Device\",\"Value\",\"Eda\",\"Package\",\"Val\",\"Manufacturer\",\"Mpn\",\"Notes\"\n\
             \"\",\"\",\"KiCad\",\"R_0402\",\"330R\",\"MFR1\",\"MPN1\",\"first\"\n\
             \"\",\"\",\"KiCad\",\"C_0402\",\"100nF\",\"MFR2\",\"MPN2\",\"\"\n",
        ),
        (
            ColumnPosition::Last,
            "\"Eda\",\"Package\",\"Val\",\"Manufacturer\",\"Mpn\",\"Notes\",\"Device\",\"Value\"\n\
             \"KiCad\",\"R_0402\",\"330R\",\"MFR1\",\"MPN1\",\"first\",\"\",\"\"\n\
             \"KiCad\",\"C_0402\",\"100nF\",\"MFR2\",\"MPN2\",\"\",\"\",\"\"\n",
        ),
    ];

    for (position, expected) in cases {
        let folder = tempfile::tempdir().unwrap();
        let source = csv_file(folder.path(), "part-mappings.csv", KICAD_MAPPINGS);

        let revision = add_columns(&source, &["Device", "Value"], position.clone());

        assert_eq!(contents(&source), expected, "{:?}", position);
        assert_eq!(
            revision,
            Ok(stores_service::csv_file::revision(expected.as_bytes())),
            "{:?}",
            position
        );
    }
}

/// `STORE-21`, a file with only its headers has columns added too.
#[test]
fn adds_columns_to_a_file_with_no_rows() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "part-mappings.csv",
        "\"Eda\",\"Name\",\"Manufacturer\",\"Mpn\"\n",
    );

    add_columns(&source, &["Value"], ColumnPosition::Before("Manufacturer".to_string())).unwrap();

    assert_eq!(
        contents(&source),
        "\"Eda\",\"Name\",\"Value\",\"Manufacturer\",\"Mpn\"\n"
    );
}

/// `STORE-21`, an EDA tool's columns are added to a part mappings file after its last pattern's column if it has a
/// manufacturer's or an MPN's column, and every pattern's column is before them, so before them if it has no pattern's
/// column, and otherwise after its last column, its other columns, and their values, as they were.
#[test]
fn adds_an_eda_tools_columns_where_the_file_puts_them() {
    let cases = [
        // every pattern's column before the part's, a column of the user's between them.
        (
            "\"Eda\",\"Name\",\"Value\",\"Notes\",\"Manufacturer\",\"Mpn\"\n\"DipTrace\",\"R1\",\"330R\",\"n\",\"MFR1\",\"MPN1\"\n",
            "\"Eda\",\"Name\",\"Value\",\"Package\",\"Val\",\"Notes\",\"Manufacturer\",\"Mpn\"\n\"DipTrace\",\"R1\",\"330R\",\"\",\"\",\"n\",\"MFR1\",\"MPN1\"\n",
        ),
        // only an MPN's column, in any case, as the part mapper finds a field's column.
        (
            "\"Eda\",\"NAME\",\"Mpn\"\n\"DipTrace\",\"R1\",\"MPN1\"\n",
            "\"Eda\",\"NAME\",\"Package\",\"Val\",\"Mpn\"\n\"DipTrace\",\"R1\",\"\",\"\",\"MPN1\"\n",
        ),
        // no pattern's column.
        (
            "\"Eda\",\"Manufacturer\",\"Mpn\"\n\"DipTrace\",\"MFR1\",\"MPN1\"\n",
            "\"Eda\",\"Package\",\"Val\",\"Manufacturer\",\"Mpn\"\n\"DipTrace\",\"\",\"\",\"MFR1\",\"MPN1\"\n",
        ),
        // the part's columns before the patterns'.
        (
            "\"Eda\",\"Manufacturer\",\"Mpn\",\"Name\",\"Value\",\"Notes\"\n\"DipTrace\",\"MFR1\",\"MPN1\",\"R1\",\"330R\",\"n\"\n",
            "\"Eda\",\"Manufacturer\",\"Mpn\",\"Name\",\"Value\",\"Notes\",\"Package\",\"Val\"\n\"DipTrace\",\"MFR1\",\"MPN1\",\"R1\",\"330R\",\"n\",\"\",\"\"\n",
        ),
        // a pattern's column after the part's.
        (
            "\"Eda\",\"Name\",\"Manufacturer\",\"Mpn\",\"Value\"\n\"DipTrace\",\"R1\",\"MFR1\",\"MPN1\",\"330R\"\n",
            "\"Eda\",\"Name\",\"Manufacturer\",\"Mpn\",\"Value\",\"Package\",\"Val\"\n\"DipTrace\",\"R1\",\"MFR1\",\"MPN1\",\"330R\",\"\",\"\"\n",
        ),
        // no part's column.
        (
            "\"Eda\",\"Name\",\"Value\",\"Notes\"\n\"DipTrace\",\"R1\",\"330R\",\"n\"\n",
            "\"Eda\",\"Name\",\"Value\",\"Notes\",\"Package\",\"Val\"\n\"DipTrace\",\"R1\",\"330R\",\"n\",\"\",\"\"\n",
        ),
    ];

    for (file, expected) in cases {
        let folder = tempfile::tempdir().unwrap();
        let source = csv_file(folder.path(), "part-mappings.csv", file);

        add_columns(&source, &["Package", "Val"], ColumnPosition::EdaToolColumns).unwrap();

        assert_eq!(contents(&source), expected, "{file}");
    }
}

/// `STORE-21`, columns already there, in any case, as the part mapper finds a field's column, or given twice, or to go
/// by a column that isn't there, are refused, the file left as it was, and a file that isn't there says so.
#[test]
fn refuses_columns_that_can_not_be_added() {
    let cases: [(&[&str], ColumnPosition, AddColumnsError); 4] = [
        (
            &["Device", "VAL"],
            ColumnPosition::Last,
            AddColumnsError::ColumnExists {
                column: "VAL".to_string(),
            },
        ),
        (
            &["Device", "device"],
            ColumnPosition::Last,
            AddColumnsError::ColumnExists {
                column: "device".to_string(),
            },
        ),
        // a column is named by its header as it's written, as a column's reference is.
        (
            &["Device"],
            ColumnPosition::Before("manufacturer".to_string()),
            AddColumnsError::ColumnNotFound {
                column: "manufacturer".to_string(),
            },
        ),
        (
            &["Device"],
            ColumnPosition::After("Comment".to_string()),
            AddColumnsError::ColumnNotFound {
                column: "Comment".to_string(),
            },
        ),
    ];

    for (columns, position, expected) in cases {
        let folder = tempfile::tempdir().unwrap();
        let source = csv_file(folder.path(), "part-mappings.csv", KICAD_MAPPINGS);

        let result = add_columns(&source, columns, position.clone());

        assert_eq!(result, Err(expected), "{:?}, {:?}", columns, position);
        assert_eq!(contents(&source), KICAD_MAPPINGS, "{:?}, {:?}", columns, position);
    }

    let folder = tempfile::tempdir().unwrap();
    let missing = Source::File(folder.path().join("missing.csv"));
    assert_eq!(
        add_columns(&missing, &["Device"], ColumnPosition::Last),
        Err(AddColumnsError::SourceNotFound)
    );
}

/// `STORE-17`, `STORE-21`, part mappings have a pattern's column for each EDA tool's field the file has a column for,
/// even with no rows, so the core knows which an EDA tool's rows still need.
#[test]
fn part_mappings_with_no_rows_have_their_fields_columns() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "part-mappings.csv",
        "\"Eda\",\"VAL\",\"Package\",\"Manufacturer\",\"Mpn\",\"Notes\"\n",
    );

    let records = read_table(TableKind::PartMappings, &source)
        .unwrap()
        .expect("a table");

    assert_eq!(
        records
            .columns
            .iter()
            .map(|column| (
                column.name.as_str(),
                column
                    .reference
                    .as_ref()
                    .map(|reference| reference.0.as_str())
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Eda", Some("Eda")),
            ("Package", Some("Package")),
            ("Val", Some("VAL")),
            ("Manufacturer", Some("Manufacturer")),
            ("Mpn", Some("Mpn")),
        ]
    );
}
