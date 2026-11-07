//! The stores core: the data sources the front ends use, such as the user's parts.
//!
//! A data source is used through this core, not by reading its files, so that it can later be kept somewhere other
//! than a CSV file, for example in a database, and so that one core can serve every front end, and every window of a
//! front end, that uses the same data source.
//!
//! The core is pure: it asks its shell for each store operation, as a [`StoresOperation`] effect, which the shell has
//! the stores service do, and is given the result as an event (`STORE-7`).

// A core does no IO, see `clippy.toml`.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::future::Future;

use crux_core::capability::Operation;
use crux_core::command::NotificationBuilder;
use crux_core::macros::effect;
use crux_core::render::RenderOperation;
pub use crux_core::Core;
use crux_core::{render, App, Command, Request};
pub use stores_protocol::{
    AssemblyRule, AssemblyRulesViewItem, BomViewItem, Cell, ColumnRef, ColumnValue, EdaPlacement,
    EdaPlacementsViewItem, EdaSubstitutionRuleTransformItem, EdaTool, FieldChange, FieldPattern, FieldValue, Part,
    PartMappingsViewItem, Parts, PartsSource, PartsViewItem, PartsWriteError, Pattern, Placement, PlacementsViewItem,
    RefDes, Revision, RowKey, RowOrdering, RowRef, RowsChange, Source, StoresOperation, StoresOutput,
    SubstitutionsViewItem, Table, TableColumn, TableKind, TableRecords, TableRow,
};
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Default)]
pub struct Stores;

#[derive(Default)]
pub struct Model {
    /// Each data source as it was asked for, and the data source the stores service identified it as (`STORE-6`).
    aliases: BTreeMap<PartsSource, PartsSource>,

    /// Each data source, as the stores service identified it.
    sources: BTreeMap<PartsSource, PartsSourceState>,

    /// Each table, by its kind and its data source as it was asked for, and the data source the stores service
    /// identified it as, once it has (`STORE-17`).
    table_aliases: BTreeMap<(TableKind, Source), Source>,

    /// Each table, by its kind and its data source as the stores service identified it (`STORE-17`).
    tables: BTreeMap<(TableKind, Source), TableState>,

    error: Option<String>,
}

/// A data source shown as a table, read as parts are, see [`PartsSourceState`] (`STORE-17`).
#[derive(Default)]
struct TableState {
    /// The table loaded, with its revision, its rows' keys and its columns' references, kept when a later load is
    /// refused.
    table: Option<TableRecords>,
    reading: bool,
    changed_while_reading: bool,
    watching: bool,
    /// Why the last load was refused, until one succeeds.
    error: Option<String>,
    /// Whether a change to the table is being written (`STORE-19`).
    changing: bool,
    /// Why the last change was refused, until one succeeds.
    change_error: Option<String>,
}

#[derive(Default)]
struct PartsSourceState {
    /// The parts loaded, in the order the data source has them, kept when a later load is refused.
    parts: Option<Vec<PartsViewItem>>,
    /// The revision the parts were read from (`STORE-8`).
    revision: Option<Revision>,
    /// Whether the parts are being read.
    reading: bool,
    /// Whether the data source changed while its parts were being read, so they're out of date once read (`STORE-9`).
    changed_while_reading: bool,
    /// Whether the core asked for the data source to be watched (`STORE-9`).
    watching: bool,
    /// Whether a change to the parts is being written (`STORE-15`).
    changing: bool,
    /// Why the last change was refused, until one succeeds.
    change_error: Option<String>,
    /// Why the last load was refused, until one succeeds.
    error: Option<String>,
}

/// What the front ends need to know after each event.
#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug, Clone)]
pub struct StoresViewModel {
    /// Why the last event was refused, if it was.
    pub error: Option<String>,

    /// Each data source the core was asked for, by the source as it was asked for, and as the stores service
    /// identified it, once it has.
    pub parts_sources: BTreeMap<PartsSource, PartsSourceStatus>,

    /// Each table the core was asked for, by its kind and its data source as it was asked for, once the stores service
    /// has identified it (`STORE-17`).
    pub tables: Vec<TableStatus>,
}

impl StoresViewModel {
    /// The status of the table, by its kind and its data source as it was asked for, or as the stores service
    /// identified it.
    pub fn table(&self, kind: TableKind, source: &Source) -> Option<&TableStatus> {
        self.tables
            .iter()
            .find(|status| status.kind == kind && (status.asked == *source || status.source == *source))
    }
}

/// What the core knows of a table (`STORE-17`).
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct TableStatus {
    pub kind: TableKind,
    /// The data source as it was asked for.
    pub asked: Source,
    /// The data source as the stores service identified it, the one its views have (`STORE-6`).
    pub source: Source,
    pub loaded: bool,
    pub revision: Option<Revision>,
    pub reading: bool,
    /// Why the last load was refused, until one succeeds. The table loaded before, if any, is kept.
    pub error: Option<String>,
    /// Whether a change to the table is being written, until which another is refused (`STORE-19`).
    pub changing: bool,
    /// Why the last change was refused, until one succeeds, even once the table is read again (`STORE-19`).
    pub change_error: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PartsSourceStatus {
    /// The data source as the stores service identified it, the one its views have (`STORE-6`).
    pub source: PartsSource,
    pub loaded: bool,
    /// The revision of the parts loaded, if any (`STORE-8`).
    pub revision: Option<Revision>,
    pub reading: bool,
    /// Whether a change to the parts is being written, until which another is refused (`STORE-15`).
    pub changing: bool,
    /// Why the last load was refused, until one succeeds. The parts loaded before, if any, are kept.
    pub error: Option<String>,
    /// Why the last change was refused, until one succeeds, even once the parts are read again (`STORE-15`).
    pub change_error: Option<String>,
}

#[effect]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    PartsView(PartsViewRendererOperation),
    TableView(TableViewRendererOperation),
    Stores(StoresOperation),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum Event {
    /// Opens the data source: loads its parts, unless they're loaded already, then sends its parts view (`STORE-5`).
    OpenParts { source: PartsSource },

    /// Loads the parts from the data source again, replacing any loaded from it before, so that loading again shows the
    /// data source's current parts, then sends its parts view, so that every view of it can be updated.
    LoadParts { source: PartsSource },

    /// Requests the parts loaded from the data source.
    RequestPartsView { source: PartsSource },

    /// Changes the data source's parts, if they're still at `revision`, the revision of the view the change was based
    /// on, then sends its parts view (`STORE-15`).
    ChangeParts {
        source: PartsSource,
        revision: Revision,
        change: PartsChange,
    },

    /// Opens the data source's records of the kind as a table, read-only: loads it, unless it's loaded already, then
    /// sends its view (`STORE-17`).
    OpenTable { kind: TableKind, source: Source },

    /// Changes the table's rows, by their keys and their columns' references, if their values are still those of the
    /// rows the change was made to, then reads the table again, and sends its view (`STORE-19`).
    ChangeTable {
        kind: TableKind,
        source: Source,
        change: TableChange,
    },

    // internal, the results of the store operations the core asked for.
    /// The rows of the table's data source, as the stores service identified it, were changed, or why not.
    RowsChanged {
        kind: TableKind,
        source: Source,
        output: StoresOutput,
    },
    /// The table's data source, as the stores service identified it.
    TableSourceResolved {
        kind: TableKind,
        source: Source,
        output: StoresOutput,
    },
    /// The table of the data source, as the stores service identified it.
    TableRead {
        kind: TableKind,
        source: Source,
        output: StoresOutput,
    },
    /// The table's data source, as the stores service identified it, may have changed (`STORE-9`).
    TableSourceChanged {
        kind: TableKind,
        source: Source,
        output: StoresOutput,
    },

    /// The data source, as the stores service identified it.
    SourceResolved {
        source: PartsSource,
        intent: Intent,
        output: StoresOutput,
    },
    /// The parts of the data source, as the stores service identified it.
    PartsRead { source: PartsSource, output: StoresOutput },
    /// The data source, as the stores service identified it, may have changed (`STORE-9`).
    SourceChanged { source: PartsSource, output: StoresOutput },
    /// The changed parts of the data source, as the stores service identified it, were written, or why not.
    PartsWritten {
        source: PartsSource,
        /// The revision the change was based on.
        revision: Revision,
        parts: Vec<PartsViewItem>,
        output: StoresOutput,
    },
}

/// A change to a data source's parts (`STORE-15`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum PartsChange {
    /// Adds the part, after the others, with its other values.
    Add {
        part: Part,
        fields: BTreeMap<String, String>,
    },
    /// Sets the part's other values that are named, keeping the others.
    SetFields {
        part: Part,
        fields: BTreeMap<String, String>,
    },
    /// Removes the part.
    Remove { part: Part },
}

/// A change to a table's rows, as a front end makes it, to the rows as they're shown: each row named by its key, as the
/// view has it, and given as it was, so the change is refused if it isn't still, and as it's to be (`STORE-19`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum TableChange {
    /// Changes the row's values that differ from those it had.
    Update {
        key: RowKey,
        old: TableRow,
        new: TableRow,
    },
    /// Adds the row, after the others.
    Add {
        row: TableRow,
    },
    Remove {
        key: RowKey,
        row: TableRow,
    },
    /// Moves the row to before another, or, if there's none, to the end.
    Move {
        key: RowKey,
        row: TableRow,
        before: Option<(RowKey, TableRow)>,
    },
}

/// What the data source was resolved for.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Intent {
    Open,
    Load,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PartsView {
    /// The data source as the stores service identified it (`STORE-6`), which may be written differently from the
    /// event's.
    pub source: PartsSource,
    /// The revision of the data source the parts were read from (`STORE-8`).
    pub revision: Revision,
    /// The names of the parts' other values, in order, for example `["LCSC"]`.
    pub field_names: Vec<String>,
    /// The parts, in the order the data source has them.
    pub parts: Vec<PartsViewItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum PartsViewRendererOperation {
    View { view: PartsView },
}

impl Operation for PartsViewRendererOperation {
    type Output = ();
}

/// A data source's records of a kind, as a table, read-only (`STORE-17`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct TableView {
    pub kind: TableKind,
    /// The data source as the stores service identified it (`STORE-6`).
    pub source: Source,
    /// The revision of the data source the table was read from (`STORE-8`).
    pub revision: Revision,
    /// Its columns, in order, worked out once, as the table's read, so a front end can show it each frame, each with its
    /// reference, if its values can be changed (`STORE-19`).
    pub columns: Vec<TableColumn>,
    pub table: Table,
    /// Each row's key, in the order of the rows, by which a change names it (`STORE-19`).
    pub keys: Vec<RowKey>,
    /// How the rows are ordered, so whether they can be moved (`STORE-19`).
    pub ordering: RowOrdering,
}

impl TableView {
    /// The row, with its key, if the table's kind can be changed, for a front end to change (`STORE-19`).
    pub fn row(&self, index: usize) -> Option<(RowKey, TableRow)> {
        self.keys
            .get(index)
            .cloned()
            .zip(self.table.row(index))
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum TableViewRendererOperation {
    View { view: TableView },
}

impl Operation for TableViewRendererOperation {
    type Output = ();
}

fn parts_view_builder<Effect, Event>(view: PartsView) -> NotificationBuilder<Effect, Event, impl Future<Output = ()>>
where
    Effect: From<Request<PartsViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    Command::notify_shell(PartsViewRendererOperation::View {
        view,
    })
}

#[derive(Error, Debug)]
pub enum StoresError {
    #[error("Unable to load the parts. source: '{parts_source}', cause: {cause}")]
    UnableToLoadParts { parts_source: PartsSource, cause: String },

    #[error("The parts are not loaded. source: '{parts_source}'")]
    PartsNotLoaded { parts_source: PartsSource },

    /// The data source changed after the revision the change was based on, so the change would lose another's.
    #[error(
        "The parts have changed since the change was made. source: '{parts_source}', revision: '{revision}', current_revision: {}",
        current_revision.as_ref().map_or("none".to_string(), |revision| format!("'{}'", revision))
    )]
    PartsChanged {
        parts_source: PartsSource,
        revision: Revision,
        current_revision: Option<Revision>,
    },

    #[error("The parts are being changed. source: '{parts_source}'")]
    PartsBeingChanged { parts_source: PartsSource },

    #[error("The part already exists. source: '{parts_source}', manufacturer: '{}', mpn: '{}'", part.manufacturer, part.mpn)]
    PartAlreadyExists { parts_source: PartsSource, part: Part },

    #[error("The part doesn't exist. source: '{parts_source}', manufacturer: '{}', mpn: '{}'", part.manufacturer, part.mpn)]
    PartNotFound { parts_source: PartsSource, part: Part },

    #[error("Unable to change the parts. source: '{parts_source}', cause: {cause}")]
    UnableToChangeParts { parts_source: PartsSource, cause: String },

    #[error("Unable to load the table. kind: {kind:?}, source: '{data_source}', cause: {cause}")]
    UnableToLoadTable {
        kind: TableKind,
        data_source: Source,
        cause: String,
    },

    #[error("Data source not found. source: '{data_source}'")]
    DataSourceNotFound { data_source: Source },

    #[error("The table is not loaded. kind: {kind:?}, source: '{data_source}'")]
    TableNotLoaded { kind: TableKind, data_source: Source },

    /// A row's values aren't those the change was made to, or the row isn't there, so the change would lose another's.
    #[error(
        "The table has changed since the change was made. kind: {kind:?}, source: '{data_source}', current_revision: {}",
        current_revision.as_ref().map_or("none".to_string(), |revision| format!("'{}'", revision))
    )]
    TableChanged {
        kind: TableKind,
        data_source: Source,
        current_revision: Option<Revision>,
    },

    #[error("The table is being changed. kind: {kind:?}, source: '{data_source}'")]
    TableBeingChanged { kind: TableKind, data_source: Source },

    #[error("The table can't be changed. kind: {kind:?}, source: '{data_source}'")]
    TableNotChangeable { kind: TableKind, data_source: Source },

    #[error("The column doesn't exist. kind: {kind:?}, source: '{data_source}', column: {column:?}")]
    ColumnNotFound {
        kind: TableKind,
        data_source: Source,
        column: ColumnValue,
    },

    #[error("The rows can't be moved. kind: {kind:?}, source: '{data_source}'")]
    RowsNotOrdered { kind: TableKind, data_source: Source },

    #[error("Unable to change the table. kind: {kind:?}, source: '{data_source}', cause: {cause}")]
    UnableToChangeTable {
        kind: TableKind,
        data_source: Source,
        cause: String,
    },

    /// The shell gave the result of a different operation, which is a fault in the shell.
    #[error("Unexpected stores output. output: {output}")]
    UnexpectedOutput { output: String },
}

impl App for Stores {
    type Event = Event;
    type Model = Model;
    type ViewModel = StoresViewModel;
    type Effect = Effect;

    fn update(&self, event: Self::Event, model: &mut Self::Model) -> Command<Self::Effect, Self::Event> {
        // `LOG-1`
        debug!("Received event. event: {:?}", event);

        match Self::update_inner(event, model) {
            Ok(command) => {
                model.error = None;
                command
            }
            Err(Refused {
                error,
                then,
            }) => {
                // `LOG-2`
                warn!("Event failed. error: {:?}", error);
                model.error = Some(error.to_string());
                render::render().and(then)
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        let status = |source: &PartsSource| {
            model
                .sources
                .get(source)
                .map(|state| PartsSourceStatus {
                    source: source.clone(),
                    loaded: state.parts.is_some(),
                    revision: state.revision.clone(),
                    reading: state.reading,
                    changing: state.changing,
                    error: state.error.clone(),
                    change_error: state.change_error.clone(),
                })
        };

        let mut parts_sources: BTreeMap<PartsSource, PartsSourceStatus> = model
            .sources
            .keys()
            .filter_map(|source| status(source).map(|status| (source.clone(), status)))
            .collect();
        for (alias, source) in model.aliases.iter() {
            if let Some(status) = status(source) {
                parts_sources.insert(alias.clone(), status);
            }
        }

        let tables = model
            .table_aliases
            .iter()
            .filter_map(|((kind, asked), source)| {
                model
                    .tables
                    .get(&(*kind, source.clone()))
                    .map(|state| TableStatus {
                        kind: *kind,
                        asked: asked.clone(),
                        source: source.clone(),
                        loaded: state.table.is_some(),
                        revision: state
                            .table
                            .as_ref()
                            .map(|records| records.revision.clone()),
                        reading: state.reading,
                        error: state.error.clone(),
                        changing: state.changing,
                        change_error: state.change_error.clone(),
                    })
            })
            .collect();

        StoresViewModel {
            error: model.error.clone(),
            parts_sources,
            tables,
        }
    }
}

/// Why an event was refused, and what to do even so, for example read a data source again.
struct Refused {
    error: StoresError,
    then: Command<Effect, Event>,
}

impl From<StoresError> for Refused {
    fn from(error: StoresError) -> Self {
        Self {
            error,
            then: Command::done(),
        }
    }
}

impl Stores {
    fn update_inner(event: Event, model: &mut Model) -> Result<Command<Effect, Event>, Refused> {
        match event {
            Event::OpenTable {
                kind,
                source,
            } => {
                // already identified and loaded, so there's nothing to ask the stores service, `STORE-5`.
                if let Some(resolved) = model
                    .table_aliases
                    .get(&(kind, source.clone()))
                    .cloned()
                {
                    if Self::is_table_loaded(model, kind, &resolved) {
                        info!("Table already loaded. kind: {:?}, source: '{}'", kind, resolved);
                        return Ok(render::render().and(Self::table_view(model, kind, resolved)?));
                    }
                }

                Ok(Self::resolve_table(kind, source))
            }
            Event::TableSourceResolved {
                kind,
                source,
                output,
            } => {
                let StoresOutput::SourceResolved {
                    source: resolved,
                } = output
                else {
                    return Err(StoresError::UnexpectedOutput {
                        output: format!("{:?}", output),
                    }
                    .into());
                };
                model
                    .table_aliases
                    .insert((kind, source), resolved.clone());
                let state = model
                    .tables
                    .entry((kind, resolved.clone()))
                    .or_default();

                // `STORE-9`, once for each table, so that its views are updated whoever changes it.
                let watch = match state.watching {
                    true => Command::done(),
                    false => {
                        state.watching = true;
                        Self::watch_table(kind, resolved.clone())
                    }
                };

                let command = if state.table.is_some() {
                    info!("Table already loaded. kind: {:?}, source: '{}'", kind, resolved);
                    render::render().and(Self::table_view(model, kind, resolved)?)
                } else if state.reading {
                    // another open is reading it, and its view will be sent to every view of the table.
                    render::render()
                } else {
                    state.reading = true;
                    render::render().and(Self::read_table(kind, resolved))
                };

                Ok(command.and(watch))
            }
            Event::TableSourceChanged {
                kind,
                source,
                output,
            } => {
                let StoresOutput::Changed = output else {
                    return Err(StoresError::UnexpectedOutput {
                        output: format!("{:?}", output),
                    }
                    .into());
                };
                info!("Data source changed. source: '{}', kind: {:?}", source, kind);
                let state = model
                    .tables
                    .entry((kind, source.clone()))
                    .or_default();

                if state.reading {
                    // the table being read may be from before the change, so it's read again once it is.
                    state.changed_while_reading = true;
                    return Ok(render::render());
                }

                state.reading = true;
                Ok(render::render().and(Self::read_table(kind, source)))
            }
            Event::TableRead {
                kind,
                source,
                output,
            } => Self::table_read(model, kind, source, output),
            Event::ChangeTable {
                kind,
                source,
                change,
            } => {
                let resolved = model
                    .table_aliases
                    .get(&(kind, source.clone()))
                    .cloned()
                    .unwrap_or(source);
                let result = Self::change_table(model, kind, &resolved, change);
                Self::keep_table_change_error(model, kind, &resolved, &result);
                result
            }
            Event::RowsChanged {
                kind,
                source,
                output,
            } => {
                let result = Self::rows_changed(model, kind, &source, output);
                Self::keep_table_change_error(model, kind, &source, &result);
                result
            }
            Event::OpenParts {
                source,
            } => {
                // already identified and loaded, so there's nothing to ask the stores service.
                if let Some(resolved) = model.aliases.get(&source).cloned() {
                    if Self::is_loaded(model, &resolved) {
                        info!("Parts already loaded. source: '{}'", resolved);
                        return Ok(render::render().and(Self::parts_view(model, resolved)?));
                    }
                }

                Ok(Self::resolve(source, Intent::Open))
            }
            Event::LoadParts {
                source,
            } => Ok(Self::resolve(source, Intent::Load)),
            Event::RequestPartsView {
                source,
            } => {
                let resolved = model
                    .aliases
                    .get(&source)
                    .cloned()
                    .unwrap_or(source);

                Ok(Self::parts_view(model, resolved)?)
            }
            Event::ChangeParts {
                source,
                revision,
                change,
            } => {
                let resolved = model
                    .aliases
                    .get(&source)
                    .cloned()
                    .unwrap_or(source);
                let result = Self::change_parts(model, &resolved, revision, change);
                Self::keep_change_error(model, &resolved, &result);
                result
            }
            Event::PartsWritten {
                source,
                revision,
                parts,
                output,
            } => {
                let result = Self::parts_written(model, &source, revision, parts, output);
                Self::keep_change_error(model, &source, &result);
                result
            }
            Event::SourceResolved {
                source,
                intent,
                output,
            } => {
                let StoresOutput::SourceResolved {
                    source: resolved,
                } = output
                else {
                    return Err(StoresError::UnexpectedOutput {
                        output: format!("{:?}", output),
                    }
                    .into());
                };
                model
                    .aliases
                    .insert(source, resolved.clone());
                let state = model
                    .sources
                    .entry(resolved.clone())
                    .or_default();

                // `STORE-9`, once for each data source, so that its views are updated whoever changes it.
                let watch = match state.watching {
                    true => Command::done(),
                    false => {
                        state.watching = true;
                        Self::watch(resolved.clone())
                    }
                };

                let command = match intent {
                    Intent::Open if state.parts.is_some() => {
                        info!("Parts already loaded. source: '{}'", resolved);
                        render::render().and(Self::parts_view(model, resolved)?)
                    }
                    // another open is reading them, and its view will be sent to every view of the data source.
                    Intent::Open if state.reading => render::render(),
                    Intent::Open | Intent::Load => {
                        state.reading = true;
                        render::render().and(Self::read(resolved))
                    }
                };

                Ok(command.and(watch))
            }
            Event::SourceChanged {
                source,
                output,
            } => {
                let StoresOutput::Changed = output else {
                    return Err(StoresError::UnexpectedOutput {
                        output: format!("{:?}", output),
                    }
                    .into());
                };
                info!("Data source changed. source: '{}'", source);
                let state = model
                    .sources
                    .entry(source.clone())
                    .or_default();

                if state.reading {
                    // the parts being read may be from before the change, so they're read again once they are.
                    state.changed_while_reading = true;
                    return Ok(render::render());
                }

                state.reading = true;
                Ok(render::render().and(Self::read(source)))
            }
            Event::PartsRead {
                source,
                output,
            } => {
                let state = model
                    .sources
                    .entry(source.clone())
                    .or_default();

                // `STORE-9`, out of date, as the data source changed while they were read, so they're read again.
                if state.changed_while_reading {
                    state.changed_while_reading = false;
                    info!(
                        "Data source changed while being read, reading it again. source: '{}'",
                        source
                    );
                    return Ok(Self::read(source));
                }
                state.reading = false;

                let parts = match output {
                    StoresOutput::PartsRead(Ok(parts)) => parts,
                    StoresOutput::PartsRead(Err(cause)) => {
                        let error = StoresError::UnableToLoadParts {
                            parts_source: source,
                            cause,
                        };
                        // `STORE-3`, the parts loaded before, if any, are kept.
                        state.error = Some(error.to_string());
                        return Err(error.into());
                    }
                    output => {
                        return Err(StoresError::UnexpectedOutput {
                            output: format!("{:?}", output),
                        }
                        .into());
                    }
                };

                let Parts {
                    revision,
                    parts,
                } = parts;
                state.error = None;

                // `STORE-8`, every view of the data source shows this revision already.
                if state.parts.is_some() && state.revision.as_ref() == Some(&revision) {
                    info!("Parts unchanged. source: '{}', revision: '{}'", source, revision);
                    return Ok(render::render());
                }

                info!(
                    "Loaded parts. source: '{}', part_count: {}, revision: '{}'",
                    source,
                    parts.len(),
                    revision
                );
                state.parts = Some(parts);
                state.revision = Some(revision);

                Ok(render::render().and(Self::parts_view(model, source)?))
            }
        }
    }

    /// Keeps the table read, and sends its view, unless its revision is the one every view shows already, or reads it
    /// again, if its data source changed while it was read (`STORE-17`, `STORE-8`, `STORE-9`).
    fn table_read(
        model: &mut Model,
        kind: TableKind,
        source: Source,
        output: StoresOutput,
    ) -> Result<Command<Effect, Event>, Refused> {
        let state = model
            .tables
            .entry((kind, source.clone()))
            .or_default();

        if state.changed_while_reading {
            state.changed_while_reading = false;
            info!(
                "Data source changed while being read, reading it again. source: '{}', kind: {:?}",
                source, kind
            );
            return Ok(Self::read_table(kind, source));
        }
        state.reading = false;

        let error = match output {
            StoresOutput::TableRead(Ok(Some(records))) => {
                state.error = None;

                if state
                    .table
                    .as_ref()
                    .is_some_and(|loaded| loaded.revision == records.revision)
                {
                    info!(
                        "Table unchanged. kind: {:?}, source: '{}', revision: '{}'",
                        kind, source, records.revision
                    );
                    return Ok(render::render());
                }

                info!(
                    "Loaded table. kind: {:?}, source: '{}', row_count: {}, revision: '{}'",
                    kind,
                    source,
                    records.table.len(),
                    records.revision
                );
                state.table = Some(records);

                return Ok(render::render().and(Self::table_view(model, kind, source)?));
            }
            StoresOutput::TableRead(Ok(None)) => StoresError::DataSourceNotFound {
                data_source: source,
            },
            StoresOutput::TableRead(Err(cause)) => StoresError::UnableToLoadTable {
                kind,
                data_source: source,
                cause,
            },
            output => {
                return Err(StoresError::UnexpectedOutput {
                    output: format!("{:?}", output),
                }
                .into());
            }
        };

        // `STORE-3`, the table loaded before, if any, is kept.
        state.error = Some(error.to_string());
        Err(error.into())
    }

    fn is_table_loaded(model: &Model, kind: TableKind, source: &Source) -> bool {
        model
            .tables
            .get(&(kind, source.clone()))
            .is_some_and(|state| state.table.is_some())
    }

    fn resolve_table(kind: TableKind, source: Source) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::ResolveSource {
            source: source.clone(),
        })
        .then_send(move |output| Event::TableSourceResolved {
            kind,
            source: source.clone(),
            output,
        })
    }

    fn watch_table(kind: TableKind, source: Source) -> Command<Effect, Event> {
        Command::stream_from_shell(StoresOperation::WatchSource {
            source: source.clone(),
        })
        .then_send(move |output| Event::TableSourceChanged {
            kind,
            source: source.clone(),
            output,
        })
    }

    fn read_table(kind: TableKind, source: Source) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::ReadTable {
            kind,
            source: source.clone(),
        })
        .then_send(move |output| Event::TableRead {
            kind,
            source: source.clone(),
            output,
        })
    }

    fn table_view(model: &Model, kind: TableKind, source: Source) -> Result<Command<Effect, Event>, StoresError> {
        let records = model
            .tables
            .get(&(kind, source.clone()))
            .and_then(|state| state.table.as_ref())
            .ok_or_else(|| StoresError::TableNotLoaded {
                kind,
                data_source: source.clone(),
            })?;

        let view = TableView {
            kind,
            source,
            revision: records.revision.clone(),
            columns: records.columns.clone(),
            table: records.table.clone(),
            keys: records.keys.clone(),
            ordering: records.ordering.clone(),
        };

        Ok(Command::notify_shell(TableViewRendererOperation::View {
            view,
        })
        .into())
    }

    /// Has the service make the change, by the rows' keys and their columns' references, if the table is loaded, its
    /// kind can be changed, and no other change is being made (`STORE-19`). The change isn't checked against the
    /// table's revision: the service checks the values the change was made to are still those of its rows.
    fn change_table(
        model: &mut Model,
        kind: TableKind,
        source: &Source,
        change: TableChange,
    ) -> Result<Command<Effect, Event>, Refused> {
        let state = model
            .tables
            .get_mut(&(kind, source.clone()))
            .filter(|state| state.table.is_some())
            .ok_or_else(|| StoresError::TableNotLoaded {
                kind,
                data_source: source.clone(),
            })?;

        if state.changing {
            return Err(StoresError::TableBeingChanged {
                kind,
                data_source: source.clone(),
            }
            .into());
        }

        let records = state
            .table
            .as_ref()
            .ok_or_else(|| StoresError::TableNotLoaded {
                kind,
                data_source: source.clone(),
            })?;
        let change = Self::rows_change(kind, source, records, change)?;

        state.changing = true;
        Ok(render::render().and(Self::change_rows(kind, source.clone(), change)))
    }

    /// The change, as the back end makes it: the rows by their keys, and their values by their columns' references, as
    /// the data source has them, if the table's kind can be changed, each value's column has a reference, and a row
    /// moved can be.
    fn rows_change(
        kind: TableKind,
        source: &Source,
        records: &TableRecords,
        change: TableChange,
    ) -> Result<RowsChange, StoresError> {
        if !records.table.is_changeable() {
            return Err(StoresError::TableNotChangeable {
                kind,
                data_source: source.clone(),
            });
        }
        let reference = |value: &ColumnValue| -> Result<ColumnRef, StoresError> {
            records
                .columns
                .iter()
                .find(|column| column.value == *value)
                .and_then(|column| column.reference.clone())
                .ok_or_else(|| StoresError::ColumnNotFound {
                    kind,
                    data_source: source.clone(),
                    column: value.clone(),
                })
        };
        let values = |row: &TableRow| -> Result<Vec<FieldValue>, StoresError> {
            row.values()
                .into_iter()
                .map(|(value, text)| {
                    Ok(FieldValue {
                        column: reference(&value)?,
                        value: text,
                    })
                })
                .collect()
        };
        let row_ref = |key: RowKey, row: &TableRow| -> Result<RowRef, StoresError> {
            Ok(RowRef {
                key,
                expected: values(row)?,
            })
        };

        Ok(match change {
            TableChange::Update {
                key,
                old,
                new,
            } => {
                let old: BTreeMap<ColumnValue, String> = old.values().into_iter().collect();
                // only the values that changed, so a change to another of the row's values isn't refused.
                let changes = new
                    .values()
                    .into_iter()
                    .filter(|(value, text)| old.get(value) != Some(text))
                    .map(|(value, text)| {
                        Ok(FieldChange {
                            column: reference(&value)?,
                            old: old
                                .get(&value)
                                .cloned()
                                .unwrap_or_default(),
                            new: text,
                        })
                    })
                    .collect::<Result<_, StoresError>>()?;
                RowsChange::Update {
                    key,
                    changes,
                }
            }
            TableChange::Add {
                row,
            } => RowsChange::Add {
                values: values(&row)?,
            },
            TableChange::Remove {
                key,
                row,
            } => RowsChange::Remove {
                row: row_ref(key, &row)?,
            },
            TableChange::Move {
                key,
                row,
                before,
            } => {
                if records.ordering == RowOrdering::Unsupported {
                    return Err(StoresError::RowsNotOrdered {
                        kind,
                        data_source: source.clone(),
                    });
                }
                RowsChange::Move {
                    row: row_ref(key, &row)?,
                    before: before
                        .map(|(key, row)| row_ref(key, &row))
                        .transpose()?,
                }
            }
        })
    }

    /// Reads the table again, once its rows were changed, so every view shows them, with the keys the back end gives
    /// them now, or, if they had changed since the change was made, so every view shows them as they are now, which
    /// the change can be made to again (`STORE-19`).
    fn rows_changed(
        model: &mut Model,
        kind: TableKind,
        source: &Source,
        output: StoresOutput,
    ) -> Result<Command<Effect, Event>, Refused> {
        let state = model
            .tables
            .entry((kind, source.clone()))
            .or_default();
        state.changing = false;

        let mut read_again = || match state.reading {
            // `STORE-9`, the read under way may be from before the change, so it's read again once it's done.
            true => {
                state.changed_while_reading = true;
                Command::done()
            }
            false => {
                state.reading = true;
                Self::read_table(kind, source.clone())
            }
        };

        match output {
            StoresOutput::RowsChanged(Ok(revision)) => {
                info!(
                    "Changed table. kind: {:?}, source: '{}', revision: '{}'",
                    kind, source, revision
                );
                Ok(render::render().and(read_again()))
            }
            StoresOutput::RowsChanged(Err(PartsWriteError::Changed {
                revision: current_revision,
            })) => Err(Refused {
                error: StoresError::TableChanged {
                    kind,
                    data_source: source.clone(),
                    current_revision,
                },
                then: read_again(),
            }),
            // the table is kept as it was, as it wasn't changed.
            StoresOutput::RowsChanged(Err(PartsWriteError::Failed {
                cause,
            })) => Err(StoresError::UnableToChangeTable {
                kind,
                data_source: source.clone(),
                cause,
            }
            .into()),
            output => Err(StoresError::UnexpectedOutput {
                output: format!("{:?}", output),
            }
            .into()),
        }
    }

    /// Keeps why the table's last change was refused, until one succeeds, as [`Self::keep_change_error`] does.
    fn keep_table_change_error(
        model: &mut Model,
        kind: TableKind,
        source: &Source,
        result: &Result<Command<Effect, Event>, Refused>,
    ) {
        if let Some(state) = model
            .tables
            .get_mut(&(kind, source.clone()))
        {
            state.change_error = result
                .as_ref()
                .err()
                .map(|refused| refused.error.to_string());
        }
    }

    fn change_rows(kind: TableKind, source: Source, change: RowsChange) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::ChangeRows {
            kind,
            source: source.clone(),
            change,
        })
        .then_send(move |output| Event::RowsChanged {
            kind,
            source: source.clone(),
            output,
        })
    }

    fn is_loaded(model: &Model, source: &PartsSource) -> bool {
        model
            .sources
            .get(source)
            .is_some_and(|state| state.parts.is_some())
    }

    fn resolve(source: PartsSource, intent: Intent) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::ResolveSource {
            source: source.clone(),
        })
        .then_send(move |output| Event::SourceResolved {
            source: source.clone(),
            intent,
            output,
        })
    }

    /// A stream: the shell answers each time the data source may have changed, for as long as it can watch it.
    fn watch(source: PartsSource) -> Command<Effect, Event> {
        Command::stream_from_shell(StoresOperation::WatchSource {
            source: source.clone(),
        })
        .then_send(move |output| Event::SourceChanged {
            source: source.clone(),
            output,
        })
    }

    /// Has the service write the parts, changed, if the change is based on the revision the core has, the parts are
    /// loaded, and no other change is being written (`STORE-15`).
    fn change_parts(
        model: &mut Model,
        source: &PartsSource,
        revision: Revision,
        change: PartsChange,
    ) -> Result<Command<Effect, Event>, Refused> {
        let state = model
            .sources
            .get_mut(source)
            .filter(|state| state.parts.is_some())
            .ok_or_else(|| StoresError::PartsNotLoaded {
                parts_source: source.clone(),
            })?;

        if state.changing {
            return Err(StoresError::PartsBeingChanged {
                parts_source: source.clone(),
            }
            .into());
        }
        // the core has read the parts since the view the change was based on, so they've changed.
        if state.revision.as_ref() != Some(&revision) {
            return Err(StoresError::PartsChanged {
                parts_source: source.clone(),
                revision,
                current_revision: state.revision.clone(),
            }
            .into());
        }

        let parts = Self::changed_parts(
            source,
            state
                .parts
                .as_deref()
                .unwrap_or_default(),
            change,
        )?;

        state.changing = true;
        Ok(render::render().and(Self::write(source.clone(), revision, parts)))
    }

    /// Keeps the parts written, or, if the data source changed since the revision the change was based on, reads it
    /// again, so every view shows the parts as they are now, which the change can be made to again (`STORE-15`).
    fn parts_written(
        model: &mut Model,
        source: &PartsSource,
        based_on: Revision,
        parts: Vec<PartsViewItem>,
        output: StoresOutput,
    ) -> Result<Command<Effect, Event>, Refused> {
        let state = model
            .sources
            .entry(source.clone())
            .or_default();
        state.changing = false;

        let revision = match output {
            StoresOutput::PartsWritten(Ok(revision)) => revision,
            StoresOutput::PartsWritten(Err(PartsWriteError::Changed {
                revision: current_revision,
            })) => {
                let error = StoresError::PartsChanged {
                    parts_source: source.clone(),
                    revision: based_on,
                    current_revision,
                };
                let then = match state.reading {
                    // `STORE-9`, the read under way may be from before the change, so it's read again once it's done.
                    true => {
                        state.changed_while_reading = true;
                        Command::done()
                    }
                    false => {
                        state.reading = true;
                        Self::read(source.clone())
                    }
                };
                return Err(Refused {
                    error,
                    then,
                });
            }
            // the parts are kept as they were, as they weren't changed.
            StoresOutput::PartsWritten(Err(PartsWriteError::Failed {
                cause,
            })) => {
                return Err(StoresError::UnableToChangeParts {
                    parts_source: source.clone(),
                    cause,
                }
                .into());
            }
            output => {
                return Err(StoresError::UnexpectedOutput {
                    output: format!("{:?}", output),
                }
                .into());
            }
        };

        info!(
            "Changed parts. source: '{}', part_count: {}, revision: '{}'",
            source,
            parts.len(),
            revision
        );
        state.parts = Some(parts);
        state.revision = Some(revision);

        Ok(render::render().and(Self::parts_view(model, source.clone())?))
    }

    /// Keeps why the data source's last change was refused, until one succeeds, so that every view of it can show it,
    /// even once the event after it, for example a read, succeeds.
    fn keep_change_error(model: &mut Model, source: &PartsSource, result: &Result<Command<Effect, Event>, Refused>) {
        if let Some(state) = model.sources.get_mut(source) {
            state.change_error = result
                .as_ref()
                .err()
                .map(|refused| refused.error.to_string());
        }
    }

    /// The parts, with the change made. As the data source keeps every value of every part, as a file's columns do, a
    /// part is given an empty value of each name it has no value of, so the parts are as they would be read back.
    fn changed_parts(
        source: &PartsSource,
        parts: &[PartsViewItem],
        change: PartsChange,
    ) -> Result<Vec<PartsViewItem>, StoresError> {
        let mut parts = parts.to_vec();
        let index_of = |parts: &[PartsViewItem], part: &Part| {
            parts
                .iter()
                .position(|item| item.part == *part)
        };
        let not_found = |part: Part| StoresError::PartNotFound {
            parts_source: source.clone(),
            part,
        };

        match change {
            PartsChange::Add {
                part,
                fields,
            } => {
                if index_of(&parts, &part).is_some() {
                    return Err(StoresError::PartAlreadyExists {
                        parts_source: source.clone(),
                        part,
                    });
                }
                parts.push(PartsViewItem {
                    part,
                    fields,
                });
            }
            PartsChange::SetFields {
                part,
                fields,
            } => {
                let index = index_of(&parts, &part).ok_or_else(|| not_found(part))?;
                parts[index].fields.extend(fields);
            }
            PartsChange::Remove {
                part,
            } => {
                let index = index_of(&parts, &part).ok_or_else(|| not_found(part))?;
                parts.remove(index);
            }
        }

        let names: Vec<String> = parts
            .iter()
            .flat_map(|item| item.fields.keys().cloned())
            .collect();
        for item in parts.iter_mut() {
            for name in names.iter() {
                item.fields
                    .entry(name.clone())
                    .or_default();
            }
        }

        Ok(parts)
    }

    fn write(source: PartsSource, revision: Revision, parts: Vec<PartsViewItem>) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::WriteParts {
            source: source.clone(),
            revision: revision.clone(),
            parts: parts.clone(),
        })
        .then_send(move |output| Event::PartsWritten {
            source: source.clone(),
            revision: revision.clone(),
            parts: parts.clone(),
            output,
        })
    }

    fn read(source: PartsSource) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::ReadParts {
            source: source.clone(),
        })
        .then_send(move |output| Event::PartsRead {
            source: source.clone(),
            output,
        })
    }

    fn parts_view(model: &Model, source: PartsSource) -> Result<Command<Effect, Event>, StoresError> {
        let (parts, revision) = model
            .sources
            .get(&source)
            .and_then(|state| {
                state
                    .parts
                    .as_ref()
                    .zip(state.revision.as_ref())
            })
            .ok_or_else(|| StoresError::PartsNotLoaded {
                parts_source: source.clone(),
            })?;

        let mut field_names: Vec<String> = parts
            .iter()
            .flat_map(|item| item.fields.keys().cloned())
            .collect();
        field_names.sort();
        field_names.dedup();

        let view = PartsView {
            source,
            revision: revision.clone(),
            field_names,
            parts: parts.clone(),
        };

        Ok(parts_view_builder(view).into())
    }
}
