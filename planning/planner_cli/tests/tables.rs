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

/// `STORE-17`, `STORE-18`, a BOM, written without headers, as JLCPCB's are, is listed with JLCPCB's columns, the comment
/// last, and its values as they are.
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
        "Designator","Footprint","JLCPCB Part","Quantity","Comment"
        "R1, R2","RES_0402","C1234","2","330R"
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

/// Part mappings with a column the table doesn't show, `Notes`.
const MAPPINGS: &str = indoc! {r#"
    "Eda","Name","Value","Manufacturer","Mpn","Notes"
    "DipTrace","R1","330R","MFR1","MPN1","note 1"
    "DipTrace","R2","","MFR1","MPN2","note 2"
"#};

/// Runs `planner_cli table --kind part-mappings --source <file>` with the arguments, failing if it does, and returns
/// what it wrote to standard output.
fn change_mappings(mappings_path: &Path, trace_path: &Path, args: &[&str]) -> Result<String, anyhow::Error> {
    let mut all_args = vec!["--kind", "part-mappings", "--source", mappings_path.to_str().unwrap()];
    all_args.extend_from_slice(args);
    let output = table(trace_path, &all_args)?;
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    Ok(String::from_utf8(output.stdout)?)
}

/// `STORE-20`, part mappings are listed with their keys, then a row is added, has its values set, is moved before
/// another and to the end, and is removed, each named by its key, and the column the table doesn't show is untouched.
#[test]
fn adds_sets_moves_and_removes_part_mappings() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let mappings_path = temp_dir
        .path()
        .join("part-mappings.csv");
    write(&mappings_path, MAPPINGS)?;
    let trace_path = temp_dir.path().join("trace.log");

    let listed = change_mappings(&mappings_path, &trace_path, &["list", "--keys"])?;
    assert_eq!(listed, indoc! {r#"
        "Key","Eda","Name","Value","Manufacturer","Mpn"
        "0","DipTrace","R1","330R","MFR1","MPN1"
        "1","DipTrace","R2","","MFR1","MPN2"
    "#});

    change_mappings(&mappings_path, &trace_path, &[
        "add",
        "--eda",
        "diptrace",
        "--pattern",
        "Name=/C.*/",
        "--manufacturer",
        "MFR2",
        "--mpn",
        "MPN3",
    ])?;
    assert_eq!(read_to_string(&mappings_path)?, indoc! {r#"
        "Eda","Name","Value","Manufacturer","Mpn","Notes"
        "DipTrace","R1","330R","MFR1","MPN1","note 1"
        "DipTrace","R2","","MFR1","MPN2","note 2"
        "DipTrace","/C.*/","","MFR2","MPN3",""
    "#});
    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: ChangeTable {",
        "stores_service: Performing stores operation. operation: ChangeRows {",
        "Changing part mappings. source: '",
        "stores_app: Changed table. kind: PartMappings, source: '",
        "stores_service: Performing stores operation. operation: ReadTable {",
    ]);
    assert!(!trace.contains("Event failed."));

    // as the user saw it, so it's set only if it's still that.
    change_mappings(&mappings_path, &trace_path, &[
        "set",
        "--row",
        "1",
        "--pattern",
        "value=10K",
        "--mpn",
        "MPN5",
        "--was-mpn",
        "MPN2",
    ])?;
    assert_eq!(read_to_string(&mappings_path)?, indoc! {r#"
        "Eda","Name","Value","Manufacturer","Mpn","Notes"
        "DipTrace","R1","330R","MFR1","MPN1","note 1"
        "DipTrace","R2","10K","MFR1","MPN5","note 2"
        "DipTrace","/C.*/","","MFR2","MPN3",""
    "#});

    change_mappings(&mappings_path, &trace_path, &["move", "--row", "2", "--before", "0"])?;
    assert_eq!(read_to_string(&mappings_path)?, indoc! {r#"
        "Eda","Name","Value","Manufacturer","Mpn","Notes"
        "DipTrace","/C.*/","","MFR2","MPN3",""
        "DipTrace","R1","330R","MFR1","MPN1","note 1"
        "DipTrace","R2","10K","MFR1","MPN5","note 2"
    "#});

    change_mappings(&mappings_path, &trace_path, &["move", "--row", "1", "--to-end"])?;
    assert_eq!(read_to_string(&mappings_path)?, indoc! {r#"
        "Eda","Name","Value","Manufacturer","Mpn","Notes"
        "DipTrace","/C.*/","","MFR2","MPN3",""
        "DipTrace","R2","10K","MFR1","MPN5","note 2"
        "DipTrace","R1","330R","MFR1","MPN1","note 1"
    "#});

    change_mappings(&mappings_path, &trace_path, &[
        "remove",
        "--row",
        "0",
        "--was-pattern",
        "name=/C.*/",
    ])?;
    assert_eq!(read_to_string(&mappings_path)?, indoc! {r#"
        "Eda","Name","Value","Manufacturer","Mpn","Notes"
        "DipTrace","R2","10K","MFR1","MPN5","note 2"
        "DipTrace","R1","330R","MFR1","MPN1","note 1"
    "#});

    Ok(())
}

/// `STORE-20`, a change whose `--was-` values aren't the row's, a row that isn't there, and a change to a table that
/// can't be changed, such as a BOM, fail, saying why, and leave the file as it was.
#[test]
fn a_change_that_is_refused_fails_and_leaves_the_file() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let mappings_path = temp_dir
        .path()
        .join("part-mappings.csv");
    write(&mappings_path, MAPPINGS)?;
    let trace_path = temp_dir.path().join("trace.log");
    let mappings = |args: &[&str]| {
        let mut all_args = vec!["--kind", "part-mappings", "--source", mappings_path.to_str().unwrap()];
        all_args.extend_from_slice(args);
        table(&trace_path, &all_args)
    };

    let cases: [(&[&str], &str); 3] = [
        (
            &["set", "--row", "1", "--mpn", "MPN5", "--was-mpn", "MPN7"],
            "The table has changed since the change was made. kind: PartMappings, source: '",
        ),
        (
            &["remove", "--row", "0", "--was-manufacturer", "MFR7"],
            "The table has changed since the change was made. kind: PartMappings, source: '",
        ),
        (&["move", "--row", "5", "--to-end"], "The row doesn't exist. key: '5'"),
    ];
    for (args, expected) in cases {
        let output = mappings(args)?;

        assert!(!output.status.success(), "{:?}", args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "{:?}: {}", args, stderr);
        assert_eq!(read_to_string(&mappings_path)?, MAPPINGS, "{:?}", args);
    }

    let bom_path = temp_dir
        .path()
        .join("board1_default_bom.csv");
    let bom = "\"330R\",\"R1,R2\",\"RES_0402\",\"C1234\",\"02\"\n";
    write(&bom_path, bom)?;
    for args in [
        &["add", "--eda", "diptrace", "--manufacturer", "MFR1", "--mpn", "MPN1"][..],
        &["remove", "--row", "0"][..],
    ] {
        let mut all_args = vec!["--kind", "bom", "--source", bom_path.to_str().unwrap()];
        all_args.extend_from_slice(args);
        let output = table(&trace_path, &all_args)?;

        assert!(!output.status.success(), "{:?}", args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("The table can't be changed. kind: Bom, source: '"),
            "{:?}: {}",
            args,
            stderr
        );
        assert_eq!(read_to_string(&bom_path)?, bom, "{:?}", args);
    }

    Ok(())
}

/// `STORE-20`, `STORE-21`, an EDA tool's columns are added, those its rows need that the table doesn't have, a column
/// shared by tools added once, after the last pattern's column, as the part's columns are after every pattern's, saying
/// which, and a row of that tool is then added; columns
/// already there need none; and named columns are added where they're to go.
#[test]
fn adds_columns_then_a_row_of_another_eda_tool() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let mappings_path = temp_dir
        .path()
        .join("part-mappings.csv");
    write(&mappings_path, MAPPINGS)?;
    let trace_path = temp_dir.path().join("trace.log");

    // `Value` is DipTrace's, so only `Device` is EasyEDA's to add.
    let said = change_mappings(&mappings_path, &trace_path, &["add-columns", "--eda", "easyeda"])?;
    assert_eq!(said, "Added columns. columns: ['Device']\n");
    assert_eq!(read_to_string(&mappings_path)?, indoc! {r#"
        "Eda","Name","Value","Device","Manufacturer","Mpn","Notes"
        "DipTrace","R1","330R","","MFR1","MPN1","note 1"
        "DipTrace","R2","","","MFR1","MPN2","note 2"
    "#});
    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: AddTableColumns {",
        "stores_service: Performing stores operation. operation: AddColumns {",
        "Adding columns. source: '",
        "columns: ['Device']",
        "stores_app: Added table columns. kind: PartMappings, source: '",
        "columns: ['Device'], revision: '",
        "stores_service: Performing stores operation. operation: ReadTable {",
    ]);
    assert!(!trace.contains("Event failed."));

    change_mappings(&mappings_path, &trace_path, &[
        "add",
        "--eda",
        "easyeda",
        "--pattern",
        "device=RES",
        "--pattern",
        "value=10k",
        "--manufacturer",
        "MFR2",
        "--mpn",
        "MPN3",
    ])?;
    assert_eq!(read_to_string(&mappings_path)?, indoc! {r#"
        "Eda","Name","Value","Device","Manufacturer","Mpn","Notes"
        "DipTrace","R1","330R","","MFR1","MPN1","note 1"
        "DipTrace","R2","","","MFR1","MPN2","note 2"
        "EasyEda","","10k","RES","MFR2","MPN3",""
    "#});

    let said = change_mappings(&mappings_path, &trace_path, &["add-columns", "--eda", "diptrace"])?;
    assert_eq!(said, "No columns needed.\n");
    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: AddTableColumns {",
        "stores_app: Table columns already there. kind: PartMappings, source: '",
        "eda_tool: DipTrace",
    ]);

    let said = change_mappings(&mappings_path, &trace_path, &[
        "add-columns",
        "--column",
        "Status",
        "--column",
        "Owner",
        "--after",
        "Notes",
    ])?;
    assert_eq!(said, "Added columns. columns: ['Status', 'Owner']\n");
    assert_eq!(read_to_string(&mappings_path)?, indoc! {r#"
        "Eda","Name","Value","Device","Manufacturer","Mpn","Notes","Status","Owner"
        "DipTrace","R1","330R","","MFR1","MPN1","note 1","",""
        "DipTrace","R2","","","MFR1","MPN2","note 2","",""
        "EasyEda","","10k","RES","MFR2","MPN3","","",""
    "#});

    Ok(())
}

/// `STORE-21`, the EDA tools' columns of a file whose part's columns aren't after every pattern's column are added after
/// its last column, KiCad's, then EasyEDA's after KiCad's, every other column where it was.
#[test]
fn an_eda_tools_columns_are_added_last_if_the_parts_are_not_after_the_patterns() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let mappings_path = temp_dir
        .path()
        .join("part-mappings.csv");
    write(&mappings_path, indoc! {r#"
        "Eda","Manufacturer","Mpn","Name","Value","Notes"
        "DipTrace","MFR1","MPN1","R1","330R","note 1"
    "#})?;
    let trace_path = temp_dir.path().join("trace.log");

    change_mappings(&mappings_path, &trace_path, &["add-columns", "--eda", "kicad"])?;
    change_mappings(&mappings_path, &trace_path, &["add-columns", "--eda", "easyeda"])?;

    assert_eq!(read_to_string(&mappings_path)?, indoc! {r#"
        "Eda","Manufacturer","Mpn","Name","Value","Notes","Package","Val","Device"
        "DipTrace","MFR1","MPN1","R1","330R","note 1","","",""
    "#});
    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_service: Performing stores operation. operation: AddColumns {",
        "position: EdaToolColumns",
        "stores_service: Performing stores operation. operation: AddColumns {",
        "position: EdaToolColumns",
    ]);
    assert!(!trace.contains("Event failed."));

    Ok(())
}

/// `STORE-20`, `STORE-21`, a pattern for a field the data source has no column for fails, a column already there, or
/// to go by one that isn't, fails, saying why, as does adding columns to a table that can't be changed, and named
/// columns need where they go, the file left as it was.
#[test]
fn adding_columns_that_can_not_be_added_fails() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let mappings_path = temp_dir
        .path()
        .join("part-mappings.csv");
    write(&mappings_path, MAPPINGS)?;
    let trace_path = temp_dir.path().join("trace.log");
    let mappings = |args: &[&str]| {
        let mut all_args = vec!["--kind", "part-mappings", "--source", mappings_path.to_str().unwrap()];
        all_args.extend_from_slice(args);
        table(&trace_path, &all_args)
    };

    let cases: [(&[&str], &str); 5] = [
        (
            &[
                "add",
                "--eda",
                "kicad",
                "--pattern",
                "package=R_0402",
                "--manufacturer",
                "MFR2",
                "--mpn",
                "MPN3",
            ],
            "The column doesn't exist. kind: PartMappings, source: '",
        ),
        (
            &["add-columns", "--column", "notes", "--last"],
            "The column already exists. kind: PartMappings, source: '",
        ),
        (
            &["add-columns", "--column", "Status", "--before", "Comment"],
            "The column doesn't exist. kind: PartMappings, source: '",
        ),
        (&["add-columns", "--column", "Status"], "--first"),
        (&["add-columns", "--eda", "kicad", "--last"], "--last"),
    ];
    for (args, expected) in cases {
        let output = mappings(args)?;

        assert!(!output.status.success(), "{:?}", args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "{:?}: {}", args, stderr);
        assert_eq!(read_to_string(&mappings_path)?, MAPPINGS, "{:?}", args);
    }

    let bom_path = temp_dir
        .path()
        .join("board1_default_bom.csv");
    let bom = "\"330R\",\"R1,R2\",\"RES_0402\",\"C1234\",\"02\"\n";
    write(&bom_path, bom)?;
    let output = table(&trace_path, &[
        "--kind",
        "bom",
        "--source",
        bom_path.to_str().unwrap(),
        "add-columns",
        "--column",
        "Notes",
        "--last",
    ])?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("The table can't be changed. kind: Bom, source: '"),
        "{}",
        stderr
    );
    assert_eq!(read_to_string(&bom_path)?, bom);

    Ok(())
}

/// Substitutions, DipTrace's, with a comment.
const SUBSTITUTIONS: &str = indoc! {r#"
    "Eda","NamePattern","ValuePattern","Name","Value","Comment"
    "DipTrace","R0402","","RES_0402","","comment 1"
    "DipTrace","/C.*/","100nF","CAP_0402","","DERATE 50V"
"#};

/// Runs `planner_cli table --kind substitutions --source <file>` with the arguments, failing if it does, and returns
/// what it wrote to standard output.
fn change_substitutions(substitutions_path: &Path, trace_path: &Path, args: &[&str]) -> Result<String, anyhow::Error> {
    let mut all_args = vec![
        "--kind",
        "substitutions",
        "--source",
        substitutions_path.to_str().unwrap(),
    ];
    all_args.extend_from_slice(args);
    let output = table(trace_path, &all_args)?;
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    Ok(String::from_utf8(output.stdout)?)
}

/// `STORE-20`, `STORE-21`, substitutions are listed with their keys, then a row is added with its patterns, values and
/// comment, has its values set, as the user saw them, is moved and removed, and KiCad's columns are added, then a KiCad
/// row.
#[test]
fn adds_sets_moves_and_removes_substitutions() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let substitutions_path = temp_dir
        .path()
        .join("substitutions.csv");
    write(&substitutions_path, SUBSTITUTIONS)?;
    let trace_path = temp_dir.path().join("trace.log");

    let listed = change_substitutions(&substitutions_path, &trace_path, &["list", "--keys"])?;
    assert_eq!(listed, indoc! {r#"
        "Key","Eda","NamePattern","ValuePattern","Name","Value","Comment"
        "0","DipTrace","R0402","","RES_0402","","comment 1"
        "1","DipTrace","/C.*/","100nF","CAP_0402","","DERATE 50V"
    "#});

    change_substitutions(&substitutions_path, &trace_path, &[
        "add",
        "--eda",
        "diptrace",
        "--pattern",
        "name=R0603",
        "--value",
        "Name=RES_0603",
        "--comment",
        "comment 3",
    ])?;
    assert_eq!(read_to_string(&substitutions_path)?, indoc! {r#"
        "Eda","NamePattern","ValuePattern","Name","Value","Comment"
        "DipTrace","R0402","","RES_0402","","comment 1"
        "DipTrace","/C.*/","100nF","CAP_0402","","DERATE 50V"
        "DipTrace","R0603","","RES_0603","","comment 3"
    "#});
    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: ChangeTable {",
        "stores_service: Performing stores operation. operation: ChangeRows {",
        "Changing eda substitutions. source: '",
        "stores_app: Changed table. kind: Substitutions, source: '",
        "stores_service: Performing stores operation. operation: ReadTable {",
    ]);
    assert!(!trace.contains("Event failed."));

    // as the user saw it, so it's set only if it's still that.
    change_substitutions(&substitutions_path, &trace_path, &[
        "set",
        "--row",
        "0",
        "--value",
        "value=100R",
        "--comment",
        "comment 1b",
        "--was-value",
        "value=",
        "--was-comment",
        "comment 1",
    ])?;
    assert_eq!(read_to_string(&substitutions_path)?, indoc! {r#"
        "Eda","NamePattern","ValuePattern","Name","Value","Comment"
        "DipTrace","R0402","","RES_0402","100R","comment 1b"
        "DipTrace","/C.*/","100nF","CAP_0402","","DERATE 50V"
        "DipTrace","R0603","","RES_0603","","comment 3"
    "#});

    change_substitutions(&substitutions_path, &trace_path, &[
        "move", "--row", "2", "--before", "0",
    ])?;
    change_substitutions(&substitutions_path, &trace_path, &[
        "remove",
        "--row",
        "2",
        "--was-value",
        "name=CAP_0402",
    ])?;
    assert_eq!(read_to_string(&substitutions_path)?, indoc! {r#"
        "Eda","NamePattern","ValuePattern","Name","Value","Comment"
        "DipTrace","R0603","","RES_0603","","comment 3"
        "DipTrace","R0402","","RES_0402","100R","comment 1b"
    "#});

    let said = change_substitutions(&substitutions_path, &trace_path, &["add-columns", "--eda", "kicad"])?;
    assert_eq!(
        said,
        "Added columns. columns: ['PackagePattern', 'ValPattern', 'Package', 'Val']\n"
    );
    change_substitutions(&substitutions_path, &trace_path, &[
        "add",
        "--eda",
        "kicad",
        "--pattern",
        "package=R_0402",
        "--pattern",
        "val=",
        "--value",
        "package=RES_0402",
        "--value",
        "val=10K",
    ])?;
    assert_eq!(read_to_string(&substitutions_path)?, indoc! {r#"
        "Eda","NamePattern","ValuePattern","PackagePattern","ValPattern","Name","Value","Package","Val","Comment"
        "DipTrace","R0603","","","","RES_0603","","","","comment 3"
        "DipTrace","R0402","","","","RES_0402","100R","","","comment 1b"
        "KiCad","","","R_0402","","","","RES_0402","10K",""
    "#});
    let trace = read_to_string(&trace_path)?;
    assert_contains_inorder!(trace, [
        "stores_app: Received event. event: AddTableColumns {",
        "stores_service: Performing stores operation. operation: AddColumns {",
        "position: EdaToolColumns",
        "stores_app: Added table columns. kind: Substitutions, source: '",
        "stores_app: Received event. event: ChangeTable {",
        "stores_app: Changed table. kind: Substitutions, source: '",
    ]);
    assert!(!trace.contains("Event failed."));

    Ok(())
}

/// `STORE-20`, a part's options for a substitution, a substitution's for a part mapping, a part mapping without its
/// part, and a substitution whose `--was-` values aren't the row's, fail, saying why, and leave the file as it was.
#[test]
fn options_not_for_the_tables_kind_fail_and_leave_the_file() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let substitutions_path = temp_dir
        .path()
        .join("substitutions.csv");
    write(&substitutions_path, SUBSTITUTIONS)?;
    let mappings_path = temp_dir
        .path()
        .join("part-mappings.csv");
    write(&mappings_path, MAPPINGS)?;
    let trace_path = temp_dir.path().join("trace.log");

    let cases: [(&str, &Path, &str, &[&str], &str); 7] = [
        (
            "substitutions",
            &substitutions_path,
            SUBSTITUTIONS,
            &[
                "add",
                "--eda",
                "diptrace",
                "--pattern",
                "name=R1",
                "--manufacturer",
                "MFR1",
            ],
            "Option not for the table's kind. option: '--manufacturer', kind: Substitutions",
        ),
        (
            "substitutions",
            &substitutions_path,
            SUBSTITUTIONS,
            &["set", "--row", "0", "--mpn", "MPN1"],
            "Option not for the table's kind. option: '--mpn', kind: Substitutions",
        ),
        (
            "substitutions",
            &substitutions_path,
            SUBSTITUTIONS,
            &["remove", "--row", "0", "--was-manufacturer", "MFR1"],
            "Option not for the table's kind. option: '--was-manufacturer', kind: Substitutions",
        ),
        (
            "substitutions",
            &substitutions_path,
            SUBSTITUTIONS,
            &[
                "set",
                "--row",
                "0",
                "--comment",
                "comment 2",
                "--was-comment",
                "comment 7",
            ],
            "The table has changed since the change was made. kind: Substitutions, source: '",
        ),
        (
            "part-mappings",
            &mappings_path,
            MAPPINGS,
            &[
                "add",
                "--eda",
                "diptrace",
                "--manufacturer",
                "MFR1",
                "--mpn",
                "MPN1",
                "--value",
                "name=R1",
            ],
            "Option not for the table's kind. option: '--value', kind: PartMappings",
        ),
        (
            "part-mappings",
            &mappings_path,
            MAPPINGS,
            &["set", "--row", "0", "--mpn", "MPN2", "--was-comment", "comment 1"],
            "Option not for the table's kind. option: '--was-comment', kind: PartMappings",
        ),
        (
            "part-mappings",
            &mappings_path,
            MAPPINGS,
            &["add", "--eda", "diptrace", "--mpn", "MPN1"],
            "Option needed for the table's kind. option: '--manufacturer', kind: PartMappings",
        ),
    ];
    for (kind, path, contents, args, expected) in cases {
        let mut all_args = vec!["--kind", kind, "--source", path.to_str().unwrap()];
        all_args.extend_from_slice(args);
        let output = table(&trace_path, &all_args)?;

        assert!(!output.status.success(), "{:?}", args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "{:?}: {}", args, stderr);
        assert_eq!(read_to_string(path)?, contents, "{:?}", args);
    }

    Ok(())
}
