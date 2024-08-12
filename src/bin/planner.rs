use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use anyhow::{bail, Error};
use clap::{Parser, Subcommand};
use csv::QuoteStyle;
use heck::ToShoutySnakeCase;
use regex::Regex;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use thiserror::Error;
use tracing::{debug, info, trace};
use makerpnp::{cli, planning, pnp};
use makerpnp::cli::args::{PcbKindArg, PcbSideArg, ProjectArgs};
use makerpnp::planning::design::{DesignName, DesignVariant};
use makerpnp::planning::reference::Reference;
use makerpnp::planning::pcb::Pcb;
use makerpnp::planning::phase::Phase;
use makerpnp::planning::placement::{PlacementSortingItem, PlacementSortingMode, PlacementState, PlacementStatus};
use makerpnp::planning::process::Process;
use makerpnp::planning::project::Project;
use makerpnp::planning::report;
use makerpnp::planning::report::{IssueKind, IssueSeverity, ProjectReportIssue};
use makerpnp::planning::variant::VariantName;
use makerpnp::pnp::load_out::LoadOutItem;
use makerpnp::pnp::object_path::{ObjectPath, UnitPath};
use makerpnp::pnp::part::Part;
use makerpnp::pnp::placement::Placement;
use makerpnp::stores::load_out;
use makerpnp::stores::placements::PlacementRecord;
use makerpnp::stores::load_out::LoadOutSource;
use makerpnp::util::sorting::SortOrder;

#[derive(Parser)]
#[command(name = "planner")]
#[command(bin_name = "planner")]
#[command(version, about, long_about = None)]
struct Opts {
    #[command(subcommand)]
    command: Command,

    /// Trace log file
    #[arg(long, num_args = 0..=1, default_missing_value = "trace.log", require_equals = true)]
    trace: Option<PathBuf>,

    /// Path
    #[arg(long, require_equals = true, default_value = ".")]
    path: PathBuf,

    // FUTURE find a way to define that project args are required when using a project specific sub-command
    //        without excessive code duplication
    #[command(flatten)]
    project_args: Option<ProjectArgs>,
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
enum Command {
    /// Create a new job
    Create {
    },
    /// Add a PCB
    AddPcb {
        /// PCB kind
        #[arg(long, require_equals = true)]
        kind: PcbKindArg,
        
        /// Name of the PCB, e.g. 'panel_1'
        #[arg(long, require_equals = true)]
        name: String,
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
    },
    /// Assign feeder to load-out item
    AssignFeederToLoadOutItem {
        /// Load-out source (e.g. 'load_out_1')
        #[arg(long, require_equals = true)]
        load_out: LoadOutSource,

        /// Feeder reference (e.g. 'FEEDER_1')
        #[arg(long, require_equals = true)]
        feeder_reference: Reference,

        /// Manufacturer pattern (regexp)
        #[arg(long, require_equals = true)]
        manufacturer: Regex,

        /// Manufacturer part number (regexp)
        #[arg(long, require_equals = true)]
        mpn: Regex,
    },
    /// Set placement ordering for a phase
    SetPlacementOrdering {
        /// Phase reference (e.g. 'top_1')
        #[arg(long, require_equals = true)]
        phase: Reference,

        /// Orderings (e.g. 'PCB_UNIT:ASC,FEEDER_REFERENCE:ASC')
        #[arg(long, num_args = 0.., require_equals = true, value_delimiter = ',', value_parser = makerpnp::cli::parsers::PlacementSortingItemParser::default())]
        orderings: Vec<PlacementSortingItem>
    },
    /// Generate artifacts
    GenerateArtifacts {
    }
}

// FUTURE consider merging the AssignProcessToParts and AssignLoadOutToParts commands
//        consider making a group for the criteria args (manufacturer/mpn/etc).

fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    cli::tracing::configure_tracing(opts.trace)?;

    match &opts.project_args {
        Some(ProjectArgs { project: name } ) if name.is_some() => {
            let name = name.as_ref().unwrap();
            let project_file_path = build_project_file_path(&name, &opts.path) ;

            match opts.command {
                Command::Create {} => {
                    let project = Project::new(name.to_string());
                    project_save(&project, &project_file_path)?;

                    info!("Created job: {}", project.name);
                },
                Command::AddPcb { kind, name } => {
                    let mut project = project_load(&project_file_path)?;

                    project_add_pcb(&mut project, kind, name)?;
                    
                    project_save(&project, &project_file_path)?;
                }
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
                Command::CreatePhase { process, reference, load_out, pcb_side: pcb_side_arg } => {
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

                    planning::load_out::add_parts_to_load_out(&phase.load_out, parts)?;

                    project_save(&project, &project_file_path)?;
                },
                Command::SetPlacementOrdering { phase: reference, orderings: sort_orderings } => {
                    let mut project = project_load(&project_file_path)?;

                    project_refresh_from_design_variants(&mut project, &opts.path)?;

                    let phase = match project.phases.get_mut(&reference) {
                        Some(phase) => Ok(phase),
                        None => Err(PlacementAssignmentError::UnknownPhase(reference.clone())),
                    }?;

                    phase.sort_orderings.clone_from(&sort_orderings);

                    trace!("Phase orderings set. phase: '{}', orderings: [{}]", reference, sort_orderings.iter().map(|item|{
                        format!("{}:{}",
                            item.mode.to_string().to_shouty_snake_case(),
                            item.sort_order.to_string().to_shouty_snake_case()
                        )
                    }).collect::<Vec<_>>().join(", "));

                    project_save(&project, &project_file_path)?;
                },
                Command::GenerateArtifacts { } => {
                    let project = project_load(&project_file_path)?;

                    project_generate_artifacts(&project, &opts.path, &name)?;
                },
                _ => {
                    bail!("invalid argument 'project'");
                }
            }
        },
        None => {
            match opts.command {
                Command::AssignFeederToLoadOutItem { load_out, feeder_reference, manufacturer, mpn } => {
                    planning::load_out::assign_feeder_to_load_out_item(load_out, feeder_reference, manufacturer, mpn)?;
                }
                _ => {
                    bail!("using a 'project' argument implies a project specific command should be used");
                }
            }
        },
        Some(_project_args) => {
            bail!("invalid arguments");
        }
    }

    Ok(())
}

#[derive(Error, Debug)]
pub enum PcbOperationError {
}

fn project_add_pcb(project: &mut Project, kind: PcbKindArg, name: String) -> Result<(), PcbOperationError> {
    project.pcbs.push(Pcb { kind: kind.clone().into(), name: name.clone() });
    
    match kind {
        PcbKindArg::Single =>  trace!("Added single PCB. name: '{}'", name),
        PcbKindArg::Panel => trace!("Added panel PCB. name: '{}'", name),
    }
    Ok(())
}

#[derive(Error, Debug)]
pub enum ArtifactGenerationError {
    #[error("Unable to generate phase placements. cause: {0:}")]
    PhasePlacementsGenerationError(Error),

    #[error("Unable to load items. source: {load_out_source}, error: {reason}")]
    UnableToLoadItems { load_out_source: LoadOutSource, reason: anyhow::Error },

    #[error("Unable to generate report. error: {reason}")]
    ReportGenerationError { reason: anyhow::Error },
}

fn project_generate_artifacts(project: &Project, path: &PathBuf, name: &String) -> Result<(), ArtifactGenerationError> {
    
    let mut issues: Vec<ProjectReportIssue> = vec![];
    
    for (_reference, phase) in project.phases.iter() {
        generate_phase_artifacts(project, phase, path, &mut issues)?;
    }
        
    report::project_generate_report(project, path, name, issues).map_err(|err|{
        ArtifactGenerationError::ReportGenerationError { reason: err.into() }
    })?;
    
    info!("Generated artifacts.");
    
    Ok(())
}


fn generate_phase_artifacts(project: &Project, phase: &Phase, path: &PathBuf, issues: &mut Vec<ProjectReportIssue>) -> Result<(), ArtifactGenerationError> {
    let load_out_items = load_out::load_items(&phase.load_out).map_err(|err|{
        ArtifactGenerationError::UnableToLoadItems { load_out_source: phase.load_out.clone(), reason: err }
    })?;

    let mut placement_states: Vec<(&ObjectPath, &PlacementState)> = project.placements.iter().filter_map(|(object_path, state)|{
        match &state.phase {
            Some(placement_phase) if placement_phase.eq(&phase.reference) => Some((object_path, state)),
            _ => None
        }
    }).collect();
    
    placement_states.sort_by(|(object_path_a, placement_state_a), (object_path_b, placement_state_b)|{
        phase.sort_orderings.iter().fold( Ordering::Equal, | mut acc, sort_ordering | {
            if !matches!(acc, Ordering::Equal) {
                return acc
            }
            acc = match sort_ordering.mode {
                PlacementSortingMode::FeederReference => {
                    let feeder_reference_a = match pnp::load_out::find_load_out_item_by_part(&load_out_items, &placement_state_a.placement.part) {
                        Some(load_out_item) => load_out_item.reference.clone(),
                        _ => "".to_string(),
                    };
                    let feeder_reference_b = match pnp::load_out::find_load_out_item_by_part(&load_out_items, &placement_state_b.placement.part) {
                        Some(load_out_item) => load_out_item.reference.clone(),
                        _ => "".to_string(),
                    };

                    trace!("Comparing feeder references. feeder_reference_a: '{}' feeder_reference_a: '{}'", feeder_reference_a, feeder_reference_b);
                    feeder_reference_a.cmp(&feeder_reference_b)
                },
                PlacementSortingMode::PcbUnit => {
                   
                    let pcb_unit_a = object_path_a.pcb_unit();
                    let pcb_unit_b = object_path_b.pcb_unit();
                    
                    trace!("Comparing pcb units, pcb_unit_a: '{}', pcb_unit_b: '{}'", pcb_unit_a, pcb_unit_b);
                    pcb_unit_a.cmp(&pcb_unit_b)
                },
            };
            
            match sort_ordering.sort_order {
                SortOrder::Asc => acc,
                SortOrder::Desc => {
                    acc.reverse()
                },
            }
        })
    });

    for (_object_path, placement_state) in placement_states.iter() {
        let feeder_reference = match pnp::load_out::find_load_out_item_by_part(&load_out_items, &placement_state.placement.part) {
            Some(load_out_item) => load_out_item.reference.clone(),
            _ => "".to_string(),
        };
        
        if feeder_reference.is_empty() {
            let issue = ProjectReportIssue {
                message: "A part has not been assigned to a feeder".to_string(),
                severity: IssueSeverity::Warning,
                kind: IssueKind::UnassignedPartFeeder { part: placement_state.placement.part.clone() },
            };
            info!("Issue detected. issue: {:?}", issue);
            issues.push(issue);
        };
    }

    let mut phase_placements_path = PathBuf::from(path);
    phase_placements_path.push(format!("{}_placements.csv", phase.reference));

    store_phase_placements_as_csv(&phase_placements_path, &placement_states, load_out_items.as_slice()).map_err(|e|{
        ArtifactGenerationError::PhasePlacementsGenerationError(e)
    })?;
    
    Ok(())
}

#[serde_as]
#[derive(Debug, serde::Serialize)]
#[serde(rename_all(serialize = "PascalCase"))]
pub struct PhasePlacementRecord {

    #[serde_as(as = "DisplayFromStr")]
    pub object_path: ObjectPath,
    
    pub feeder_reference: String,
    pub manufacturer: String,
    pub mpn: String,
    pub x: Decimal,
    pub y: Decimal,
    pub rotation: Decimal,
}

pub fn store_phase_placements_as_csv(output_path: &PathBuf, placement_states: &[(&ObjectPath, &PlacementState)], load_out_items: &[LoadOutItem]) -> Result<(), Error> {
    
    trace!("Writing phase placements. output_path: {:?}", output_path);

    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_path(output_path)?;

    for (object_path, placement_state) in placement_states.iter() {
        
        let feeder_reference = match pnp::load_out::find_load_out_item_by_part(&load_out_items, &placement_state.placement.part) {
            Some(load_out_item) => load_out_item.reference.clone(),
            _ => "".to_string(),
        };
        
        writer.serialize(
            PhasePlacementRecord {
                object_path: (*object_path).clone(),
                feeder_reference,
                manufacturer: placement_state.placement.part.manufacturer.to_string(),
                mpn: placement_state.placement.part.mpn.to_string(),
                x: placement_state.placement.x,
                y: placement_state.placement.y,
                rotation: placement_state.placement.rotation,
            }
        )?;
    }

    writer.flush()?;

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
        let path: ObjectPath = ObjectPath::try_from_unit_path_and_refdes(&unit_path, &placement.ref_des).expect("always ok");

        let placement_state_entry = project.placements.entry(path);

        match (change, placement) {
            (Change::New, placement) => {
                info!("New placement. placement: {:?}", placement);

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
                    info!("Updating placement. old: {:?}, new: {:?}", ps.placement, placement);
                    ps.placement = placement.clone();
                });
            }
            (Change::Unused, placement) => {
                info!("Marking placement as unused. placement: {:?}", placement);

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
                let path: ObjectPath = ObjectPath::try_from_unit_path_and_refdes(&unit_path, &placement.ref_des).expect("always ok");

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
