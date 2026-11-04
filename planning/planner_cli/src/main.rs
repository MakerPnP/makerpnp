use std::sync::Arc;

use anyhow::bail;
use clap::Parser;
use crossbeam_channel::unbounded;
use planner_app::{Effect, Event};
use stores_app::{PartsView, PartsViewRendererOperation, Stores};

use crate::core::Core;
use crate::opts::{
    build_project_file_path, ModeCommand, Opts, PartsCommand, PartsCommandArgs, PcbCommand, ProcessCommand,
    ProjectCommand,
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

/// The parts mode uses the stores core, which has the data sources, not the planner's core.
fn run_parts(parts_args: &PartsCommandArgs) -> anyhow::Result<()> {
    let core: crux_core::Core<Stores> = crux_core::Core::new();

    let process = |event: stores_app::Event| -> anyhow::Result<Vec<PartsView>> {
        let mut views = vec![];
        for effect in core.process_event(event) {
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
                    views.push(view);
                }
            }
        }
        Ok(views)
    };

    let source = parts_args.source.clone();
    match parts_args.command {
        PartsCommand::List => {
            process(stores_app::Event::LoadParts {
                source: source.clone(),
            })?;
            for view in process(stores_app::Event::RequestPartsView {
                source,
            })? {
                write_parts_csv(&view, std::io::stdout())?;
            }
        }
    }

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
        }
    }
    Ok(())
}
