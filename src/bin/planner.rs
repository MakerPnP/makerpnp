use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use clap::{Parser, Subcommand, ValueEnum};
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, trace};
use makerpnp::cli;
use makerpnp::loaders::load_out;
use makerpnp::loaders::placements::PlacementRecord;
use makerpnp::planning::{DesignName, DesignVariant, LoadOutSource, PcbSide, Phase, PlacementState, PlacementStatus, Process, Project, Reference, UnitPath, VariantName};
use makerpnp::pnp::load_out_item::LoadOutItem;
use makerpnp::pnp::object_path::ObjectPath;
use makerpnp::pnp::part::Part;
use makerpnp::pnp::placement::Placement;
use crate::LoadOutError::{UnableToLoadItems, UnableToStoreItems};

#[derive(Parser)]
#[command(name = "planner")]
#[command(bin_name = "planner")]
#[command(version, about, long_about = None)]
struct Opts {
    #[command(subcommand)]
    command: Option<Command>,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log", require_equals = true)]
    trace: Option<PathBuf>,

    /// Path
    #[arg(long, require_equals = true, default_value = ".")]
    path: PathBuf,

    /// Job name
    #[arg(long, require_equals = true)]
    name: String,
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
enum Command {
    /// Create a new job
    Create {
    },
    /// Assign a design variant to a PCB unit
    AssignVariantToUnit {
        /// Name of the design
        #[arg(long, require_equals = true, value_parser = clap::value_parser!(DesignName), value_name = "DESIGN_NAME")]
        design: DesignName,

        /// Variant of the design
        #[arg(long, require_equals = true, value_parser = clap::value_parser!(VariantName), value_name = "VARIANT_NAME")]
        variant: VariantName,

        /// PCB unit path
        #[arg(long, require_equals = true, value_parser = clap::value_parser!(UnitPath), value_name = "UNIT_PATH")]
        unit: UnitPath,
    },
    /// Assign a process to parts
    AssignProcessToParts {
        /// Process name
        #[arg(long, require_equals = true)]
        process: Process,

        /// Manufacturer pattern (regexp)
        #[arg(long, require_equals = true)]
        manufacturer: Regex,

        /// Manufacturer part number (regexp)
        #[arg(long, require_equals = true)]
        mpn: Regex,
    },
    /// Create a phase
    CreatePhase {
        /// Process name
        #[arg(long, require_equals = true)]
        process: Process,
        
        /// Phase reference (e.g. 'top_1')
        #[arg(long, require_equals = true)]
        reference: Reference,
        
        /// Load-out source (e.g. 'load_out_1')
        #[arg(long, require_equals = true)]
        load_out: LoadOutSource,

        /// PCB side
        #[arg(long, require_equals = true)]
        pcb_side: PcbSideArg,
    },
    /// Assign placements to a phase
    AssignPlacementsToPhase {
        /// Phase reference (e.g. 'top_1')
        #[arg(long, require_equals = true)]
        phase: Reference,

        /// Placements pattern (regexp)
        #[arg(long, require_equals = true)]
        placements: Regex,
    }
}

#[derive(ValueEnum, Clone)]
#[value(rename_all = "lower")]
enum PcbSideArg {
    Top,
    Bottom,
}

impl From<PcbSideArg> for PcbSide {
    fn from(value: PcbSideArg) -> Self {
        match value {
            PcbSideArg::Top => Self::Top,
            PcbSideArg::Bottom => Self::Bottom,
        }
    }
}

// FUTURE consider merging the AssignProcessToParts and AssignLoadOutToParts commands
//        consider making a group for the criteria args (manufacturer/mpn/etc).

fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    cli::tracing::configure_tracing(opts.trace)?;

    let project_file_path = build_project_file_path(&opts.name, &opts.path);

    // TODO print help if no command specified, currently this panics
    match opts.command.unwrap() {
        Command::Create {} => {
            let project = Project::new(opts.name.to_string());
            project_save(&project, &project_file_path)?;

            info!("Created job: {}", project.name);
        },
        Command::AssignVariantToUnit { design, variant, unit } => {
            let mut project = project_load(&project_file_path)?;
            
            project.update_assignment(unit.clone(), DesignVariant { design_name: design.clone(), variant_name: variant.clone() })?;

            project_refresh_from_design_variants(&mut project, &opts.path)?;

            project_save(&project, &project_file_path)?;
        },
        Command::AssignProcessToParts { process, manufacturer: manufacturer_pattern, mpn: mpn_pattern } => {
            let mut project = project_load(&project_file_path)?;

            // TODO validate that process is a process used by the project

            let all_parts = project_refresh_from_design_variants(&mut project, &opts.path)?;

            project_update_applicable_processes(&mut project, all_parts.as_slice(), process, manufacturer_pattern, mpn_pattern);

            project_save(&project, &project_file_path)?;
        },
        Command::CreatePhase { process, reference, load_out , pcb_side: pcb_side_arg } => {
            let mut project = project_load(&project_file_path)?;

            let pcb_side = pcb_side_arg.into();

            project.update_phase(reference, process, load_out, pcb_side)?;

            project_save(&project, &project_file_path)?;
        },
        Command::AssignPlacementsToPhase { phase: reference, placements: placements_pattern } => {
            let mut project = project_load(&project_file_path)?;

            project_refresh_from_design_variants(&mut project, &opts.path)?;

            let phase = match project.phases.get(&reference) {
                Some(phase) => Ok(phase),
                None => Err(PlacementAssignmentError::UnknownPhase(reference.clone())),
            }?.clone();

            let parts = assign_placements_to_phase(&mut project, &phase, placements_pattern);
            trace!("Required load_out parts: {:?}", parts);
            
            add_parts_to_phase_load_out(&phase, parts)?;

            project_save(&project, &project_file_path)?;
        }
    }

    Ok(())
}

#[derive(Error, Debug)]
pub enum LoadOutError {
    #[error("Unable to load items. source: {load_out_source}, error: {reason}")]
    UnableToLoadItems { load_out_source: LoadOutSource, reason: anyhow::Error },
    
    #[error("Unable to store items. source: {load_out_source}, error: {reason}")]
    UnableToStoreItems { load_out_source: LoadOutSource, reason: anyhow::Error },
}

fn add_parts_to_phase_load_out(phase: &Phase, parts: BTreeSet<Part>) -> Result<(), LoadOutError> {
    info!("Loading load-out. source: '{}'", phase.load_out);

    let mut load_out_items = load_out::load_items(&phase.load_out.to_string()).map_err(|err|{
        UnableToLoadItems { load_out_source: phase.load_out.clone(), reason: err }
    })?;
    
    for part in parts.iter() {
        trace!("Checking for part in load_out. part: {:?}", part);
        
        let matched = load_out_items.iter().find(|load_out_item|{
            load_out_item.manufacturer.eq(&part.manufacturer)
                && load_out_item.mpn.eq(&part.mpn)
        });
        
        if matched.is_some() {
            continue
        }
        
        let load_out_item = LoadOutItem {
            reference: "".to_string(),
            manufacturer: part.manufacturer.clone(),
            mpn: part.mpn.clone(),
        };
        
        load_out_items.push(load_out_item)
    }

    info!("Storing load-out. source: '{}'", phase.load_out);

    load_out::store_items(&phase.load_out, &load_out_items).map_err(|err|{
        UnableToStoreItems { load_out_source: phase.load_out.clone(), reason: err }
    })?;
 
    Ok(())
}

#[derive(Error, Debug)]
pub enum PlacementAssignmentError {
    #[error("Unknown phase. phase: '{0:}'")]
    UnknownPhase(Reference)
}

fn assign_placements_to_phase(project: &mut Project, phase: &Phase, placements_pattern: Regex) -> BTreeSet<Part> {
    let mut unique_assigned_parts= BTreeSet::new();

    for (placement_path, state) in project.placements.iter_mut().filter(|(path, state)| {
        let path_str = format!("{}", path);

        placements_pattern.is_match(&path_str) &&
            state.placement.pcb_side.eq(&phase.pcb_side)
    }) {
        let should_assign = match &state.phase {
            Some(other) if !other.eq(&phase.reference) => true,
            None => true,
            _ => false,
        };

        if should_assign {
            info!("Assigning placement to phase. phase: {}, placement_path: {}", phase.reference, placement_path);
            state.phase = Some(phase.reference.clone());
            let _inserted = unique_assigned_parts.insert(state.placement.part.clone());
        }
    }

    unique_assigned_parts
}

fn project_refresh_from_design_variants(project: &mut Project, path: &PathBuf) -> anyhow::Result<Vec<Part>> {
    let unique_design_variants = build_unique_design_variants(project);
    let design_variant_placement_map = load_all_placements(unique_design_variants.as_slice(), path)?;

    let unique_parts = build_unique_parts(&design_variant_placement_map);

    project_refresh_parts(project, unique_parts.as_slice());

    project_refresh_placements(project, &design_variant_placement_map);

    Ok(unique_parts)
}

fn project_refresh_placements(project: &mut Project, design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>) {
    let changes: Vec<(Change, UnitPath, Placement)> = find_placement_changes(project, design_variant_placement_map);

    for (change, unit_path, placement) in changes.iter() {
        let mut path: ObjectPath = ObjectPath::from_str(&unit_path.to_string()).expect("always ok");
        path.push("ref_des".to_string(), placement.ref_des.clone());

        let placement_state_entry = project.placements.entry(path);

        match (change, placement) {
            (Change::New, placement) => {
                debug!("New placement. placement: {:?}", placement);

                let placement_state = PlacementState {
                    unit_path: unit_path.clone(),
                    placement: placement.clone(),
                    placed: false,
                    status: PlacementStatus::Known,
                    phase: None,
                };

                placement_state_entry.or_insert(placement_state);
            }
            (Change::Existing, _) => {
                placement_state_entry.and_modify(|ps| {
                    ps.placement = placement.clone();
                });
            }
            (Change::Unused, placement) => {
                debug!("Marking placement as unused. placement: {:?}", placement);

                placement_state_entry.and_modify(|ps|{
                    ps.status = PlacementStatus::Unknown;
                });
            }
        }
    }
}

fn find_placement_changes(project: &mut Project, design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>) -> Vec<(Change, UnitPath, Placement)> {
    let mut changes: Vec<(Change, UnitPath, Placement)> = vec![];

    // find new or existing placements that are in the updated design_variant_placement_map

    for (design_variant, placements) in design_variant_placement_map.iter() {

        for (unit_path, assignment_design_variant) in project.unit_assignments.iter() {
            if !design_variant.eq(assignment_design_variant) {
                continue
            }

            for placement in placements {
                let mut path: ObjectPath = ObjectPath::from_str(&unit_path.to_string()).expect("always ok");
                path.push("ref_des".to_string(), placement.ref_des.clone());

                // look for a placement state for the placement for this object path

                match project.placements.contains_key(&path) {
                    true => changes.push((Change::Existing, unit_path.clone(), placement.clone())),
                    false => changes.push((Change::New, unit_path.clone(), placement.clone())),
                }
            }
        }
    }

    // find the placements that we knew about previously, but that are no-longer in the design_variant_placement_map

    for (path, state) in project.placements.iter_mut() {

        for (unit_path, design_variant) in project.unit_assignments.iter() {

            let path_str = path.to_string();
            let unit_path_str = unit_path.to_string();
            let is_matched_unit = path_str.starts_with(&unit_path_str);
            trace!("path_str: {}, unit_path_str: {}, is_matched_unit: {}", path_str, unit_path_str, is_matched_unit);

            if is_matched_unit {
                if let Some(placements) = design_variant_placement_map.get(design_variant) {
                    match placements.iter().find(|placement| placement.ref_des.eq(&state.placement.ref_des)) {
                        Some(_) => {
                            trace!("known placement");
                        }
                        None => {
                            trace!("unknown placement");
                            match state.status {
                                PlacementStatus::Unknown => (),
                                PlacementStatus::Known => changes.push((Change::Unused, unit_path.clone(), state.placement.clone())),
                            }
                        }
                    }
                }
            }
        }
    }

    info!("placement changes:\n{:?}", changes);

    changes
}

#[derive(Debug)]
enum Change {
    New,
    Existing,
    Unused,
}

fn project_refresh_parts(project: &mut Project, all_parts: &[Part]) {
    let changes = find_part_changes(project, all_parts);

    for change_item in changes.iter() {
        match change_item {
            (Change::New, part) => {
                debug!("New part. part: {:?}", part);
                let _ = project.part_states.entry(part.clone()).or_default();
            }
            (Change::Existing, _) => {}
            (Change::Unused, part) => {
                debug!("Removing previously part. part: {:?}", part);
                let _ = project.part_states.remove(&part);
            }
        }
    }
}

fn find_part_changes(project: &mut Project, all_parts: &[Part]) -> Vec<(Change, Part)> {
    let mut changes: Vec<(Change, Part)> = vec![];

    for part in all_parts.iter() {
        match project.part_states.contains_key(part) {
            true => changes.push((Change::Existing, part.clone())),
            false => changes.push((Change::New, part.clone())),
        }
    }

    for (part, _process) in project.part_states.iter() {
        if !all_parts.contains(part) {
            changes.push((Change::Unused, part.clone()))
        }
    }

    info!("part changes:\n{:?}", changes);

    changes
}

// TODO currently only supports adding a process, add support for removing a process too.
fn project_update_applicable_processes(project: &mut Project, all_parts: &[Part], process: Process, manufacturer_pattern: Regex, mpn_pattern: Regex) {

    let changes = find_part_changes(project, all_parts);

    for change in changes.iter() {
        match change {
            (Change::Existing, part) => {
                if manufacturer_pattern.is_match(part.manufacturer.as_str()) && mpn_pattern.is_match(part.mpn.as_str()) {
                    project.part_states.entry(part.clone())
                        .and_modify(|v| {

                            let inserted = v.applicable_processes.insert(process.clone());

                            if inserted {
                                info!("Added process. part: {:?}, applicable_processes: {:?}", part, v.applicable_processes);
                            }
                        });
                }
            },
            _ => {
                panic!("unexpected change. change: {:?}", change);
            }
        }
    }
}

fn build_unique_design_variants(project: &Project) -> Vec<DesignVariant> {
    let unique_design_variants: Vec<DesignVariant> = project.unit_assignments.iter().fold(vec![], |mut acc, (_path, design_variant)| {
        if !acc.contains(design_variant) {
            acc.push(design_variant.clone())
        }

        acc
    });

    unique_design_variants
}

fn load_placements(placements_path: PathBuf) -> Result<Vec<Placement>, csv::Error>{
    let mut csv_reader = csv::ReaderBuilder::new().from_path(placements_path)?;

    let records = csv_reader.deserialize().into_iter()
        .inspect(|record| {
            trace!("{:?}", record);
        })
        .filter_map(|record: Result<PlacementRecord, csv::Error> | {
            // TODO report errors
            match record {
                Ok(record) => Some(record.as_placement()),
                _ => None
            }
        })
        .collect();

    Ok(records)
}

fn load_all_placements(unique_design_variants: &[DesignVariant], path: &PathBuf) -> anyhow::Result<BTreeMap<DesignVariant, Vec<Placement>>> {
    let mut all_placements: BTreeMap<DesignVariant, Vec<Placement>> = Default::default();

    for design_variant in unique_design_variants {
        let DesignVariant { design_name: design, variant_name: variant } = design_variant;

        let mut placements_path = PathBuf::from(path);
        placements_path.push(format!("{}_{}_placements.csv", design, variant));

        let placements = load_placements(placements_path)?;
        let _ = all_placements.insert(design_variant.clone(), placements);
    }

    Ok(all_placements)
}

fn build_unique_parts(design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>) -> Vec<Part> {

    let mut unique_parts: Vec<Part> = vec![];
    for placements in design_variant_placement_map.values() {

        for record in placements {
            if !unique_parts.contains(&record.part) {
                unique_parts.push(record.part.clone());
            }
        }
    }

    unique_parts
}

fn build_project_file_path(name: &str, path: &PathBuf) -> PathBuf {
    let mut project_file_path: PathBuf = path.clone();
    project_file_path.push(format!("project-{}.mpnp.json", name));
    project_file_path
}

fn project_load(project_file_path: &PathBuf) -> anyhow::Result<Project> {
    let project_file = File::open(project_file_path.clone())?;
    let mut de = serde_json::Deserializer::from_reader(project_file);
    let project = Project::deserialize(&mut de)?;
    Ok(project)
}

fn project_save(project: &Project, project_file_path: &PathBuf) -> anyhow::Result<()> {
    let project_file = File::create(project_file_path)?;
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(project_file, formatter);
    project.serialize(&mut ser)?;

    let mut project_file = ser.into_inner();
    project_file.write(b"\n")?;

    Ok(())
}
