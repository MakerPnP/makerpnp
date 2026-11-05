//! The CLI's commands for a variant builder's file: `create`, `configure` and `build` (`VARBCLI`).

#[macro_use]
extern crate util;

use std::fs::{create_dir, read_to_string, write};
use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use indoc::indoc;
use tempfile::tempdir;

/// Runs the CLI in the folder, logging to `trace.log` in it, at debug level.
fn run(folder: &Path, args: &[&str]) -> Result<Output, anyhow::Error> {
    Ok(Command::cargo_bin("variantbuilder_cli")?
        .current_dir(folder)
        .args(["--trace", "trace.log", "-v"])
        .args(args)
        .output()?)
}

fn trace(folder: &Path) -> Result<String, anyhow::Error> {
    Ok(read_to_string(folder.join("trace.log"))?)
}

/// `VARBCLI-1`, `VARB-2`
#[test]
fn create_refuses_to_replace_a_file_without_overwrite() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let folder = temp_dir.path();
    let file = folder.join("job1.variant.json");

    let output = run(folder, &["create", "--file", "job1.variant.json"])?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let created = read_to_string(&file)?;
    assert!(created.contains("\"variant_name\": \"default\""), "{created}");
    assert_contains_inorder!(trace(folder)?, [
        "variantbuilder_app: Received event. event: Create {",
        "stores_service: Performing stores operation. operation: ReadDocument {",
        "stores_service: Performing stores operation. operation: WriteDocument {",
        "variantbuilder_app: Created variant builder. path: 'job1.variant.json'",
    ]);

    write(&file, "{}")?;
    let output = run(folder, &["create", "--file", "job1.variant.json"])?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("Variant builder exists already. path: 'job1.variant.json'"),
        "{stderr}"
    );
    assert_eq!(read_to_string(&file)?, "{}");
    assert_contains_inorder!(trace(folder)?, [
        "variantbuilder_app: Created variant builder.",
        "variantbuilder_app: Received event. event: Create {",
        "variantbuilder_app: Event failed. error: ExistsAlready {",
    ]);

    let output = run(folder, &["create", "--file", "job1.variant.json", "--overwrite"])?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(read_to_string(&file)?, created);

    Ok(())
}

/// `VARBCLI-2`, `VARB-4`, each path is relative to the variant builder's folder, as given from the folder above it, or
/// absolute if it's outside it, and only the options given are changed, an empty list or `--clear` clearing one.
#[test]
fn configure_sets_only_the_options_given() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let folder = temp_dir.path();
    create_dir(folder.join("job1"))?;
    let file = Path::new("job1").join("job1.variant.json");
    let file = file.to_str().unwrap();
    let outside = folder.join("shared").join("parts.csv");

    run(folder, &["create", "--file", file, "--variant-name", "Variant_1"])?;
    let output = run(folder, &[
        "configure",
        "--file",
        file,
        "--eda",
        "kicad",
        "--placements",
        Path::new("job1")
            .join("placements.csv")
            .to_str()
            .unwrap(),
        "--parts",
        outside.to_str().unwrap(),
        "--global-substitutions",
        "job1/a.csv,job1/b.csv",
        "--design-substitutions",
        "job1/c.csv",
        "--ref-des-exclusion-list",
        "TP1,TP2",
        "--ref-des-no-place-list",
        "R1",
    ])?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let configured: serde_json::Value = serde_json::from_str(&read_to_string(folder.join(file))?)?;
    assert_eq!(configured["variant_name"], "Variant_1");
    assert_eq!(configured["eda_tool"], "KiCad");
    assert_eq!(configured["placements"], "placements.csv");
    assert_eq!(configured["parts"], outside.to_str().unwrap());
    assert_eq!(
        configured["global_substitutions"],
        serde_json::json!(["a.csv", "b.csv"])
    );
    assert_eq!(configured["design_substitutions"], serde_json::json!(["c.csv"]));
    assert_eq!(configured["ref_des_exclusion_list"], serde_json::json!(["TP1", "TP2"]));
    assert_eq!(configured["ref_des_no_place_list"], serde_json::json!(["R1"]));
    assert_contains_inorder!(trace(folder)?, [
        "variantbuilder_app: Received event. event: Configure {",
        "variantbuilder_app: Configured variant builder. path: '",
        "change_count: 7",
    ]);

    let output = run(folder, &[
        "configure",
        "--file",
        file,
        "--ref-des-exclusion-list",
        "--clear",
        "eda,parts",
    ])?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let configured: serde_json::Value = serde_json::from_str(&read_to_string(folder.join(file))?)?;
    assert_eq!(configured["variant_name"], "Variant_1");
    assert_eq!(configured["eda_tool"], serde_json::Value::Null);
    assert_eq!(configured["placements"], "placements.csv");
    assert_eq!(configured["parts"], serde_json::Value::Null);
    assert_eq!(configured["ref_des_exclusion_list"], serde_json::json!([]));

    Ok(())
}

/// `VARBCLI-1`, `VARB-2`, a new variant builder is created with the options given, and its design is named after its
/// placements' file unless it's named.
#[test]
fn create_sets_the_options_given() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let folder = temp_dir.path();

    let output = run(folder, &[
        "create",
        "--file",
        "board1_default.variant.json",
        "--eda",
        "kicad",
        "--placements",
        "board1.csv",
    ])?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let created: serde_json::Value =
        serde_json::from_str(&read_to_string(folder.join("board1_default.variant.json"))?)?;
    assert_eq!(created["design_name"], "board1");
    assert_eq!(created["variant_name"], "default");
    assert_eq!(created["eda_tool"], "KiCad");
    assert_eq!(created["placements"], "board1.csv");

    Ok(())
}

/// `VARBCLI-4`, `VARB-8`
#[test]
fn create_assembly_rules_writes_a_file_with_none_and_refuses_to_replace_one() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let folder = temp_dir.path();
    run(folder, &["create", "--file", "job1.variant.json"])?;

    let output = run(folder, &[
        "create-assembly-rules",
        "--file",
        "job1.variant.json",
        "--assembly-rules",
        "rules.csv",
    ])?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        read_to_string(folder.join("rules.csv"))?,
        "\"RefDes\",\"Manufacturer\",\"Mpn\"\n"
    );
    let configured: serde_json::Value = serde_json::from_str(&read_to_string(folder.join("job1.variant.json"))?)?;
    assert_eq!(configured["assembly_rules"], "rules.csv");
    assert_contains_inorder!(trace(folder)?, [
        "variantbuilder_app: Received event. event: CreateAssemblyRules {",
        "variantbuilder_app: Created assembly rules. path: 'job1.variant.json', assembly_rules: 'rules.csv'",
    ]);

    write(folder.join("rules.csv"), "kept")?;
    let output = run(folder, &[
        "create-assembly-rules",
        "--file",
        "job1.variant.json",
        "--assembly-rules",
        "rules.csv",
    ])?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("Assembly rules exist already. path: 'rules.csv'"),
        "{stderr}"
    );
    assert_eq!(read_to_string(folder.join("rules.csv"))?, "kept");

    Ok(())
}

/// `VARBCLI-2`
#[test]
fn configure_says_a_variant_builder_that_does_not_exist_is_not_found() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let folder = temp_dir.path();

    let output = run(folder, &["configure", "--file", "job1.variant.json", "--eda", "kicad"])?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("Variant builder not found. path: 'job1.variant.json'"),
        "{stderr}"
    );
    assert!(!folder
        .join("job1.variant.json")
        .exists());

    Ok(())
}

/// `VARBCLI-3`, `VARB-5`
#[test]
fn build_names_what_a_variant_builder_is_missing() -> Result<(), anyhow::Error> {
    let temp_dir = tempdir()?;
    let folder = temp_dir.path();
    run(folder, &["create", "--file", "job1.variant.json"])?;
    run(folder, &["configure", "--file", "job1.variant.json", "--eda", "kicad"])?;

    let output = run(folder, &["build", "--file", "job1.variant.json"])?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains(indoc! {r#"
        Variant builder incomplete. path: 'job1.variant.json', missing: ["placements", "parts", "part_mappings"]"#}),
        "{stderr}"
    );
    assert_contains_inorder!(trace(folder)?, [
        "variantbuilder_app: Received event. event: Build {",
        "stores_service: Performing stores operation. operation: ReadDocument {",
        "variantbuilder_app: Event failed. error: Incomplete {",
    ]);
    assert!(!trace(folder)?.contains("operation: ReadRecords {"));

    Ok(())
}
