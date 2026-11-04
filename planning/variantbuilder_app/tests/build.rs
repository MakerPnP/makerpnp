//! The Variant Builder's core, with no files: its tests answer the store operations it asks for themselves, from the
//! records of data sources kept in memory, and keep the documents it writes (`STORE-7`, `STORE-14`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crux_core::Core;
use eda::placement::{EdaPlacement, EdaPlacementField};
use pnp::pcb::PcbSide;
use rust_decimal_macros::dec;
use variantbuilder_app::{
    AssemblyVariant, EdaTool, Event, Part, Parts, PartsViewItem, Records, RecordsOf, Revision, Source, StoresOperation,
    StoresOutput, VariantBuilder,
};

/// A shell, which answers each read of records, or of parts, from those it keeps, and each write of a document by
/// keeping it, unless told to refuse.
#[derive(Default)]
struct Shell {
    core: Core<VariantBuilder>,
    /// The records of each data source.
    records: BTreeMap<Source, RecordsOf>,
    /// The parts of each data source.
    parts: BTreeMap<Source, Vec<PartsViewItem>>,
    /// The documents written, by path.
    documents: BTreeMap<PathBuf, String>,
    /// Refuses to write these, saying why.
    refused: BTreeMap<PathBuf, String>,
    /// The store operations done, in order.
    operations: Vec<StoresOperation>,
}

impl Shell {
    fn process(&mut self, event: Event) {
        let effects = self.core.process_event(event);
        self.run(effects);
    }

    fn run(&mut self, effects: Vec<variantbuilder_app::Effect>) {
        for effect in effects {
            if let variantbuilder_app::Effect::Stores(mut request) = effect {
                let output = self.answer(&request.operation);
                let effects = self
                    .core
                    .resolve(&mut request, output)
                    .unwrap();
                self.run(effects);
            }
        }
    }

    fn answer(&mut self, operation: &StoresOperation) -> StoresOutput {
        self.operations.push(operation.clone());
        match operation {
            StoresOperation::ReadRecords {
                kind,
                source,
            } => StoresOutput::RecordsRead(Ok(self
                .records
                .get(source)
                .filter(|records| records.kind() == *kind)
                .map(|records| Records {
                    revision: Revision(format!("{:?}", records)),
                    records: records.clone(),
                }))),
            StoresOperation::ReadParts {
                source,
            } => StoresOutput::PartsRead(match self.parts.get(source) {
                Some(parts) => Ok(Parts {
                    revision: Revision(format!("{:?}", parts)),
                    parts: parts.clone(),
                }),
                None => Err(format!("No parts. source: '{}'", source)),
            }),
            StoresOperation::WriteDocument {
                path,
                contents,
            } => StoresOutput::DocumentWritten(match self.refused.get(path) {
                Some(cause) => Err(cause.clone()),
                None => {
                    self.documents
                        .insert(path.clone(), contents.clone());
                    Ok(Revision(contents.clone()))
                }
            }),
            operation => panic!("unexpected operation: {:?}", operation),
        }
    }

    /// What was read and written, in order, as `<operation> <source or path>`.
    fn done(&self) -> Vec<String> {
        self.operations
            .iter()
            .map(|operation| match operation {
                StoresOperation::ReadRecords {
                    kind,
                    source,
                } => format!("read {:?} {}", kind, source),
                StoresOperation::ReadParts {
                    source,
                } => format!("read parts {}", source),
                StoresOperation::WriteDocument {
                    path, ..
                } => format!("write {}", path.display()),
                operation => format!("{:?}", operation),
            })
            .collect()
    }

    fn error(&self) -> Option<String> {
        self.core.view().error
    }
}

fn source(name: &str) -> Source {
    Source::from_path(Path::new("in"), Path::new(name))
}

fn row(fields: &[(&str, &str)]) -> BTreeMap<String, String> {
    fields
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// A shell with a DipTrace design of one resistor, a part for it, and a substitution, a load-out and an assembly rule.
fn shell_with_a_design() -> Shell {
    let mut shell = Shell::default();
    shell
        .records
        .insert(source("placements.csv"), RecordsOf::EdaPlacements {
            eda_tool: EdaTool::DipTrace,
            placements: vec![EdaPlacement {
                ref_des: "R1".to_string(),
                place: true,
                fields: vec![
                    EdaPlacementField::new("name".to_string(), "RES_0402".to_string()),
                    EdaPlacementField::new("value".to_string(), "330R".to_string()),
                ],
                pcb_side: PcbSide::Top,
                x: dec!(10),
                y: dec!(20),
                rotation: dec!(90),
            }],
        });
    shell.records.insert(
        source("substitutions.csv"),
        RecordsOf::Substitutions(vec![row(&[
            ("Eda", "DipTrace"),
            ("NamePattern", "RES_0402"),
            ("ValuePattern", "330R"),
            ("Name", "RES_0402"),
            ("Value", "330R 1%"),
        ])]),
    );
    shell
        .parts
        .insert(source("parts.csv"), vec![PartsViewItem {
            part: Part::new("MFR1".to_string(), "MPN1".to_string()),
            fields: [("LCSC".to_string(), "C1".to_string())].into(),
        }]);
    shell.records.insert(
        source("part-mappings.csv"),
        RecordsOf::PartMappings(vec![row(&[
            ("Eda", "DipTrace"),
            ("Name", "RES_0402"),
            ("Value", "330R 1%"),
            ("Manufacturer", "MFR1"),
            ("Mpn", "MPN1"),
        ])]),
    );
    shell
        .records
        .insert(source("load-out.csv"), RecordsOf::LoadOut(vec![]));
    shell.records.insert(
        source("assembly-rules.csv"),
        RecordsOf::AssemblyRules(vec![row(&[
            ("RefDes", "R1"),
            ("Manufacturer", "MFR1"),
            ("Mpn", "MPN1"),
        ])]),
    );
    shell
}

fn build(output_bom: Option<&str>) -> Event {
    Event::Build {
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
    assert_eq!(
        shell.documents[Path::new("out/placements.csv")],
        "\"RefDes\",\"Manufacturer\",\"Mpn\",\"Place\",\"PcbSide\",\"X\",\"Y\",\"Rotation\"\n\
         \"R1\",\"MFR1\",\"MPN1\",\"true\",\"Top\",\"10\",\"20\",\"90\"\n"
    );
    assert_eq!(
        shell.documents[Path::new("out/bom.csv")],
        "\"MPN1 (MFR1)\",\"R1\",\"RES_0402\",\"C1\",\"1\"\n"
    );
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
