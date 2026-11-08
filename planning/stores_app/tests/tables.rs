//! The stores core's tables: a data source's records shown (`STORE-17`), and changed (`STORE-19`).
//!
//! The core is pure, so these tests answer the store operations it asks for themselves, with no files (`STORE-7`). The
//! stores service's tests check the operations are done (`planning/stores_service/tests/`).

use std::collections::VecDeque;
use std::path::PathBuf;

use crux_core::Core;
use stores_app::{
    AddColumnsError, AssemblyRule, AssemblyRulesViewItem, BomViewItem, Cell, ColumnPosition, ColumnRef, ColumnValue,
    ColumnsToAdd, EdaSubstitutionRuleTransformItem, EdaTool, Effect, Event, FieldChange, FieldPattern, FieldValue,
    Part, PartMappingsViewItem, PartsWriteError, Pattern, RefDes, Revision, RowKey, RowOrdering, RowRef, RowsChange,
    Source, Stores, StoresOperation, StoresOutput, SubstitutionsViewItem, Table, TableChange, TableKind, TableRecords,
    TableRow, TableView, TableViewRendererOperation,
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

/// A read of a data source with the table, whose revision, as a file's, is the same only if its contents are, and whose
/// rows' keys are their places. Part mappings can be changed, as a CSV file's: each column's reference is its name, and
/// the rows are ordered by their place (`STORE-19`).
fn read(table: Table) -> TableRecords {
    let mut records = TableRecords::read_only(Revision(format!("{:?}", table)), table);
    if records.table.is_changeable() {
        for column in records.columns.iter_mut() {
            column.reference = Some(ColumnRef(column.name.clone()));
        }
        records.ordering = RowOrdering::Position;
    }
    records
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

    /// Processes the event, and keeps the one operation it asks for, unanswered, as the service would while it's done.
    fn process_unanswered(&mut self, event: Event) -> crux_core::Request<StoresOperation> {
        let mut requests: Vec<_> = self
            .core
            .process_event(event)
            .into_iter()
            .filter_map(|effect| match effect {
                Effect::Stores(request) => Some(request),
                _ => None,
            })
            .collect();
        assert_eq!(requests.len(), 1);
        requests.remove(0)
    }

    /// Answers an operation kept by [`Self::process_unanswered`], and every operation it leads to.
    fn answer(
        &mut self,
        mut request: crux_core::Request<StoresOperation>,
        output: StoresOutput,
        answer: impl FnMut(&StoresOperation) -> StoresOutput,
    ) -> (Vec<TableView>, Option<String>) {
        let effects = self
            .core
            .resolve(&mut request, output)
            .unwrap();
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
        // `STORE-19`, each row's key, as the back end made it, and a BOM's rows can't be moved.
        keys: vec![RowKey("0".to_string()), RowKey("1".to_string())],
        ordering: RowOrdering::Unsupported,
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

const MAPPINGS: TableKind = TableKind::PartMappings;

/// A DipTrace part mapping, matching a placement's name with the pattern, to `MFR1`'s part.
fn mapping(name: &str, mpn: &str) -> PartMappingsViewItem {
    PartMappingsViewItem {
        eda_tool: EdaTool::DipTrace,
        criteria: vec![FieldPattern {
            field_name: "name".to_string(),
            pattern: Pattern::parse(name),
        }],
        part: Part {
            manufacturer: "MFR1".to_string(),
            mpn: mpn.to_string(),
        },
    }
}

fn mappings(rows: &[(&str, &str)]) -> Table {
    Table::PartMappings(
        rows.iter()
            .map(|(name, mpn)| mapping(name, mpn))
            .collect(),
    )
}

fn row(name: &str, mpn: &str) -> TableRow {
    TableRow::PartMappings(mapping(name, mpn))
}

fn key(key: &str) -> RowKey {
    RowKey(key.to_string())
}

fn value(column: &str, value: &str) -> FieldValue {
    FieldValue {
        column: ColumnRef(column.to_string()),
        value: value.to_string(),
    }
}

/// The values the data source has of a part mapping made by [`mapping`], by their columns' references.
fn values(name: &str, mpn: &str) -> Vec<FieldValue> {
    vec![
        value("Eda", "DipTrace"),
        value("Name", name),
        value("Manufacturer", "MFR1"),
        value("Mpn", mpn),
    ]
}

fn row_ref(key_: &str, name: &str, mpn: &str) -> RowRef {
    RowRef {
        key: key(key_),
        expected: values(name, mpn),
    }
}

const CHANGED: &str = "changed";

/// A stores service that changes the rows, the data source's revision then `changed`, and then reads the table as
/// `after`.
fn changes_then_reads(resolved: &Source, after: Table) -> impl FnMut(&StoresOperation) -> StoresOutput {
    let resolved = resolved.clone();
    move |operation| match operation {
        StoresOperation::ChangeRows {
            source, ..
        } => {
            assert_eq!(*source, resolved);
            StoresOutput::RowsChanged(Ok(Revision(CHANGED.to_string())))
        }
        StoresOperation::ReadTable {
            source, ..
        } => {
            assert_eq!(*source, resolved);
            StoresOutput::TableRead(Ok(Some(read(after.clone()))))
        }
        operation => panic!("unexpected operation: {:?}", operation),
    }
}

/// Opens part mappings, `mappings.csv`, identified as `/folder/mappings.csv`, with the rows `R1`, `R2` and `R3`, and
/// takes the operations. Returns the data source as asked for, as identified, and the view sent.
fn given_open_part_mappings(shell: &mut Shell) -> (Source, Source, TableView) {
    let as_asked = source("mappings.csv");
    let resolved = source("/folder/mappings.csv");
    let rows = mappings(&[("R1", "MPN1"), ("R2", "MPN2"), ("R3", "MPN3")]);
    let (mut views, error) = shell.process(
        Event::OpenTable {
            kind: MAPPINGS,
            source: as_asked.clone(),
        },
        service(&as_asked, &resolved, Ok(Some(rows))),
    );
    assert_eq!(error, None);
    shell.take_operations();
    (as_asked, resolved, views.remove(0))
}

/// `STORE-19`, a table's view has each row's key, each column's reference, and how its rows are ordered, and a row, with
/// its key, can be had from it for a front end to change.
#[test]
fn a_tables_view_has_its_rows_keys_its_columns_references_and_its_ordering() {
    let mut shell = Shell::new();
    let (_as_asked, _resolved, view) = given_open_part_mappings(&mut shell);

    assert_eq!(view.keys, vec![key("0"), key("1"), key("2")]);
    assert_eq!(view.ordering, RowOrdering::Position);
    assert_eq!(
        view.columns
            .iter()
            .map(|column| column.reference.clone())
            .collect::<Vec<_>>(),
        ["Eda", "Name", "Manufacturer", "Mpn"]
            .map(|reference| Some(ColumnRef(reference.to_string())))
            .to_vec()
    );
    assert_eq!(view.row(1), Some((key("1"), row("R2", "MPN2"))));
    assert_eq!(view.row(3), None);
}

/// `STORE-19`, a change has the service change the rows by their keys and their columns' references, updating only a
/// row's values that changed, with their values as they were, adding a row's values, and removing or moving a row,
/// checking its values, and the other's it's moved before, then reads the table again, and sends its view.
#[test]
fn a_change_has_the_service_change_the_rows_then_reads_the_table_again() {
    let cases = [
        (
            TableChange::Update {
                key: key("1"),
                old: row("R2", "MPN2"),
                new: row("/R[/", "MPN5"),
            },
            RowsChange::Update {
                key: key("1"),
                changes: vec![
                    FieldChange {
                        column: ColumnRef("Name".to_string()),
                        old: "R2".to_string(),
                        new: "/R[/".to_string(),
                    },
                    FieldChange {
                        column: ColumnRef("Mpn".to_string()),
                        old: "MPN2".to_string(),
                        new: "MPN5".to_string(),
                    },
                ],
            },
        ),
        (
            TableChange::Add {
                row: row("/C.*/", "MPN4"),
            },
            RowsChange::Add {
                values: values("/C.*/", "MPN4"),
            },
        ),
        (
            TableChange::Remove {
                key: key("0"),
                row: row("R1", "MPN1"),
            },
            RowsChange::Remove {
                row: row_ref("0", "R1", "MPN1"),
            },
        ),
        (
            TableChange::Move {
                key: key("2"),
                row: row("R3", "MPN3"),
                before: Some((key("0"), row("R1", "MPN1"))),
            },
            RowsChange::Move {
                row: row_ref("2", "R3", "MPN3"),
                before: Some(row_ref("0", "R1", "MPN1")),
            },
        ),
        (
            TableChange::Move {
                key: key("0"),
                row: row("R1", "MPN1"),
                before: None,
            },
            RowsChange::Move {
                row: row_ref("0", "R1", "MPN1"),
                before: None,
            },
        ),
    ];

    for (change, expected) in cases {
        let mut shell = Shell::new();
        let (as_asked, resolved, _view) = given_open_part_mappings(&mut shell);
        let after = mappings(&[("R1", "MPN1"), ("R9", "MPN9")]);

        let (views, error) = shell.process(
            Event::ChangeTable {
                kind: MAPPINGS,
                source: as_asked.clone(),
                change: change.clone(),
            },
            changes_then_reads(&resolved, after.clone()),
        );

        assert_eq!(error, None, "{:?}", change);
        assert_eq!(
            shell.take_operations(),
            vec![
                StoresOperation::ChangeRows {
                    kind: MAPPINGS,
                    source: resolved.clone(),
                    change: expected,
                },
                StoresOperation::ReadTable {
                    kind: MAPPINGS,
                    source: resolved.clone(),
                },
            ],
            "{:?}",
            change
        );
        // every view shows the change, with the keys the back end gives the rows now.
        assert_eq!(views.len(), 1, "{:?}", change);
        assert_eq!(views[0].table, after, "{:?}", change);
        assert_eq!(views[0].keys, vec![key("0"), key("1")], "{:?}", change);
        let view_model = shell.core.view();
        let status = view_model
            .table(MAPPINGS, &as_asked)
            .unwrap();
        assert!(!status.changing);
        assert_eq!(status.change_error, None);
    }
}

/// `STORE-19`, the status says a change is being made, until which another is refused.
#[test]
fn a_change_while_another_is_made_is_refused() {
    let mut shell = Shell::new();
    let (as_asked, resolved, _view) = given_open_part_mappings(&mut shell);
    let remove = |key_: &str, name: &str, mpn: &str| Event::ChangeTable {
        kind: MAPPINGS,
        source: as_asked.clone(),
        change: TableChange::Remove {
            key: key(key_),
            row: row(name, mpn),
        },
    };

    let change = shell.process_unanswered(remove("0", "R1", "MPN1"));
    assert!(
        shell
            .core
            .view()
            .table(MAPPINGS, &as_asked)
            .unwrap()
            .changing
    );

    let (views, error) = shell.process(remove("1", "R2", "MPN2"), no_operations);
    assert_eq!(views, vec![]);
    assert_eq!(
        error,
        Some("The table is being changed. kind: PartMappings, source: '/folder/mappings.csv'".to_string())
    );

    let after = mappings(&[("R2", "MPN2"), ("R3", "MPN3")]);
    let (views, error) = shell.answer(
        change,
        StoresOutput::RowsChanged(Ok(Revision(CHANGED.to_string()))),
        changes_then_reads(&resolved, after.clone()),
    );
    assert_eq!(error, None);
    assert_eq!(views[0].table, after);
    assert!(
        !shell
            .core
            .view()
            .table(MAPPINGS, &as_asked)
            .unwrap()
            .changing
    );
}

/// `STORE-19`, a change to a table that isn't loaded, of a kind that can't be changed yet, such as a BOM, which the
/// Variant Builder writes, with a value in a column the data source doesn't have, or moving a row of a table whose rows
/// aren't ordered, is refused, changing nothing.
#[test]
fn a_change_the_table_can_not_take_is_refused() {
    // not loaded.
    let mut shell = Shell::new();
    let (views, error) = shell.process(
        Event::ChangeTable {
            kind: MAPPINGS,
            source: source("mappings.csv"),
            change: TableChange::Add {
                row: row("R4", "MPN4"),
            },
        },
        no_operations,
    );
    assert_eq!(views, vec![]);
    assert_eq!(
        error,
        Some("The table is not loaded. kind: PartMappings, source: 'mappings.csv'".to_string())
    );

    // a BOM.
    let mut shell = Shell::new();
    let (as_asked, _resolved) = given_an_open_table(&mut shell);
    let (_views, error) = shell.process(
        Event::ChangeTable {
            kind: KIND,
            source: as_asked,
            change: TableChange::Add {
                row: row("R4", "MPN4"),
            },
        },
        no_operations,
    );
    assert_eq!(
        error,
        Some("The table can't be changed. kind: Bom, source: '/folder/bom.csv'".to_string())
    );

    // a pattern of a field whose column the data source doesn't have, such as a placement's value.
    let mut shell = Shell::new();
    let (as_asked, _resolved, _view) = given_open_part_mappings(&mut shell);
    let mut with_a_value = mapping("R4", "MPN4");
    with_a_value
        .criteria
        .push(FieldPattern {
            field_name: "value".to_string(),
            pattern: Pattern::parse("330R"),
        });
    let (_views, error) = shell.process(
        Event::ChangeTable {
            kind: MAPPINGS,
            source: as_asked.clone(),
            change: TableChange::Add {
                row: TableRow::PartMappings(with_a_value),
            },
        },
        no_operations,
    );
    assert_eq!(
        error,
        Some(
            "The column doesn't exist. kind: PartMappings, source: '/folder/mappings.csv', column: Pattern(\"value\")"
                .to_string()
        )
    );
    assert_eq!(
        shell
            .core
            .view()
            .table(MAPPINGS, &as_asked)
            .unwrap()
            .change_error,
        error
    );

    // rows that aren't ordered, as a database table's without an ordering column.
    let mut shell = Shell::new();
    let as_asked = source("mappings.csv");
    let resolved = source("/folder/mappings.csv");
    let (_views, error) = shell.process(
        Event::OpenTable {
            kind: MAPPINGS,
            source: as_asked.clone(),
        },
        |operation| match operation {
            StoresOperation::ResolveSource {
                ..
            } => StoresOutput::SourceResolved {
                source: resolved.clone(),
            },
            StoresOperation::ReadTable {
                ..
            } => {
                let mut records = read(mappings(&[("R1", "MPN1"), ("R2", "MPN2")]));
                records.ordering = RowOrdering::Unsupported;
                StoresOutput::TableRead(Ok(Some(records)))
            }
            operation => panic!("unexpected operation: {:?}", operation),
        },
    );
    assert_eq!(error, None);
    let (_views, error) = shell.process(
        Event::ChangeTable {
            kind: MAPPINGS,
            source: as_asked,
            change: TableChange::Move {
                key: key("1"),
                row: row("R2", "MPN2"),
                before: None,
            },
        },
        no_operations,
    );
    assert_eq!(
        error,
        Some("The rows can't be moved. kind: PartMappings, source: '/folder/mappings.csv'".to_string())
    );
}

/// `STORE-19`, a change the service refuses, as a row's values aren't those the change was made to, reads the table
/// again, sends its view, and keeps the change's error until a change succeeds.
#[test]
fn a_change_the_service_refuses_reads_the_table_again() {
    let mut shell = Shell::new();
    let (as_asked, resolved, _view) = given_open_part_mappings(&mut shell);
    let changed_by_another = mappings(&[("R1", "MPN1"), ("R2", "MPN7"), ("R3", "MPN3")]);
    let current_revision = read(changed_by_another.clone()).revision;
    let mut service = service(&as_asked, &resolved, Ok(Some(changed_by_another.clone())));

    let (views, error) = shell.process(
        Event::ChangeTable {
            kind: MAPPINGS,
            source: as_asked.clone(),
            change: TableChange::Update {
                key: key("1"),
                old: row("R2", "MPN2"),
                new: row("R2", "MPN5"),
            },
        },
        |operation| match operation {
            StoresOperation::ChangeRows {
                ..
            } => StoresOutput::RowsChanged(Err(PartsWriteError::Changed {
                revision: Some(current_revision.clone()),
            })),
            operation => service(operation),
        },
    );

    // the read after the refusal succeeds, so it's the table's status that says why the change was refused.
    assert_eq!(error, None);
    assert_eq!(shell.take_operations()[1..], [StoresOperation::ReadTable {
        kind: MAPPINGS,
        source: resolved.clone(),
    }]);
    // every view shows the table as it is now, which the change can be made to again.
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].table, changed_by_another);
    let expected_error = format!(
        "The table has changed since the change was made. kind: PartMappings, source: '/folder/mappings.csv', current_revision: '{}'",
        current_revision
    );
    let view_model = shell.core.view();
    let status = view_model
        .table(MAPPINGS, &as_asked)
        .unwrap();
    assert_eq!(status.change_error, Some(expected_error));
    assert_eq!(status.revision, Some(current_revision));

    // a change that succeeds clears it.
    let (_views, error) = shell.process(
        Event::ChangeTable {
            kind: MAPPINGS,
            source: as_asked.clone(),
            change: TableChange::Update {
                key: key("1"),
                old: row("R2", "MPN7"),
                new: row("R2", "MPN5"),
            },
        },
        changes_then_reads(&resolved, mappings(&[("R1", "MPN1"), ("R2", "MPN5"), ("R3", "MPN3")])),
    );
    assert_eq!(error, None);
    assert_eq!(
        shell
            .core
            .view()
            .table(MAPPINGS, &as_asked)
            .unwrap()
            .change_error,
        None
    );
}

/// `STORE-19`, a change that can't be made says why, and keeps the table as it was, reading nothing.
#[test]
fn a_change_that_can_not_be_made_says_why_and_keeps_the_table() {
    let mut shell = Shell::new();
    let (as_asked, _resolved, view) = given_open_part_mappings(&mut shell);

    let (views, error) = shell.process(
        Event::ChangeTable {
            kind: MAPPINGS,
            source: as_asked.clone(),
            change: TableChange::Remove {
                key: key("0"),
                row: row("R1", "MPN1"),
            },
        },
        |operation| match operation {
            StoresOperation::ChangeRows {
                ..
            } => StoresOutput::RowsChanged(Err(PartsWriteError::Failed {
                cause: "Read-only.".to_string(),
            })),
            operation => panic!("unexpected operation: {:?}", operation),
        },
    );

    assert_eq!(views, vec![]);
    assert_eq!(
        error,
        Some(
            "Unable to change the table. kind: PartMappings, source: '/folder/mappings.csv', cause: Read-only."
                .to_string()
        )
    );
    let (views, _error) = shell.process(
        Event::OpenTable {
            kind: MAPPINGS,
            source: as_asked,
        },
        no_operations,
    );
    assert_eq!(views[0], view);
}

/// A stores service that adds the columns, the data source's revision then `changed`, and then reads the table as
/// `after`.
fn adds_columns_then_reads(resolved: &Source, after: Table) -> impl FnMut(&StoresOperation) -> StoresOutput {
    let resolved = resolved.clone();
    move |operation| match operation {
        StoresOperation::AddColumns {
            source, ..
        } => {
            assert_eq!(*source, resolved);
            StoresOutput::ColumnsAdded(Ok(Revision(CHANGED.to_string())))
        }
        StoresOperation::ReadTable {
            source, ..
        } => {
            assert_eq!(*source, resolved);
            StoresOutput::TableRead(Ok(Some(read(after.clone()))))
        }
        operation => panic!("unexpected operation: {:?}", operation),
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.to_string())
        .collect()
}

/// Opens part mappings, `mappings.csv`, identified as `/folder/mappings.csv`, whose one row has a pattern of each of the
/// fields, so the table has a column of each, and takes the operations.
fn given_part_mappings_with(shell: &mut Shell, field_names: &[&str]) -> (Source, Source) {
    let as_asked = source("mappings.csv");
    let resolved = source("/folder/mappings.csv");
    let mut item = mapping("R1", "MPN1");
    item.criteria = field_names
        .iter()
        .map(|field_name| FieldPattern {
            field_name: field_name.to_string(),
            pattern: Pattern::parse(""),
        })
        .collect();
    let (_views, error) = shell.process(
        Event::OpenTable {
            kind: MAPPINGS,
            source: as_asked.clone(),
        },
        service(&as_asked, &resolved, Ok(Some(Table::PartMappings(vec![item])))),
    );
    assert_eq!(error, None);
    shell.take_operations();
    (as_asked, resolved)
}

/// `STORE-21`, named columns are added where they're asked to go, then the table is read again, and its view sent.
#[test]
fn named_columns_are_added_then_the_table_is_read_again() {
    let mut shell = Shell::new();
    let (as_asked, resolved, _view) = given_open_part_mappings(&mut shell);
    let after = mappings(&[("R1", "MPN1")]);

    let (views, error) = shell.process(
        Event::AddTableColumns {
            kind: MAPPINGS,
            source: as_asked.clone(),
            columns: ColumnsToAdd::Named {
                columns: strings(&["Notes", "Status"]),
                position: ColumnPosition::After("Mpn".to_string()),
            },
        },
        adds_columns_then_reads(&resolved, after.clone()),
    );

    assert_eq!(error, None);
    assert_eq!(shell.take_operations(), vec![
        StoresOperation::AddColumns {
            kind: MAPPINGS,
            source: resolved.clone(),
            columns: strings(&["Notes", "Status"]),
            position: ColumnPosition::After("Mpn".to_string()),
        },
        StoresOperation::ReadTable {
            kind: MAPPINGS,
            source: resolved.clone(),
        },
    ]);
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].table, after);
    let view_model = shell.core.view();
    let status = view_model
        .table(MAPPINGS, &as_asked)
        .unwrap();
    assert!(!status.changing);
    assert_eq!(status.change_error, None);
}

/// `STORE-21`, an EDA tool's columns are those of its fields the table doesn't have, whichever tool's they are, so a
/// column shared by tools is added once, each named as `STORE-17` names it, where the data source puts an EDA tool's
/// columns.
#[test]
fn an_eda_tools_columns_are_those_of_its_fields_the_table_does_not_have() {
    let cases: [(&[&str], EdaTool, &[&str]); 5] = [
        (&["name"], EdaTool::KiCad, &["Package", "Val"]),
        (&["name"], EdaTool::EasyEda, &["Device", "Value"]),
        (&["name"], EdaTool::DipTrace, &["Value"]),
        // KiCad's to EasyEDA's: only `device`, as `value` is there, DipTrace's.
        (&["name", "value", "package", "val"], EdaTool::EasyEda, &["Device"]),
        (&["package"], EdaTool::KiCad, &["Val"]),
    ];

    for (field_names, eda_tool, expected) in cases {
        let mut shell = Shell::new();
        let (as_asked, resolved) = given_part_mappings_with(&mut shell, field_names);

        let (_views, error) = shell.process(
            Event::AddTableColumns {
                kind: MAPPINGS,
                source: as_asked.clone(),
                columns: ColumnsToAdd::EdaTool {
                    eda_tool,
                },
            },
            adds_columns_then_reads(&resolved, mappings(&[("R1", "MPN1")])),
        );

        assert_eq!(error, None, "{:?}, {:?}", field_names, eda_tool);
        assert_eq!(
            shell.take_operations()[0],
            StoresOperation::AddColumns {
                kind: MAPPINGS,
                source: resolved.clone(),
                columns: strings(expected),
                // where the service puts them, as only it knows the order of the file's columns.
                position: ColumnPosition::EdaToolColumns,
            },
            "{:?}, {:?}",
            field_names,
            eda_tool
        );
    }
}

/// `STORE-21`, a table that has an EDA tool's columns already is left as it is, nothing asked of the service.
#[test]
fn an_eda_tools_columns_already_there_are_not_added() {
    let mut shell = Shell::new();
    let (as_asked, _resolved) = given_part_mappings_with(&mut shell, &["name", "value", "device"]);

    for eda_tool in [EdaTool::DipTrace, EdaTool::EasyEda] {
        let (views, error) = shell.process(
            Event::AddTableColumns {
                kind: MAPPINGS,
                source: as_asked.clone(),
                columns: ColumnsToAdd::EdaTool {
                    eda_tool,
                },
            },
            no_operations,
        );
        assert_eq!(views, vec![]);
        assert_eq!(error, None);
    }
}

/// `STORE-21`, `STORE-19`, adding columns to a table that isn't loaded, of a kind that can't be changed, or while a
/// change is being made, is refused, changing nothing.
#[test]
fn adding_columns_the_table_can_not_take_is_refused() {
    let notes = || ColumnsToAdd::Named {
        columns: strings(&["Notes"]),
        position: ColumnPosition::Last,
    };

    // not loaded.
    let mut shell = Shell::new();
    let (_views, error) = shell.process(
        Event::AddTableColumns {
            kind: MAPPINGS,
            source: source("mappings.csv"),
            columns: notes(),
        },
        no_operations,
    );
    assert_eq!(
        error,
        Some("The table is not loaded. kind: PartMappings, source: 'mappings.csv'".to_string())
    );

    // a BOM.
    let mut shell = Shell::new();
    let (as_asked, _resolved) = given_an_open_table(&mut shell);
    let (_views, error) = shell.process(
        Event::AddTableColumns {
            kind: KIND,
            source: as_asked,
            columns: notes(),
        },
        no_operations,
    );
    assert_eq!(
        error,
        Some("The table can't be changed. kind: Bom, source: '/folder/bom.csv'".to_string())
    );

    // while a change is being made, until which the status says so.
    let mut shell = Shell::new();
    let (as_asked, resolved, _view) = given_open_part_mappings(&mut shell);
    let add = || Event::AddTableColumns {
        kind: MAPPINGS,
        source: as_asked.clone(),
        columns: notes(),
    };
    let adding = shell.process_unanswered(add());
    assert!(
        shell
            .core
            .view()
            .table(MAPPINGS, &as_asked)
            .unwrap()
            .changing
    );
    let (_views, error) = shell.process(add(), no_operations);
    assert_eq!(
        error,
        Some("The table is being changed. kind: PartMappings, source: '/folder/mappings.csv'".to_string())
    );
    let (_views, error) = shell.answer(
        adding,
        StoresOutput::ColumnsAdded(Ok(Revision(CHANGED.to_string()))),
        adds_columns_then_reads(&resolved, mappings(&[("R1", "MPN1")])),
    );
    assert_eq!(error, None);
    assert!(
        !shell
            .core
            .view()
            .table(MAPPINGS, &as_asked)
            .unwrap()
            .changing
    );
}

/// `STORE-21`, columns the service refuses to add, as one is there already, the column they're to go by isn't, or the
/// back end can't add them, say why, keep the table as it was, reading nothing, and keep the error until a change
/// succeeds, and a data source that's gone says so, and is read again.
#[test]
fn columns_the_service_refuses_to_add_say_why() {
    let cases = [
        (
            AddColumnsError::ColumnExists {
                column: "Name".to_string(),
            },
            "The column already exists. kind: PartMappings, source: '/folder/mappings.csv', column: 'Name'",
        ),
        (
            AddColumnsError::ColumnNotFound {
                column: "Notes".to_string(),
            },
            "The column doesn't exist. kind: PartMappings, source: '/folder/mappings.csv', column: 'Notes'",
        ),
        (
            AddColumnsError::Failed {
                cause: "Read-only.".to_string(),
            },
            "Unable to change the table. kind: PartMappings, source: '/folder/mappings.csv', cause: Read-only.",
        ),
    ];

    for (refusal, expected) in cases {
        let mut shell = Shell::new();
        let (as_asked, resolved, view) = given_open_part_mappings(&mut shell);

        let (views, error) = shell.process(
            Event::AddTableColumns {
                kind: MAPPINGS,
                source: as_asked.clone(),
                columns: ColumnsToAdd::Named {
                    columns: strings(&["Name"]),
                    position: ColumnPosition::After("Notes".to_string()),
                },
            },
            |operation| match operation {
                StoresOperation::AddColumns {
                    ..
                } => StoresOutput::ColumnsAdded(Err(refusal.clone())),
                operation => panic!("unexpected operation: {:?}", operation),
            },
        );

        assert_eq!(views, vec![]);
        assert_eq!(error.as_deref(), Some(expected));
        let view_model = shell.core.view();
        let status = view_model
            .table(MAPPINGS, &as_asked)
            .unwrap();
        assert_eq!(status.change_error.as_deref(), Some(expected));
        assert!(!status.changing);
        let (views, _error) = shell.process(
            Event::OpenTable {
                kind: MAPPINGS,
                source: as_asked.clone(),
            },
            no_operations,
        );
        assert_eq!(views[0], view);

        // a change that succeeds clears it.
        let (_views, error) = shell.process(
            Event::AddTableColumns {
                kind: MAPPINGS,
                source: as_asked.clone(),
                columns: ColumnsToAdd::EdaTool {
                    eda_tool: EdaTool::KiCad,
                },
            },
            adds_columns_then_reads(&resolved, mappings(&[("R1", "MPN1")])),
        );
        assert_eq!(error, None);
        assert_eq!(
            shell
                .core
                .view()
                .table(MAPPINGS, &as_asked)
                .unwrap()
                .change_error,
            None
        );
    }

    // a data source that's gone.
    let mut shell = Shell::new();
    let (as_asked, resolved, _view) = given_open_part_mappings(&mut shell);
    let mut service = service(&as_asked, &resolved, Ok(None));
    let (_views, error) = shell.process(
        Event::AddTableColumns {
            kind: MAPPINGS,
            source: as_asked.clone(),
            columns: ColumnsToAdd::EdaTool {
                eda_tool: EdaTool::KiCad,
            },
        },
        |operation| match operation {
            StoresOperation::AddColumns {
                ..
            } => StoresOutput::ColumnsAdded(Err(AddColumnsError::SourceNotFound)),
            operation => service(operation),
        },
    );
    assert_eq!(
        error,
        Some("Data source not found. source: '/folder/mappings.csv'".to_string())
    );
    assert_eq!(shell.take_operations()[1..], [StoresOperation::ReadTable {
        kind: MAPPINGS,
        source: resolved.clone(),
    }]);
}

const SUBSTITUTIONS: TableKind = TableKind::Substitutions;

/// A DipTrace substitution, of the fields' patterns and values, with the comment.
fn substitution(fields: &[(&str, &str, &str)], comment: &str) -> SubstitutionsViewItem {
    SubstitutionsViewItem {
        eda_tool: EdaTool::DipTrace,
        criteria: fields
            .iter()
            .map(|(field_name, pattern, _)| FieldPattern {
                field_name: field_name.to_string(),
                pattern: Pattern::parse(pattern),
            })
            .collect(),
        transforms: fields
            .iter()
            .map(|(field_name, _, value)| EdaSubstitutionRuleTransformItem {
                field_name: field_name.to_string(),
                field_value: value.to_string(),
            })
            .collect(),
        comment: Some(comment.to_string()),
    }
}

/// Opens substitutions, `substitutions.csv`, identified as `/folder/substitutions.csv`, with the rows, and takes the
/// operations.
fn given_open_substitutions(shell: &mut Shell, rows: &[SubstitutionsViewItem]) -> (Source, Source) {
    let as_asked = source("substitutions.csv");
    let resolved = source("/folder/substitutions.csv");
    let (_views, error) = shell.process(
        Event::OpenTable {
            kind: SUBSTITUTIONS,
            source: as_asked.clone(),
        },
        service(&as_asked, &resolved, Ok(Some(Table::Substitutions(rows.to_vec())))),
    );
    assert_eq!(error, None);
    shell.take_operations();
    (as_asked, resolved)
}

/// `STORE-19`, a change to substitutions has the service change the rows by their keys and their columns' references,
/// updating only a row's values that changed, its tool, a pattern, a value and its comment, adding a row's values, and
/// removing or moving a row, checking its values, as part mappings' are.
#[test]
fn a_change_to_substitutions_has_the_service_change_their_rows() {
    let r1 = substitution(&[("name", "R1", "RES")], "c1");
    let r2 = substitution(&[("name", "R2", "RES")], "");
    let mut changed = r2.clone();
    changed.eda_tool = EdaTool::EasyEda;
    changed.criteria[0].pattern = Pattern::parse("/R.*/");
    changed.transforms[0].field_value = "RES_0402".to_string();
    changed.comment = Some("derate 16V".to_string());
    let expected = |name_pattern: &str, name: &str, comment: &str| {
        vec![
            value("Eda", "DipTrace"),
            value("NamePattern", name_pattern),
            value("Name", name),
            value("Comment", comment),
        ]
    };
    let field = |column: &str, old: &str, new: &str| FieldChange {
        column: ColumnRef(column.to_string()),
        old: old.to_string(),
        new: new.to_string(),
    };
    let cases = [
        (
            TableChange::Update {
                key: key("1"),
                old: TableRow::Substitutions(r2.clone()),
                new: TableRow::Substitutions(changed),
            },
            RowsChange::Update {
                key: key("1"),
                changes: vec![
                    field("Eda", "DipTrace", "EasyEda"),
                    field("NamePattern", "R2", "/R.*/"),
                    field("Name", "RES", "RES_0402"),
                    field("Comment", "", "derate 16V"),
                ],
            },
        ),
        (
            TableChange::Add {
                row: TableRow::Substitutions(substitution(&[("name", "L1", "IND")], "")),
            },
            RowsChange::Add {
                values: expected("L1", "IND", ""),
            },
        ),
        (
            TableChange::Remove {
                key: key("0"),
                row: TableRow::Substitutions(r1.clone()),
            },
            RowsChange::Remove {
                row: RowRef {
                    key: key("0"),
                    expected: expected("R1", "RES", "c1"),
                },
            },
        ),
        (
            TableChange::Move {
                key: key("1"),
                row: TableRow::Substitutions(r2.clone()),
                before: Some((key("0"), TableRow::Substitutions(r1.clone()))),
            },
            RowsChange::Move {
                row: RowRef {
                    key: key("1"),
                    expected: expected("R2", "RES", ""),
                },
                before: Some(RowRef {
                    key: key("0"),
                    expected: expected("R1", "RES", "c1"),
                }),
            },
        ),
    ];

    for (change, expected) in cases {
        let mut shell = Shell::new();
        let (as_asked, resolved) = given_open_substitutions(&mut shell, &[r1.clone(), r2.clone()]);

        let (views, error) = shell.process(
            Event::ChangeTable {
                kind: SUBSTITUTIONS,
                source: as_asked.clone(),
                change: change.clone(),
            },
            changes_then_reads(&resolved, Table::Substitutions(vec![r1.clone()])),
        );

        assert_eq!(error, None, "{:?}", change);
        assert_eq!(
            shell.take_operations()[0],
            StoresOperation::ChangeRows {
                kind: SUBSTITUTIONS,
                source: resolved.clone(),
                change: expected,
            },
            "{:?}",
            change
        );
        assert_eq!(views.len(), 1, "{:?}", change);
    }
}

/// `STORE-21`, an EDA tool's columns for substitutions are its fields' patterns' and values' columns the table doesn't
/// have, whichever tool's they are, the patterns' first, each in the tool's order, where the data source puts them.
#[test]
fn an_eda_tools_substitutions_columns_are_its_fields_patterns_and_values() {
    let cases: [(&[(&str, &str, &str)], EdaTool, &[&str]); 3] = [
        (&[("name", "", "")], EdaTool::KiCad, &[
            "PackagePattern",
            "ValPattern",
            "Package",
            "Val",
        ]),
        // `value` is DipTrace's too.
        (&[("name", "", ""), ("value", "", "")], EdaTool::EasyEda, &[
            "DevicePattern",
            "Device",
        ]),
        (&[("name", "", "")], EdaTool::DipTrace, &["ValuePattern", "Value"]),
    ];

    for (fields, eda_tool, expected) in cases {
        let mut shell = Shell::new();
        let (as_asked, resolved) = given_open_substitutions(&mut shell, &[substitution(fields, "")]);

        let (_views, error) = shell.process(
            Event::AddTableColumns {
                kind: SUBSTITUTIONS,
                source: as_asked.clone(),
                columns: ColumnsToAdd::EdaTool {
                    eda_tool,
                },
            },
            adds_columns_then_reads(&resolved, Table::Substitutions(vec![substitution(fields, "")])),
        );

        assert_eq!(error, None, "{:?}", eda_tool);
        assert_eq!(
            shell.take_operations()[0],
            StoresOperation::AddColumns {
                kind: SUBSTITUTIONS,
                source: resolved.clone(),
                columns: strings(expected),
                position: ColumnPosition::EdaToolColumns,
            },
            "{:?}",
            eda_tool
        );
    }

    // a value's column without its pattern's, or a pattern's without its value's, has only the other added.
    let mut item = substitution(&[("name", "", "")], "");
    item.criteria.clear();
    let mut shell = Shell::new();
    let (as_asked, resolved) = given_open_substitutions(&mut shell, &[item.clone()]);
    shell.process(
        Event::AddTableColumns {
            kind: SUBSTITUTIONS,
            source: as_asked,
            columns: ColumnsToAdd::EdaTool {
                eda_tool: EdaTool::DipTrace,
            },
        },
        adds_columns_then_reads(&resolved, Table::Substitutions(vec![item])),
    );
    assert_eq!(shell.take_operations()[0], StoresOperation::AddColumns {
        kind: SUBSTITUTIONS,
        source: resolved,
        columns: strings(&["NamePattern", "ValuePattern", "Value"]),
        position: ColumnPosition::EdaToolColumns,
    });
}

const ASSEMBLY_RULES: TableKind = TableKind::AssemblyRules;

fn assembly_rule(ref_des: &str, manufacturer: &str, mpn: &str) -> AssemblyRulesViewItem {
    AssemblyRulesViewItem {
        rule: AssemblyRule {
            ref_des: ref_des.to_string(),
            manufacturer: manufacturer.to_string(),
            mpn: mpn.to_string(),
        },
    }
}

/// Opens assembly rules, `rules.csv`, identified as `/folder/rules.csv`, with the rows, and takes the operations.
fn given_open_assembly_rules(shell: &mut Shell, rows: &[AssemblyRulesViewItem]) -> (Source, Source) {
    let as_asked = source("rules.csv");
    let resolved = source("/folder/rules.csv");
    let (_views, error) = shell.process(
        Event::OpenTable {
            kind: ASSEMBLY_RULES,
            source: as_asked.clone(),
        },
        service(&as_asked, &resolved, Ok(Some(Table::AssemblyRules(rows.to_vec())))),
    );
    assert_eq!(error, None);
    shell.take_operations();
    (as_asked, resolved)
}

/// `STORE-19`, a change to assembly rules has the service change the rows by their keys and their columns' references,
/// updating only a row's values that changed, its designator, manufacturer and MPN, adding a row's values, and removing
/// or moving a row, checking its values, as part mappings' are.
#[test]
fn a_change_to_assembly_rules_has_the_service_change_their_rows() {
    let r1 = assembly_rule("R1", "MFR1", "MPN1");
    let c1 = assembly_rule("C1", "MFR2", "MPN2");
    let row = |item: &AssemblyRulesViewItem| TableRow::AssemblyRules(item.clone());
    let expected = |ref_des: &str, manufacturer: &str, mpn: &str| {
        vec![
            value("RefDes", ref_des),
            value("Manufacturer", manufacturer),
            value("Mpn", mpn),
        ]
    };
    let field = |column: &str, old: &str, new: &str| FieldChange {
        column: ColumnRef(column.to_string()),
        old: old.to_string(),
        new: new.to_string(),
    };
    let cases = [
        (
            TableChange::Update {
                key: key("1"),
                old: row(&c1),
                new: row(&assembly_rule("C2", "MFR3", "MPN3")),
            },
            RowsChange::Update {
                key: key("1"),
                changes: vec![
                    field("RefDes", "C1", "C2"),
                    field("Manufacturer", "MFR2", "MFR3"),
                    field("Mpn", "MPN2", "MPN3"),
                ],
            },
        ),
        (
            TableChange::Add {
                row: row(&assembly_rule("D1", "MFR4", "MPN4")),
            },
            RowsChange::Add {
                values: expected("D1", "MFR4", "MPN4"),
            },
        ),
        (
            TableChange::Remove {
                key: key("0"),
                row: row(&r1),
            },
            RowsChange::Remove {
                row: RowRef {
                    key: key("0"),
                    expected: expected("R1", "MFR1", "MPN1"),
                },
            },
        ),
        (
            TableChange::Move {
                key: key("1"),
                row: row(&c1),
                before: Some((key("0"), row(&r1))),
            },
            RowsChange::Move {
                row: RowRef {
                    key: key("1"),
                    expected: expected("C1", "MFR2", "MPN2"),
                },
                before: Some(RowRef {
                    key: key("0"),
                    expected: expected("R1", "MFR1", "MPN1"),
                }),
            },
        ),
    ];

    for (change, expected) in cases {
        let mut shell = Shell::new();
        let (as_asked, resolved) = given_open_assembly_rules(&mut shell, &[r1.clone(), c1.clone()]);

        let (views, error) = shell.process(
            Event::ChangeTable {
                kind: ASSEMBLY_RULES,
                source: as_asked.clone(),
                change: change.clone(),
            },
            changes_then_reads(&resolved, Table::AssemblyRules(vec![r1.clone()])),
        );

        assert_eq!(error, None, "{:?}", change);
        assert_eq!(
            shell.take_operations()[0],
            StoresOperation::ChangeRows {
                kind: ASSEMBLY_RULES,
                source: resolved.clone(),
                change: expected,
            },
            "{:?}",
            change
        );
        assert_eq!(views.len(), 1, "{:?}", change);
    }
}

/// `STORE-21`, an EDA tool's columns asked for assembly rules, which have no EDA tool, are refused, nothing asked of the
/// service, though named columns are added.
#[test]
fn an_eda_tools_columns_for_assembly_rules_are_refused() {
    let mut shell = Shell::new();
    let (as_asked, resolved) = given_open_assembly_rules(&mut shell, &[assembly_rule("R1", "MFR1", "MPN1")]);

    let (views, error) = shell.process(
        Event::AddTableColumns {
            kind: ASSEMBLY_RULES,
            source: as_asked.clone(),
            columns: ColumnsToAdd::EdaTool {
                eda_tool: EdaTool::DipTrace,
            },
        },
        no_operations,
    );

    assert_eq!(views, vec![]);
    assert_eq!(
        error,
        Some("The table has no EDA tool's columns. kind: AssemblyRules, source: '/folder/rules.csv'".to_string())
    );
    assert_eq!(shell.take_operations(), vec![]);

    let (_views, error) = shell.process(
        Event::AddTableColumns {
            kind: ASSEMBLY_RULES,
            source: as_asked,
            columns: ColumnsToAdd::Named {
                columns: strings(&["Notes"]),
                position: ColumnPosition::Last,
            },
        },
        adds_columns_then_reads(
            &resolved,
            Table::AssemblyRules(vec![assembly_rule("R1", "MFR1", "MPN1")]),
        ),
    );
    assert_eq!(error, None);
    assert_eq!(shell.take_operations()[0], StoresOperation::AddColumns {
        kind: ASSEMBLY_RULES,
        source: resolved,
        columns: strings(&["Notes"]),
        position: ColumnPosition::Last,
    });
}
