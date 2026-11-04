//! The planner's core, with no files: its tests answer the store operations it asks for themselves, from documents kept
//! in memory (`STORE-7`, `STORE-10`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crux_core::{Core, Request};
use planner_app::{Effect, Event, Planner, StoresOperation, StoresOutput};
use stores_protocol::Revision;
use util::source::Source;

/// A shell, which answers each write of a document by keeping it, unless told to refuse, and keeps the other effects.
#[derive(Default)]
struct Shell {
    core: Core<Planner>,
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
            operation => panic!("unexpected operation: {:?}", operation),
        }
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
