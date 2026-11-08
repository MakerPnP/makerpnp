//! The store operations a core asks its shell for, as Crux effects, and their results (`STORE-7`).
//!
//! A core never reads or writes a data source itself. It asks for a [`StoresOperation`], the shell has the stores
//! service do it, and the core is given the [`StoresOutput`]. So the cores are pure, and are tested with outputs made
//! up by the tests, and every core in a process shares the one stores service.
//!
//! A data source, such as a parts file, is read and written by the service, which knows its format, so that it can have
//! back ends of other kinds, such as a database. A document, such as a project, is read and written as it is, by the
//! core that keeps it (`STORE-10`).

// A core does no IO, see `clippy.toml`.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;

pub use assembly::rules::AssemblyRule;
pub use criteria::{FieldPattern, Pattern};
use crux_core::capability::Operation;
pub use eda::placement::EdaPlacement;
pub use eda::substitution::EdaSubstitutionRuleTransformItem;
pub use eda::EdaTool;
use heck::ToUpperCamelCase;
pub use pnp::load_out::LoadOutItem;
pub use pnp::package::Package;
pub use pnp::part::Part;
pub use pnp::placement::{Placement, RefDes};
pub use util::source::Source;

pub type PartsSource = Source;

/// A part, and the data source's other values for it, for example a supplier's part code.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PartsViewItem {
    pub part: Part,
    /// The data source's other values for the part, by name, for example `"LCSC"`.
    pub fields: BTreeMap<String, String>,
}

/// Which version of a data source's contents something was read from, or based on (`STORE-8`).
///
/// Made by the back end, and only compared: two reads of a data source have the same revision only if its contents
/// were the same, in any process. For a file, it's a hash of the file's contents.
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord
)]
pub struct Revision(pub String);

impl Display for Revision {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The parts of a parts data source, and the revision they were read from.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Parts {
    pub revision: Revision,
    /// In the order the data source has them.
    pub parts: Vec<PartsViewItem>,
}

/// A document, such as a project, a PCB or a process, as its core keeps it, and the revision it was read from
/// (`STORE-10`).
///
/// The core that keeps the document reads and writes its contents, so the service doesn't need to know what's in it.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct Document {
    pub revision: Revision,
    pub contents: String,
}

/// Says how long the contents are, not what they are, as a document is logged, and may be large.
impl Debug for Document {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Document(<{} bytes>, revision: {:?})",
            self.contents.len(),
            self.revision.0
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum StoresOperation {
    /// The data source as the stores service identifies it, so that one file is one data source however its path is
    /// written (`STORE-6`). Answered with [`StoresOutput::SourceResolved`].
    ResolveSource { source: Source },

    /// The parts of a parts data source, in the order it has them. Answered with [`StoresOutput::PartsRead`].
    ReadParts { source: PartsSource },

    /// Watches the data source, as the stores service identified it, for changes, by anyone, for example another
    /// process (`STORE-9`). A stream: answered with [`StoresOutput::Changed`] each time it may have changed, for as long
    /// as the shell can watch it. A shell that can't, such as the CLI, which is done before anything could change,
    /// doesn't answer it.
    WatchSource { source: Source },

    /// The document at the path, if there is one (`STORE-10`). Answered with [`StoresOutput::DocumentRead`].
    ReadDocument { path: PathBuf },

    /// Writes the document's contents to the path, replacing any document there (`STORE-10`). Answered with
    /// [`StoresOutput::DocumentWritten`].
    WriteDocument { path: PathBuf, contents: String },

    /// The records of a data source of the kind, such as a load-out, if there is one (`STORE-11`). Answered with
    /// [`StoresOutput::RecordsRead`].
    ReadRecords { kind: RecordsKind, source: Source },

    /// Writes the records, replacing the data source's (`STORE-11`). Answered with [`StoresOutput::RecordsWritten`].
    WriteRecords { source: Source, records: RecordsOf },

    /// Writes the parts, replacing the data source's, only if it's still at the revision the change was based on, so
    /// that a change made since, by anyone, isn't lost (`STORE-15`). Answered with [`StoresOutput::PartsWritten`].
    WriteParts {
        source: PartsSource,
        revision: Revision,
        parts: Vec<PartsViewItem>,
    },

    /// The records of a data source of the kind, as a table, if there is one (`STORE-17`). Answered with
    /// [`StoresOutput::TableRead`].
    ReadTable { kind: TableKind, source: Source },

    /// Changes the rows of a data source of the kind, by their keys and their columns' references, only if their values
    /// are still as they were (`STORE-19`). Answered with [`StoresOutput::RowsChanged`].
    ChangeRows {
        kind: TableKind,
        source: Source,
        change: RowsChange,
    },
}

/// A row, as the back end that read it names it, such as a database's primary key, or a CSV file's row's place when it
/// was read (`STORE-19`). Only the back end knows what it means.
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord
)]
pub struct RowKey(pub String);

impl Display for RowKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A column, as the data source names it, such as a CSV file's header, or a query result's field (`STORE-19`).
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord
)]
pub struct ColumnRef(pub String);

impl Display for ColumnRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// How a table's rows are ordered, so whether they can be moved (`STORE-19`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum RowOrdering {
    /// They're not, so they can't be moved, as a database table without an ordering column's.
    Unsupported,
    /// By their place in the data source, as a CSV file's lines are.
    Position,
    /// By the values of a column, which a database or a web API would have.
    Field(ColumnRef),
}

/// A column's value, as the data source has it, such as a pattern's text.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct FieldValue {
    pub column: ColumnRef,
    pub value: String,
}

/// A column's value as it was, which it must still be, and as it's to be.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct FieldChange {
    pub column: ColumnRef,
    pub old: String,
    pub new: String,
}

/// A row, by its key, and the values it must still have, so it's the row the user saw.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct RowRef {
    pub key: RowKey,
    pub expected: Vec<FieldValue>,
}

/// A change to a data source's rows, as the back end makes it, by their keys and their columns' references, refused if
/// a value isn't as it was (`STORE-19`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum RowsChange {
    /// Changes the row's columns that are named, only those.
    Update {
        key: RowKey,
        changes: Vec<FieldChange>,
    },
    /// Adds a row, after the others, with the values named, and the data source's default, such as an empty value, of
    /// each other column.
    Add {
        values: Vec<FieldValue>,
    },
    Remove {
        row: RowRef,
    },
    /// Moves the row to before another, or, if there's none, to the end.
    Move {
        row: RowRef,
        before: Option<RowRef>,
    },
}

/// The kinds of data source shown as a table, read-only, each with a format the service knows (`STORE-17`).
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord
)]
pub enum TableKind {
    /// A design's placements, from an EDA tool's file, whose format depends on the tool.
    EdaPlacements {
        eda_tool: EdaTool,
    },
    PartMappings,
    Substitutions,
    AssemblyRules,
    /// A variant's placements, as built.
    Placements,
    /// A variant's BOM, as built.
    Bom,
}

/// A data source's records, of its kind, each a view item, in the order the data source has them (`STORE-17`).
///
/// Each view item has the domain's types, as [`PartsViewItem`] does, such as a [`Part`] or a [`Pattern`], so a front end
/// shows each value as what it is, a pattern's regular expression parsed once, as it's read.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum Table {
    EdaPlacements(Vec<EdaPlacementsViewItem>),
    PartMappings(Vec<PartMappingsViewItem>),
    Substitutions(Vec<SubstitutionsViewItem>),
    AssemblyRules(Vec<AssemblyRulesViewItem>),
    Placements(Vec<PlacementsViewItem>),
    Bom(Vec<BomViewItem>),
}

/// A row of a table whose kind can be changed, as a front end shows it, and changes it (`STORE-19`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum TableRow {
    PartMappings(PartMappingsViewItem),
}

impl TableRow {
    /// The row's values, by the column that shows each, as the data source has them, such as a pattern's text. Only
    /// those it has, so a part mapping has no value of another EDA tool's field.
    pub fn values(&self) -> Vec<(ColumnValue, String)> {
        match self {
            TableRow::PartMappings(item) => {
                let mut values = vec![(ColumnValue::Eda, EdaToolName(item.eda_tool).to_string())];
                values.extend(item.criteria.iter().map(|criterion| {
                    (
                        ColumnValue::Pattern(criterion.field_name.clone()),
                        criterion.pattern.text().to_string(),
                    )
                }));
                values.push((ColumnValue::Manufacturer, item.part.manufacturer.clone()));
                values.push((ColumnValue::Mpn, item.part.mpn.clone()));
                values
            }
        }
    }
}

/// A design's placement, as its EDA tool's file has it.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct EdaPlacementsViewItem {
    pub placement: EdaPlacement,
}

/// A part mapping: the patterns an EDA tool's placement's fields are matched with, and the part it's mapped to.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PartMappingsViewItem {
    pub eda_tool: EdaTool,
    /// Of every EDA tool's fields the data source has, not only this tool's, in the EDA tools' order (see
    /// [`EdaTool::all_field_names`]), such as `name`, `value`, then `package`, so every value is shown whichever tool
    /// the row has. Matching uses only the tool's own fields (see [`EdaTool::field_names`]).
    pub criteria: Vec<FieldPattern>,
    pub part: Part,
}

/// A substitution: the patterns an EDA tool's placement's fields are matched with, the values they're substituted
/// with, and its comment.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct SubstitutionsViewItem {
    pub eda_tool: EdaTool,
    /// Of the EDA tool's fields, in its order, such as `name` then `value`.
    pub criteria: Vec<FieldPattern>,
    /// A value for each of the criteria's fields.
    pub transforms: Vec<EdaSubstitutionRuleTransformItem>,
    pub comment: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct AssemblyRulesViewItem {
    pub rule: AssemblyRule,
}

/// A variant's placement, as built.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PlacementsViewItem {
    pub placement: Placement,
}

/// A row of a variant's BOM, as built, in JLCPCB's format: the placements of a part, and how many there are.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct BomViewItem {
    pub designators: Vec<RefDes>,
    pub footprint: String,
    /// JLCPCB's part code, such as `C1234`, if the part has one.
    pub jlcpcb_part: Option<String>,
    pub quantity: usize,
    /// Says which part it is, such as `MPN1 (MFR1)`.
    pub comment: String,
}

/// A column of a table: its name, as the data source names it, which of a view item's values it shows, and, if its
/// values can be changed, its reference, as the data source names it, by which a change names it (`STORE-19`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct TableColumn {
    pub name: String,
    pub value: ColumnValue,
    pub reference: Option<ColumnRef>,
}

impl TableColumn {
    fn new(name: impl Into<String>, value: ColumnValue) -> Self {
        Self {
            name: name.into(),
            value,
            reference: None,
        }
    }
}

/// Which of a view item's values a column shows.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColumnValue {
    RefDes,
    Place,
    Side,
    X,
    Y,
    Rotation,
    /// An EDA placement's field's value, by the field's name.
    EdaField(String),
    Eda,
    /// The pattern a field's value is matched with, by the field's name.
    Pattern(String),
    /// The value a field is substituted with, by the field's name.
    Substitute(String),
    Manufacturer,
    Mpn,
    Designator,
    Footprint,
    JlcpcbPart,
    Quantity,
    Comment,
}

/// A table's value, as a column shows it.
#[derive(Debug, PartialEq)]
pub enum Cell<'a> {
    Text(Cow<'a, str>),
    /// Shown as what it is, an exact value, or a regular expression, valid or not.
    Pattern(&'a Pattern),
}

impl Display for Cell<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Cell::Text(text) => f.write_str(text),
            Cell::Pattern(pattern) => Display::fmt(pattern, f),
        }
    }
}

impl Table {
    pub fn len(&self) -> usize {
        match self {
            Table::EdaPlacements(items) => items.len(),
            Table::PartMappings(items) => items.len(),
            Table::Substitutions(items) => items.len(),
            Table::AssemblyRules(items) => items.len(),
            Table::Placements(items) => items.len(),
            Table::Bom(items) => items.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the table's kind can be changed (`STORE-19`), part mappings for now.
    pub fn is_changeable(&self) -> bool {
        matches!(self, Table::PartMappings(_))
    }

    /// The row, if the table's kind can be changed, and it's there, for a front end to change and set (`STORE-19`).
    pub fn row(&self, index: usize) -> Option<TableRow> {
        match self {
            Table::PartMappings(items) => items
                .get(index)
                .cloned()
                .map(TableRow::PartMappings),
            _ => None,
        }
    }

    /// The table's columns, in order, named as the data source names them, a comment's last, as there's one at most,
    /// and it's the longest. The EDA tool's fields, and the fields matched, are in the order they're first found.
    pub fn columns(&self) -> Vec<TableColumn> {
        use ColumnValue::*;
        match self {
            Table::EdaPlacements(items) => {
                let mut columns = vec![
                    TableColumn::new("RefDes", RefDes),
                    TableColumn::new("Place", Place),
                    TableColumn::new("Side", Side),
                    TableColumn::new("X", X),
                    TableColumn::new("Y", Y),
                    TableColumn::new("Rotation", Rotation),
                ];
                let names = first_found(
                    items
                        .iter()
                        .flat_map(|item| item.placement.fields.iter())
                        .map(|field| &field.name),
                );
                columns.extend(
                    names
                        .into_iter()
                        .map(|name| TableColumn::new(name.clone(), EdaField(name))),
                );
                columns
            }
            Table::PartMappings(items) => {
                let mut columns = vec![TableColumn::new("Eda", Eda)];
                let names = first_found(
                    items
                        .iter()
                        .flat_map(|item| item.criteria.iter())
                        .map(|criterion| &criterion.field_name),
                );
                columns.extend(
                    names
                        .into_iter()
                        .map(|name| TableColumn::new(name.to_upper_camel_case(), Pattern(name))),
                );
                columns.push(TableColumn::new("Manufacturer", Manufacturer));
                columns.push(TableColumn::new("Mpn", Mpn));
                columns
            }
            Table::Substitutions(items) => {
                let mut columns = vec![TableColumn::new("Eda", Eda)];
                let names = first_found(
                    items
                        .iter()
                        .flat_map(|item| item.criteria.iter())
                        .map(|criterion| &criterion.field_name),
                );
                for name in names {
                    columns.push(TableColumn::new(
                        format!("{}_pattern", name).to_upper_camel_case(),
                        Pattern(name.clone()),
                    ));
                    columns.push(TableColumn::new(name.to_upper_camel_case(), Substitute(name)));
                }
                columns.push(TableColumn::new("Comment", Comment));
                columns
            }
            Table::AssemblyRules(_) => vec![
                TableColumn::new("RefDes", RefDes),
                TableColumn::new("Manufacturer", Manufacturer),
                TableColumn::new("Mpn", Mpn),
            ],
            Table::Placements(_) => vec![
                TableColumn::new("RefDes", RefDes),
                TableColumn::new("Manufacturer", Manufacturer),
                TableColumn::new("Mpn", Mpn),
                TableColumn::new("Place", Place),
                TableColumn::new("PcbSide", Side),
                TableColumn::new("X", X),
                TableColumn::new("Y", Y),
                TableColumn::new("Rotation", Rotation),
            ],
            Table::Bom(_) => vec![
                TableColumn::new("Designator", Designator),
                TableColumn::new("Footprint", Footprint),
                TableColumn::new("JLCPCB Part", JlcpcbPart),
                TableColumn::new("Quantity", Quantity),
                TableColumn::new("Comment", Comment),
            ],
        }
    }

    /// The row's value the column shows, empty if the row has none, such as a field a placement doesn't have.
    pub fn cell(&self, row: usize, value: &ColumnValue) -> Cell<'_> {
        use ColumnValue::*;
        let text = |text: &str| Cell::Text(Cow::Owned(text.to_string()));
        let display = |value: &dyn Display| Cell::Text(Cow::Owned(value.to_string()));
        let borrowed = |text| Cell::Text(Cow::Borrowed(text));
        let empty = || Cell::Text(Cow::Borrowed(""));
        fn pattern<'a>(criteria: &'a [FieldPattern], name: &str) -> Option<&'a criteria::Pattern> {
            criteria
                .iter()
                .find(|criterion| criterion.field_name == *name)
                .map(|criterion| &criterion.pattern)
        }

        match self {
            Table::EdaPlacements(items) => {
                let placement = &items[row].placement;
                match value {
                    RefDes => borrowed(placement.ref_des.as_str()),
                    Place => display(&placement.place),
                    Side => display(&placement.pcb_side),
                    X => display(&placement.x),
                    Y => display(&placement.y),
                    Rotation => display(&placement.rotation),
                    EdaField(name) => placement
                        .fields
                        .iter()
                        .find(|field| field.name == *name)
                        .map_or_else(empty, |field| borrowed(field.value.as_str())),
                    _ => empty(),
                }
            }
            Table::PartMappings(items) => {
                let item = &items[row];
                match value {
                    Eda => display(&EdaToolName(item.eda_tool)),
                    Pattern(name) => pattern(&item.criteria, name).map_or_else(empty, Cell::Pattern),
                    Manufacturer => borrowed(item.part.manufacturer.as_str()),
                    Mpn => borrowed(item.part.mpn.as_str()),
                    _ => empty(),
                }
            }
            Table::Substitutions(items) => {
                let item = &items[row];
                match value {
                    Eda => display(&EdaToolName(item.eda_tool)),
                    Pattern(name) => pattern(&item.criteria, name).map_or_else(empty, Cell::Pattern),
                    Substitute(name) => item
                        .transforms
                        .iter()
                        .find(|transform| transform.field_name == *name)
                        .map_or_else(empty, |transform| borrowed(transform.field_value.as_str())),
                    Comment => item
                        .comment
                        .as_deref()
                        .map_or_else(empty, borrowed),
                    _ => empty(),
                }
            }
            Table::AssemblyRules(items) => {
                let rule = &items[row].rule;
                match value {
                    RefDes => borrowed(rule.ref_des.as_str()),
                    Manufacturer => borrowed(rule.manufacturer.as_str()),
                    Mpn => borrowed(rule.mpn.as_str()),
                    _ => empty(),
                }
            }
            Table::Placements(items) => {
                let placement = &items[row].placement;
                match value {
                    RefDes => display(&placement.ref_des),
                    Manufacturer => borrowed(placement.part.manufacturer.as_str()),
                    Mpn => borrowed(placement.part.mpn.as_str()),
                    Place => display(&placement.place),
                    Side => display(&placement.pcb_side),
                    X => display(&placement.x),
                    Y => display(&placement.y),
                    Rotation => display(&placement.rotation),
                    _ => empty(),
                }
            }
            Table::Bom(items) => {
                let item = &items[row];
                match value {
                    Designator => text(
                        &item
                            .designators
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    Footprint => borrowed(item.footprint.as_str()),
                    JlcpcbPart => item
                        .jlcpcb_part
                        .as_deref()
                        .map_or_else(empty, borrowed),
                    Quantity => display(&item.quantity),
                    Comment => borrowed(item.comment.as_str()),
                    _ => empty(),
                }
            }
        }
    }
}

/// The names, each once, in the order they're first found.
fn first_found<'a>(names: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut found: Vec<String> = vec![];
    for name in names {
        if !found.contains(name) {
            found.push(name.clone());
        }
    }
    found
}

/// An EDA tool as a CSV's `Eda` value names it, such as `DipTrace`.
struct EdaToolName(EdaTool);

impl Display for EdaToolName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

/// A data source's table, and the revision it was read from (`STORE-8`), with what a change needs to name its rows and
/// columns as the data source does (`STORE-19`).
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct TableRecords {
    pub revision: Revision,
    pub table: Table,
    /// Its columns, in order, each with its reference, if its values can be changed.
    pub columns: Vec<TableColumn>,
    /// Each row's key, in the order of the rows.
    pub keys: Vec<RowKey>,
    pub ordering: RowOrdering,
}

impl TableRecords {
    /// A table that can't be changed: its columns have no references, its rows' keys are their places, and they can't
    /// be moved.
    pub fn read_only(revision: Revision, table: Table) -> Self {
        Self {
            revision,
            columns: table.columns(),
            keys: (0..table.len())
                .map(|index| RowKey(index.to_string()))
                .collect(),
            ordering: RowOrdering::Unsupported,
            table,
        }
    }
}

/// Says how many rows there are, not what they are, as they're logged.
impl Debug for TableRecords {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TableRecords(<{} rows>, revision: {:?})",
            self.table.len(),
            self.revision.0
        )
    }
}

/// Why parts, or a table, weren't written (`STORE-15`, `STORE-19`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum PartsWriteError {
    /// The data source has changed since the revision the change was based on, to this revision, or `None` if it no
    /// longer exists, so the change would lose another's.
    Changed { revision: Option<Revision> },
    /// The parts couldn't be written, for example as the file is read-only.
    Failed { cause: String },
}

/// The kinds of data source whose records a core reads, each with a format the service knows (`STORE-11`).
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord
)]
pub enum RecordsKind {
    LoadOut,
    Packages,
    /// Each record's fields by name, as each mapping is made with the packages, by the core.
    PackageMappings,
    Placements,
    /// The placements from an EDA tool's file, whose format depends on the tool.
    EdaPlacements {
        eda_tool: EdaTool,
    },
    /// Each record's fields by name, as each mapping is made with the parts, by the core.
    PartMappings,
    /// Each record's fields by name, as each rule is made by the core.
    Substitutions,
    /// Each record's fields by name, as each rule is made by the core.
    AssemblyRules,
}

/// A data source's records, of its kind.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum RecordsOf {
    LoadOut(Vec<LoadOutItem>),
    Packages(Vec<Package>),
    PackageMappings(Vec<BTreeMap<String, String>>),
    Placements(Vec<Placement>),
    EdaPlacements {
        eda_tool: EdaTool,
        placements: Vec<EdaPlacement>,
    },
    PartMappings(Vec<BTreeMap<String, String>>),
    Substitutions(Vec<BTreeMap<String, String>>),
    AssemblyRules(Vec<BTreeMap<String, String>>),
}

impl RecordsOf {
    pub fn kind(&self) -> RecordsKind {
        match self {
            RecordsOf::LoadOut(_) => RecordsKind::LoadOut,
            RecordsOf::Packages(_) => RecordsKind::Packages,
            RecordsOf::PackageMappings(_) => RecordsKind::PackageMappings,
            RecordsOf::Placements(_) => RecordsKind::Placements,
            RecordsOf::EdaPlacements {
                eda_tool, ..
            } => RecordsKind::EdaPlacements {
                eda_tool: *eda_tool,
            },
            RecordsOf::PartMappings(_) => RecordsKind::PartMappings,
            RecordsOf::Substitutions(_) => RecordsKind::Substitutions,
            RecordsOf::AssemblyRules(_) => RecordsKind::AssemblyRules,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            RecordsOf::LoadOut(records) => records.len(),
            RecordsOf::Packages(records) => records.len(),
            RecordsOf::PackageMappings(records) => records.len(),
            RecordsOf::Placements(records) => records.len(),
            RecordsOf::EdaPlacements {
                placements, ..
            } => placements.len(),
            RecordsOf::PartMappings(records) => records.len(),
            RecordsOf::Substitutions(records) => records.len(),
            RecordsOf::AssemblyRules(records) => records.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Says how many records there are, of what kind, not what they are, as they're logged.
struct RecordsSummary<'a>(&'a RecordsOf);

impl Debug for RecordsSummary<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{} {:?} records>", self.0.len(), self.0.kind())
    }
}

/// A data source's records, and the revision they were read from (`STORE-8`).
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct Records {
    pub revision: Revision,
    pub records: RecordsOf,
}

impl Debug for Records {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Records({:?}, revision: {:?})",
            RecordsSummary(&self.records),
            self.revision.0
        )
    }
}

/// Says how long a document written is, not what it is, as an operation is logged.
impl Debug for StoresOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StoresOperation::ResolveSource {
                source,
            } => f
                .debug_struct("ResolveSource")
                .field("source", source)
                .finish(),
            StoresOperation::ReadParts {
                source,
            } => f
                .debug_struct("ReadParts")
                .field("source", source)
                .finish(),
            StoresOperation::WatchSource {
                source,
            } => f
                .debug_struct("WatchSource")
                .field("source", source)
                .finish(),
            StoresOperation::ReadDocument {
                path,
            } => f
                .debug_struct("ReadDocument")
                .field("path", path)
                .finish(),
            StoresOperation::WriteDocument {
                path,
                contents,
            } => write!(
                f,
                "WriteDocument {{ path: {:?}, contents: <{} bytes> }}",
                path,
                contents.len()
            ),
            StoresOperation::ReadRecords {
                kind,
                source,
            } => f
                .debug_struct("ReadRecords")
                .field("kind", kind)
                .field("source", source)
                .finish(),
            StoresOperation::WriteRecords {
                source,
                records,
            } => f
                .debug_struct("WriteRecords")
                .field("source", source)
                .field("records", &RecordsSummary(records))
                .finish(),
            StoresOperation::WriteParts {
                source,
                revision,
                parts,
            } => write!(
                f,
                "WriteParts {{ source: {:?}, revision: {:?}, parts: <{} parts> }}",
                source,
                revision.0,
                parts.len()
            ),
            StoresOperation::ReadTable {
                kind,
                source,
            } => f
                .debug_struct("ReadTable")
                .field("kind", kind)
                .field("source", source)
                .finish(),
            StoresOperation::ChangeRows {
                kind,
                source,
                change,
            } => f
                .debug_struct("ChangeRows")
                .field("kind", kind)
                .field("source", source)
                .field("change", change)
                .finish(),
        }
    }
}

impl StoresOperation {
    /// Whether the operation is answered more than once, see [`StoresOperation::WatchSource`].
    pub fn is_stream(&self) -> bool {
        matches!(self, StoresOperation::WatchSource { .. })
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum StoresOutput {
    /// For a file, its canonical path, or the source as it is if the file doesn't exist.
    SourceResolved { source: Source },

    /// The parts, and the revision they were read from, or why they couldn't be read, for example which header is
    /// missing.
    PartsRead(Result<Parts, String>),

    /// The data source watched may have changed. It may not have, for example when a file was written with the same
    /// contents, which its revision shows once it's read again (`STORE-8`).
    Changed,

    /// The document, or `None` if there's no document at the path, or why it couldn't be read.
    DocumentRead(Result<Option<Document>, String>),

    /// The revision of the document written, or why it couldn't be written.
    DocumentWritten(Result<Revision, String>),

    /// The records, or `None` if there's no data source, for example no file, or why they couldn't be read.
    RecordsRead(Result<Option<Records>, String>),

    /// The revision of the records written, or why they couldn't be written.
    RecordsWritten(Result<Revision, String>),

    /// The revision of the parts written, or why they weren't.
    PartsWritten(Result<Revision, PartsWriteError>),

    /// The table, or `None` if there's no data source, for example no file, or why it couldn't be read.
    TableRead(Result<Option<TableRecords>, String>),

    /// The revision of the data source once its rows were changed, or why they weren't (`STORE-19`).
    RowsChanged(Result<Revision, PartsWriteError>),
}

impl Operation for StoresOperation {
    type Output = StoresOutput;
}

/// Says how many parts were read, not what they are, as an output is logged, and may have thousands.
impl Debug for StoresOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StoresOutput::SourceResolved {
                source,
            } => f
                .debug_struct("SourceResolved")
                .field("source", source)
                .finish(),
            StoresOutput::PartsRead(Ok(Parts {
                revision,
                parts,
            })) => write!(f, "PartsRead(Ok(<{} parts>, revision: {:?}))", parts.len(), revision.0),
            StoresOutput::PartsRead(Err(error)) => write!(f, "PartsRead(Err({:?}))", error),
            StoresOutput::Changed => f.write_str("Changed"),
            StoresOutput::DocumentRead(result) => write!(f, "DocumentRead({:?})", result),
            StoresOutput::DocumentWritten(result) => write!(f, "DocumentWritten({:?})", result),
            StoresOutput::RecordsRead(result) => write!(f, "RecordsRead({:?})", result),
            StoresOutput::RecordsWritten(result) => write!(f, "RecordsWritten({:?})", result),
            StoresOutput::PartsWritten(result) => write!(f, "PartsWritten({:?})", result),
            StoresOutput::TableRead(result) => write!(f, "TableRead({:?})", result),
            StoresOutput::RowsChanged(result) => write!(f, "RowsChanged({:?})", result),
        }
    }
}
