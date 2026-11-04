//! The CLI's parts mode, which uses the stores core (`STORE`).

#[macro_use]
extern crate util;

use std::fs::{read_to_string, write};

use assert_cmd::Command;
use indoc::indoc;
use tempfile::tempdir;

/// `STORE-1`, `STORE-2`, `STORE-4`, `ISSUE-23`
#[test]
fn lists_the_parts_with_their_other_values() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let parts_path = temp_dir.path().join("parts.csv");
    write(&parts_path, indoc! {r#"
        "Manufacturer","Mpn","Package","LCSC"
        "RESCO","1234","0402","C1234"
        "CAPCO","CAP1","0603",""
    "#})?;
    let trace_path = temp_dir.path().join("trace.log");

    let mut cmd = Command::cargo_bin("planner_cli")?;
    let output = cmd
        .args([
            "--trace",
            trace_path.to_str().unwrap(),
            "-v",
            "parts",
            "--source",
            parts_path.to_str().unwrap(),
            "list",
        ])
        .output()?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    // the other values are in the order of their names.
    assert_eq!(String::from_utf8(output.stdout)?, indoc! {r#"
        "Manufacturer","Mpn","LCSC","Package"
        "RESCO","1234","C1234","0402"
        "CAPCO","CAP1","","0603"
    "#});

    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: OpenParts {",
        "stores_service: Performing stores operation. operation: ResolveSource {",
        "stores_app: Received event. event: SourceResolved {",
        "stores_service: Performing stores operation. operation: ReadParts {",
        "stores_app: Received event. event: PartsRead {",
        "stores_app: Loaded parts. source: '",
        "part_count: 2",
    ]);
    assert!(!trace.contains("Event failed."));
    // `STORE-9`, the CLI is done before the file could change, so it doesn't watch it.
    assert!(!trace.contains("Watching the data source."));

    Ok(())
}

/// `STORE-3`
#[test]
fn says_which_header_is_missing() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let parts_path = temp_dir.path().join("parts.csv");
    write(&parts_path, "\"Maker\",\"Mpn\"\n\"RESCO\",\"RES1\"\n")?;
    let trace_path = temp_dir.path().join("trace.log");

    let mut cmd = Command::cargo_bin("planner_cli")?;
    let output = cmd
        .args([
            "--trace",
            trace_path.to_str().unwrap(),
            "-v",
            "parts",
            "--source",
            parts_path.to_str().unwrap(),
            "list",
        ])
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("Unable to load the parts."), "{stderr}");
    assert!(stderr.contains("Missing header. header: 'Manufacturer'"), "{stderr}");

    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: OpenParts {",
        "stores_service: Performing stores operation. operation: ReadParts {",
        "stores_app: Received event. event: PartsRead {",
        "stores_app: Event failed. error: UnableToLoadParts {",
    ]);

    Ok(())
}
