//! The CLI's parts mode, which uses the stores core (`STORE`).

#[macro_use]
extern crate util;

use std::fs::{read_to_string, write};
use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use indoc::indoc;
use tempfile::tempdir;

/// `STORE-1`, `STORE-2`, `STORE-4`
///
/// Regression test for `ISSUE-23`, from the CLI, see `keeps_values_that_look_like_numbers` in
/// `planning/stores_service/tests/csv_file.rs`.
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

/// Runs the parts mode on the data source, with `args` after it, logging to `trace.log` in the folder.
fn parts(folder: &Path, parts_path: &Path, args: &[&str]) -> Result<Output, anyhow::Error> {
    let trace_path = folder.join("trace.log");
    let mut cmd = Command::cargo_bin("planner_cli")?;
    Ok(cmd
        .args([
            "--trace",
            trace_path.to_str().unwrap(),
            "-v",
            "parts",
            "--source",
            parts_path.to_str().unwrap(),
        ])
        .args(args)
        .output()?)
}

/// `STORE-15`, a part is added, has its values set, and is removed, each change based on the revision read before it,
/// and the file keeps its columns' order.
#[test]
fn adds_sets_and_removes_parts() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let parts_path = temp_dir.path().join("parts.csv");
    write(&parts_path, indoc! {r#"
        "Manufacturer","Mpn","Package","LCSC"
        "RESCO","RES1","0402","C1"
    "#})?;

    let output = parts(temp_dir.path(), &parts_path, &[
        "add",
        "--manufacturer",
        "CAPCO",
        "--mpn",
        "CAP1",
        "--value",
        "LCSC=C2",
        "--value",
        "Notes=X7R",
    ])?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(read_to_string(&parts_path)?, indoc! {r#"
        "Manufacturer","Mpn","Package","LCSC","Notes"
        "RESCO","RES1","0402","C1",""
        "CAPCO","CAP1","","C2","X7R"
    "#});
    let trace = read_to_string(temp_dir.path().join("trace.log"))?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: OpenParts {",
        "stores_app: Loaded parts. source: '",
        "stores_app: Received event. event: ChangeParts {",
        "stores_service: Performing stores operation. operation: WriteParts {",
        "stores_service::csv_file: Storing parts. source: '",
        "stores_app: Received event. event: PartsWritten {",
        "stores_app: Changed parts. source: '",
        "part_count: 2",
    ]);
    assert!(!trace.contains("Event failed."));

    let output = parts(temp_dir.path(), &parts_path, &[
        "set",
        "--manufacturer",
        "RESCO",
        "--mpn",
        "RES1",
        "--value",
        "Notes=1%",
    ])?;
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let output = parts(temp_dir.path(), &parts_path, &[
        "remove",
        "--manufacturer",
        "CAPCO",
        "--mpn",
        "CAP1",
    ])?;
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(read_to_string(&parts_path)?, indoc! {r#"
        "Manufacturer","Mpn","Package","LCSC","Notes"
        "RESCO","RES1","0402","C1","1%"
    "#});

    Ok(())
}

/// `STORE-15`, a change based on a revision other than the file's is refused, and the file isn't changed.
#[test]
fn refuses_a_change_based_on_another_revision() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let parts_path = temp_dir.path().join("parts.csv");
    let contents = "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n";
    write(&parts_path, contents)?;

    let output = parts(temp_dir.path(), &parts_path, &[
        "remove",
        "--manufacturer",
        "RESCO",
        "--mpn",
        "RES1",
        "--revision",
        "an-older-revision",
    ])?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("The parts have changed since the change was made."),
        "{stderr}"
    );
    assert!(stderr.contains("revision: 'an-older-revision'"), "{stderr}");
    assert_eq!(read_to_string(&parts_path)?, contents);

    let trace = read_to_string(temp_dir.path().join("trace.log"))?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: ChangeParts {",
        "stores_app: Event failed. error: PartsChanged {",
    ]);
    assert!(!trace.contains("operation: WriteParts {"));

    Ok(())
}

/// `STORE-15`, a part that's there already isn't added again.
#[test]
fn refuses_to_add_a_part_twice() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let parts_path = temp_dir.path().join("parts.csv");
    let contents = "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n";
    write(&parts_path, contents)?;

    let output = parts(temp_dir.path(), &parts_path, &[
        "add",
        "--manufacturer",
        "RESCO",
        "--mpn",
        "RES1",
    ])?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("The part already exists."), "{stderr}");
    assert_eq!(read_to_string(&parts_path)?, contents);

    Ok(())
}
