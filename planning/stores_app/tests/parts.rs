//! The stores core's parts data sources (`STORE-1` to `STORE-7`).
//!
//! The core is pure, so these tests answer the store operations it asks for themselves, with no files (`STORE-7`). The
//! stores service's tests check the operations are done (`planning/stores_service/tests/`).

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use crux_core::Core;
use stores_app::{
    Effect, Event, Part, PartsSource, PartsSourceStatus, PartsView, PartsViewItem, PartsViewRendererOperation, Stores,
    StoresOperation, StoresOutput,
};

fn source(path: &str) -> PartsSource {
    PartsSource::File(PathBuf::from(path))
}

fn item(manufacturer: &str, mpn: &str, fields: &[(&str, &str)]) -> PartsViewItem {
    PartsViewItem {
        part: Part::new(manufacturer.to_string(), mpn.to_string()),
        fields: fields
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// A shell, which answers each store operation the core asks for with `answer`, and keeps the views the core sends.
struct Shell {
    core: Core<Stores>,
    /// The operations the core asked for, in order, since they were last taken.
    operations: Vec<StoresOperation>,
}

impl Shell {
    fn new() -> Self {
        Self {
            core: Core::new(),
            operations: vec![],
        }
    }

    /// Processes the event, and answers every operation it leads to, until the core asks for nothing more. Returns
    /// the views sent, and the core's error after the last of them.
    fn process(
        &mut self,
        event: Event,
        mut answer: impl FnMut(&StoresOperation) -> StoresOutput,
    ) -> (Vec<PartsView>, Option<String>) {
        let mut views = vec![];
        let mut effects: VecDeque<Effect> = self.core.process_event(event).into();
        while let Some(effect) = effects.pop_front() {
            match effect {
                Effect::Render(_) => {}
                Effect::PartsView(request) => {
                    let PartsViewRendererOperation::View {
                        view,
                    } = request.operation;
                    views.push(view);
                }
                Effect::Stores(mut request) => {
                    self.operations
                        .push(request.operation.clone());
                    let output = answer(&request.operation);
                    effects.extend(
                        self.core
                            .resolve(&mut request, output)
                            .unwrap(),
                    );
                }
            }
        }
        (views, self.core.view().error)
    }

    fn take_operations(&mut self) -> Vec<StoresOperation> {
        std::mem::take(&mut self.operations)
    }

    fn status(&self, source: &PartsSource) -> Option<PartsSourceStatus> {
        self.core
            .view()
            .parts_sources
            .get(source)
            .cloned()
    }
}

/// A stores service with one data source, which `as_asked` is resolved to, as `resolved`, and has `parts`.
fn service(
    as_asked: &[PartsSource],
    resolved: &PartsSource,
    parts: Result<Vec<PartsViewItem>, String>,
) -> impl FnMut(&StoresOperation) -> StoresOutput {
    let as_asked = as_asked.to_vec();
    let resolved = resolved.clone();
    move |operation| match operation {
        StoresOperation::ResolveSource {
            source,
        } if as_asked.contains(source) || *source == resolved => StoresOutput::SourceResolved {
            source: resolved.clone(),
        },
        StoresOperation::ResolveSource {
            source,
        } => StoresOutput::SourceResolved {
            source: source.clone(),
        },
        StoresOperation::ReadParts {
            source,
        } if *source == resolved => StoresOutput::PartsRead(parts.clone()),
        StoresOperation::ReadParts {
            ..
        } => StoresOutput::PartsRead(Err("No such file.".to_string())),
    }
}

fn no_operations(operation: &StoresOperation) -> StoresOutput {
    panic!("unexpected operation: {:?}", operation)
}

/// `STORE-1`, `STORE-2`, `STORE-5`, `STORE-7`, opening asks the service to identify the data source and read it, then
/// sends the view, with the other values' names in order, and the data source as the service identified it.
#[test]
fn opening_reads_the_parts_and_sends_the_view() {
    let mut shell = Shell::new();
    let as_asked = source("parts.csv");
    let resolved = source("/folder/parts.csv");
    let parts = vec![
        item("RESCO", "RES1", &[("Notes", ""), ("LCSC", "C1234")]),
        item("CAPCO", "CAP1", &[("Notes", "0402"), ("LCSC", "C5678")]),
    ];

    let (views, error) = shell.process(
        Event::OpenParts {
            source: as_asked.clone(),
        },
        service(&[as_asked.clone()], &resolved, Ok(parts.clone())),
    );

    assert_eq!(error, None);
    assert_eq!(shell.take_operations(), vec![
        StoresOperation::ResolveSource {
            source: as_asked.clone(),
        },
        StoresOperation::ReadParts {
            source: resolved.clone(),
        },
    ]);
    let expected_view = PartsView {
        source: resolved.clone(),
        field_names: vec!["LCSC".to_string(), "Notes".to_string()],
        parts,
    };
    assert_eq!(views, vec![expected_view.clone()]);

    // the view can be asked for again, by the data source as it was asked for, or as identified.
    for source in [as_asked.clone(), resolved.clone()] {
        let (views, error) = shell.process(
            Event::RequestPartsView {
                source,
            },
            no_operations,
        );
        assert_eq!(error, None);
        assert_eq!(views, vec![expected_view.clone()]);
    }

    let expected_status = PartsSourceStatus {
        source: resolved.clone(),
        loaded: true,
        reading: false,
        error: None,
    };
    assert_eq!(shell.status(&as_asked), Some(expected_status.clone()));
    assert_eq!(shell.status(&resolved), Some(expected_status));
}

/// `STORE-1`, a data source with no parts.
#[test]
fn a_data_source_with_no_parts() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");

    let (views, error) = shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![])),
    );

    assert_eq!(error, None);
    assert_eq!(views, vec![PartsView {
        source: parts,
        field_names: vec![],
        parts: vec![],
    }]);
}

/// `STORE-5`, opening a data source that's loaded already uses the parts loaded, asking the service for nothing.
#[test]
fn opening_again_uses_the_parts_loaded_before() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");
    shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![item("RESCO", "RES1", &[])])),
    );
    shell.take_operations();

    let (views, error) = shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        no_operations,
    );

    assert_eq!(error, None);
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);
}

/// `STORE-5`, `STORE-6`, a data source written differently is identified by the service, and, being the same data
/// source, isn't read again.
#[test]
fn a_data_source_written_differently_is_not_read_again() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");
    let other_spelling = source("/folder/other/../parts.csv");
    let mut service = service(&[other_spelling.clone()], &parts, Ok(vec![item("RESCO", "RES1", &[])]));
    shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        &mut service,
    );
    shell.take_operations();

    let (views, error) = shell.process(
        Event::OpenParts {
            source: other_spelling.clone(),
        },
        &mut service,
    );

    assert_eq!(error, None);
    assert_eq!(shell.take_operations(), vec![StoresOperation::ResolveSource {
        source: other_spelling.clone(),
    }]);
    assert_eq!(views[0].source, parts);
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);
    assert_eq!(
        shell
            .status(&other_spelling)
            .map(|status| status.source),
        Some(parts)
    );
}

/// `STORE-1`, loading again reads the data source again, and sends the changed parts.
#[test]
fn loading_again_replaces_the_parts() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");
    shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![item("RESCO", "RES1", &[])])),
    );
    shell.take_operations();

    let (views, error) = shell.process(
        Event::LoadParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![item("CAPCO", "CAP1", &[])])),
    );

    assert_eq!(error, None);
    assert_eq!(shell.take_operations(), vec![
        StoresOperation::ResolveSource {
            source: parts.clone(),
        },
        StoresOperation::ReadParts {
            source: parts.clone(),
        },
    ]);
    assert_eq!(views[0].parts, vec![item("CAPCO", "CAP1", &[])]);
}

/// `STORE-3`, a refused read says why, sends no view, and keeps the parts loaded before, and opening again reads the
/// data source again.
#[test]
fn a_refused_read_keeps_the_parts_loaded_before() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");
    shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![item("RESCO", "RES1", &[])])),
    );

    let (views, error) = shell.process(
        Event::LoadParts {
            source: parts.clone(),
        },
        service(&[], &parts, Err("Missing header. header: 'Manufacturer'".to_string())),
    );

    let expected_error =
        "Unable to load the parts. source: '/folder/parts.csv', cause: Missing header. header: 'Manufacturer'"
            .to_string();
    assert_eq!(error, Some(expected_error.clone()));
    assert_eq!(views, vec![]);
    let status = shell.status(&parts).unwrap();
    assert!(status.loaded);
    assert_eq!(status.error, Some(expected_error));

    let (views, error) = shell.process(
        Event::RequestPartsView {
            source: parts.clone(),
        },
        no_operations,
    );
    assert_eq!(error, None);
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);
}

/// `STORE-3`, `STORE-5`, a refused open sends no view, and opening again, once the data source is fixed, reads it.
#[test]
fn a_refused_open_sends_no_view() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");

    let (views, error) = shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Err("Missing header. header: 'Manufacturer'".to_string())),
    );
    assert!(error.is_some());
    assert_eq!(views, vec![]);
    assert_eq!(
        shell
            .status(&parts)
            .map(|status| status.loaded),
        Some(false)
    );

    let (views, error) = shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![item("RESCO", "RES1", &[])])),
    );
    assert_eq!(error, None);
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);
    assert_eq!(
        shell
            .status(&parts)
            .and_then(|status| status.error),
        None
    );
}

/// `STORE-2`, parts can only be viewed once they're loaded, and an event that succeeds clears the error.
#[test]
fn refuses_to_show_parts_that_are_not_loaded() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");

    let (views, error) = shell.process(
        Event::RequestPartsView {
            source: parts.clone(),
        },
        no_operations,
    );
    assert_eq!(views, vec![]);
    assert_eq!(
        error,
        Some("The parts are not loaded. source: '/folder/parts.csv'".to_string())
    );

    let (_views, error) = shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![])),
    );
    assert_eq!(error, None);
}

/// `STORE-5`, two opens of a data source that isn't loaded, before it's read, read it once, and both are sent its view,
/// as it's sent to every view of the data source.
#[test]
fn two_opens_before_the_parts_are_read_read_them_once() {
    let core: Core<Stores> = Core::new();
    let parts = source("/folder/parts.csv");
    let resolved = |request: &mut crux_core::Request<StoresOperation>| {
        core.resolve(request, StoresOutput::SourceResolved {
            source: parts.clone(),
        })
        .unwrap()
    };
    let stores_request = |effects: Vec<Effect>| {
        effects
            .into_iter()
            .find_map(|effect| match effect {
                Effect::Stores(request) => Some(request),
                _ => None,
            })
    };

    let mut first = stores_request(core.process_event(Event::OpenParts {
        source: parts.clone(),
    }))
    .unwrap();
    let mut second = stores_request(core.process_event(Event::OpenParts {
        source: parts.clone(),
    }))
    .unwrap();

    let mut read = stores_request(resolved(&mut first)).expect("a read");
    assert_eq!(read.operation, StoresOperation::ReadParts {
        source: parts.clone(),
    });
    // being read already, so not read again.
    assert!(stores_request(resolved(&mut second)).is_none());

    let effects = core
        .resolve(&mut read, StoresOutput::PartsRead(Ok(vec![item("RESCO", "RES1", &[])])))
        .unwrap();
    let views: Vec<PartsView> = effects
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::PartsView(request) => {
                let PartsViewRendererOperation::View {
                    view,
                } = request.operation;
                Some(view)
            }
            _ => None,
        })
        .collect();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);
}

/// `STORE-7`, the result of a different operation is a fault in the shell, and is refused.
#[test]
fn refuses_the_result_of_a_different_operation() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");

    let (views, error) = shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        |_operation| StoresOutput::PartsRead(Ok(vec![])),
    );

    assert_eq!(views, vec![]);
    assert!(
        error
            .as_deref()
            .is_some_and(|error| error.starts_with("Unexpected stores output.")),
        "{error:?}"
    );
}
