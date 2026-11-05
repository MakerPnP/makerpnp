//! A variant builder's file, created, opened, configured and built from by the Variant Builder's core, with no files
//! (`VARB`).

mod common;

use std::path::{Path, PathBuf};

use common::{revision, shell_with_a_design, source, Shell, BOM, PLACEMENTS};
use variantbuilder_app::{EdaTool, Event, Variant, VariantBuilderChange, VariantBuilderFile, VariantBuilderView};

const PATH: &str = "in/job1.variant.json";

/// A new variant builder's file.
const NEW: &str = r#"{
    "variant": {
        "name": "Default",
        "ref_des_list": []
    },
    "eda_tool": null,
    "placements": null,
    "parts": null,
    "part_mappings": null,
    "substitutions": [],
    "load_out": null,
    "assembly_rules": null,
    "ref_des_exclude_list": [],
    "ref_des_disable_list": [],
    "output_placements": null,
    "output_bom": null
}"#;

fn path() -> PathBuf {
    PathBuf::from(PATH)
}

fn create(overwrite: bool) -> Event {
    Event::Create {
        path: path(),
        overwrite,
    }
}

fn open() -> Event {
    Event::Open {
        path: path(),
    }
}

fn configure(changes: Vec<VariantBuilderChange>) -> Event {
    Event::Configure {
        path: path(),
        changes,
    }
}

fn build() -> Event {
    Event::Build {
        path: path(),
    }
}

fn view(shell: &Shell) -> Option<VariantBuilderView> {
    shell
        .core
        .view()
        .variant_builders
        .get(&path())
        .cloned()
}

/// A variant builder of the design, with its data sources relative to its folder, `in`.
fn complete() -> VariantBuilderFile {
    VariantBuilderFile {
        variant: Variant {
            name: "Variant 1".to_string(),
            ref_des_list: vec![],
        },
        eda_tool: Some(EdaTool::DipTrace),
        placements: Some(PathBuf::from("placements.csv")),
        parts: Some(PathBuf::from("parts.csv")),
        part_mappings: Some(PathBuf::from("part-mappings.csv")),
        substitutions: vec![PathBuf::from("substitutions.csv")],
        load_out: Some(PathBuf::from("load-out.csv")),
        assembly_rules: Some(PathBuf::from("assembly-rules.csv")),
        ..VariantBuilderFile::default()
    }
}

fn given(shell: &mut Shell, file: &VariantBuilderFile) {
    shell
        .documents
        .insert(path(), file.contents().unwrap());
}

/// `VARB-1`, `VARB-2`
#[test]
fn creating_writes_a_new_variant_builders_file_and_opens_it() {
    let mut shell = Shell::default();

    shell.process(create(false));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.done(), vec![
        format!("read {}", path().display()),
        format!("write {}", path().display())
    ]);
    assert_eq!(shell.documents[&path()], NEW);
    assert_eq!(
        view(&shell),
        Some(VariantBuilderView {
            file: VariantBuilderFile::default(),
            revision: revision(NEW),
        })
    );
}

/// `VARB-2`
#[test]
fn creating_refuses_to_replace_a_file_unless_asked_to() {
    let mut shell = Shell::default();
    shell
        .documents
        .insert(path(), "{}".to_string());

    shell.process(create(false));

    assert_eq!(
        shell.error(),
        Some(format!("Variant builder exists already. path: '{}'", path().display()))
    );
    assert_eq!(shell.documents[&path()], "{}");
    assert_eq!(view(&shell), None);

    shell.process(create(true));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.documents[&path()], NEW);
    // the file isn't read when it's to be replaced.
    assert_eq!(shell.done().last().unwrap(), &format!("write {}", path().display()));
    assert!(view(&shell).is_some());
}

/// `VARB-2`
#[test]
fn a_variant_builder_whose_file_can_not_be_written_is_not_created() {
    let mut shell = Shell::default();
    shell
        .refused
        .insert(path(), "Access denied.".to_string());

    shell.process(create(false));

    assert_eq!(shell.error(), Some("Access denied.".to_string()));
    assert_eq!(view(&shell), None);
}

/// `VARB-3`
#[test]
fn opening_reads_the_file_and_the_view_has_what_it_says() {
    let mut shell = Shell::default();
    given(&mut shell, &complete());

    shell.process(open());

    assert_eq!(shell.error(), None);
    assert_eq!(shell.done(), vec![format!("read {}", path().display())]);
    assert_eq!(
        view(&shell),
        Some(VariantBuilderView {
            file: complete(),
            revision: revision(&complete().contents().unwrap()),
        })
    );
}

/// `VARB-3`
#[test]
fn a_variant_builder_that_does_not_exist_or_is_not_one_is_not_opened() {
    let mut shell = Shell::default();

    shell.process(open());

    assert_eq!(
        shell.error(),
        Some(format!("Variant builder not found. path: '{}'", path().display()))
    );
    assert_eq!(view(&shell), None);

    shell
        .documents
        .insert(path(), "not json".to_string());

    shell.process(open());

    let error = shell.error().unwrap();
    assert!(
        error.starts_with(&format!(
            "Invalid variant builder. path: '{}', cause: '",
            path().display()
        )),
        "{error}"
    );
    assert_eq!(view(&shell), None);
}

/// `VARB-1`
#[test]
fn fields_left_out_of_the_file_are_as_they_are_when_it_is_new() {
    let mut shell = Shell::default();
    shell.documents.insert(
        path(),
        r#"{ "variant": { "name": "Variant 1" }, "eda_tool": "KiCad" }"#.to_string(),
    );

    shell.process(open());

    assert_eq!(view(&shell).unwrap().file, VariantBuilderFile {
        variant: Variant {
            name: "Variant 1".to_string(),
            ref_des_list: vec![],
        },
        eda_tool: Some(EdaTool::KiCad),
        ..VariantBuilderFile::default()
    });
}

/// `VARB-4`
#[test]
fn configuring_makes_the_changes_keeps_the_rest_and_updates_the_view() {
    let mut shell = Shell::default();
    given(&mut shell, &complete());
    shell.process(open());

    shell.process(configure(vec![
        VariantBuilderChange::VariantName("Variant 2".to_string()),
        VariantBuilderChange::LoadOut(None),
        VariantBuilderChange::RefDesExcludeList(vec!["TP1".to_string(), "FID1".to_string()]),
    ]));

    assert_eq!(shell.error(), None);
    let expected = VariantBuilderFile {
        variant: Variant {
            name: "Variant 2".to_string(),
            ref_des_list: vec![],
        },
        load_out: None,
        ref_des_exclude_list: vec!["TP1".to_string(), "FID1".to_string()],
        ..complete()
    };
    assert_eq!(shell.documents[&path()], expected.contents().unwrap());
    assert_eq!(&shell.done()[1..], &[
        format!("read {}", path().display()),
        format!("write {}", path().display())
    ]);
    assert_eq!(
        view(&shell),
        Some(VariantBuilderView {
            revision: revision(&expected.contents().unwrap()),
            file: expected,
        })
    );
}

/// `VARB-4`, the file is read again, so a change made by anyone since it was opened is kept, and one that isn't open
/// isn't opened.
#[test]
fn configuring_keeps_a_change_made_since_the_file_was_opened() {
    let mut shell = Shell::default();
    given(&mut shell, &complete());
    let changed = VariantBuilderFile {
        eda_tool: Some(EdaTool::KiCad),
        ..complete()
    };
    given(&mut shell, &changed);

    shell.process(configure(vec![VariantBuilderChange::OutputBom(Some(PathBuf::from(
        "bom.csv",
    )))]));

    assert_eq!(shell.error(), None);
    assert_eq!(
        shell.documents[&path()],
        VariantBuilderFile {
            output_bom: Some(PathBuf::from("bom.csv")),
            ..changed
        }
        .contents()
        .unwrap()
    );
    assert_eq!(view(&shell), None);
}

/// `VARB-4`
#[test]
fn configuring_a_variant_builder_that_does_not_exist_writes_nothing() {
    let mut shell = Shell::default();

    shell.process(configure(vec![VariantBuilderChange::EdaTool(Some(EdaTool::KiCad))]));

    assert_eq!(
        shell.error(),
        Some(format!("Variant builder not found. path: '{}'", path().display()))
    );
    assert!(shell.documents.is_empty());
}

/// `VARB-5`, `STORE-14`
#[test]
fn building_reads_the_file_then_its_data_sources_relative_to_its_folder() {
    let mut shell = shell_with_a_design();
    given(&mut shell, &complete());

    shell.process(build());

    assert_eq!(shell.error(), None);
    assert_eq!(shell.done(), vec![
        format!("read {}", path().display()),
        format!(
            "read EdaPlacements {{ eda_tool: DipTrace }} {}",
            source("placements.csv")
        ),
        format!("read Substitutions {}", source("substitutions.csv")),
        format!("read parts {}", source("parts.csv")),
        format!("read PartMappings {}", source("part-mappings.csv")),
        format!("read LoadOut {}", source("load-out.csv")),
        format!("read AssemblyRules {}", source("assembly-rules.csv")),
        format!(
            "write {}",
            Path::new("in")
                .join("job1_placements.csv")
                .display()
        ),
        format!(
            "write {}",
            Path::new("in")
                .join("job1_bom.csv")
                .display()
        ),
    ]);
    // the substitution is made before the part is mapped, which it's mapped by.
    assert_eq!(
        shell.documents[&Path::new("in").join("job1_placements.csv")],
        PLACEMENTS
    );
    assert_eq!(shell.documents[&Path::new("in").join("job1_bom.csv")], BOM);
}

/// `STORE-14`
#[test]
fn a_data_source_that_does_not_exist_says_so_and_nothing_is_written() {
    let mut shell = shell_with_a_design();
    given(&mut shell, &complete());
    shell
        .records
        .remove(&source("part-mappings.csv"));

    shell.process(build());

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
    assert_eq!(
        shell
            .documents
            .keys()
            .collect::<Vec<_>>(),
        vec![&path()]
    );
}

/// `STORE-14`
#[test]
fn placements_that_can_not_be_written_say_why_and_the_bom_is_not_written() {
    let mut shell = shell_with_a_design();
    given(&mut shell, &complete());
    shell.refused.insert(
        Path::new("in").join("job1_placements.csv"),
        "Access denied.".to_string(),
    );

    shell.process(build());

    assert_eq!(shell.error(), Some("Access denied.".to_string()));
    assert_eq!(
        shell
            .documents
            .keys()
            .collect::<Vec<_>>(),
        vec![&path()]
    );
}

/// `VARB-5`
#[test]
fn building_writes_the_outputs_set_relative_to_the_files_folder() {
    let mut shell = shell_with_a_design();
    given(&mut shell, &VariantBuilderFile {
        output_placements: Some(PathBuf::from("out/placements.csv")),
        output_bom: Some(PathBuf::from("out/bom.csv")),
        ..complete()
    });

    shell.process(build());

    assert_eq!(shell.error(), None);
    assert_eq!(shell.documents[&Path::new("in").join("out/placements.csv")], PLACEMENTS);
    assert_eq!(shell.documents[&Path::new("in").join("out/bom.csv")], BOM);
}

/// `VARB-5`
#[test]
fn building_a_variant_builder_that_is_not_complete_names_what_is_missing() {
    let mut shell = shell_with_a_design();
    given(&mut shell, &VariantBuilderFile {
        eda_tool: None,
        part_mappings: None,
        ..complete()
    });

    shell.process(build());

    assert_eq!(
        shell.error(),
        Some(format!(
            "Variant builder incomplete. path: '{}', missing: [\"eda_tool\", \"part_mappings\"]",
            path().display()
        ))
    );
    // no data source is read.
    assert_eq!(shell.done(), vec![format!("read {}", path().display())]);
}
