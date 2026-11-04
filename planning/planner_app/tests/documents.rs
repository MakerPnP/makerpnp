//! The planner's core, with no files: its tests answer the store operations it asks for themselves, from documents kept
//! in memory, and from the records of data sources kept in memory (`STORE-7`, `STORE-10`, `STORE-11`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crux_core::{Core, Request};
use indexmap::IndexSet;
use planner_app::{
    Document, Effect, Event, LoadOutItem, PcbAssemblyFlip, PcbAssemblyOrientation, PcbSide, Planner, Records,
    RecordsKind, RecordsOf, Reference, Revision, StoresOperation, StoresOutput,
};
use planning::file::{self, FileReference};
use planning::pcb::Pcb;
use planning::process::{ProcessDefinition, ProcessPresetFactory, ProcessReference};
use planning::project::{Project, ProjectPcb, ProjectProcess};
use regex::Regex;
use util::source::Source;

/// A shell, which answers each read of a document from those it keeps, and each write by keeping it, unless told to
/// refuse, and keeps the other effects.
#[derive(Default)]
struct Shell {
    core: Core<Planner>,
    /// The documents written, by path.
    documents: BTreeMap<PathBuf, String>,
    /// Refuses to read or write these, saying why.
    refused: BTreeMap<PathBuf, String>,
    /// The records of each data source, as written.
    records: BTreeMap<Source, RecordsOf>,
    /// Refuses to read or write the records of these, saying why.
    refused_records: BTreeMap<Source, String>,
    /// Refuses to write the records of these, saying why.
    refused_records_writes: BTreeMap<Source, String>,
    /// The store operations done, in order.
    operations: Vec<StoresOperation>,
}

impl Shell {
    fn process(&mut self, event: Event) {
        let effects = self.core.process_event(event);
        self.run(effects);
    }

    fn run(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            if let Effect::Stores(mut request) = effect {
                let output = self.answer(&request.operation);
                let effects = self
                    .core
                    .resolve(&mut request, output)
                    .unwrap();
                self.run(effects);
            }
        }
    }

    /// Processes the event, but keeps the store operations it asks for, unanswered, so a test can answer them later.
    fn process_without_answering(&mut self, event: Event) -> Vec<Request<StoresOperation>> {
        self.core
            .process_event(event)
            .into_iter()
            .filter_map(|effect| match effect {
                Effect::Stores(request) => Some(request),
                _ => None,
            })
            .collect()
    }

    fn answer_later(&mut self, mut request: Request<StoresOperation>) {
        let output = self.answer(&request.operation);
        let effects = self
            .core
            .resolve(&mut request, output)
            .unwrap();
        self.run(effects);
    }

    fn answer(&mut self, operation: &StoresOperation) -> StoresOutput {
        self.operations.push(operation.clone());
        match operation {
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
            StoresOperation::ReadDocument {
                path,
            } => StoresOutput::DocumentRead(match self.refused.get(path) {
                Some(cause) => Err(cause.clone()),
                None => Ok(self
                    .documents
                    .get(path)
                    .map(|contents| Document {
                        revision: Revision(contents.clone()),
                        contents: contents.clone(),
                    })),
            }),
            StoresOperation::ReadRecords {
                kind,
                source,
            } => StoresOutput::RecordsRead(match self.refused_records.get(source) {
                Some(cause) => Err(cause.clone()),
                None => Ok(self
                    .records
                    .get(source)
                    .filter(|records| records.kind() == *kind)
                    .map(|records| Records {
                        revision: Revision(format!("{:?}", records)),
                        records: records.clone(),
                    })),
            }),
            StoresOperation::WriteRecords {
                source,
                records,
            } => StoresOutput::RecordsWritten(
                match self
                    .refused_records
                    .get(source)
                    .or(self.refused_records_writes.get(source))
                {
                    Some(cause) => Err(cause.clone()),
                    None => {
                        self.records
                            .insert(source.clone(), records.clone());
                        Ok(Revision(format!("{:?}", records)))
                    }
                },
            ),
            operation => panic!("unexpected operation: {:?}", operation),
        }
    }

    /// The records read, in order.
    fn records_reads(&self) -> Vec<(RecordsKind, Source)> {
        self.operations
            .iter()
            .filter_map(|operation| match operation {
                StoresOperation::ReadRecords {
                    kind,
                    source,
                } => Some((*kind, source.clone())),
                _ => None,
            })
            .collect()
    }

    /// The records written, in order.
    fn records_writes(&self) -> Vec<(Source, RecordsOf)> {
        self.operations
            .iter()
            .filter_map(|operation| match operation {
                StoresOperation::WriteRecords {
                    source,
                    records,
                } => Some((source.clone(), records.clone())),
                _ => None,
            })
            .collect()
    }

    /// The paths read, in order.
    fn reads(&self) -> Vec<PathBuf> {
        self.operations
            .iter()
            .filter_map(|operation| match operation {
                StoresOperation::ReadDocument {
                    path,
                } => Some(path.clone()),
                _ => None,
            })
            .collect()
    }

    /// The paths written, in order.
    fn writes(&self) -> Vec<PathBuf> {
        self.operations
            .iter()
            .filter_map(|operation| match operation {
                StoresOperation::WriteDocument {
                    path, ..
                } => Some(path.clone()),
                _ => None,
            })
            .collect()
    }

    fn given_a_document<T: serde::Serialize>(&mut self, path: &PathBuf, document: &T) {
        self.documents
            .insert(path.clone(), file::to_string(document).unwrap());
    }

    fn project_modified(&self) -> bool {
        self.core.view().project_modified
    }

    fn error(&self) -> Option<String> {
        self.core
            .view()
            .error
            .map(|(_, error)| error)
    }

    fn document(&self, path: &PathBuf) -> serde_json::Value {
        serde_json::from_str(&self.documents[path]).expect("a JSON document")
    }
}

fn project_path(name: &str) -> PathBuf {
    PathBuf::from("projects").join(format!("{}.mpnp.json", name))
}

fn create_project(name: &str) -> Event {
    Event::CreateProject {
        name: name.to_string(),
        path: project_path(name),
        packages: None,
        package_mappings: None,
    }
}

/// Changes the project, with no store operations.
fn change_the_project() -> Event {
    Event::ApplyPackageSources {
        packages_source: Some(Source::File(PathBuf::from("packages.csv"))),
        package_mappings_source: Some(Source::File(PathBuf::from("package_mappings.csv"))),
    }
}

/// `DOC-1`, `STORE-10`, creating a project has the stores service write its file, and it then has no unsaved changes.
#[test]
fn creating_a_project_writes_its_file() {
    let mut shell = Shell::default();

    shell.process(create_project("job1"));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.operations.len(), 1);
    assert!(matches!(
        &shell.operations[0],
        StoresOperation::WriteDocument { path, .. } if path == &project_path("job1")
    ));
    assert_eq!(shell.document(&project_path("job1"))["name"], "job1");
    // as the project is saved, the file ends with a new line.
    assert!(shell.documents[&project_path("job1")].ends_with("}\n"));
    assert!(!shell.project_modified());
}

/// `DOC-1`, `STORE-10`, a project whose file can't be written isn't created, and says why.
#[test]
fn a_project_whose_file_can_not_be_written_is_not_created() {
    let mut shell = Shell::default();
    shell
        .refused
        .insert(project_path("job1"), "Unable to write the document.".to_string());

    shell.process(create_project("job1"));

    let error = shell.error().expect("an error");
    assert!(error.contains("Unable to write the document."), "{error}");

    // there's no project to save.
    shell.process(Event::Save);
    assert!(shell
        .error()
        .expect("an error")
        .contains("OperationRequiresProject"));
    assert_eq!(shell.operations.len(), 1);
}

/// `DOC-1`, the project before one that couldn't be created is kept.
#[test]
fn the_project_before_one_that_can_not_be_created_is_kept() {
    let mut shell = Shell::default();
    shell.process(create_project("job1"));
    shell
        .refused
        .insert(project_path("job2"), "Unable to write the document.".to_string());

    shell.process(create_project("job2"));
    assert!(shell.error().is_some());

    shell.process(Event::Save);
    assert_eq!(shell.error(), None);
    assert!(matches!(
        shell.operations.last(),
        Some(StoresOperation::WriteDocument { path, .. }) if path == &project_path("job1")
    ));
}

/// `STORE-10`, saving has the stores service write the project's changes, and it then has none unsaved.
#[test]
fn saving_writes_the_project() {
    let mut shell = Shell::default();
    shell.process(create_project("job1"));
    shell.process(change_the_project());
    assert!(shell.project_modified());

    shell.process(Event::Save);

    assert_eq!(shell.error(), None);
    assert!(!shell.project_modified());
    assert_eq!(
        shell.document(&project_path("job1"))["library_config"]["package_source"],
        serde_json::json!({ "File": "packages.csv" })
    );
}

/// `STORE-10`, a project whose file can't be written says why, and keeps its unsaved changes.
#[test]
fn a_refused_save_keeps_the_changes_unsaved() {
    let mut shell = Shell::default();
    shell.process(create_project("job1"));
    shell.process(change_the_project());
    shell
        .refused
        .insert(project_path("job1"), "Unable to write the document.".to_string());

    shell.process(Event::Save);

    assert!(shell
        .error()
        .expect("an error")
        .contains("Unable to write the document."));
    assert!(shell.project_modified());
}

/// `STORE-10`, a project changed while it's written still has unsaved changes once it is, as they weren't written.
#[test]
fn changes_made_while_the_project_is_written_are_unsaved() {
    let mut shell = Shell::default();
    shell.process(create_project("job1"));
    shell.process(change_the_project());

    let mut requests = shell.process_without_answering(Event::Save);
    assert_eq!(requests.len(), 1);
    shell.process(Event::ApplyPackageSources {
        packages_source: Some(Source::File(PathBuf::from("other_packages.csv"))),
        package_mappings_source: Some(Source::File(PathBuf::from("package_mappings.csv"))),
    });
    shell.answer_later(requests.remove(0));

    assert_eq!(shell.error(), None);
    assert!(shell.project_modified());
    // what was written is what was saved, not the change made since.
    assert_eq!(
        shell.document(&project_path("job1"))["library_config"]["package_source"],
        serde_json::json!({ "File": "packages.csv" })
    );
}

/// `STORE-7`, the result of a different operation is refused, as a fault in the shell.
#[test]
fn refuses_the_result_of_a_different_operation() {
    let mut shell = Shell::default();

    let mut requests = shell.process_without_answering(create_project("job1"));
    let effects = shell
        .core
        .resolve(&mut requests[0], StoresOutput::Changed)
        .unwrap();
    shell.run(effects);

    assert!(shell
        .error()
        .expect("an error")
        .contains("UnexpectedOutput"));
}

fn pcb_path(name: &str) -> PathBuf {
    PathBuf::from("projects").join(format!("{}.pcb.json", name))
}

fn a_pcb(name: &str) -> Pcb {
    Pcb::new(name.to_string(), 1, IndexSet::new(), BTreeMap::new())
}

/// A project using the PCBs, in the project's folder.
fn a_project_using(name: &str, pcbs: &[&str]) -> Project {
    let mut project = Project::new(name.to_string(), None, None);
    project.pcbs = pcbs
        .iter()
        .map(|pcb| ProjectPcb::new(FileReference::Relative(PathBuf::from(format!("{}.pcb.json", pcb)))))
        .collect();
    project
}

/// A project from before processes had files, with a copy of the process, and no file for it.
fn a_project_with_a_process_without_a_file(name: &str, process: &ProcessDefinition) -> Project {
    let mut project = Project::new(name.to_string(), None, None);
    project.processes = vec![ProjectProcess {
        process_file: None,
        definition: process.clone(),
    }];
    project
}

fn load(name: &str) -> Event {
    Event::Load {
        path: project_path(name),
    }
}

/// `STORE-10`, loading has the stores service read the project, and then each PCB it uses, and the project has no
/// unsaved changes.
#[test]
fn loading_reads_the_project_and_its_pcbs() {
    let mut shell = Shell::default();
    shell.given_a_document(&project_path("job1"), &a_project_using("job1", &["panel_a", "panel_b"]));
    shell.given_a_document(&pcb_path("panel_a"), &a_pcb("panel_a"));
    shell.given_a_document(&pcb_path("panel_b"), &a_pcb("panel_b"));

    shell.process(load("job1"));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.reads(), vec![
        project_path("job1"),
        pcb_path("panel_a"),
        pcb_path("panel_b")
    ]);
    assert!(shell.writes().is_empty());
    assert!(!shell.project_modified());
    assert!(!shell.core.view().pcbs_modified);

    // the project is loaded, so it can be saved.
    shell.process(Event::Save);
    assert_eq!(shell.error(), None);
    assert_eq!(shell.writes(), vec![project_path("job1")]);
}

/// `STORE-10`, the PCBs already loaded aren't read again.
#[test]
fn loading_again_does_not_read_the_pcbs_again() {
    let mut shell = Shell::default();
    shell.given_a_document(&project_path("job1"), &a_project_using("job1", &["panel_a"]));
    shell.given_a_document(&pcb_path("panel_a"), &a_pcb("panel_a"));
    shell.process(load("job1"));

    shell.process(load("job1"));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.reads(), vec![
        project_path("job1"),
        pcb_path("panel_a"),
        project_path("job1")
    ]);
}

/// `STORE-10`, a project that doesn't exist says so, and isn't loaded.
#[test]
fn a_project_that_does_not_exist_is_not_loaded() {
    let mut shell = Shell::default();

    shell.process(load("job1"));

    let error = shell.error().expect("an error");
    assert!(error.contains("Project not found."), "{error}");
    shell.process(Event::Save);
    assert!(shell
        .error()
        .expect("an error")
        .contains("OperationRequiresProject"));
}

/// `STORE-10`, a project that can't be read, or isn't a project, says why, and isn't loaded.
#[test]
fn a_project_that_can_not_be_read_is_not_loaded() {
    let mut shell = Shell::default();
    shell
        .refused
        .insert(project_path("job1"), "Unable to read the document.".to_string());
    shell
        .documents
        .insert(project_path("job2"), "not a project".to_string());

    shell.process(load("job1"));
    let error = shell.error().expect("an error");
    assert!(error.contains("Unable to read the document."), "{error}");

    shell.process(load("job2"));
    let error = shell.error().expect("an error");
    assert!(error.contains("Unable to read the project."), "{error}");

    shell.process(Event::Save);
    assert!(shell
        .error()
        .expect("an error")
        .contains("OperationRequiresProject"));
}

/// `STORE-10`, a PCB that can't be read says why, and the project is still loaded, with the PCBs read before it, as it
/// was when the core read its files itself.
#[test]
fn a_pcb_that_can_not_be_read_says_why() {
    let mut shell = Shell::default();
    shell.given_a_document(&project_path("job1"), &a_project_using("job1", &["panel_a", "panel_b"]));
    shell.given_a_document(&pcb_path("panel_b"), &a_pcb("panel_b"));

    shell.process(load("job1"));

    let error = shell.error().expect("an error");
    assert!(error.contains("PCB not found."), "{error}");
    // it stops at the first.
    assert_eq!(shell.reads(), vec![project_path("job1"), pcb_path("panel_a")]);
    shell.process(Event::Save);
    assert_eq!(shell.error(), None);
}

/// `STORE-10`, a process without a file is given one in the project's folder, written by the stores service, and the
/// project then has unsaved changes.
#[test]
fn loading_gives_a_process_without_a_file_a_file() {
    let mut shell = Shell::default();
    let process = ProcessPresetFactory::by_preset_name("pnp").unwrap();
    shell.given_a_document(
        &project_path("job1"),
        &a_project_with_a_process_without_a_file("job1", &process),
    );

    shell.process(load("job1"));

    assert_eq!(shell.error(), None);
    let process_path = PathBuf::from("projects").join("pnp.process.json");
    assert_eq!(shell.writes(), vec![process_path.clone()]);
    assert_eq!(shell.documents[&process_path], file::to_string(&process).unwrap());
    assert!(shell.project_modified());

    shell.process(Event::Save);
    assert_eq!(
        shell.document(&project_path("job1"))["processes"][0]["process_file"],
        serde_json::json!({ "Relative": "pnp.process.json" })
    );
}

/// `STORE-10`, a file with the same process is used as it is, and one with another is kept, the process being given
/// another file.
#[test]
fn loading_uses_a_file_with_the_same_process_and_keeps_one_with_another() {
    let process = ProcessPresetFactory::by_preset_name("pnp").unwrap();
    let other_process = ProcessPresetFactory::by_preset_name("manual").unwrap();

    // the same.
    let mut shell = Shell::default();
    shell.given_a_document(
        &project_path("job1"),
        &a_project_with_a_process_without_a_file("job1", &process),
    );
    shell.given_a_document(&PathBuf::from("projects").join("pnp.process.json"), &process);
    shell.process(load("job1"));
    assert_eq!(shell.error(), None);
    assert!(shell.writes().is_empty());

    // another.
    let mut shell = Shell::default();
    shell.given_a_document(
        &project_path("job1"),
        &a_project_with_a_process_without_a_file("job1", &process),
    );
    shell.given_a_document(&PathBuf::from("projects").join("pnp.process.json"), &other_process);
    shell.process(load("job1"));
    assert_eq!(shell.error(), None);
    assert_eq!(shell.writes(), vec![
        PathBuf::from("projects").join("pnp_2.process.json")
    ]);
    assert_eq!(
        shell.documents[&PathBuf::from("projects").join("pnp.process.json")],
        file::to_string(&other_process).unwrap()
    );
}

fn create_pcb(name: &str) -> Event {
    Event::CreatePcb {
        name: name.to_string(),
        units: 1,
        path: pcb_path(name),
        unit_map: None,
    }
}

/// Changes the PCB, with no store operations.
fn change_the_pcb(name: &str) -> Event {
    let mut assembly_orientation = PcbAssemblyOrientation::default();
    assembly_orientation.top.flip = PcbAssemblyFlip::Roll;
    Event::ApplyAssemblyOrientation {
        path: pcb_path(name),
        assembly_orientation,
    }
}

fn pcbs_modified(shell: &Shell) -> bool {
    shell.core.view().pcbs_modified
}

/// `DOC-1`, `STORE-10`, creating a PCB has the stores service write its file, and it then has no unsaved changes.
#[test]
fn creating_a_pcb_writes_its_file() {
    let mut shell = Shell::default();

    shell.process(create_pcb("panel_a"));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.writes(), vec![pcb_path("panel_a")]);
    assert_eq!(shell.document(&pcb_path("panel_a"))["name"], "panel_a");
    assert!(!pcbs_modified(&shell));
}

/// `DOC-1`, `STORE-10`, a PCB whose file can't be written isn't created, and says why.
#[test]
fn a_pcb_whose_file_can_not_be_written_is_not_created() {
    let mut shell = Shell::default();
    shell
        .refused
        .insert(pcb_path("panel_a"), "Unable to write the document.".to_string());

    shell.process(create_pcb("panel_a"));

    let error = shell.error().expect("an error");
    assert!(error.contains("Unable to write the document."), "{error}");
    shell.process(Event::SavePcb {
        path: pcb_path("panel_a"),
    });
    let error = shell.error().expect("an error");
    assert!(error.contains("PCB not loaded."), "{error}");
}

/// `STORE-10`, saving a PCB has the stores service write its changes, and a refused save keeps them unsaved.
#[test]
fn saving_a_pcb_writes_it() {
    let mut shell = Shell::default();
    shell.process(create_pcb("panel_a"));
    shell.process(change_the_pcb("panel_a"));
    assert!(pcbs_modified(&shell));

    shell
        .refused
        .insert(pcb_path("panel_a"), "Unable to write the document.".to_string());
    shell.process(Event::SavePcb {
        path: pcb_path("panel_a"),
    });
    assert!(shell.error().is_some());
    assert!(pcbs_modified(&shell));

    shell.refused.clear();
    shell.process(Event::SavePcb {
        path: pcb_path("panel_a"),
    });
    assert_eq!(shell.error(), None);
    assert!(!pcbs_modified(&shell));
    assert_eq!(
        shell.document(&pcb_path("panel_a"))["orientation"]["top"]["flip"],
        "Roll"
    );
}

/// `STORE-10`, saving every PCB writes each of them.
#[test]
fn saving_every_pcb_writes_each() {
    let mut shell = Shell::default();
    shell.process(create_pcb("panel_a"));
    shell.process(create_pcb("panel_b"));
    shell.process(change_the_pcb("panel_a"));
    shell.process(change_the_pcb("panel_b"));

    shell.process(Event::SaveAllPcbs);

    assert_eq!(shell.error(), None);
    assert!(!pcbs_modified(&shell));
    assert_eq!(shell.writes(), vec![
        pcb_path("panel_a"),
        pcb_path("panel_b"),
        pcb_path("panel_a"),
        pcb_path("panel_b")
    ]);
}

/// `STORE-10`, loading a PCB has the stores service read it, and one that doesn't exist says so.
#[test]
fn loading_a_pcb_reads_it() {
    let mut shell = Shell::default();
    shell.given_a_document(&pcb_path("panel_a"), &a_pcb("panel_a"));

    shell.process(Event::LoadPcb {
        path: pcb_path("panel_a"),
    });
    assert_eq!(shell.error(), None);
    assert_eq!(shell.reads(), vec![pcb_path("panel_a")]);

    shell.process(Event::LoadPcb {
        path: pcb_path("panel_b"),
    });
    let error = shell.error().expect("an error");
    assert!(error.contains("PCB not found."), "{error}");
}

/// `STORE-10`, refreshing reads each PCB loaded again, replacing it.
#[test]
fn refreshing_reads_each_pcb_again() {
    let mut shell = Shell::default();
    shell.process(create_pcb("panel_a"));
    shell.process(change_the_pcb("panel_a"));

    shell.process(Event::RefreshPcbs);

    assert_eq!(shell.error(), None);
    assert_eq!(shell.reads(), vec![pcb_path("panel_a")]);
    // the PCB read replaces the one changed.
    assert!(!pcbs_modified(&shell));
}

/// `STORE-10`, adding a PCB to a project has the stores service read it, and the project then has unsaved changes.
#[test]
fn adding_a_pcb_to_a_project_reads_it() {
    let mut shell = Shell::default();
    shell.process(create_project("job1"));
    shell.given_a_document(&pcb_path("panel_a"), &a_pcb("panel_a"));

    shell.process(Event::AddPcb {
        pcb_file: FileReference::Relative(PathBuf::from("panel_a.pcb.json")),
    });

    assert_eq!(shell.error(), None);
    assert_eq!(shell.reads(), vec![pcb_path("panel_a")]);
    assert!(shell.project_modified());
}

/// `STORE-10`, a PCB that can't be read isn't added to the project.
#[test]
fn a_pcb_that_can_not_be_read_is_not_added_to_the_project() {
    let mut shell = Shell::default();
    shell.process(create_project("job1"));

    shell.process(Event::AddPcb {
        pcb_file: FileReference::Relative(PathBuf::from("panel_a.pcb.json")),
    });

    let error = shell.error().expect("an error");
    assert!(error.contains("PCB not found."), "{error}");
    assert!(!shell.project_modified());
}

/// `STORE-10`, a project's PCBs are read when it's loaded, or when they're added, so an event that needs one that
/// couldn't be read says so, and doesn't read it itself.
#[test]
fn an_event_needing_a_pcb_that_is_not_loaded_says_so() {
    let mut shell = Shell::default();
    shell.given_a_document(&project_path("job1"), &a_project_using("job1", &["panel_a"]));
    shell.process(load("job1"));
    assert!(shell.error().is_some());
    let reads = shell.reads().len();

    shell.process(Event::RequestProjectTreeView {});

    let error = shell.error().expect("an error");
    assert!(error.contains("PCB not loaded."), "{error}");
    assert_eq!(shell.reads().len(), reads);
}

fn process_path(name: &str) -> PathBuf {
    PathBuf::from("projects").join(format!("{}.process.json", name))
}

fn create_process(name: &str, preset: &str) -> Event {
    Event::CreateProcess {
        path: process_path(name),
        reference: ProcessReference::from_raw_str(name),
        preset: Some(preset.to_string()),
    }
}

/// Changes the process document, with no store operations.
fn change_the_process(name: &str) -> Event {
    let mut process_definition = ProcessPresetFactory::by_preset_name("manual").unwrap();
    process_definition.reference = ProcessReference::from_raw_str(name);
    Event::ApplyProcessDefinition {
        path: process_path(name),
        process_definition,
    }
}

fn processes_modified(shell: &Shell) -> bool {
    shell.core.view().processes_modified
}

/// `DOC-1`, `STORE-10`, creating a process has the stores service write its file, and it then has no unsaved changes,
/// and one whose file can't be written isn't created.
#[test]
fn creating_a_process_writes_its_file() {
    let mut shell = Shell::default();

    shell.process(create_process("pnp", "pnp"));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.writes(), vec![process_path("pnp")]);
    assert_eq!(shell.document(&process_path("pnp"))["reference"], "pnp");
    assert!(!processes_modified(&shell));

    shell
        .refused
        .insert(process_path("other"), "Unable to write the document.".to_string());
    shell.process(create_process("other", "pnp"));
    assert!(shell.error().is_some());
    shell.process(Event::SaveProcess {
        path: process_path("other"),
    });
    let error = shell.error().expect("an error");
    assert!(error.contains("ProcessNotLoaded"), "{error}");
}

/// `STORE-10`, saving a process has the stores service write its changes, a refused save keeps them unsaved, and
/// saving every process writes each.
#[test]
fn saving_a_process_writes_it() {
    let mut shell = Shell::default();
    shell.process(create_process("pnp", "pnp"));
    shell.process(create_process("manual", "pnp"));
    shell.process(change_the_process("pnp"));
    assert!(processes_modified(&shell));

    shell
        .refused
        .insert(process_path("pnp"), "Unable to write the document.".to_string());
    shell.process(Event::SaveProcess {
        path: process_path("pnp"),
    });
    assert!(shell.error().is_some());
    assert!(processes_modified(&shell));

    shell.refused.clear();
    shell.process(Event::SaveAllProcesses);
    assert_eq!(shell.error(), None);
    assert!(!processes_modified(&shell));
    // after the two created, and the one refused.
    assert_eq!(shell.writes()[3..], [process_path("manual"), process_path("pnp")]);
}

/// `STORE-10`, loading a process has the stores service read it, and one that doesn't exist says so.
#[test]
fn loading_a_process_reads_it() {
    let mut shell = Shell::default();
    shell.given_a_document(
        &process_path("pnp"),
        &ProcessPresetFactory::by_preset_name("pnp").unwrap(),
    );

    shell.process(Event::LoadProcess {
        path: process_path("pnp"),
    });
    assert_eq!(shell.error(), None);
    assert_eq!(shell.reads(), vec![process_path("pnp")]);

    shell.process(Event::LoadProcess {
        path: process_path("other"),
    });
    let error = shell.error().expect("an error");
    assert!(error.contains("Process not found."), "{error}");
}

/// `STORE-10`, adding a process to a project has the stores service read it, and one that can't be read isn't added.
#[test]
fn adding_a_process_to_a_project_reads_it() {
    let mut shell = Shell::default();
    shell.process(create_project("job1"));

    shell.process(Event::AddProcess {
        process_file: FileReference::Relative(PathBuf::from("pnp.process.json")),
    });
    let error = shell.error().expect("an error");
    assert!(error.contains("Process not found."), "{error}");
    assert!(!shell.project_modified());

    shell.given_a_document(
        &process_path("pnp"),
        &ProcessPresetFactory::by_preset_name("pnp").unwrap(),
    );
    shell.process(Event::AddProcess {
        process_file: FileReference::Relative(PathBuf::from("pnp.process.json")),
    });
    assert_eq!(shell.error(), None);
    assert!(shell.project_modified());
    shell.process(Event::Save);
    assert_eq!(
        shell.document(&project_path("job1"))["processes"][0]["reference"],
        "pnp"
    );
}

/// `STORE-10`, refreshing has the stores service read each of the project's processes, and if one can't be read,
/// nothing changes.
#[test]
fn refreshing_processes_reads_each() {
    let mut shell = Shell::default();
    shell.process(create_project("job1"));
    shell.given_a_document(
        &process_path("pnp"),
        &ProcessPresetFactory::by_preset_name("pnp").unwrap(),
    );
    shell.process(Event::AddProcess {
        process_file: FileReference::Relative(PathBuf::from("pnp.process.json")),
    });
    shell.process(Event::Save);

    // changed outside the core.
    let mut changed = ProcessPresetFactory::by_preset_name("manual").unwrap();
    changed.reference = ProcessReference::from_raw_str("pnp");
    shell.given_a_document(&process_path("pnp"), &changed);
    shell
        .refused
        .insert(process_path("pnp"), "Unable to read the document.".to_string());
    shell.process(Event::RefreshProcesses);
    assert!(shell.error().is_some());
    assert!(!shell.project_modified());

    shell.refused.clear();
    shell.process(Event::RefreshProcesses);
    assert_eq!(shell.error(), None);
    assert!(shell.project_modified());
    assert_eq!(shell.reads().last(), Some(&process_path("pnp")));
}

/// The load-out, in the project's folder, as a phase given its name uses it.
fn load_out_source(name: &str) -> Source {
    Source::File(PathBuf::from("projects").join(load_out_name(name)))
}

fn load_out_name(name: &str) -> String {
    format!("{}_load_out.csv", name)
}

/// A project with a process, `pnp`, loaded.
fn a_loaded_project_with_a_process(shell: &mut Shell) {
    let process = ProcessPresetFactory::by_preset_name("pnp").unwrap();
    shell.given_a_document(
        &project_path("job1"),
        &a_project_with_a_process_without_a_file("job1", &process),
    );
    shell.process(load("job1"));
    assert_eq!(shell.error(), None);
}

/// A phase whose load-out is given relative to the project's folder.
fn create_phase(reference: &str, load_out: &str) -> Event {
    Event::CreatePhase {
        process: ProcessReference::from_raw_str("pnp"),
        reference: Reference::from_raw_str(reference),
        load_out: Source::File(PathBuf::from(load_out_name(load_out))),
        pcb_side: PcbSide::Top,
    }
}

fn an_item(manufacturer: &str, mpn: &str) -> LoadOutItem {
    LoadOutItem {
        reference: None,
        manufacturer: manufacturer.to_string(),
        mpn: mpn.to_string(),
    }
}

/// `STORE-11`, creating a phase has the stores service read its load-out first, and write an empty one if there isn't
/// one, in the project's folder, as the load-out is given relative to it (`ISSUE-7`).
#[test]
fn creating_a_phase_reads_its_load_out_and_writes_one_if_there_is_none() {
    let mut shell = Shell::default();
    a_loaded_project_with_a_process(&mut shell);

    shell.process(create_phase("top_1", "top_1"));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.records_reads(), vec![(
        RecordsKind::LoadOut,
        load_out_source("top_1")
    )]);
    assert_eq!(shell.records_writes(), vec![(
        load_out_source("top_1"),
        RecordsOf::LoadOut(vec![])
    )]);
    assert!(shell.project_modified());
}

/// `STORE-11`, a phase whose load-out exists keeps it, and each event reads the records it uses again.
#[test]
fn creating_a_phase_keeps_its_load_out_if_there_is_one() {
    let mut shell = Shell::default();
    a_loaded_project_with_a_process(&mut shell);
    shell.records.insert(
        load_out_source("shared"),
        RecordsOf::LoadOut(vec![an_item("MFR1", "MPN1")]),
    );

    shell.process(create_phase("top_1", "shared"));
    shell.process(create_phase("top_2", "shared"));

    assert_eq!(shell.error(), None);
    assert_eq!(shell.records_reads(), vec![
        (RecordsKind::LoadOut, load_out_source("shared")),
        (RecordsKind::LoadOut, load_out_source("shared")),
    ]);
    assert_eq!(shell.records_writes(), vec![]);
}

/// `STORE-11`, an event whose records can't be read fails, and changes nothing.
#[test]
fn a_phase_whose_load_out_can_not_be_read_is_not_created() {
    let mut shell = Shell::default();
    a_loaded_project_with_a_process(&mut shell);
    let modified_before = shell.project_modified();
    shell
        .refused_records
        .insert(load_out_source("top_1"), "Unable to read the records.".to_string());

    shell.process(create_phase("top_1", "top_1"));

    let error = shell.error().expect("an error");
    assert!(error.contains("Unable to read the records."), "{error}");
    assert_eq!(shell.records_writes(), vec![]);
    assert_eq!(shell.project_modified(), modified_before);
}

/// `STORE-11`, assigning a feeder has the stores service write the load-out, with the feeder assigned to the item.
#[test]
fn assigning_a_feeder_writes_the_load_out() {
    let mut shell = Shell::default();
    a_loaded_project_with_a_process(&mut shell);
    shell.records.insert(
        load_out_source("top_1"),
        RecordsOf::LoadOut(vec![an_item("MFR1", "MPN1"), an_item("MFR2", "MPN2")]),
    );
    shell.process(create_phase("top_1", "top_1"));

    shell.process(Event::AssignFeederToLoadOutItem {
        phase: Reference::from_raw_str("top_1"),
        feeder_reference: Some(Reference::from_raw_str("FEEDER_1")),
        manufacturer: Regex::new("^MFR1$").unwrap(),
        mpn: Regex::new("^MPN1$").unwrap(),
    });

    assert_eq!(shell.error(), None);
    let mut assigned = an_item("MFR1", "MPN1");
    assigned.reference = Some(Reference::from_raw_str("FEEDER_1"));
    assert_eq!(shell.records_writes(), vec![(
        load_out_source("top_1"),
        RecordsOf::LoadOut(vec![assigned, an_item("MFR2", "MPN2")])
    )]);
}

/// `STORE-11`, an event that fails after reading its records changes nothing, and writes nothing.
#[test]
fn a_feeder_that_can_not_be_assigned_writes_nothing() {
    let mut shell = Shell::default();
    a_loaded_project_with_a_process(&mut shell);
    shell.records.insert(
        load_out_source("top_1"),
        RecordsOf::LoadOut(vec![an_item("MFR1", "MPN1")]),
    );
    shell.process(create_phase("top_1", "top_1"));

    shell.process(Event::AssignFeederToLoadOutItem {
        phase: Reference::from_raw_str("top_1"),
        feeder_reference: Some(Reference::from_raw_str("FEEDER_1")),
        manufacturer: Regex::new("^OTHER$").unwrap(),
        mpn: Regex::new(".*").unwrap(),
    });

    assert!(shell.error().is_some());
    assert_eq!(shell.records_writes(), vec![]);
}

/// `STORE-11`, a load-out that can't be written says why.
#[test]
fn a_load_out_that_can_not_be_written_says_why() {
    let mut shell = Shell::default();
    a_loaded_project_with_a_process(&mut shell);
    shell
        .refused_records_writes
        .insert(load_out_source("top_1"), "Unable to write the records.".to_string());

    shell.process(create_phase("top_1", "top_1"));

    let error = shell.error().expect("an error");
    assert!(error.contains("Unable to write the records."), "{error}");
}

/// `STORE-11`, a load-out that doesn't exist, used by an event that needs it, says so.
#[test]
fn a_load_out_that_does_not_exist_says_so() {
    let mut shell = Shell::default();
    a_loaded_project_with_a_process(&mut shell);
    shell.process(create_phase("top_1", "top_1"));
    shell.records.clear();

    shell.process(Event::RequestPhaseLoadOutView {
        phase_reference: Reference::from_raw_str("top_1"),
    });

    let error = shell.error().expect("an error");
    assert!(error.contains("Data source not found."), "{error}");
}

/// `STORE-11`, assigning a variant to a unit has the stores service read the variant's placements before the project is
/// changed, once.
#[test]
fn assigning_a_variant_reads_its_placements_first() {
    let mut shell = Shell::default();
    let pcb = Pcb::new(
        "panel_a".to_string(),
        1,
        IndexSet::from(["design_a".into()]),
        BTreeMap::from([(0, 0)]),
    );
    shell.given_a_document(&pcb_path("panel_a"), &pcb);
    shell.given_a_document(&project_path("job1"), &a_project_using("job1", &["panel_a"]));
    shell.process(load("job1"));
    assert_eq!(shell.error(), None);
    let placements = Source::File(PathBuf::from("projects").join("design_a_variant_a_placements.csv"));
    shell
        .records
        .insert(placements.clone(), RecordsOf::Placements(vec![]));

    shell.process(Event::AssignVariantToUnit {
        unit: "pcb=1::unit=1".parse().unwrap(),
        variant: Some("variant_a".into()),
    });

    assert_eq!(shell.error(), None);
    assert_eq!(shell.records_reads(), vec![(RecordsKind::Placements, placements)]);
    assert!(shell.project_modified());
}

fn artifact_path(name: &str) -> PathBuf {
    PathBuf::from("projects").join(name)
}

/// `STORE-12`, generating the artifacts has the stores service write each phase's placements, then the report, and,
/// with the `markdown` feature, its markdown.
#[test]
fn generating_artifacts_writes_each_artifact_in_order() {
    let mut shell = Shell::default();
    a_loaded_project_with_a_process(&mut shell);
    shell.process(create_phase("top_1", "top_1"));
    let written_before = shell.writes().len();

    shell.process(Event::GenerateArtifacts);

    assert_eq!(shell.error(), None);
    let written = shell.writes()[written_before..].to_vec();
    assert_eq!(written[..2], [
        artifact_path("top_1_placements.csv"),
        artifact_path("job1_report.json")
    ]);
    // the markdown report, only with the `markdown` feature, which another crate in the workspace may enable.
    assert!(
        written[2..].is_empty() || written[2..] == [artifact_path("job1_report.md")],
        "{written:?}"
    );
    assert!(shell.documents[&artifact_path("job1_report.json")].ends_with("}\n"));
}

/// `STORE-12`, an artifact that can't be written says why, and the ones after it aren't written.
#[test]
fn an_artifact_that_can_not_be_written_stops_the_rest() {
    let mut shell = Shell::default();
    a_loaded_project_with_a_process(&mut shell);
    shell.process(create_phase("top_1", "top_1"));
    shell.refused.insert(
        artifact_path("top_1_placements.csv"),
        "Unable to write the document.".to_string(),
    );

    shell.process(Event::GenerateArtifacts);

    let error = shell.error().expect("an error");
    assert!(error.contains("Unable to write the document."), "{error}");
    assert!(!shell
        .writes()
        .contains(&artifact_path("job1_report.json")));
}
