//! The stores core's tables: a data source's records shown read-only (`STORE-17`).
//!
//! The core is pure, so these tests answer the store operations it asks for themselves, with no files (`STORE-7`). The
//! stores service's tests check the operations are done (`planning/stores_service/tests/`).

use std::collections::VecDeque;
use std::path::PathBuf;

use crux_core::Core;
use stores_app::{
    BomViewItem, Cell, ColumnValue, Effect, Event, RefDes, Revision, Source, Stores, StoresOperation, StoresOutput,
    Table, TableKind, TableRecords, TableView, TableViewRendererOperation,
};

fn source(path: &str) -> Source {
    Source::File(PathBuf::from(path))
}

/// A BOM, a row for each designator and its quantity.
fn table(rows: &[&[&str]]) -> Table {
    Table::Bom(
        rows.iter()
            .map(|row| BomViewItem {
                designators: row[0]
                    .split(", ")
                    .map(|ref_des| RefDes::from(ref_des.to_string()))
                    .collect(),
                footprint: "RES_0402".to_string(),
                jlcpcb_part: None,
                quantity: row[1].parse().unwrap(),
                comment: "MPN1 (MFR1)".to_string(),
            })
            .collect(),
    )
}

/// A read of a data source with the table, whose revision, as a file's, is the same only if its contents are.
fn read(table: Table) -> TableRecords {
    TableRecords {
        revision: Revision(format!("{:?}", table)),
        table,
    }
}

/// A shell, which answers each store operation the core asks for, keeps the table views the core sends, and keeps the
/// watches it asks for, so a test can say a data source changed (`STORE-9`).
struct Shell {
    core: Core<Stores>,
    /// The operations the core asked for, in order, since they were last taken, except watches.
    operations: Vec<StoresOperation>,
    watches: Vec<(Source, crux_core::Request<StoresOperation>)>,
}

impl Shell {
    fn new() -> Self {
        Self {
            core: Core::new(),
            operations: vec![],
            watches: vec![],
        }
    }

    /// Processes the event, and answers every operation it leads to. Returns the views sent, and the core's error
    /// after the last of them.
    fn process(
        &mut self,
        event: Event,
        answer: impl FnMut(&StoresOperation) -> StoresOutput,
    ) -> (Vec<TableView>, Option<String>) {
        let effects = self.core.process_event(event);
        self.run(effects, answer)
    }

    /// Says the data source watched changed, as the stores service would, and answers every operation it leads to.
    fn change(
        &mut self,
        source: &Source,
        answer: impl FnMut(&StoresOperation) -> StoresOutput,
    ) -> (Vec<TableView>, Option<String>) {
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
    ) -> (Vec<TableView>, Option<String>) {
        let mut views = vec![];
        let mut effects: VecDeque<Effect> = effects.into();
        while let Some(effect) = effects.pop_front() {
            match effect {
                Effect::Render(_) => {}
                Effect::TableView(request) => {
                    let TableViewRendererOperation::View {
                        view,
                    } = request.operation;
                    views.push(view);
                }
                Effect::PartsView(request) => panic!("unexpected parts view: {:?}", request.operation),
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

    fn watched(&self) -> Vec<Source> {
        self.watches
            .iter()
            .map(|(source, _request)| source.clone())
            .collect()
    }
}

/// A stores service with one data source, which `as_asked` is resolved to, as `resolved`, read as `table`.
fn service(
    as_asked: &Source,
    resolved: &Source,
    table: Result<Option<Table>, String>,
) -> impl FnMut(&StoresOperation) -> StoresOutput {
    let as_asked = as_asked.clone();
    let resolved = resolved.clone();
    move |operation| match operation {
        StoresOperation::ResolveSource {
            source,
        } => {
            assert_eq!(*source, as_asked);
            StoresOutput::SourceResolved {
                source: resolved.clone(),
            }
        }
        StoresOperation::ReadTable {
            source, ..
        } => {
            assert_eq!(*source, resolved);
            StoresOutput::TableRead(
                table
                    .clone()
                    .map(|table| table.map(read)),
            )
        }
        operation => panic!("unexpected operation: {:?}", operation),
    }
}

fn no_operations(operation: &StoresOperation) -> StoresOutput {
    panic!("unexpected operation: {:?}", operation)
}

const KIND: TableKind = TableKind::Bom;

/// Opens the BOM, `bom.csv`, identified as `/folder/bom.csv`, with one row, and takes the operations.
fn given_an_open_table(shell: &mut Shell) -> (Source, Source) {
    let as_asked = source("bom.csv");
    let resolved = source("/folder/bom.csv");
    let (_views, error) = shell.process(
        Event::OpenTable {
            kind: KIND,
            source: as_asked.clone(),
        },
        service(&as_asked, &resolved, Ok(Some(table(&[&["R1", "1"]])))),
    );
    assert_eq!(error, None);
    shell.take_operations();
    (as_asked, resolved)
}

/// `STORE-17`, opening a table identifies its data source, reads it, watches it, and sends its view, with its kind,
/// columns and rows in order, and the data source as the service identified it.
#[test]
fn opening_a_table_reads_it_watches_it_and_sends_its_view() {
    let mut shell = Shell::new();
    let as_asked = source("bom.csv");
    let resolved = source("/folder/bom.csv");
    let rows = table(&[&["R1, R2", "2"], &["C1", "1"]]);

    let (views, error) = shell.process(
        Event::OpenTable {
            kind: KIND,
            source: as_asked.clone(),
        },
        service(&as_asked, &resolved, Ok(Some(rows.clone()))),
    );

    assert_eq!(error, None);
    assert_eq!(shell.take_operations(), vec![
        StoresOperation::ResolveSource {
            source: as_asked.clone(),
        },
        StoresOperation::ReadTable {
            kind: KIND,
            source: resolved.clone(),
        },
    ]);
    assert_eq!(shell.watched(), vec![resolved.clone()]);
    assert_eq!(views, vec![TableView {
        kind: KIND,
        source: resolved.clone(),
        revision: read(rows.clone()).revision,
        columns: rows.columns(),
        table: rows.clone(),
    }]);
    // the columns are named as the data source names them, the comment last.
    let view = &views[0];
    assert_eq!(
        view.columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Designator", "Footprint", "JLCPCB Part", "Quantity", "Comment"]
    );
    assert_eq!(
        view.table
            .cell(0, &ColumnValue::Designator),
        Cell::Text("R1, R2".into())
    );

    // the view model says the table's status, by the data source as asked for, and as identified.
    let view_model = shell.core.view();
    let status = view_model
        .table(KIND, &as_asked)
        .expect("a status");
    assert_eq!(status.source, resolved);
    assert!(status.loaded);
    assert!(!status.reading);
    assert_eq!(status.revision, Some(read(rows).revision));
    assert_eq!(status.error, None);
    assert_eq!(view_model.table(KIND, &resolved), Some(status));
    assert_eq!(view_model.table(TableKind::PartMappings, &as_asked), None);
}

/// `STORE-17`, `STORE-5`, a table loaded already is sent again, with nothing asked of the service.
#[test]
fn opening_a_table_again_reads_nothing() {
    let mut shell = Shell::new();
    let (as_asked, resolved) = given_an_open_table(&mut shell);

    let (views, error) = shell.process(
        Event::OpenTable {
            kind: KIND,
            source: as_asked,
        },
        no_operations,
    );

    assert_eq!(error, None);
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].source, resolved);
    assert_eq!(shell.watched().len(), 1);
}

/// `STORE-17`, `STORE-9`, `STORE-8`, a change reads the table again and sends it, and one leaving it as it was sends
/// no view.
#[test]
fn a_change_reads_the_table_again() {
    let mut shell = Shell::new();
    let (as_asked, resolved) = given_an_open_table(&mut shell);
    let changed = table(&[&["R1", "1"], &["R2", "1"]]);

    let (views, error) = shell.change(&resolved, service(&as_asked, &resolved, Ok(Some(changed.clone()))));

    assert_eq!(error, None);
    assert_eq!(shell.take_operations(), vec![StoresOperation::ReadTable {
        kind: KIND,
        source: resolved.clone(),
    }]);
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].table, changed);

    let (views, error) = shell.change(&resolved, service(&as_asked, &resolved, Ok(Some(changed))));
    assert_eq!(error, None);
    assert_eq!(views, vec![]);
}

/// `STORE-17`, `STORE-3`, a refused read says why, sends no view, and keeps the rows loaded before.
#[test]
fn a_refused_read_says_why_and_keeps_the_rows() {
    let mut shell = Shell::new();
    let (as_asked, resolved) = given_an_open_table(&mut shell);

    let (views, error) = shell.change(
        &resolved,
        service(&as_asked, &resolved, Err("Missing header.".to_string())),
    );

    assert_eq!(views, vec![]);
    assert_eq!(
        error,
        Some("Unable to load the table. kind: Bom, source: '/folder/bom.csv', cause: Missing header.".to_string())
    );
    let view_model = shell.core.view();
    let status = view_model
        .table(KIND, &as_asked)
        .unwrap();
    assert!(status.loaded);
    assert_eq!(status.error, error);

    // the rows loaded before are still shown.
    let (views, _error) = shell.process(
        Event::OpenTable {
            kind: KIND,
            source: as_asked,
        },
        no_operations,
    );
    assert_eq!(views[0].table, table(&[&["R1", "1"]]));
}

/// `STORE-17`, a data source that doesn't exist, such as a BOM not yet built, says so, and is still watched, so it's
/// shown once it's written.
#[test]
fn a_data_source_that_does_not_exist_says_so_and_is_shown_once_it_is_written() {
    let mut shell = Shell::new();
    let as_asked = source("bom.csv");
    let resolved = source("/folder/bom.csv");

    let (views, error) = shell.process(
        Event::OpenTable {
            kind: KIND,
            source: as_asked.clone(),
        },
        service(&as_asked, &resolved, Ok(None)),
    );

    assert_eq!(views, vec![]);
    assert_eq!(
        error,
        Some("Data source not found. source: '/folder/bom.csv'".to_string())
    );
    assert_eq!(shell.watched(), vec![resolved.clone()]);
    let view_model = shell.core.view();
    let status = view_model
        .table(KIND, &as_asked)
        .unwrap();
    assert!(!status.loaded);
    assert_eq!(status.error, error);

    let built = table(&[&["R1", "1"]]);
    let (views, error) = shell.change(&resolved, service(&as_asked, &resolved, Ok(Some(built.clone()))));

    assert_eq!(error, None);
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].table, built);
    assert_eq!(
        shell
            .core
            .view()
            .table(KIND, &as_asked)
            .unwrap()
            .error,
        None
    );
}

/// `STORE-17`, `STORE-7`, the result of a different operation is a fault in the shell, and is refused.
#[test]
fn the_result_of_a_different_operation_is_refused() {
    let mut shell = Shell::new();
    let as_asked = source("bom.csv");

    let (views, error) = shell.process(
        Event::OpenTable {
            kind: KIND,
            source: as_asked.clone(),
        },
        |operation| match operation {
            StoresOperation::ResolveSource {
                source,
            } => StoresOutput::SourceResolved {
                source: source.clone(),
            },
            _ => StoresOutput::PartsRead(Err("a different result".to_string())),
        },
    );

    assert_eq!(views, vec![]);
    assert!(
        error
            .as_deref()
            .is_some_and(|error| error.starts_with("Unexpected stores output.")),
        "error: {:?}",
        error
    );
}
