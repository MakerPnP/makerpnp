use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use anyhow::{anyhow, bail};
use clap::Parser;
use cli::args::EdaToolArg;
use crossbeam_channel::unbounded;
use futures::executor::block_on;
use planner_app::{Effect, Event};
use stores_app::{
    quoted_list, ColumnPosition, ColumnsToAdd, EdaSubstitutionRuleTransformItem, FieldPattern, Part,
    PartMappingsViewItem, PartsChange, PartsView, PartsViewRendererOperation, Pattern, Revision, RowKey, Source,
    Stores, StoresError, SubstitutionsViewItem, TableChange, TableKind, TableRow, TableView,
    TableViewRendererOperation,
};

use crate::core::Core;
use crate::opts::{
    build_project_file_path, ModeCommand, Opts, PartArgs, PartsCommand, PartsCommandArgs, PcbCommand, ProcessCommand,
    ProjectCommand, TableCommand, TableCommandArgs, WasRowArgs,
};

mod core;
mod opts;

fn main() -> anyhow::Result<()> {
    let args = argfile::expand_args(argfile::parse_fromfile, argfile::PREFIX).unwrap();

    let opts = Opts::parse_from(args);

    cli::tracing::configure_tracing(opts.trace.clone(), opts.verbose.clone())?;

    let core = core::new();

    let event = match &opts.command {
        ModeCommand::Project(project_args) => {
            if !matches!(project_args.command, ProjectCommand::Create { .. }) {
                let project_name = &project_args.project;
                let directory = project_args.path.clone();

                let path = build_project_file_path(project_name, &directory);
                run_loop(&core, Event::Load {
                    path,
                })?;
            }
            Event::try_from(opts)?
        }
        ModeCommand::Pcb(pcb_args) => {
            if !matches!(pcb_args.command, PcbCommand::Create { .. }) {
                let path = pcb_args.pcb_file.clone();
                run_loop(&core, Event::LoadPcb {
                    path,
                })?;
            }
            Event::try_from(opts)?
        }
        ModeCommand::Parts(parts_args) => {
            return run_parts(parts_args);
        }
        ModeCommand::Table(table_args) => {
            return run_table(table_args);
        }
        ModeCommand::Process(process_args) => {
            if !matches!(process_args.command, ProcessCommand::Create { .. }) {
                let path = process_args.process_file.clone();
                run_loop(&core, Event::LoadProcess {
                    path,
                })?;
            }
            Event::try_from(opts)?
        }
    };

    run_loop(&core, event)?;

    Ok(())
}

/// The views the stores core sent while processing an event.
#[derive(Default)]
struct StoresViews {
    parts: Vec<PartsView>,
    tables: Vec<TableView>,
}

/// Processes the event with the stores core, having the stores service do each operation it asks for, and waiting for
/// it, as the CLI does one thing at a time, and fails with the core's error, if it refuses.
fn process_stores(core: &crux_core::Core<Stores>, event: stores_app::Event) -> anyhow::Result<StoresViews> {
    // `STORE-7`, does the store operations the core asks for, the one the planner's core uses too.
    let service = crate::core::stores_service();

    let mut views = StoresViews::default();
    let mut effects: VecDeque<stores_app::Effect> = core.process_event(event).into();
    while let Some(effect) = effects.pop_front() {
        match effect {
            stores_app::Effect::Render(_) => {
                if let Some(error) = core.view().error {
                    bail!(error)
                }
            }
            stores_app::Effect::PartsView(request) => {
                let PartsViewRendererOperation::View {
                    view,
                } = request.operation;
                views.parts.push(view);
            }
            stores_app::Effect::TableView(request) => {
                let TableViewRendererOperation::View {
                    view,
                } = request.operation;
                views.tables.push(view);
            }
            // `STORE-9`, the CLI is done before a data source could change, so it doesn't watch them.
            stores_app::Effect::Stores(request) if request.operation.is_stream() => {}
            stores_app::Effect::Stores(mut request) => {
                let output = block_on(service.perform_once(request.operation.clone()))
                    .ok_or_else(|| anyhow!("No result from the stores service. operation: {:?}", request.operation))?;
                effects.extend(core.resolve(&mut request, output)?);
            }
        }
    }
    Ok(views)
}

/// The table mode uses the stores core, which has the data sources, and shows a data source's records as a table
/// (`STORE-18`), and changes part mappings and substitutions (`STORE-20`).
fn run_table(table_args: &TableCommandArgs) -> anyhow::Result<()> {
    let core: crux_core::Core<Stores> = crux_core::Core::new();
    let kind = table_args
        .kind
        .build(table_args.eda_tool.as_ref())
        .map_err(|error| anyhow!(error))?;
    let source = table_args.source.clone();

    // opening sends the table's view.
    let views = process_stores(&core, stores_app::Event::OpenTable {
        kind,
        source: source.clone(),
    })?;

    if let TableCommand::List {
        keys,
    } = table_args.command
    {
        for view in views.tables.iter() {
            write_table_csv(view, keys, std::io::stdout())?;
        }
        return Ok(());
    }

    let view = views
        .tables
        .last()
        .ok_or_else(|| anyhow!("No table view. source: '{}'", source))?;

    if let TableCommand::AddColumns {
        eda,
        columns,
        first,
        last,
        before,
        after,
    } = &table_args.command
    {
        let to_add = match eda {
            Some(eda) => ColumnsToAdd::EdaTool {
                eda_tool: eda.build(),
            },
            None => ColumnsToAdd::Named {
                columns: columns.clone(),
                position: match (first, last, before, after) {
                    (true, _, _, _) => ColumnPosition::First,
                    (_, true, _, _) => ColumnPosition::Last,
                    (_, _, Some(before), _) => ColumnPosition::Before(before.clone()),
                    (_, _, _, Some(after)) => ColumnPosition::After(after.clone()),
                    _ => bail!("Where the columns go is needed: '--first', '--last', '--before' or '--after'."),
                },
            },
        };
        return add_columns(&core, view, kind, source, to_add);
    }

    // `STORE-20`, each row changed as it was just read, or as the user saw it, so a change made since is refused.
    let change = match &table_args.command {
        TableCommand::List {
            ..
        } => unreachable!("listed above"),
        TableCommand::AddColumns {
            ..
        } => unreachable!("added above"),
        TableCommand::Add {
            values,
        } => {
            let eda_tool = values.eda.build();
            let mut row = match view.kind {
                TableKind::PartMappings => {
                    for (option, value) in [("--manufacturer", &values.manufacturer), ("--mpn", &values.mpn)] {
                        if value.is_none() {
                            bail!(
                                "Option needed for the table's kind. option: '{}', kind: {:?}",
                                option,
                                view.kind
                            );
                        }
                    }
                    TableRow::PartMappings(PartMappingsViewItem {
                        eda_tool,
                        criteria: vec![],
                        part: Part::new(String::new(), String::new()),
                    })
                }
                TableKind::Substitutions => TableRow::Substitutions(SubstitutionsViewItem {
                    eda_tool,
                    criteria: vec![],
                    transforms: vec![],
                    comment: None,
                }),
                _ => bail!(not_changeable(view)),
            };
            set_row(view, &mut row, &RowValues {
                options: "--",
                eda: None,
                patterns: &values.patterns,
                manufacturer: values.manufacturer.as_ref(),
                mpn: values.mpn.as_ref(),
                values: &values.values,
                comment: values.comment.as_ref(),
            })?;
            TableChange::Add {
                row,
            }
        }
        TableCommand::Set {
            row,
            values,
            was,
        } => {
            let (key, old) = row_as_it_was(view, &row.row, was)?;
            let mut new = old.clone();
            set_row(view, &mut new, &RowValues {
                options: "--",
                eda: values.eda.as_ref(),
                patterns: &values.patterns,
                manufacturer: values.manufacturer.as_ref(),
                mpn: values.mpn.as_ref(),
                values: &values.values,
                comment: values.comment.as_ref(),
            })?;
            TableChange::Update {
                key,
                old,
                new,
            }
        }
        TableCommand::Remove {
            row,
            was,
        } => {
            let (key, row) = row_as_it_was(view, &row.row, was)?;
            TableChange::Remove {
                key,
                row,
            }
        }
        TableCommand::Move {
            row,
            before,
            to_end: _,
            was,
        } => {
            let (key, row) = row_as_it_was(view, &row.row, was)?;
            let before = before
                .as_ref()
                .map(|before| table_row(view, before))
                .transpose()?;
            TableChange::Move {
                key,
                row,
                before,
            }
        }
    };

    process_stores(&core, stores_app::Event::ChangeTable {
        kind,
        source,
        change,
    })?;

    Ok(())
}

/// Adds the columns to the table, then says which were added, or that none were needed, as the core works out an EDA
/// tool's (`STORE-20`, `STORE-21`).
fn add_columns(
    core: &crux_core::Core<Stores>,
    view: &TableView,
    kind: TableKind,
    source: Source,
    to_add: ColumnsToAdd,
) -> anyhow::Result<()> {
    let views = process_stores(core, stores_app::Event::AddTableColumns {
        kind,
        source,
        columns: to_add.clone(),
    })?;

    let added = match (&to_add, views.tables.last()) {
        // a view is sent only if the table changed, so the columns were added.
        (
            ColumnsToAdd::Named {
                columns, ..
            },
            Some(_),
        ) => columns.clone(),
        // the core worked them out, so they're the columns the table has now that it didn't.
        (
            ColumnsToAdd::EdaTool {
                ..
            },
            Some(after),
        ) => after
            .columns
            .iter()
            .filter(|column| {
                !view
                    .columns
                    .iter()
                    .any(|before| before.value == column.value)
            })
            .map(|column| column.name.clone())
            .collect(),
        (_, None) => vec![],
    };
    match added.is_empty() {
        true => println!("No columns needed."),
        false => println!("Added columns. columns: {}", quoted_list(&added)),
    }

    Ok(())
}

/// The row with the key, as it was read, failing if it isn't there, or the table can't be changed.
fn table_row(view: &TableView, key: &str) -> anyhow::Result<(RowKey, TableRow)> {
    let index = view
        .keys
        .iter()
        .position(|row_key| row_key.0 == key)
        .ok_or_else(|| anyhow!("The row doesn't exist. key: '{}'", key))?;
    view.row(index)
        .ok_or_else(|| anyhow!(not_changeable(view)))
}

/// The table can't be changed, as the core says (`STORE-19`), or not yet by the CLI.
fn not_changeable(view: &TableView) -> StoresError {
    StoresError::TableNotChangeable {
        kind: view.kind,
        data_source: view.source.clone(),
    }
}

/// The row with the key, with the values the user gave as they saw them in place of those read.
fn row_as_it_was(view: &TableView, key: &str, was: &WasRowArgs) -> anyhow::Result<(RowKey, TableRow)> {
    let (key, mut row) = table_row(view, key)?;
    set_row(view, &mut row, &RowValues {
        options: "--was-",
        eda: was.was_eda.as_ref(),
        patterns: &was.was_patterns,
        manufacturer: was.was_manufacturer.as_ref(),
        mpn: was.was_mpn.as_ref(),
        values: &was.was_values,
        comment: was.was_comment.as_ref(),
    })?;
    Ok((key, row))
}

/// A row's values, as given with the options, a part mapping's or a substitution's.
struct RowValues<'a> {
    /// What the options' names start with, such as "`--was-`", to say which was refused.
    options: &'static str,
    eda: Option<&'a EdaToolArg>,
    patterns: &'a [(String, String)],
    manufacturer: Option<&'a String>,
    mpn: Option<&'a String>,
    values: &'a [(String, String)],
    comment: Option<&'a String>,
}

/// Sets the row's values given, keeping the others, refusing those its kind doesn't have, such as a substitution's
/// manufacturer, so the file is left as it was (`STORE-20`).
fn set_row(view: &TableView, row: &mut TableRow, values: &RowValues) -> anyhow::Result<()> {
    let refuse = |given: &[(&str, bool)]| -> anyhow::Result<()> {
        match given
            .iter()
            .find(|(_option, given)| *given)
        {
            Some((option, _given)) => bail!(
                "Option not for the table's kind. option: '{}{}', kind: {:?}",
                values.options,
                option,
                view.kind
            ),
            None => Ok(()),
        }
    };
    match row {
        TableRow::PartMappings(item) => {
            refuse(&[
                ("value", !values.values.is_empty()),
                ("comment", values.comment.is_some()),
            ])?;
            if let Some(eda) = values.eda {
                item.eda_tool = eda.build();
            }
            set_patterns(&mut item.criteria, values.patterns);
            if let Some(manufacturer) = values.manufacturer {
                item.part.manufacturer = manufacturer.clone();
            }
            if let Some(mpn) = values.mpn {
                item.part.mpn = mpn.clone();
            }
        }
        TableRow::Substitutions(item) => {
            refuse(&[
                ("manufacturer", values.manufacturer.is_some()),
                ("mpn", values.mpn.is_some()),
            ])?;
            if let Some(eda) = values.eda {
                item.eda_tool = eda.build();
            }
            set_patterns(&mut item.criteria, values.patterns);
            set_values(&mut item.transforms, values.values);
            if let Some(comment) = values.comment {
                item.comment = Some(comment.clone());
            }
        }
    }
    Ok(())
}

/// Sets each field's pattern, adding a field the row has no pattern of.
fn set_patterns(criteria: &mut Vec<FieldPattern>, patterns: &[(String, String)]) {
    for (field_name, pattern) in patterns {
        let pattern = Pattern::parse(pattern);
        match criteria
            .iter_mut()
            .find(|criterion| criterion.field_name == *field_name)
        {
            Some(criterion) => criterion.pattern = pattern,
            None => criteria.push(FieldPattern {
                field_name: field_name.clone(),
                pattern,
            }),
        }
    }
}

/// Sets each field's value, adding a field the substitution has no value of.
fn set_values(transforms: &mut Vec<EdaSubstitutionRuleTransformItem>, values: &[(String, String)]) {
    for (field_name, value) in values {
        match transforms
            .iter_mut()
            .find(|transform| transform.field_name == *field_name)
        {
            Some(transform) => transform.field_value = value.clone(),
            None => transforms.push(EdaSubstitutionRuleTransformItem {
                field_name: field_name.clone(),
                field_value: value.clone(),
            }),
        }
    }
}

/// The table, as CSV, every value quoted, with its columns' names as the headers, and each value as its column shows
/// it, a pattern as the data source has it, and each row's key first, in a `Key` column, if asked (`STORE-20`).
fn write_table_csv(view: &TableView, keys: bool, writer: impl std::io::Write) -> anyhow::Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .from_writer(writer);

    let key_header = keys.then_some("Key");
    writer.write_record(
        key_header.into_iter().chain(
            view.columns
                .iter()
                .map(|column| column.name.as_str()),
        ),
    )?;
    for row in 0..view.table.len() {
        let key = keys.then(|| view.keys[row].0.clone());
        writer.write_record(
            key.into_iter()
                .chain(view.columns.iter().map(|column| {
                    view.table
                        .cell(row, &column.value)
                        .to_string()
                })),
        )?;
    }
    writer.flush()?;

    Ok(())
}

/// The parts mode uses the stores core, which has the data sources, not the planner's core.
fn run_parts(parts_args: &PartsCommandArgs) -> anyhow::Result<()> {
    let core: crux_core::Core<Stores> = crux_core::Core::new();
    let process = |event: stores_app::Event| process_stores(&core, event).map(|views| views.parts);

    let source = parts_args.source.clone();
    // opening sends the parts view.
    let views = process(stores_app::Event::OpenParts {
        source: source.clone(),
    })?;

    let to_part = |args: &PartArgs| Part::new(args.manufacturer.clone(), args.mpn.clone());
    let to_fields = |values: &[(String, String)]| {
        values
            .iter()
            .cloned()
            .collect::<BTreeMap<_, _>>()
    };

    let (change, revision) = match &parts_args.command {
        PartsCommand::List => {
            for view in views.iter() {
                write_parts_csv(view, std::io::stdout())?;
            }
            return Ok(());
        }
        PartsCommand::Add {
            part,
            values,
            revision,
        } => (
            PartsChange::Add {
                part: to_part(part),
                fields: to_fields(values),
            },
            revision,
        ),
        PartsCommand::Set {
            part,
            values,
            revision,
        } => (
            PartsChange::SetFields {
                part: to_part(part),
                fields: to_fields(values),
            },
            revision,
        ),
        PartsCommand::Remove {
            part,
            revision,
        } => (
            PartsChange::Remove {
                part: to_part(part),
            },
            revision,
        ),
    };

    // `STORE-15`, based on the revision given, or else the one just read, so a change made since is refused.
    let revision = match &revision.revision {
        Some(revision) => Revision(revision.clone()),
        None => views
            .last()
            .map(|view| view.revision.clone())
            .ok_or_else(|| anyhow!("No parts view. source: '{}'", source))?,
    };
    process(stores_app::Event::ChangeParts {
        source,
        revision,
        change,
    })?;

    Ok(())
}

/// The parts, as CSV, with the headers the data source has: `Manufacturer`, `Mpn`, then the others in order.
fn write_parts_csv(view: &PartsView, writer: impl std::io::Write) -> anyhow::Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .from_writer(writer);

    let mut headers = vec!["Manufacturer".to_string(), "Mpn".to_string()];
    headers.extend(view.field_names.iter().cloned());
    writer.write_record(&headers)?;

    for item in view.parts.iter() {
        let mut record = vec![item.part.manufacturer.clone(), item.part.mpn.clone()];
        record.extend(view.field_names.iter().map(|name| {
            item.fields
                .get(name)
                .cloned()
                .unwrap_or_default()
        }));
        writer.write_record(&record)?;
    }
    writer.flush()?;

    Ok(())
}

fn run_loop(core: &Core, event: Event) -> Result<(), anyhow::Error> {
    let (tx, rx) = unbounded::<Effect>();

    core::update(&core, event, &Arc::new(tx))?;

    while let Ok(effect) = rx.recv() {
        match effect {
            _render @ Effect::Render(_) => {
                let view = core.view();

                if let Some((_date_time, error)) = view.error {
                    bail!(error)
                }

                // Saving after any operation is implicit for the CLI.
                // FUTURE: Maybe it would be useful to have a 'dry-run' flag that doesn't trigger a save.
                if view.project_modified {
                    run_loop(core, Event::Save)?
                }
                if view.pcbs_modified {
                    run_loop(core, Event::SaveAllPcbs)?
                }
                if view.processes_modified {
                    run_loop(core, Event::SaveAllProcesses)?
                }
            }
            Effect::ProjectView(_) => {
                // Currently, the CLI app should not cause these effects.
                unreachable!()
            }
            Effect::PcbView(_) | Effect::ProcessView(_) => {
                // Currently, the CLI app should not cause these effects.
                unreachable!()
            }
            Effect::Stores(_) => {
                // done, and their results given to the core, as they're asked for, see `core::process_effect`.
                unreachable!()
            }
        }
    }
    Ok(())
}
