//! The stores core's parts data sources (`STORE-1` to `STORE-8`).
//!
//! The core is pure, so these tests answer the store operations it asks for themselves, with no files (`STORE-7`). The
//! stores service's tests check the operations are done (`planning/stores_service/tests/`).

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use crux_core::Core;
use stores_app::{
    Effect, Event, Part, Parts, PartsSource, PartsSourceStatus, PartsView, PartsViewItem, PartsViewRendererOperation,
    Revision, Stores, StoresOperation, StoresOutput,
};

fn source(path: &str) -> PartsSource {
    PartsSource::File(PathBuf::from(path))
}

/// The revision of a data source with `items`, which, as a file's, is the same only if its contents are (`STORE-8`).
fn revision_of(items: &[PartsViewItem]) -> Revision {
    Revision(format!("{:?}", items))
}

/// A read of a data source with `items`.
fn read(items: Vec<PartsViewItem>) -> Parts {
    Parts {
        revision: revision_of(&items),
        parts: items,
    }
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
///
/// It keeps the watches the core asks for, which are streams, so a test can say a data source changed (`STORE-9`).
struct Shell {
    core: Core<Stores>,
    /// The operations the core asked for, in order, since they were last taken, except watches.
    operations: Vec<StoresOperation>,
    /// The watches the core asked for, by data source.
    watches: Vec<(PartsSource, crux_core::Request<StoresOperation>)>,
}

impl Shell {
    fn new() -> Self {
        Self {
            core: Core::new(),
            operations: vec![],
            watches: vec![],
        }
    }

    /// Processes the event, and answers every operation it leads to, until the core asks for nothing more. Returns
    /// the views sent, and the core's error after the last of them.
    fn process(
        &mut self,
        event: Event,
        answer: impl FnMut(&StoresOperation) -> StoresOutput,
    ) -> (Vec<PartsView>, Option<String>) {
        let effects = self.core.process_event(event);
        self.run(effects, answer)
    }

    /// Says the data source watched changed, as the stores service would, and answers every operation it leads to.
    fn change(
        &mut self,
        source: &PartsSource,
        answer: impl FnMut(&StoresOperation) -> StoresOutput,
    ) -> (Vec<PartsView>, Option<String>) {
        let index = self
            .watches
            .iter()
            .position(|(watched, _request)| watched == source)
            .expect("watched");
        let effects = self
            .core
            .resolve(&mut self.watches[index].1, StoresOutput::Changed)
            .unwrap();
        self.run(effects, answer)
    }

    fn run(
        &mut self,
        effects: Vec<Effect>,
        mut answer: impl FnMut(&StoresOperation) -> StoresOutput,
    ) -> (Vec<PartsView>, Option<String>) {
        let mut views = vec![];
        let mut effects: VecDeque<Effect> = effects.into();
        while let Some(effect) = effects.pop_front() {
            match effect {
                Effect::Render(_) => {}
                Effect::PartsView(request) => {
                    let PartsViewRendererOperation::View {
                        view,
                    } = request.operation;
                    views.push(view);
                }
                Effect::Stores(request) if request.operation.is_stream() => {
                    let StoresOperation::WatchSource {
                        source,
                    } = &request.operation
                    else {
                        panic!("unexpected stream: {:?}", request.operation)
                    };
                    self.watches
                        .push((source.clone(), request));
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

    fn watched(&self) -> Vec<PartsSource> {
        self.watches
            .iter()
            .map(|(source, _request)| source.clone())
            .collect()
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
        } if *source == resolved => StoresOutput::PartsRead(parts.clone().map(read)),
        StoresOperation::ReadParts {
            ..
        } => StoresOutput::PartsRead(Err("No such file.".to_string())),
        // kept by the shell, not answered.
        StoresOperation::WatchSource {
            ..
        } => unreachable!(),
        // the stores core has no documents.
        operation @ (StoresOperation::ReadDocument {
            ..
        }
        | StoresOperation::WriteDocument {
            ..
        }) => panic!("unexpected operation: {:?}", operation),
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
        revision: revision_of(&parts),
        field_names: vec!["LCSC".to_string(), "Notes".to_string()],
        parts: parts.clone(),
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
        revision: Some(revision_of(&parts)),
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
        revision: revision_of(&[]),
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

    let mut read_request = stores_request(resolved(&mut first)).expect("a read");
    assert_eq!(read_request.operation, StoresOperation::ReadParts {
        source: parts.clone(),
    });
    // being read already, so not read again.
    assert!(stores_request(resolved(&mut second)).is_none());

    let effects = core
        .resolve(
            &mut read_request,
            StoresOutput::PartsRead(Ok(read(vec![item("RESCO", "RES1", &[])]))),
        )
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
        |_operation| StoresOutput::PartsRead(Ok(read(vec![]))),
    );

    assert_eq!(views, vec![]);
    assert!(
        error
            .as_deref()
            .is_some_and(|error| error.starts_with("Unexpected stores output.")),
        "{error:?}"
    );
}

/// `STORE-8`, loading again a data source whose revision hasn't changed sends no view, as every view of it shows that
/// revision already, and a changed revision is sent.
#[test]
fn loading_again_an_unchanged_data_source_sends_no_view() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");
    let items = vec![item("RESCO", "RES1", &[])];
    shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(items.clone())),
    );

    let (views, error) = shell.process(
        Event::LoadParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(items.clone())),
    );
    assert_eq!(error, None);
    assert_eq!(views, vec![]);
    assert_eq!(
        shell
            .status(&parts)
            .and_then(|status| status.revision),
        Some(revision_of(&items))
    );

    let changed = vec![item("CAPCO", "CAP1", &[])];
    let (views, error) = shell.process(
        Event::LoadParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(changed.clone())),
    );
    assert_eq!(error, None);
    assert_eq!(views[0].revision, revision_of(&changed));
    assert_eq!(
        shell
            .status(&parts)
            .and_then(|status| status.revision),
        Some(revision_of(&changed))
    );
}

/// `STORE-3`, `STORE-8`, an unchanged data source, loaded again after a refused load, clears the error, though it sends
/// no view.
#[test]
fn an_unchanged_load_after_a_refused_one_clears_the_error() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");
    let items = vec![item("RESCO", "RES1", &[])];
    shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(items.clone())),
    );
    shell.process(
        Event::LoadParts {
            source: parts.clone(),
        },
        service(&[], &parts, Err("Missing header. header: 'Manufacturer'".to_string())),
    );

    let (views, error) = shell.process(
        Event::LoadParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(items)),
    );

    assert_eq!(error, None);
    assert_eq!(views, vec![]);
    assert_eq!(
        shell
            .status(&parts)
            .and_then(|status| status.error),
        None
    );
}

/// `STORE-9`, a data source is watched once it's identified, once however often it's opened, and a change reads it
/// again, and sends its view, without being asked.
#[test]
fn a_change_to_a_data_source_watched_reads_it_again() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");
    let other_spelling = source("/folder/other/../parts.csv");
    shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![item("RESCO", "RES1", &[])])),
    );
    shell.process(
        Event::OpenParts {
            source: other_spelling.clone(),
        },
        service(&[other_spelling.clone()], &parts, Ok(vec![item("RESCO", "RES1", &[])])),
    );
    shell.process(
        Event::LoadParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![item("RESCO", "RES1", &[])])),
    );
    assert_eq!(shell.watched(), vec![parts.clone()]);
    shell.take_operations();

    let (views, error) = shell.change(&parts, service(&[], &parts, Ok(vec![item("CAPCO", "CAP1", &[])])));

    assert_eq!(error, None);
    assert_eq!(shell.take_operations(), vec![StoresOperation::ReadParts {
        source: parts.clone(),
    }]);
    assert_eq!(views[0].parts, vec![item("CAPCO", "CAP1", &[])]);

    // still watched, so another change is seen too.
    let (views, _error) = shell.change(&parts, service(&[], &parts, Ok(vec![item("DIODECO", "D1", &[])])));
    assert_eq!(views[0].parts, vec![item("DIODECO", "D1", &[])]);
}

/// `STORE-8`, `STORE-9`, a change that leaves the contents as they were, for example a file written again unchanged,
/// sends no view.
#[test]
fn a_change_that_leaves_the_contents_unchanged_sends_no_view() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");
    let items = vec![item("RESCO", "RES1", &[])];
    shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(items.clone())),
    );

    let (views, error) = shell.change(&parts, service(&[], &parts, Ok(items)));

    assert_eq!(error, None);
    assert_eq!(views, vec![]);
}

/// `STORE-3`, `STORE-9`, a change that leaves the data source unreadable, for example a file saved half-written, says
/// why, and keeps the parts loaded before, and the next change, once it's readable again, sends its view.
#[test]
fn a_change_that_leaves_the_data_source_unreadable_keeps_the_parts() {
    let mut shell = Shell::new();
    let parts = source("/folder/parts.csv");
    shell.process(
        Event::OpenParts {
            source: parts.clone(),
        },
        service(&[], &parts, Ok(vec![item("RESCO", "RES1", &[])])),
    );

    let (views, error) = shell.change(&parts, service(&[], &parts, Err("Missing header.".to_string())));
    assert!(error.is_some());
    assert_eq!(views, vec![]);
    let status = shell.status(&parts).unwrap();
    assert!(status.loaded);
    assert!(status.error.is_some());

    let (views, error) = shell.change(&parts, service(&[], &parts, Ok(vec![item("CAPCO", "CAP1", &[])])));
    assert_eq!(error, None);
    assert_eq!(views[0].parts, vec![item("CAPCO", "CAP1", &[])]);
    assert_eq!(
        shell
            .status(&parts)
            .and_then(|status| status.error),
        None
    );
}

/// `STORE-9`, a change while the parts are being read makes them out of date, so they're read again once they are,
/// and only the parts read after the change are sent.
#[test]
fn a_change_while_the_parts_are_read_reads_them_again() {
    let core: Core<Stores> = Core::new();
    let parts = source("/folder/parts.csv");
    let take = |effects: Vec<Effect>| {
        let mut stores = vec![];
        let mut views = vec![];
        for effect in effects {
            match effect {
                Effect::Stores(request) => stores.push(request),
                Effect::PartsView(request) => {
                    let PartsViewRendererOperation::View {
                        view,
                    } = request.operation;
                    views.push(view);
                }
                Effect::Render(_) => {}
            }
        }
        (stores, views)
    };

    let (mut stores, _views) = take(core.process_event(Event::OpenParts {
        source: parts.clone(),
    }));
    let (stores_after_resolve, _views) = take(
        core.resolve(&mut stores[0], StoresOutput::SourceResolved {
            source: parts.clone(),
        })
        .unwrap(),
    );
    let mut read_request = None;
    let mut watch = None;
    for request in stores_after_resolve {
        match request.operation {
            StoresOperation::ReadParts {
                ..
            } => read_request = Some(request),
            StoresOperation::WatchSource {
                ..
            } => watch = Some(request),
            _ => panic!("unexpected operation"),
        }
    }
    let (mut read_request, mut watch) = (read_request.expect("a read"), watch.expect("a watch"));

    // changed while being read.
    let (stores, views) = take(
        core.resolve(&mut watch, StoresOutput::Changed)
            .unwrap(),
    );
    assert!(stores.is_empty());
    assert!(views.is_empty());

    // the parts read before the change aren't used, and are read again.
    let (mut stores, views) = take(
        core.resolve(
            &mut read_request,
            StoresOutput::PartsRead(Ok(read(vec![item("OLDCO", "OLD1", &[])]))),
        )
        .unwrap(),
    );
    assert!(views.is_empty());
    assert_eq!(stores.len(), 1);
    assert_eq!(stores[0].operation, StoresOperation::ReadParts {
        source: parts.clone(),
    });

    let (_stores, views) = take(
        core.resolve(
            &mut stores[0],
            StoresOutput::PartsRead(Ok(read(vec![item("NEWCO", "NEW1", &[])]))),
        )
        .unwrap(),
    );
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].parts, vec![item("NEWCO", "NEW1", &[])]);
}
