//! The CLI's table mode, which shows a data source's records as a table, read-only, with the stores core (`STORE-18`).

#[macro_use]
extern crate util;

use std::fs::{read_to_string, write};
use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use indoc::indoc;
use tempfile::tempdir;

/// Runs `planner_cli table` with the arguments, tracing to the file.
fn table(trace_path: &Path, args: &[&str]) -> Result<Output, anyhow::Error> {
    let mut cmd = Command::cargo_bin("planner_cli")?;
    let mut all_args = vec!["--trace", trace_path.to_str().unwrap(), "-v", "table"];
    all_args.extend_from_slice(args);
    Ok(cmd.args(all_args).output()?)
}

/// `STORE-17`, `STORE-18`, a BOM, written without headers, as JLCPCB's are, is listed with JLCPCB's columns, and its
/// values as they are.
#[test]
fn lists_a_bom() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let bom_path = temp_dir
        .path()
        .join("board1_default_bom.csv");
    write(&bom_path, indoc! {r#"
        "330R","R1,R2","RES_0402","C1234","02"
    "#})?;
    let trace_path = temp_dir.path().join("trace.log");

    let output = table(&trace_path, &[
        "--kind",
        "bom",
        "--source",
        bom_path.to_str().unwrap(),
        "list",
    ])?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout)?, indoc! {r#"
        "Comment","Designator","Footprint","JLCPCB Part","Quantity"
        "330R","R1,R2","RES_0402","C1234","02"
    "#});

    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: OpenTable {",
        "stores_service: Performing stores operation. operation: ResolveSource {",
        "stores_app: Received event. event: TableSourceResolved {",
        "stores_service: Performing stores operation. operation: ReadTable {",
        "stores_app: Received event. event: TableRead {",
        "stores_app: Loaded table. kind: Bom, source: '",
        "row_count: 1",
    ]);
    assert!(!trace.contains("Event failed."));
    // `STORE-9`, the CLI is done before the file could change, so it doesn't watch it.
    assert!(!trace.contains("Watching the data source."));

    Ok(())
}

/// `STORE-17`, `STORE-18`, EDA placements are read in the EDA tool's format.
#[test]
fn lists_eda_placements() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let placements_path = temp_dir.path().join("board1.csv");
    write(&placements_path, indoc! {r#"
        "RefDes","Name","Value","Side","X","Y","Rotation"
        "R1","RES_0402","330R","Top","10","20","90"
    "#})?;
    let trace_path = temp_dir.path().join("trace.log");

    let output = table(&trace_path, &[
        "--kind",
        "eda-placements",
        "--eda-tool",
        "diptrace",
        "--source",
        placements_path.to_str().unwrap(),
        "list",
    ])?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout)?, indoc! {r#"
        "RefDes","Place","Side","X","Y","Rotation","name","value"
        "R1","true","top","10","20","90","RES_0402","330R"
    "#});

    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: OpenTable { kind: EdaPlacements { eda_tool: DipTrace }",
        "stores_service: Performing stores operation. operation: ReadTable {",
        "stores_app: Loaded table. kind: EdaPlacements { eda_tool: DipTrace }, source: '",
    ]);

    Ok(())
}

/// `STORE-17`, `STORE-18`, a data source that doesn't exist, such as a BOM not yet built, fails, saying so.
#[test]
fn a_data_source_that_does_not_exist_fails() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let bom_path = temp_dir.path().join("missing_bom.csv");
    let trace_path = temp_dir.path().join("trace.log");

    let output = table(&trace_path, &[
        "--kind",
        "bom",
        "--source",
        bom_path.to_str().unwrap(),
        "list",
    ])?;

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Data source not found. source: '"), "{}", stderr);
    assert!(stderr.contains("missing_bom.csv"), "{}", stderr);

    Ok(())
}

/// `STORE-18`, EDA placements need their EDA tool.
#[test]
fn eda_placements_need_their_eda_tool() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let trace_path = temp_dir.path().join("trace.log");

    let output = table(&trace_path, &[
        "--kind",
        "eda-placements",
        "--source",
        "board1.csv",
        "list",
    ])?;

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--eda-tool"));

    Ok(())
}
