//! A shell for the Variant Builder's core, with no files: it answers the store operations the core asks for itself, from
//! the records of data sources and the documents kept in memory, and keeps the documents the core writes (`STORE-7`,
//! `STORE-14`).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crux_core::Core;
use eda::placement::{EdaPlacement, EdaPlacementField};
use pnp::pcb::PcbSide;
use rust_decimal_macros::dec;
use variantbuilder_app::{
    Document, EdaTool, Event, Part, Parts, PartsViewItem, Records, RecordsOf, Revision, Source, StoresOperation,
    StoresOutput, VariantBuilder,
};

/// A shell, which answers each read of records, of parts, or of a document, from those it keeps, and each write of a
/// document by keeping it, unless told to refuse.
#[derive(Default)]
pub struct Shell {
    pub core: Core<VariantBuilder>,
    /// The records of each data source.
    pub records: BTreeMap<Source, RecordsOf>,
    /// The parts of each data source.
    pub parts: BTreeMap<Source, Vec<PartsViewItem>>,
    /// The documents, by path, as given, or as written.
    pub documents: BTreeMap<PathBuf, String>,
    /// Refuses to write these, saying why.
    pub refused: BTreeMap<PathBuf, String>,
    /// The store operations done, in order.
    pub operations: Vec<StoresOperation>,
}

impl Shell {
    pub fn process(&mut self, event: Event) {
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
            StoresOperation::ReadDocument {
                path,
            } => StoresOutput::DocumentRead(Ok(self
                .documents
                .get(path)
                .map(|contents| Document {
                    revision: revision(contents),
                    contents: contents.clone(),
                }))),
            StoresOperation::WriteDocument {
                path,
                contents,
            } => StoresOutput::DocumentWritten(match self.refused.get(path) {
                Some(cause) => Err(cause.clone()),
                None => {
                    self.documents
                        .insert(path.clone(), contents.clone());
                    Ok(revision(contents))
                }
            }),
            operation => panic!("unexpected operation: {:?}", operation),
        }
    }

    /// What was read and written, in order, as `<operation> <source or path>`.
    pub fn done(&self) -> Vec<String> {
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
                StoresOperation::ReadDocument {
                    path,
                } => format!("read {}", path.display()),
                StoresOperation::WriteDocument {
                    path, ..
                } => format!("write {}", path.display()),
                operation => format!("{:?}", operation),
            })
            .collect()
    }

    pub fn error(&self) -> Option<String> {
        self.core.view().error
    }
}

/// A document's revision, its contents, so a test can tell which contents it's of.
pub fn revision(contents: &str) -> Revision {
    Revision(contents.to_string())
}

/// The data source, in the folder `in`.
pub fn source(name: &str) -> Source {
    Source::from_path(Path::new("in"), Path::new(name))
}

pub fn row(fields: &[(&str, &str)]) -> BTreeMap<String, String> {
    fields
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// A shell with a DipTrace design of one resistor, in the folder `in`, a part for it, and a substitution, a load-out and
/// an assembly rule.
pub fn shell_with_a_design() -> Shell {
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

/// The placements file the design's variant has.
pub const PLACEMENTS: &str = "\"RefDes\",\"Manufacturer\",\"Mpn\",\"Place\",\"PcbSide\",\"X\",\"Y\",\"Rotation\"\n\
                              \"R1\",\"MFR1\",\"MPN1\",\"true\",\"Top\",\"10\",\"20\",\"90\"\n";

/// The BOM file the design's variant has.
pub const BOM: &str = "\"MPN1 (MFR1)\",\"R1\",\"RES_0402\",\"C1\",\"1\"\n";
