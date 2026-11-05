//! The Variant Builder's core, building from the arguments given, with no files (`STORE-7`, `STORE-14`).

mod common;

use std::path::{Path, PathBuf};

use common::{shell_with_a_design, source, BOM, PLACEMENTS};
use variantbuilder_app::{AssemblyVariant, EdaTool, Event};

fn build(output_bom: Option<&str>) -> Event {
    Event::BuildWith {
        eda_tool: EdaTool::DipTrace,
        placements: source("placements.csv"),
        assembly_variant: AssemblyVariant::new("Variant 1".to_string(), vec![]),
        parts: source("parts.csv"),
        part_mappings: source("part-mappings.csv"),
        substitutions: vec![source("substitutions.csv")],
        load_out: Some(source("load-out.csv")),
        assembly_rules: Some(source("assembly-rules.csv")),
        output: "out/placements.csv".to_string(),
        output_bom: output_bom.map(str::to_string),
        ref_des_exclude_list: vec![],
        ref_des_disable_list: vec![],
    }
}

/// `STORE-14`
#[test]
fn building_reads_each_data_source_then_writes_the_placements_and_the_bom() {
    let mut shell = shell_with_a_design();

    shell.process(build(Some("out/bom.csv")));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.done(), vec![
        format!(
            "read EdaPlacements {{ eda_tool: DipTrace }} {}",
            source("placements.csv")
        ),
        format!("read Substitutions {}", source("substitutions.csv")),
        format!("read parts {}", source("parts.csv")),
        format!("read PartMappings {}", source("part-mappings.csv")),
        format!("read LoadOut {}", source("load-out.csv")),
        format!("read AssemblyRules {}", source("assembly-rules.csv")),
        format!("write {}", Path::new("out/placements.csv").display()),
        format!("write {}", Path::new("out/bom.csv").display()),
    ]);
    // the substitution is made before the part is mapped, which it's mapped by.
    assert_eq!(shell.documents[Path::new("out/placements.csv")], PLACEMENTS);
    assert_eq!(shell.documents[Path::new("out/bom.csv")], BOM);
}

/// `STORE-14`
#[test]
fn building_with_no_bom_writes_only_the_placements() {
    let mut shell = shell_with_a_design();

    shell.process(build(None));

    assert_eq!(shell.error(), None);
    assert_eq!(
        shell
            .documents
            .keys()
            .collect::<Vec<_>>(),
        vec![Path::new("out/placements.csv")]
    );
}

/// `STORE-14`
#[test]
fn a_data_source_that_does_not_exist_says_so_and_nothing_is_written() {
    let mut shell = shell_with_a_design();
    shell
        .records
        .remove(&source("part-mappings.csv"));

    shell.process(build(Some("out/bom.csv")));

    assert_eq!(
        shell.error(),
        Some(format!(
            "Data source not found. source: '{}'",
            source("part-mappings.csv")
        ))
    );
    // the data sources after it aren't read.
    assert_eq!(
        shell.done().last().unwrap(),
        &format!("read PartMappings {}", source("part-mappings.csv"))
    );
    assert!(shell.documents.is_empty());
}

/// `STORE-14`
#[test]
fn placements_that_can_not_be_written_say_why_and_the_bom_is_not_written() {
    let mut shell = shell_with_a_design();
    shell
        .refused
        .insert(PathBuf::from("out/placements.csv"), "Access denied.".to_string());

    shell.process(build(Some("out/bom.csv")));

    assert_eq!(shell.error(), Some("Access denied.".to_string()));
    assert!(shell.documents.is_empty());
}
