//! The planner's core, with no files: its tests answer the store operations it asks for themselves, from documents kept
//! in memory (`STORE-7`, `STORE-10`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crux_core::{Core, Request};
use indexmap::IndexSet;
use planner_app::{
    Document, Effect, Event, PcbAssemblyFlip, PcbAssemblyOrientation, Planner, Revision, StoresOperation, StoresOutput,
};
use planning::file::{self, FileReference};
use planning::pcb::Pcb;
use planning::process::{ProcessDefinition, ProcessPresetFactory};
use planning::project::{Project, ProjectPcb, ProjectProcess};
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
            operation => panic!("unexpected operation: {:?}", operation),
        }
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
