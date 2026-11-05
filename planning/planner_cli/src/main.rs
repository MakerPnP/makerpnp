use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use anyhow::{anyhow, bail};
use clap::Parser;
use crossbeam_channel::unbounded;
use futures::executor::block_on;
use planner_app::{Effect, Event};
use stores_app::{
    Part, PartsChange, PartsView, PartsViewRendererOperation, Revision, Stores, TableView, TableViewRendererOperation,
};

use crate::core::Core;
use crate::opts::{
    build_project_file_path, ModeCommand, Opts, PartArgs, PartsCommand, PartsCommandArgs, PcbCommand, ProcessCommand,
    ProjectCommand, TableCommand, TableCommandArgs,
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
/// (`STORE-18`).
fn run_table(table_args: &TableCommandArgs) -> anyhow::Result<()> {
    let core: crux_core::Core<Stores> = crux_core::Core::new();
    let kind = table_args
        .kind
        .build(table_args.eda_tool.as_ref())
        .map_err(|error| anyhow!(error))?;

    // opening sends the table's view.
    let views = process_stores(&core, stores_app::Event::OpenTable {
        kind,
        source: table_args.source.clone(),
    })?;

    match table_args.command {
        TableCommand::List => {
            for view in views.tables.iter() {
                write_table_csv(view, std::io::stdout())?;
            }
        }
    }
    Ok(())
}

/// The table, as CSV, every value quoted, with its columns as the headers.
fn write_table_csv(view: &TableView, writer: impl std::io::Write) -> anyhow::Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .from_writer(writer);

    writer.write_record(&view.table.columns)?;
    for row in view.table.rows.iter() {
        writer.write_record(row)?;
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
