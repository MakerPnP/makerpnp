use std::collections::btree_map::Entry;
use tracing::{debug, info, trace, warn};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::cmp::Ordering;
use thiserror::Error;
use anyhow::Error;
use indexmap::IndexSet;
use csv::QuoteStyle;
use std::fs::File;
use rust_decimal::Decimal;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Write;
use crate::stores::load_out;
use crate::stores::load_out::LoadOutSource;
use crate::planning::design::DesignVariant;
use crate::planning::reference::Reference;
use crate::planning::part::PartState;
use crate::planning::pcb::{Pcb, PcbKind, PcbSide};
use crate::planning::phase::Phase;
use crate::planning::placement::{PlacementOperation, PlacementSortingMode, PlacementState, PlacementStatus};
use crate::planning::process::Process;
use crate::planning::{placement, report};
use crate::planning::report::{IssueKind, IssueSeverity, ProjectReportIssue};
use crate::pnp;
use crate::pnp::load_out::LoadOutItem;
use crate::pnp::object_path::ObjectPath;
use crate::pnp::part::Part;
use crate::pnp::placement::Placement;
use crate::util::sorting::SortOrder;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Project {
    pub name: String,

    pub processes: Vec<Process>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub pcbs: Vec<Pcb>,

    #[serde_as(as = "Vec<(DisplayFromStr, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub unit_assignments: BTreeMap<ObjectPath, DesignVariant>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub part_states: BTreeMap<Part, PartState>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub phases: BTreeMap<Reference, Phase>,

    #[serde(skip_serializing_if = "IndexSet::is_empty")]
    #[serde(default)]
    pub phase_orderings: IndexSet<Reference>,

    #[serde_as(as = "Vec<(DisplayFromStr, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub placements: BTreeMap<ObjectPath, PlacementState>,
}

impl Project {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }

    pub fn ensure_process(&mut self, process: &Process) -> anyhow::Result<()> {
        if !self.processes.contains(process) {
            info!("Adding process to project.  process: '{process:}'");
            self.processes.push(process.clone())
        }
        Ok(())
    }

    pub fn update_assignment(&mut self, object_path: ObjectPath, design_variant: DesignVariant) -> anyhow::Result<()> {
        match self.unit_assignments.entry(object_path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(design_variant.clone());
                info!("Unit assignment added. unit: '{}', design_variant: {}", object_path, design_variant )
            }
            Entry::Occupied(mut entry) => {
                if entry.get().eq(&design_variant) {
                    info!("Unit assignment unchanged.")
                } else {
                    let old_value = entry.insert(design_variant.clone());
                    info!("Unit assignment updated. unit: '{}', old: {}, new: {}", object_path, old_value, design_variant )
                }
            }
        }

        Ok(())
    }

    pub fn update_phase(&mut self, reference: Reference, process: Process, load_out: LoadOutSource, pcb_side: PcbSide) -> anyhow::Result<()> {

        load_out::ensure_load_out(&load_out)?;
        
        match self.phases.entry(reference.clone()) {
            Entry::Vacant(entry) => {
                let phase = Phase { reference: reference.clone(), process: process.clone(), load_out: load_out.clone(), pcb_side: pcb_side.clone(), placement_orderings: vec![] };
                entry.insert(phase);
                info!("Created phase. reference: '{}', process: {}, load_out: {:?}", reference, process, load_out);
                self.phase_orderings.insert(reference);
            }
            Entry::Occupied(mut entry) => {
                let existing_phase = entry.get_mut();
                let old_phase = existing_phase.clone();

                existing_phase.process = process;
                existing_phase.load_out = load_out;

                info!("Updated phase. old: {:?}, new: {:?}", old_phase, existing_phase);
            }
        }

        Ok(())
    }
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_string(),
            processes: vec![Process::new("pnp"), Process::new("manual")],
            pcbs: vec![],
            unit_assignments: Default::default(),
            part_states: Default::default(),
            phases: Default::default(),
            placements: Default::default(),
            phase_orderings: Default::default(),
        }
    }
}

#[derive(Error, Debug)]
pub enum PcbOperationError {
}

pub fn add_pcb(project: &mut Project, kind: PcbKind, name: String) -> Result<(), PcbOperationError> {
    project.pcbs.push(Pcb { kind: kind.clone(), name: name.clone() });
    
    match kind {
        PcbKind::Single =>  trace!("Added single PCB. name: '{}'", name),
        PcbKind::Panel => trace!("Added panel PCB. name: '{}'", name),
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

pub fn generate_artifacts(project: &Project, path: &PathBuf, name: &String) -> Result<(), ArtifactGenerationError> {
    
    let mut issues: BTreeSet<ProjectReportIssue> = BTreeSet::new();
    
    let mut phase_load_out_items_map: BTreeMap<Reference, Vec<LoadOutItem>> = BTreeMap::new();
    
    for (reference, phase) in project.phases.iter() {
        let load_out_items = load_out::load_items(&phase.load_out).map_err(|err|{
            ArtifactGenerationError::UnableToLoadItems { load_out_source: phase.load_out.clone(), reason: err }
        })?;
        
        generate_phase_artifacts(project, phase, load_out_items.as_slice(), path, &mut issues)?;
        
        phase_load_out_items_map.insert(reference.clone(), load_out_items);
    }
        
    report::project_generate_report(project, path, name, &phase_load_out_items_map, &mut issues).map_err(|err|{
        ArtifactGenerationError::ReportGenerationError { reason: err.into() }
    })?;
    
    info!("Generated artifacts.");
    
    Ok(())
}

fn generate_phase_artifacts(project: &Project, phase: &Phase, load_out_items: &[LoadOutItem], path: &PathBuf, issues: &mut BTreeSet<ProjectReportIssue>) -> Result<(), ArtifactGenerationError> {
    let mut placement_states: Vec<(&ObjectPath, &PlacementState)> = project.placements.iter().filter_map(|(object_path, state)|{
        match &state.phase {
            Some(placement_phase) if placement_phase.eq(&phase.reference) => Some((object_path, state)),
            _ => None
        }
    }).collect();
    
    placement_states.sort_by(|(object_path_a, placement_state_a), (object_path_b, placement_state_b)|{
        phase.placement_orderings.iter().fold(Ordering::Equal, |mut acc, sort_ordering | {
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
            issues.insert(issue);
        };
    }

    let mut phase_placements_path = PathBuf::from(path);
    phase_placements_path.push(format!("{}_placements.csv", phase.reference));

    store_phase_placements_as_csv(&phase_placements_path, &placement_states, load_out_items).map_err(|e|{
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

pub fn assign_placements_to_phase(project: &mut Project, phase: &Phase, placements_pattern: Regex) -> BTreeSet<Part> {
    let mut required_load_out_parts = BTreeSet::new();

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
        }
        let _inserted = required_load_out_parts.insert(state.placement.part.clone());
    }

    required_load_out_parts
}

pub fn refresh_from_design_variants(project: &mut Project, path: &PathBuf) -> anyhow::Result<Vec<Part>> {
    let unique_design_variants = build_unique_design_variants(project);
    let design_variant_placement_map = placement::load_all_placements(unique_design_variants.as_slice(), path)?;

    let unique_parts = placement::build_unique_parts(&design_variant_placement_map);

    refresh_parts(project, unique_parts.as_slice());

    refresh_placements(project, &design_variant_placement_map);

    Ok(unique_parts)
}

fn refresh_placements(project: &mut Project, design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>) {
    let changes: Vec<(Change, ObjectPath, Placement)> = find_placement_changes(project, design_variant_placement_map);

    for (change, unit_path, placement) in changes.iter() {
        let mut path: ObjectPath = unit_path.clone();
        path.set_ref_des(placement.ref_des.clone());

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
                    if !ps.placement.eq(placement) {
                        info!("Updating placement. old: {:?}, new: {:?}", ps.placement, placement);
                        ps.placement = placement.clone();
                    }
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

fn find_placement_changes(project: &mut Project, design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>) -> Vec<(Change, ObjectPath, Placement)> {
    let mut changes: Vec<(Change, ObjectPath, Placement)> = vec![];

    // find new or existing placements that are in the updated design_variant_placement_map

    for (design_variant, placements) in design_variant_placement_map.iter() {

        for (unit_path, assignment_design_variant) in project.unit_assignments.iter() {
            if !design_variant.eq(assignment_design_variant) {
                continue
            }

            for placement in placements {
                let mut path: ObjectPath = unit_path.clone();
                path.set_ref_des(placement.ref_des.clone());

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

    debug!("placement changes:\n{:?}", changes);

    changes
}

#[derive(Debug)]
enum Change {
    New,
    Existing,
    Unused,
}

fn refresh_parts(project: &mut Project, all_parts: &[Part]) {
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

    debug!("part changes:\n{:?}", changes);

    changes
}

// TODO currently only supports adding a process, add support for removing a process too.
pub fn update_applicable_processes(project: &mut Project, all_parts: &[Part], process: Process, manufacturer_pattern: Regex, mpn_pattern: Regex) {

    let changes = find_part_changes(project, all_parts);

    for change in changes.iter() {
        match change {
            (Change::Existing, part) => {
                if manufacturer_pattern.is_match(part.manufacturer.as_str()) && mpn_pattern.is_match(part.mpn.as_str()) {
                    project.part_states.entry(part.clone())
                        .and_modify(|part_state| {
                            add_process_to_part(part_state, part, process.clone());
                        });
                }
            },
            _ => {
                panic!("unexpected change. change: {:?}", change);
            }
        }
    }
}

pub fn add_process_to_part(part_state: &mut PartState, part: &Part, process: Process) {
    let inserted = part_state.applicable_processes.insert(process);

    if inserted {
        info!("Added process. part: {:?}, applicable_processes: {:?}", part, part_state.applicable_processes);
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

pub fn build_project_file_path(name: &str, path: &PathBuf) -> PathBuf {
    let mut project_file_path: PathBuf = path.clone();
    project_file_path.push(format!("project-{}.mpnp.json", name));
    project_file_path
}

pub fn load(project_file_path: &PathBuf) -> anyhow::Result<Project> {
    let project_file = File::open(project_file_path.clone())?;
    let mut de = serde_json::Deserializer::from_reader(project_file);
    let project = Project::deserialize(&mut de)?;
    Ok(project)
}

pub fn save(project: &Project, project_file_path: &PathBuf) -> anyhow::Result<()> {
    let project_file = File::create(project_file_path)?;
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(project_file, formatter);
    project.serialize(&mut ser)?;

    let mut project_file = ser.into_inner();
    project_file.write(b"\n")?;

    Ok(())
}

pub fn update_placements_operation(project: &mut Project, object_path_patterns: Vec<Regex>, operation: PlacementOperation) -> anyhow::Result<bool> {

    let mut modified = false;
    
    for object_path_pattern in object_path_patterns.iter() {
        let placement = project.placements.iter_mut().find(|(object_path, _placement_state)|{
            object_path_pattern.is_match(&object_path.to_string())
        });
        
        match placement {
            Some((object_path, placement_state)) => {
                match operation {
                    PlacementOperation::Placed => {
                        if placement_state.placed {
                            warn!("Placed flag already set. object_path: {}", object_path);
                        } else {
                            info!("Setting placed flag. object_path: {}", object_path);
                            placement_state.placed = true;
                            modified = true;
                        }
                    }
                }
            },
            None => {
                warn!("Unmatched object path pattern. object_path_pattern: {}", object_path_pattern);
            }
        }
    }
    
    Ok(modified)
}

#[derive(Error, Debug)]
pub enum PartStateError {
    #[error("No part state found. manufacturer: {}, mpn: {}", part.manufacturer, part.mpn)]
    NoPartStateFound { part: Part }
}
