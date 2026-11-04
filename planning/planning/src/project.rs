use std::cmp::Ordering;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Error;
use csv::QuoteStyle;
use eda_units::eda_units::dimension_unit::{DimensionUnitVector2, DimensionUnitVector2Ext};
use eda_units::eda_units::unit_system::UnitSystem;
use heck::ToShoutySnakeCase;
use indexmap::IndexSet;
use pnp;
use pnp::load_out::LoadOutItem;
use pnp::object_path::ObjectPath;
use pnp::package::Package;
use pnp::part::Part;
use pnp::pcb::{PcbInstanceIndex, PcbInstanceNumber, PcbSide, PcbUnitIndex, PcbUnitNumber};
use pnp::placement::Placement;
use pnp::reference::Reference;
use regex::Regex;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use thiserror::Error;
use time::OffsetDateTime;
use tracing::{debug, info, trace, warn};
use util::sorting::SortOrder;
use util::source::Source;

use crate::actions::{AddOrRemoveAction, SetOrClearAction};
use crate::design::{DesignIndex, DesignName, DesignVariant};
use crate::file::FileReference;
use crate::library::LibraryConfig;
use crate::operation_history::{
    OperationHistoryItem, OperationHistoryKind, PlacementOperationHistoryKind, TaskOperationHistoryKind,
};
use crate::part::PartState;
use crate::pcb::{Pcb, PcbError, PcbUnitTransform, UnitPlacementPosition};
use crate::phase::{Phase, PhaseError, PhaseOrderings, PhaseReference, PhaseState};
use crate::placement::{
    PlacementOperation, PlacementSortingItem, PlacementSortingMode, PlacementState, PlacementStatus,
    ProjectPlacementStatus,
};
use crate::process::{
    can_modify_operation, can_modify_task, OperationReference, OperationStatus, ProcessDefinition, ProcessError,
    ProcessReference, SerializableTaskState, TaskAction, TaskKind, TaskReference, TaskStatus,
};
use crate::variant::VariantName;
use crate::{operation_history, placement, report};

#[serde_as]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Project {
    pub name: String,

    #[serde(default)]
    pub library_config: LibraryConfig,

    /// The processes used by this project, each a copy of the definition in its file.
    #[serde(default)]
    pub processes: Vec<ProjectProcess>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub pcbs: Vec<ProjectPcb>,

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

    /// The state of the phases, and the process operations
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub phase_states: BTreeMap<Reference, PhaseState>,

    #[serde_as(as = "Vec<(DisplayFromStr, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub placements: BTreeMap<ObjectPath, PlacementState>,
}

impl Project {
    pub fn new(name: String, package_source: Option<Source>, package_mappings_source: Option<Source>) -> Self {
        Self {
            name,
            library_config: LibraryConfig {
                package_source,
                package_mappings_source,
            },
            ..Self::default()
        }
    }

    /// Safety: Silently ignores errors when building unit assignments fails. e.g. pcb not loaded.
    ///
    /// FUTURE improve this so it returns a `Result` with an `Err` if one of the Pcbs has not been loaded.
    pub fn all_unit_assignments(&self, pcbs: &[&Pcb]) -> Vec<(ObjectPath, Option<DesignVariant>)> {
        self.pcbs
            .iter()
            .zip(pcbs)
            .enumerate()
            .flat_map(|(pcb_index, (project_pcb, pcb))| {
                project_pcb
                    .unit_assignments(pcb)
                    .unwrap_or_default()
                    .into_iter()
                    .enumerate()
                    .map(move |(unit_index, unit_assignment)| {
                        let mut object_path = ObjectPath::default();
                        object_path.set_pcb_instance(pcb_index as u16 + 1);
                        object_path.set_pcb_unit(unit_index as u16 + 1);

                        (object_path, unit_assignment)
                    })
            })
            .collect::<Vec<_>>()
    }

    /// Adds a copy of a process, from its file, to the project.
    pub fn add_process(
        &mut self,
        process_file: FileReference,
        definition: ProcessDefinition,
    ) -> Result<(), ProcessError> {
        definition
            .validate()
            .map_err(ProcessError::InvalidProcess)?;

        if self
            .processes
            .iter()
            .any(|process| matches!(&process.process_file, Some(candidate) if candidate.eq(&process_file)))
        {
            return Err(ProcessError::DuplicateProcessFile {
                process_file,
            });
        }

        if self
            .find_process(&definition.reference)
            .is_ok()
        {
            return Err(ProcessError::DuplicateProcessReference {
                process_reference: definition.reference,
            });
        }

        info!(
            "Added process. process: '{}', file: {}",
            definition.reference, process_file
        );
        self.processes.push(ProjectProcess {
            process_file: Some(process_file),
            definition,
        });

        Ok(())
    }

    /// Replaces the project's copy of a process with a newer definition, e.g. from its file.
    ///
    /// A process can't be changed while a phase that uses it is in progress.  If its reference changes, the phases and
    /// parts that use it are changed too.  The state of each phase that uses it is rebuilt from the new definition.
    ///
    /// Returns whether the project changed.
    pub fn replace_process(
        &mut self,
        process_reference: &ProcessReference,
        definition: ProcessDefinition,
    ) -> Result<bool, ProcessError> {
        let index = self
            .processes
            .iter()
            .position(|process| {
                process
                    .definition
                    .reference
                    .eq(process_reference)
            })
            .ok_or_else(|| self.undefined_process_error(process_reference))?;

        if self.processes[index]
            .definition
            .eq(&definition)
        {
            return Ok(false);
        }

        definition
            .validate()
            .map_err(ProcessError::InvalidProcess)?;

        let new_process_reference = definition.reference.clone();
        let renamed = !new_process_reference.eq(process_reference);
        if renamed
            && self
                .find_process(&new_process_reference)
                .is_ok()
        {
            return Err(ProcessError::DuplicateProcessReference {
                process_reference: new_process_reference,
            });
        }

        self.ensure_process_not_in_progress(process_reference)?;

        self.processes[index].definition = definition;

        if renamed {
            for (part, part_state) in self.part_states.iter_mut() {
                if part_state
                    .applicable_processes
                    .remove(process_reference)
                {
                    part_state
                        .applicable_processes
                        .insert(new_process_reference.clone());
                    info!(
                        "Renamed part's process. part: {:?}, old: '{}', new: '{}'",
                        part, process_reference, new_process_reference
                    );
                }
            }
        }

        let phases_to_update = self
            .phases
            .iter()
            .filter(|(_, phase)| phase.process.eq(process_reference))
            .map(|(phase_reference, phase)| (phase_reference.clone(), phase.load_out_source.clone(), phase.pcb_side))
            .collect::<Vec<_>>();

        for (phase_reference, load_out_source, pcb_side) in phases_to_update {
            self.update_phase(
                phase_reference,
                new_process_reference.clone(),
                load_out_source,
                pcb_side,
            )
            .map_err(|_cause| self.undefined_process_error(&new_process_reference))?;
        }

        info!(
            "Updated process. process: '{}', new reference: '{}'",
            process_reference, new_process_reference
        );

        Ok(true)
    }

    /// The kind of a task of a phase's process.
    pub fn find_task_kind(
        &self,
        phase_reference: &PhaseReference,
        operation_reference: &OperationReference,
        task_reference: &TaskReference,
    ) -> Option<TaskKind> {
        let phase = self.phases.get(phase_reference)?;
        let process = self.find_process(&phase.process).ok()?;

        process
            .find_task(operation_reference, task_reference)
            .map(|task| task.kind)
    }

    /// makes the assignment if possible.
    ///
    /// returns if the assignment was modified (added or changed) or an error.
    pub fn update_assignment(
        &mut self,
        pcbs: &[&Pcb],
        object_path: ObjectPath,
        variant_name: Option<VariantName>,
    ) -> anyhow::Result<bool> {
        // reminder: instance and pcb_unit are 1-based in the object path

        let Ok((pcb_instance, pcb_unit)) = object_path.pcb_instance_and_unit() else {
            return Err(anyhow::anyhow!(
                "Unable to determine PCB instance and unit from object path: {:?}",
                object_path
            ));
        };

        let pcb_instance_index: u16 = pcb_instance - 1;
        let pcb_unit_index: u16 = pcb_unit - 1;

        let (Some(project_pcb), Some(pcb)) = (
            self.pcbs
                .get_mut(pcb_instance_index as usize),
            pcbs.get(pcb_instance_index as usize),
        ) else {
            return Err(anyhow::anyhow!("Unable to find PCB. instance: {}", pcb_instance));
        };

        let modified = match (
            project_pcb.assign_unit(pcb, pcb_unit_index, variant_name.clone()),
            variant_name,
        ) {
            (Ok(None), Some(variant_name)) => {
                info!(
                    "Unit assignment added. unit: '{}', variant_name: {}",
                    object_path, variant_name
                );
                true
            }
            (Ok(Some(old_design_variant)), Some(variant_name)) => {
                info!(
                    "Unit assignment updated. unit: '{}', old: {}, new: {}",
                    object_path, old_design_variant, variant_name
                );
                true
            }
            (Ok(Some(old_design_variant)), None) => {
                info!(
                    "Unit assignment removed. unit: '{}', old: {}",
                    object_path, old_design_variant
                );
                true
            }
            (Ok(None), None) => unreachable!(),
            (
                Err(ProjectPcbError::UnitAlreadyAssigned {
                    ..
                }),
                Some(_),
            ) => {
                info!("Unit already assigned.");
                false
            }
            (
                Err(ProjectPcbError::UnitNotAssignedToAVariant {
                    ..
                }),
                None,
            )
            | (
                Err(ProjectPcbError::UnitNotAssignedToADesign {
                    ..
                }),
                None,
            ) => {
                info!("Unit not assigned.");
                false
            }
            (Err(cause), _) => return Err(anyhow::anyhow!("Unable to assign unit to PCB. cause: {:?}", cause)),
        };

        Ok(modified)
    }

    /// Update a phase
    ///
    /// Call when changing the process, load-out source or pcb_side.
    ///
    /// Safety: The phase must not be in-progress.
    pub fn update_phase(
        &mut self,
        reference: PhaseReference,
        process_reference: ProcessReference,
        load_out_source: String,
        pcb_side: PcbSide,
    ) -> Result<(), PhaseError> {
        let process = self
            .find_process(&process_reference)
            .map_err(|_cause| PhaseError::UnknownProcess(process_reference.clone()))?;
        let phase_state = PhaseState::from_process(process);

        match self.phases.entry(reference.clone()) {
            Entry::Vacant(entry) => {
                let phase = Phase {
                    reference: reference.clone(),
                    process: process_reference.clone(),
                    load_out_source: load_out_source.clone(),
                    pcb_side,
                    placement_orderings: vec![],
                };
                entry.insert(phase);
                info!(
                    "Created phase. reference: '{}', process: {}, load_out: {:?}",
                    reference, process_reference, load_out_source
                );
                self.phase_orderings
                    .insert(reference.clone());
                info!("Phase ordering: {}", PhaseOrderings(&self.phase_orderings));

                self.phase_states
                    .insert(reference, phase_state);
            }
            Entry::Occupied(mut entry) => {
                let existing_phase = entry.get_mut();
                let old_phase = existing_phase.clone();

                existing_phase.process = process_reference;
                // FIXME if the load out source changed ensure the loadout contains all the parts assigned to the phase
                existing_phase.load_out_source = load_out_source;

                let _old_state = self
                    .phase_states
                    .insert(reference, phase_state);

                info!("Updated phase. old: {:?}, new: {:?}", old_phase, existing_phase);
            }
        }

        Ok(())
    }

    /// Deletes a phase and unassigns the phase from all placements using it.
    ///
    /// If the phase is not 'pending' it is considered in-use and cannot be deleted.
    ///
    /// After deleting a phase, artifacts will need re-generating.
    pub fn delete_phase(&mut self, reference: PhaseReference) -> Result<(), PhaseError> {
        if !self.phases.contains_key(&reference) {
            return Err(PhaseError::UnknownPhase(reference.clone()));
        }

        let phase_state = self
            .phase_states
            .get(&reference)
            .unwrap();
        if !phase_state.is_pending() {
            return Err(PhaseError::PhaseInUse(reference.clone()));
        }

        let _phase = self.phases.remove(&reference).unwrap();
        let _phase_state = self
            .phase_states
            .remove(&reference)
            .unwrap();

        for (_path, state) in self.placements.iter_mut() {
            if matches!(&state.phase, Some(assigned_phase_reference) if assigned_phase_reference.eq(&reference)) {
                state.phase.take();
            }
        }

        self.phase_orderings
            .shift_remove(&reference);

        Ok(())
    }

    pub fn can_start_phase(&self, phase_reference: &PhaseReference) -> bool {
        let mut log = vec![];
        let mut can_start_phase = true;

        for phase in &self.phase_orderings {
            let phase_state = self.phase_states.get(phase).unwrap();

            if phase.eq(phase_reference) {
                let is_pending = phase_state.is_pending();
                log.push((phase.clone(), if is_pending { "pending" } else { "not pending" }));

                can_start_phase &= is_pending;
                // no need to check subsequent phases
                break;
            }

            let is_complete = phase_state.is_complete();
            log.push((phase.clone(), if is_complete { "complete" } else { "not complete" }));
            can_start_phase &= is_complete;

            if !can_start_phase {
                // no need to check subsequent phases if a preceding phase is not complete.
                break;
            }
        }

        debug!(
            "can start phase: {}. phase: {}, log: {:?}",
            can_start_phase, phase_reference, log
        );

        can_start_phase
    }

    pub fn find_process(&self, process_reference: &ProcessReference) -> Result<&ProcessDefinition, ProcessError> {
        self.find_project_process(process_reference)
            .map(|process| &process.definition)
    }

    pub fn find_project_process(&self, process_reference: &ProcessReference) -> Result<&ProjectProcess, ProcessError> {
        self.processes
            .iter()
            .find(|&process| {
                process
                    .definition
                    .reference
                    .eq(&process_reference)
            })
            .ok_or_else(|| self.undefined_process_error(process_reference))
    }

    fn undefined_process_error(&self, process_reference: &ProcessReference) -> ProcessError {
        ProcessError::UndefinedProcessError {
            processes: self
                .processes
                .iter()
                .map(|process| process.definition.reference.clone())
                .collect(),
            process: process_reference.to_string(),
        }
    }

    /// Removes a process from the project.  Its file is not deleted.
    ///
    /// A process used by a phase can't be removed.  Parts keep the process assigned to them, so that adding the process
    /// again restores them.
    pub fn remove_process(&mut self, process_reference: &ProcessReference) -> Result<(), ProcessError> {
        self.find_process(process_reference)?;
        self.ensure_process_not_in_use(process_reference)?;
        self.processes.retain(|process| {
            !process
                .definition
                .reference
                .eq(process_reference)
        });
        info!("Removed process. process: '{}'", process_reference);

        Ok(())
    }

    /// Check to see if the process is in-use by a phase
    /// Returns Err if it is, otherwise Ok
    ///
    /// Note: The given process may not exist
    pub fn ensure_process_not_in_use(&self, process_reference: &ProcessReference) -> Result<(), ProcessError> {
        let in_use = self
            .phases
            .iter()
            .any(|(_, phase)| phase.process.eq(process_reference));

        if in_use {
            Err(ProcessError::ProcessInUse {
                process_reference: process_reference.clone(),
            })
        } else {
            Ok(())
        }
    }

    /// Check to see if the process is in-use by a phase that is in-progress
    /// Returns Err if it is, otherwise Ok
    ///
    /// Note: The given process may not exist
    pub fn ensure_process_not_in_progress(&self, process_reference: &ProcessReference) -> Result<(), ProcessError> {
        let in_progress = self
            .phase_states
            .iter()
            .zip(self.phases.iter())
            .any(|((_, phase_state), (_, phase))| {
                phase.process.eq(process_reference)
                    && phase_state
                        .operation_states
                        .iter()
                        .any(|os| {
                            os.task_states
                                .iter()
                                .any(|(_, ts)| !ts.is_pending())
                        })
            });

        if in_progress {
            Err(ProcessError::ProcessInProgress {
                process_reference: process_reference.clone(),
            })
        } else {
            Ok(())
        }
    }
    /// Warning: Silently ignores errors when building unit assignments fails. e.g. pcb not loaded.
    ///
    /// FUTURE improve this so it returns a `Result` with an `Err` if one of the Pcbs has not been loaded.
    pub fn unique_design_variants(&self, pcbs: &[&Pcb]) -> HashSet<DesignVariant> {
        self.pcbs
            .iter()
            .zip(pcbs)
            .filter_map(|(project_pcb, pcb)| project_pcb.unit_assignments(pcb).ok())
            .flat_map(|unit_assignments| unit_assignments)
            .flatten()
            .collect()
    }

    #[must_use]
    pub fn remove_unused_placements(&mut self, phase_reference: Option<Reference>) -> bool {
        let mut modified = false;

        self.placements
            .retain(|object_path, state| match state.project_status {
                ProjectPlacementStatus::Unused => {
                    let should_remove = match (&phase_reference, &state.phase) {
                        (None, _) => true,
                        (Some(phase), Some(candidate)) if phase.eq(candidate) => true,
                        _ => false,
                    };

                    if should_remove {
                        info!("Removing unknown placement, object_path: {:?}", object_path);
                        modified |= true;
                    }
                    !should_remove
                }
                _ => true,
            });

        modified
    }
}

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ProjectPcb {
    pub pcb_file: FileReference,

    /// Individual units can have a design variant assigned.
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub unit_assignments: BTreeMap<PcbUnitIndex, DesignVariant>,
}

/// A process used by a project: a copy of its definition, and the file it was copied from.
///
/// The copy is only updated from the file when the user asks, see [`Project::replace_process`], since a phase's state is
/// built from its process's definition.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ProjectProcess {
    /// Always present, except in projects from before processes had files, which are given files when they're loaded,
    /// see [`Project::processes_without_files`].
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_file: Option<FileReference>,

    #[serde(flatten)]
    pub definition: ProcessDefinition,
}

impl Project {
    /// The indexes of the processes that have no file, see [`ProjectProcess::process_file`].
    pub fn processes_without_files(&self) -> Vec<usize> {
        self.processes
            .iter()
            .enumerate()
            .filter(|(_, process)| process.process_file.is_none())
            .map(|(index, _)| index)
            .collect()
    }
}

impl ProjectPcb {
    pub fn new(pcb_file: FileReference) -> Self {
        Self {
            pcb_file,
            unit_assignments: BTreeMap::default(),
        }
    }

    /// Returns an vector of optional design variant assignments, one for each unit of the pcb.
    /// The assignment for a pcb unit can be None when the design name is unknown or no assignment is made.
    pub fn unit_assignments(&self, pcb: &Pcb) -> Result<Vec<Option<DesignVariant>>, ProjectPcbError> {
        let mut unit_assignments = vec![None; pcb.units as usize];

        for (unit_index, design_variant) in self.unit_assignments.iter() {
            // it's possible that the design variant is no-longer in the PCB
            if pcb
                .design_names
                .contains(&design_variant.design_name)
            {
                unit_assignments[*unit_index as usize] = Some(design_variant.clone())
            }
        }

        Ok(unit_assignments)
    }

    /// Makes an assignment, or un-assigns a unit
    ///
    /// unit is 0-based
    ///
    /// Returns the previous assignment, which may be `None`
    ///
    /// Returns an error
    /// * if the assignment has already been made
    /// * if the unit is out of range
    pub fn assign_unit(
        &mut self,
        pcb: &Pcb,
        unit: u16,
        variant_name: Option<VariantName>,
    ) -> Result<Option<DesignVariant>, ProjectPcbError> {
        if unit >= pcb.units {
            return Err(ProjectPcbError::UnitOutOfRange {
                unit,
                min: 0,
                max: pcb.units - 1,
            });
        }

        let design_index: DesignIndex = *pcb
            .unit_map
            .get(&unit)
            .ok_or(ProjectPcbError::UnitNotAssignedToADesign {
                unit,
            })?;

        if let Some(variant_name) = variant_name {
            let design_name = pcb.design_names[design_index as usize].clone();
            let design_variant = DesignVariant {
                design_name,
                variant_name,
            };

            match self.unit_assignments.entry(unit) {
                Entry::Vacant(entry) => {
                    entry.insert(design_variant);
                    Ok(None)
                }
                Entry::Occupied(mut entry) => {
                    let other_design_variant = entry.get();
                    if other_design_variant.eq(&design_variant) {
                        return Err(ProjectPcbError::UnitAlreadyAssigned {
                            unit,
                        });
                    }

                    let old_assigment = entry.insert(design_variant);

                    Ok(Some(old_assigment))
                }
            }
        } else {
            let old_assignment = self.unit_assignments.remove(&unit);
            if old_assignment.is_none() {
                return Err(ProjectPcbError::UnitNotAssignedToAVariant {
                    unit,
                });
            }
            Ok(old_assignment)
        }
    }
}

#[derive(Debug, Error)]
pub enum ProjectPcbError {
    #[error("Unit {unit} is out of range [{min}..{max}] (inclusive)")]
    UnitOutOfRange { unit: u16, min: u16, max: u16 },
    #[error("Unit {unit} has already been assigned")]
    UnitAlreadyAssigned { unit: u16 },

    #[error("Unknown design. design: {0}.  Assign the design to the PCB.")]
    UnknownDesign(DesignName),

    #[error("Project PCB not loaded")]
    NotLoaded,
    #[error("Unit {unit} has not been assigned to a design.")]
    UnitNotAssignedToADesign { unit: u16 },
    #[error("Unit {unit} has not been assigned to a design variant.")]
    UnitNotAssignedToAVariant { unit: u16 },
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_string(),
            processes: vec![],
            pcbs: vec![],
            part_states: Default::default(),
            phases: Default::default(),
            placements: Default::default(),
            phase_orderings: Default::default(),
            phase_states: Default::default(),
            library_config: Default::default(),
        }
    }
}

#[derive(Error, Debug)]
pub enum PcbOperationError {
    #[error("Unknown error")]
    Unknown,
    #[error("PCB not loaded")]
    PcbNotLoaded,
    #[error("PCB error. cause: {0}")]
    PcbError(PcbError),

    #[error("Invalid design set. Entries must be unique.")]
    InvalidDesignSet,
    #[error("Invalid instance index.")]
    InvalidInstanceIndex,

    #[error("Design sizing count mismatch. expected: {expected}, actual: {actual}")]
    DesignSizingCountMismatch { expected: usize, actual: usize },
    #[error("Unit sizing count mismatch. expected: {expected}, actual: {actual}")]
    UnitSizingCountMismatch { expected: u16, actual: u16 },
    #[error("Missing design sizing. design: {0}")]
    MissingDesignSizing(String),
    #[error("Missing unit sizing. unit: {0}")]
    MissingPcbUnitPositioning(PcbUnitIndex),
    #[error("PCB is in use.")]
    PcbInUse,
}

pub fn add_pcb(project: &mut Project, pcb_file: &FileReference) -> Result<(), PcbOperationError> {
    info!("Added PCB to project. pcb_file: {}", pcb_file);

    project
        .pcbs
        .push(ProjectPcb::new(pcb_file.clone()));

    Ok(())
}

pub fn remove_pcb(
    project: &mut Project,
    pcb_instance_index: PcbInstanceIndex,
    project_directory: &PathBuf,
) -> Result<PathBuf, PcbOperationError> {
    if let Some(pcb) = project
        .pcbs
        .get(pcb_instance_index as usize)
    {
        if !pcb.unit_assignments.is_empty() {
            return Err(PcbOperationError::PcbInUse);
        }
    } else {
        return Err(PcbOperationError::InvalidInstanceIndex);
    }

    let pcb = project
        .pcbs
        .remove(pcb_instance_index as usize);

    info!("Removed PCB from project. pcb_file: {}", pcb.pcb_file);

    let path = pcb
        .pcb_file
        .build_path(project_directory);

    Ok(path)
}

#[derive(Error, Debug)]
pub enum ArtifactGenerationError {
    #[error("Unable to generate phase placements. cause: {0:}")]
    PhasePlacementsGenerationError(Error),

    #[error("Unable to load items. source: {load_out_source}, error: {reason}")]
    UnableToLoadItems { load_out_source: String, reason: Error },

    #[error("Unable to generate report. error: {reason}")]
    ReportGenerationError { reason: Error },

    #[error("Unable to save report. cause: {reason:}")]
    UnableToSaveReport { reason: Error },
}

/// A file made by generating the artifacts, to be written by the stores service (`STORE-12`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub path: PathBuf,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ArtifactKind {
    PhasePlacements { phase: PhaseReference },
    JsonReport,
    MarkdownReport,
}

/// The artifacts, in the order they're written: each phase's placements, in the phases' order, then the report.
///
/// Nothing is written, so a core can have the stores service write them (`STORE-12`).
pub fn generate_artifacts(
    project: &Project,
    pcbs: &[&Pcb],
    directory: &Path,
    phase_load_out_items_map: BTreeMap<Reference, Vec<LoadOutItem>>,
    part_packages: &BTreeMap<&Part, &Package>,
) -> Result<Vec<Artifact>, ArtifactGenerationError> {
    let mut artifacts = vec![];

    for reference in project.phase_orderings.iter() {
        let phase = project.phases.get(reference).unwrap();

        let load_out_items = phase_load_out_items_map
            .get(reference)
            .unwrap();

        let phase_placement_states = build_phase_placement_states(project, reference);

        artifacts.push(generate_phase_artifacts(
            pcbs,
            phase,
            load_out_items.as_slice(),
            part_packages,
            directory,
            &phase_placement_states,
        )?);
    }

    let report = report::project_generate_report(project, pcbs, &phase_load_out_items_map);

    let report_file_path = report::build_report_file_path(&project.name, directory);

    let json = report::project_report_as_json(&report).map_err(|err| ArtifactGenerationError::UnableToSaveReport {
        reason: err,
    })?;

    #[cfg(feature = "markdown")]
    let markdown =
        report::project_report_json_to_markdown(&json).map_err(|err| ArtifactGenerationError::UnableToSaveReport {
            reason: err,
        })?;

    artifacts.push(Artifact {
        kind: ArtifactKind::JsonReport,
        path: report_file_path.clone(),
        contents: json,
    });

    #[cfg(feature = "markdown")]
    artifacts.push(Artifact {
        kind: ArtifactKind::MarkdownReport,
        path: report::markdown_report_file_path(&report_file_path),
        contents: markdown,
    });

    Ok(artifacts)
}

pub fn build_phase_placement_states<'a>(
    project: &'a Project,
    phase_reference: &'_ PhaseReference,
) -> Vec<(&'a ObjectPath, &'a PlacementState)> {
    let phase_placement_states: Vec<(&ObjectPath, &PlacementState)> = project
        .placements
        .iter()
        .filter_map(|(object_path, state)| match &state.phase {
            Some(placement_phase) if placement_phase.eq(phase_reference) => Some((object_path, state)),
            _ => None,
        })
        .collect();

    phase_placement_states
}

fn generate_phase_artifacts(
    pcbs: &[&Pcb],
    phase: &Phase,
    load_out_items: &[LoadOutItem],
    part_packages: &BTreeMap<&Part, &Package>,
    directory: &Path,
    phase_placement_states: &[(&ObjectPath, &PlacementState)],
) -> Result<Artifact, ArtifactGenerationError> {
    let pcb_unit_positioning_map = build_pcbs_unit_positioning_map(pcbs);

    // make a Vec so we can sort it, we're not cloning the paths and states themselves
    let mut phase_placement_states = Vec::from(phase_placement_states);

    sort_placements(
        &mut phase_placement_states,
        &phase.placement_orderings,
        load_out_items,
        part_packages,
        &pcb_unit_positioning_map,
    );

    let mut phase_placements_path = PathBuf::from(directory);
    phase_placements_path.push(format!("{}_placements.csv", phase.reference));

    let contents = phase_placements_as_csv(&phase_placement_states, load_out_items)
        .map_err(|e| ArtifactGenerationError::PhasePlacementsGenerationError(e))?;

    Ok(Artifact {
        kind: ArtifactKind::PhasePlacements {
            phase: phase.reference.clone(),
        },
        path: phase_placements_path,
        contents,
    })
}

/// returns a vector containing a vector of unit positions.
/// where the index of the first vector matches in the index of the pcbs
/// and where the index of the second vector matches the index of the unit for the pcb
pub fn build_pcbs_unit_positioning_map(pcbs: &[&Pcb]) -> Vec<Vec<DimensionUnitVector2>> {
    pcbs.iter()
        .map(|pcb| {
            let pcb_unit_positions = pcb
                .panel_sizing
                .pcb_unit_positionings
                .iter()
                .map(|positioning| {
                    // FIXME hard-coded use of UnitSystem::Millimeters
                    //       the pcb needs to have a unit system defined for it.
                    DimensionUnitVector2::new_dim_vector2_f64(positioning.offset, UnitSystem::Millimeters)
                })
                .collect::<Vec<_>>();

            pcb_unit_positions
        })
        .collect::<Vec<_>>()
}

pub fn sort_placements(
    placement_states: &mut Vec<(&ObjectPath, &PlacementState)>,
    placement_orderings: &[PlacementSortingItem],
    load_out_items: &[LoadOutItem],
    part_packages: &BTreeMap<&Part, &Package>,
    pcb_unit_positioning_map: &Vec<Vec<DimensionUnitVector2>>,
) {
    placement_states.sort_by(
        |(object_path_a, placement_state_a), (object_path_b, placement_state_b)| {
            placement_orderings
                .iter()
                .fold(Ordering::Equal, |mut acc, sort_ordering| {
                    if !matches!(acc, Ordering::Equal) {
                        return acc;
                    }
                    acc = match sort_ordering.mode {
                        PlacementSortingMode::FeederReference => {
                            let feeder_reference_a = match pnp::load_out::find_load_out_item_by_part(
                                load_out_items,
                                &placement_state_a.placement.part,
                            ) {
                                Some(load_out_item) => load_out_item.reference.clone(),
                                _ => None,
                            };
                            let feeder_reference_b = match pnp::load_out::find_load_out_item_by_part(
                                load_out_items,
                                &placement_state_b.placement.part,
                            ) {
                                Some(load_out_item) => load_out_item.reference.clone(),
                                _ => None,
                            };

                            trace!(
                                "Comparing feeder references. feeder_reference_a: '{:?}' feeder_reference_a: '{:?}'",
                                feeder_reference_a,
                                feeder_reference_b
                            );
                            feeder_reference_a.cmp(&feeder_reference_b)
                        }
                        PlacementSortingMode::PcbUnit => {
                            let pcb_unit_a = object_path_a.pcb_unit_path();
                            let pcb_unit_b = object_path_b.pcb_unit_path();

                            trace!(
                                "Comparing pcb units, pcb_unit_a: '{:?}', pcb_unit_b: '{:?}'",
                                pcb_unit_a,
                                pcb_unit_b
                            );
                            pcb_unit_a.cmp(&pcb_unit_b)
                        }
                        PlacementSortingMode::Pcb => {
                            let pcb_a = object_path_a.pcb_instance().unwrap();
                            let pcb_b = object_path_b.pcb_instance().unwrap();

                            trace!("Comparing pcb instance, pcb_a: '{:?}', pcb_b: '{:?}'", pcb_a, pcb_b);
                            pcb_a.cmp(&pcb_b)
                        }
                        PlacementSortingMode::RefDes => {
                            trace!(
                                "Comparing ref-des, ref_des_a: '{:?}', ref_des_b: '{:?}'",
                                placement_state_a.placement.ref_des,
                                placement_state_b.placement.ref_des,
                            );

                            placement_state_a
                                .placement
                                .ref_des
                                .cmp(&placement_state_b.placement.ref_des)
                        }
                        PlacementSortingMode::PcbUnitXY => {
                            let (pcb_index_a, pcb_unit_index_a) = object_path_a
                                .pcb_instance_and_unit()
                                .map(|(pcb_number, pcb_unit_number)| {
                                    (pcb_number as usize - 1, pcb_unit_number as usize - 1)
                                })
                                .unwrap();
                            let (pcb_index_b, pcb_unit_index_b) = object_path_b
                                .pcb_instance_and_unit()
                                .map(|(pcb_number, pcb_unit_number)| {
                                    (pcb_number as usize - 1, pcb_unit_number as usize - 1)
                                })
                                .unwrap();

                            let a_unit_position = pcb_unit_positioning_map[pcb_index_a][pcb_unit_index_a];
                            let b_unit_position = pcb_unit_positioning_map[pcb_index_b][pcb_unit_index_b];

                            a_unit_position
                                .x
                                .partial_cmp(&b_unit_position.x)
                                .unwrap()
                                .then(
                                    a_unit_position
                                        .y
                                        .partial_cmp(&b_unit_position.y)
                                        .unwrap(),
                                )
                        }
                        PlacementSortingMode::PcbUnitYX => {
                            let (pcb_index_a, pcb_unit_index_a) = object_path_a
                                .pcb_instance_and_unit()
                                .map(|(pcb_number, pcb_unit_number)| {
                                    (pcb_number as usize - 1, pcb_unit_number as usize - 1)
                                })
                                .unwrap();
                            let (pcb_index_b, pcb_unit_index_b) = object_path_b
                                .pcb_instance_and_unit()
                                .map(|(pcb_number, pcb_unit_number)| {
                                    (pcb_number as usize - 1, pcb_unit_number as usize - 1)
                                })
                                .unwrap();

                            let a_unit_position = pcb_unit_positioning_map[pcb_index_a][pcb_unit_index_a];
                            let b_unit_position = pcb_unit_positioning_map[pcb_index_b][pcb_unit_index_b];

                            a_unit_position
                                .y
                                .partial_cmp(&b_unit_position.y)
                                .unwrap()
                                .then(
                                    a_unit_position
                                        .x
                                        .partial_cmp(&b_unit_position.x)
                                        .unwrap(),
                                )
                        }
                        PlacementSortingMode::Area => {
                            let package_area = |part| {
                                part_packages
                                    .get(part)
                                    .map(|package| {
                                        package
                                            .dimensions_mm
                                            .as_ref()
                                            .map(|dimensions| dimensions.area())
                                    })
                                    .flatten()
                                    .unwrap_or(dec!(0))
                            };

                            let area_a = package_area(&placement_state_a.placement.part);
                            let area_b = package_area(&placement_state_b.placement.part);

                            area_a.cmp(&area_b)
                        }
                        PlacementSortingMode::Height => {
                            let package_height = |part| {
                                part_packages
                                    .get(part)
                                    .map(|package| {
                                        package
                                            .dimensions_mm
                                            .as_ref()
                                            .map(|dimensions| dimensions.size_z())
                                    })
                                    .flatten()
                                    .unwrap_or(dec!(0))
                            };

                            let height_a = package_height(&placement_state_a.placement.part);
                            let height_b = package_height(&placement_state_b.placement.part);

                            height_a.cmp(&height_b)
                        }
                        PlacementSortingMode::Part => placement_state_a
                            .placement
                            .part
                            .cmp(&placement_state_b.placement.part), //PlacementSortingMode::Cost => todo!(),
                                                                     //PlacementSortingMode::Description => todo!(),
                    };

                    match sort_ordering.sort_order {
                        SortOrder::Asc => acc,
                        SortOrder::Desc => acc.reverse(),
                    }
                })
        },
    );
}

#[serde_as]
#[derive(Debug, serde::Serialize)]
#[serde(rename_all(serialize = "PascalCase"))]
pub struct PhasePlacementRecord {
    #[serde_as(as = "DisplayFromStr")]
    pub object_path: ObjectPath,

    pub feeder_reference: Option<Reference>,
    pub manufacturer: String,
    pub mpn: String,
    pub x: Decimal,
    pub y: Decimal,
    pub rotation: Decimal,
}

/// The phase's placements, as their CSV.
pub fn phase_placements_as_csv(
    placement_states: &[(&ObjectPath, &PlacementState)],
    load_out_items: &[LoadOutItem],
) -> Result<String, Error> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_writer(vec![]);

    for (object_path, placement_state) in placement_states.iter() {
        let feeder_reference =
            match pnp::load_out::find_load_out_item_by_part(&load_out_items, &placement_state.placement.part) {
                Some(load_out_item) => load_out_item.reference.clone(),
                _ => None,
            };

        writer.serialize(PhasePlacementRecord {
            object_path: (*object_path).clone(),
            feeder_reference,
            manufacturer: placement_state
                .placement
                .part
                .manufacturer
                .to_string(),
            mpn: placement_state
                .placement
                .part
                .mpn
                .to_string(),
            x: placement_state.unit_position.x,
            y: placement_state.unit_position.y,
            rotation: placement_state.unit_position.rotation,
        })?;
    }

    let contents = writer
        .into_inner()
        .map_err(|error| anyhow::anyhow!("Unable to write the CSV. cause: {}", error))?;
    Ok(String::from_utf8(contents)?)
}

#[derive(Error, Debug)]
pub enum AssignmentError {
    #[error("Project state error. All phases must be pending to perform assignments")]
    ProjectStateError,
}

pub fn assign_placements_to_phase(
    project: &mut Project,
    phase: &Phase,
    action: SetOrClearAction,
    placements_pattern: Regex,
) -> Result<BTreeSet<Part>, AssignmentError> {
    if !project
        .phase_states
        .iter()
        .all(|(_, phase_state)| phase_state.is_pending())
    {
        warn!(
            "Unable to assign placements to phase, all phases must be pending. phase: '{}'",
            phase.reference
        );
        return Err(AssignmentError::ProjectStateError);
    }

    let mut required_load_out_parts = BTreeSet::new();

    debug!(
        "Assigning phase placements to {:?}, action: {:?}, pattern: {:?}",
        phase, action, placements_pattern
    );
    let matched_placements: Vec<(&ObjectPath, &mut PlacementState)> = project
        .placements
        .iter_mut()
        .filter(|(path, state)| {
            let path_str = format!("{}", path);

            placements_pattern.is_match(&path_str)
                && state
                    .placement
                    .pcb_side
                    .eq(&phase.pcb_side)
        })
        .collect();

    trace!("matched_placements: {:?}", matched_placements);

    for (placement_path, state) in matched_placements {
        // FUTURE consider refactoring this into the filter above, and then working on the remaining results...
        match action {
            SetOrClearAction::Set => {
                let should_assign = match &state.phase {
                    // different
                    Some(assigned_phase) if !assigned_phase.eq(&phase.reference) => true,
                    // none
                    None => true,
                    // same (ignore)
                    _ => false,
                };

                if should_assign {
                    info!(
                        "Assigning placement to phase. phase: {}, placement_path: {}",
                        phase.reference, placement_path
                    );
                    state.phase = Some(phase.reference.clone());
                }
            }
            SetOrClearAction::Clear => {
                let should_remove = match &state.phase {
                    // different
                    Some(assigned_phase) if !assigned_phase.eq(&phase.reference) => false,
                    // none (ignore)
                    None => false,
                    // same
                    _ => true,
                };

                if should_remove {
                    info!(
                        "Removing placement from phase. phase: {}, placement_path: {}",
                        phase.reference, placement_path
                    );
                    state.phase.take();
                }
            }
        }

        let _inserted = required_load_out_parts.insert(state.placement.part.clone());
    }

    Ok(required_load_out_parts)
}

pub fn refresh_from_design_variants<'a>(
    project: &'a mut Project,
    pcbs: &[&Pcb],
    design_variant_placement_map: BTreeMap<DesignVariant, Vec<Placement>>,
) -> Result<bool, ProjectError> {
    let unique_parts = placement::build_unique_parts_from_design_variant_placement_map(&design_variant_placement_map);

    let mut modified = refresh_parts(project, unique_parts.as_slice());

    modified |= refresh_placements(project, pcbs, &design_variant_placement_map)?;

    Ok(modified)
}

/// It is possible that the EDA files and/or PCBs have changed since the last time the project was refreshed.
/// We need to determine which placements:
/// 1. are still known (existing),
/// 2. previously in the EDA files, but no longer. (unknown).
/// 3. were added to the EDA files (new).
///
/// Additionally, the placements in the map will have X/Y/Rotation values as per the exported EDA, but we need
/// to calculate updated X/Y/Rotation values based on the design variant unit assignments and corresponding PCB
/// configuration.
///
/// Returns 'true' if project is modified
fn refresh_placements(
    project: &mut Project,
    pcbs: &[&Pcb],
    design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>,
) -> Result<bool, ProjectError> {
    let unit_assignments = project.all_unit_assignments(pcbs);

    let changes: Vec<(Change, ObjectPath, Placement)> =
        find_placement_changes(project, design_variant_placement_map, &unit_assignments);

    let all_placements = changes
        .iter()
        .map(|(_change, unit_path, placement)| (unit_path.clone(), placement))
        .collect::<Vec<_>>();

    let mut unit_positions = build_placement_unit_positions(all_placements, &unit_assignments, pcbs)?;

    let mut modified = false;

    for (change, path, placement) in changes.into_iter() {
        let unit_path = path.pcb_unit_path().unwrap();
        let unit_position = unit_positions.remove(&path).unwrap();
        let placement_state_entry = project.placements.entry(path.clone());

        match change {
            Change::New => {
                info!("New placement. placement: {:?} ({:?})", placement, unit_position);
                modified |= true;

                let placement_state = PlacementState {
                    unit_path,
                    placement,
                    unit_position,
                    operation_status: PlacementStatus::Pending,
                    project_status: ProjectPlacementStatus::Used,
                    phase: None,
                };

                placement_state_entry.or_insert(placement_state);
            }
            Change::Existing => {
                placement_state_entry.and_modify(|ps| {
                    if !ps
                        .project_status
                        .eq(&ProjectPlacementStatus::Used)
                    {
                        // this can happen:
                        // 1) if a placement is removed, then later re-added. (e.g. changes to a placements.csv file)
                        // 2) if unit mappings are changed, first causing the placement to become unused, then later
                        //     changed again such that the placement becomes required again.
                        info!("Marking previously unused placement as used. path: {}", path);
                        ps.project_status = ProjectPlacementStatus::Used;
                        modified |= true;
                    }
                    if !ps.placement.eq(&placement) {
                        info!("Updating placement. old: {:?}, new: {:?}", ps.placement, placement);
                        ps.placement = placement;
                        modified |= true;
                    }
                    if !ps.unit_position.eq(&unit_position) {
                        info!(
                            "Updating placement unit position. path: {}, old: {:?}, new: {:?}",
                            path, ps.unit_position, unit_position
                        );
                        ps.unit_position = unit_position;
                        modified |= true;
                    }
                });
            }
            Change::Unused => {
                info!(
                    "Marking placement as unused. placement: {:?} ({:?})",
                    placement, unit_position
                );
                modified |= true;

                placement_state_entry.and_modify(|ps| {
                    ps.project_status = ProjectPlacementStatus::Unused;
                    ps.unit_position = unit_position;
                });
            }
        }
    }

    Ok(modified)
}

/// attempts to pnp build unit positioning for each eda placement.
/// requires that the PCB has been configured with sizing information.
///
/// there should be one pcb_orientation for each pcb.
pub(crate) fn build_placement_unit_positions(
    placements: Vec<(ObjectPath, &Placement)>,
    unit_assignments: &Vec<(ObjectPath, Option<DesignVariant>)>,
    pcbs: &[&Pcb],
) -> Result<BTreeMap<ObjectPath, UnitPlacementPosition>, ProjectError> {
    type BuildTransformResult = Result<PcbUnitTransform, PcbError>;
    let pcb_unit_transforms: HashMap<(PcbInstanceNumber, PcbUnitNumber), (BuildTransformResult, BuildTransformResult)> =
        unit_assignments
            .iter()
            .map(|(path, _variant)| {
                let (pcb_instance_number, pcb_unit_number) = path.pcb_instance_and_unit().unwrap();
                let (pcb_instance_index, pcb_unit_index) = (
                    pcb_instance_number as PcbUnitIndex - 1,
                    pcb_unit_number as PcbUnitIndex - 1,
                );

                let pcb = pcbs[pcb_instance_index as usize];

                let top_transform = pcb.build_unit_transform(pcb_unit_index, &pcb.orientation.top);
                let bottom_transform = pcb.build_unit_transform(pcb_unit_index, &pcb.orientation.bottom);

                trace!(
                    "unit: {:?}, top_transform: {:?}, bottom_transform: {:?}",
                    (pcb_instance_number, pcb_unit_number),
                    top_transform,
                    bottom_transform
                );

                (
                    (pcb_instance_number, pcb_unit_number),
                    (top_transform, bottom_transform),
                )
            })
            .collect::<HashMap<_, _>>();

    let mut failures: HashSet<_> = HashSet::new();
    let mut results = BTreeMap::new();
    for (path, placement) in placements.into_iter() {
        let (pcb_instance_number, pcb_unit_number) = path.pcb_instance_and_unit().unwrap();

        let transforms = pcb_unit_transforms.get(&(pcb_instance_number, pcb_unit_number));

        if let Some((Ok(top_transform), Ok(bottom_transform))) = transforms {
            let transform = match placement.pcb_side {
                PcbSide::Top => top_transform,
                PcbSide::Bottom => bottom_transform,
            };

            let unit_placement_position = transform.apply_to_placement_matrix(placement);
            results.insert(path, unit_placement_position);
        } else {
            failures.insert((pcb_instance_number, pcb_unit_number));
        }
    }
    if failures.len() > 0 {
        let message = format!(
            "Unable to build transforms for the following PCB/Unit combinations: {:?}",
            failures
        );
        Err(ProjectError::UnableToBuildUnitPositions(message))
    } else {
        Ok(results)
    }
}

fn find_placement_changes(
    project: &mut Project,
    design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>,
    unit_assignments: &Vec<(ObjectPath, Option<DesignVariant>)>,
) -> Vec<(Change, ObjectPath, Placement)> {
    let mut changes: Vec<(Change, ObjectPath, Placement)> = vec![];

    // find new or existing placements that are in the updated design_variant_placement_map

    for (design_variant, placements) in design_variant_placement_map.iter() {
        for (unit_path, _assigned_design_variant) in unit_assignments
            .iter()
            .filter_map(|(path, assignment)| {
                assignment
                    .as_ref()
                    .filter(|assignment_design_variant| assignment_design_variant.eq(&design_variant))
                    .map(|assigned_design_variant| (path, assigned_design_variant))
            })
        {
            for placement in placements {
                let mut path: ObjectPath = unit_path.clone();
                path.set_ref_des(placement.ref_des.clone());

                // look for a placement state for the placement for this object path

                match project.placements.contains_key(&path) {
                    true => changes.push((Change::Existing, path, placement.clone())),
                    false => changes.push((Change::New, path, placement.clone())),
                }
            }
        }
    }

    // find the placements that we knew about previously, but that are no-longer used.

    for (path, state) in project.placements.iter_mut() {
        for (unit_path, unit_assignment) in unit_assignments.iter() {
            let path_str = path.to_string();
            let unit_path_str = unit_path.to_string();
            let is_matched_unit = path_str.starts_with(&unit_path_str);
            trace!(
                "path_str: {}, unit_path_str: {}, is_matched_unit: {}",
                path_str,
                unit_path_str,
                is_matched_unit
            );

            if is_matched_unit {
                if let Some(design_variant) = unit_assignment {
                    if let Some(placements) = design_variant_placement_map.get(design_variant) {
                        match placements.iter().find(|placement| {
                            placement
                                .ref_des
                                .eq(&state.placement.ref_des)
                        }) {
                            Some(_) => {
                                trace!("known placement");
                            }
                            None => {
                                trace!("unknown placement");
                                match state.project_status {
                                    ProjectPlacementStatus::Unused => (),
                                    ProjectPlacementStatus::Used => {
                                        changes.push((Change::Unused, path.clone(), state.placement.clone()))
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // the unit is no longer assigned, all the associated placements must be marked as unused
                    trace!("unknown placement, unused design_variant");
                    changes.push((Change::Unused, path.clone(), state.placement.clone()))
                }
            }
        }
    }

    trace!("placement changes:\n{:?}", changes);

    changes
}

#[derive(Debug)]
enum Change {
    New,
    Existing,
    Unused,
}

/// Returns 'true' if any changes were made.
fn refresh_parts(project: &mut Project, all_parts: &[&Part]) -> bool {
    let changes = find_part_changes(project, all_parts);

    let mut modified = false;

    let mut new_parts = vec![];
    let mut parts_to_remove = vec![];

    for change_item in changes.iter() {
        match change_item {
            (Change::New, part) => {
                info!("New part. part: {:?}", part);
                modified = true;
                new_parts.push((*part).clone());
            }
            (Change::Existing, _part) => {}
            (Change::Unused, part) => {
                info!("Removing unused part. part: {:?}", part);
                modified = true;
                parts_to_remove.push((*part).clone());
            }
        }
    }

    for part in new_parts {
        let _ = project
            .part_states
            .entry(part)
            .or_default();
    }

    for part in parts_to_remove {
        let _ = project.part_states.remove(&part);
    }

    modified
}

fn find_part_changes<'a: 'b, 'b>(project: &'b Project, all_parts: &[&'a Part]) -> Vec<(Change, &'b Part)> {
    let mut changes: Vec<(Change, &Part)> = vec![];

    for &part in all_parts.iter() {
        match project.part_states.contains_key(part) {
            true => changes.push((Change::Existing, part)),
            false => changes.push((Change::New, part)),
        }
    }

    for (part, _process) in project.part_states.iter() {
        if !all_parts.contains(&part) {
            changes.push((Change::Unused, part))
        }
    }

    trace!("part changes:\n{:?}", changes);

    changes
}

#[must_use]
pub fn find_parts_to_modify(
    project: &Project,
    unique_parts: &[&Part],
    manufacturer_pattern: Regex,
    mpn_pattern: Regex,
) -> Vec<Part> {
    let parts_to_modify: Vec<Part> = find_part_changes(project, unique_parts)
        .into_iter()
        .filter_map(|(change, part)| {
            trace!("change: {:?}, part: {:?}", change, part);

            match change {
                Change::New | Change::Existing => {
                    if manufacturer_pattern.is_match(part.manufacturer.as_str())
                        && mpn_pattern.is_match(part.mpn.as_str())
                    {
                        Some((*part).clone())
                    } else {
                        None
                    }
                }
                _ => {
                    panic!("unexpected change. change: {:?}", change);
                }
            }
        })
        .collect();

    parts_to_modify
}

#[must_use]
pub fn update_applicable_processes(
    project: &mut Project,
    parts_to_modify: Vec<Part>,
    process: ProcessDefinition,
    action: AddOrRemoveAction,
) -> bool {
    let mut modified = false;

    for part in parts_to_modify {
        project
            .part_states
            .entry(part.clone())
            .and_modify(|part_state| {
                modified |= match action {
                    AddOrRemoveAction::Add => add_process_to_part(part_state, &part, process.reference.clone()),
                    AddOrRemoveAction::Remove => remove_process_from_part(part_state, &part, process.reference.clone()),
                }
            });
    }

    modified
}

#[must_use]
pub fn add_process_to_part(part_state: &mut PartState, part: &Part, process: ProcessReference) -> bool {
    let inserted = part_state
        .applicable_processes
        .insert(process);

    if inserted {
        info!(
            "Added process. part: {:?}, applicable_processes: {:?}",
            part,
            part_state
                .applicable_processes
                .iter()
                .map(|it| it.to_string())
                .collect::<Vec<String>>()
        );
    }

    inserted
}

#[must_use]
pub fn remove_process_from_part(part_state: &mut PartState, part: &Part, process: ProcessReference) -> bool {
    let removed = part_state
        .applicable_processes
        .remove(&process);

    if removed {
        info!(
            "Removed process. part: {:?}, applicable_processes: {:?}",
            part,
            part_state
                .applicable_processes
                .iter()
                .map(|it| it.to_string())
                .collect::<Vec<String>>()
        );
    }

    removed
}

pub fn update_placements_operation(
    project: &mut Project,
    directory: &Path,
    object_path_patterns: Vec<Regex>,
    placement_operation: PlacementOperation,
) -> anyhow::Result<bool> {
    let mut modified = false;

    let phase_operation_task_map = build_phase_operation_task_map(&placement_operation, &project.phase_states);

    let mut history_item_map: HashMap<Reference, Vec<Box<dyn OperationHistoryKind>>> = HashMap::new();

    for object_path_pattern in object_path_patterns.iter() {
        let placements: Vec<_> = project
            .placements
            .iter_mut()
            .filter(|(object_path, _placement_state)| object_path_pattern.is_match(&object_path.to_string()))
            .collect();

        if placements.is_empty() {
            warn!(
                "Unmatched object path pattern. object_path_pattern: {}",
                object_path_pattern
            );
        }

        for (object_path, placement_state) in placements {
            if placement_state.phase.is_none() {
                // we cannot modify placement state when no phase is assigned.
                continue;
            }

            let placement_phase_reference = placement_state.phase.as_ref().unwrap();

            let phase_map_entry = phase_operation_task_map.get(placement_phase_reference);
            if phase_map_entry.is_none() {
                // if a phase doesn't have a map entry then we cannot update any placement with that phase reference
                continue;
            }

            let should_log = match placement_operation {
                PlacementOperation::Place => match placement_state.operation_status {
                    PlacementStatus::Placed => {
                        warn!("Placement already marked as placed. object_path: {}", object_path);
                        false
                    }
                    PlacementStatus::Skipped => {
                        warn!("Placement was previously skipped. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Placed;
                        modified = true;
                        true
                    }
                    PlacementStatus::Pending => {
                        info!("Placement marked as placed. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Placed;
                        modified = true;
                        true
                    }
                },
                PlacementOperation::Reset => match placement_state.operation_status {
                    PlacementStatus::Placed | PlacementStatus::Skipped => {
                        info!("Resetting placed flag. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Pending;
                        modified = true;
                        true
                    }
                    PlacementStatus::Pending => {
                        warn!("Placed flag already pending. object_path: {}", object_path);
                        false
                    }
                },
                PlacementOperation::Skip => match placement_state.operation_status {
                    PlacementStatus::Placed => {
                        warn!("Placement was previously placed. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Skipped;
                        modified = true;
                        true
                    }
                    PlacementStatus::Skipped => {
                        warn!("Placement already marked as skipped. object_path: {}", object_path);
                        false
                    }
                    PlacementStatus::Pending => {
                        info!("Placement marked as skipped. object_path: {}", object_path);
                        placement_state.operation_status = PlacementStatus::Skipped;
                        modified = true;
                        true
                    }
                },
            };

            if should_log {
                let task_history = Box::new(PlacementOperationHistoryKind {
                    object_path: object_path.clone(),
                    operation: placement_operation.clone(),
                }) as Box<dyn OperationHistoryKind>;

                let history_items = history_item_map
                    .entry(placement_phase_reference.clone())
                    .or_default();

                history_items.push(task_history);

                modified = true;
            }
        }
    }

    if modified {
        let states_modified = refresh_phase_operation_states(project);
        // redundant, but consistent.
        modified |= states_modified;

        for (phase_reference, task_histories) in history_item_map {
            // Safety: code above should prevent this unwrap from failing
            let (operation_reference, _task_reference) = phase_operation_task_map
                .get(&phase_reference)
                .unwrap();

            let now = OffsetDateTime::now_utc();

            let history_items = task_histories
                .into_iter()
                .map(|task_history| OperationHistoryItem {
                    date_time: now,
                    phase: phase_reference.clone(),
                    extra: Default::default(),
                    operation_reference: operation_reference.clone(),
                    task_reference: TaskReference::from_raw_str("core::place_components"),
                    task_history,
                })
                .collect::<Vec<_>>();

            let mut phase_log_path = PathBuf::from(directory);
            phase_log_path.push(format!("{}_log.json", phase_reference));

            let mut operation_history: Vec<OperationHistoryItem> = operation_history::read_or_default(&phase_log_path)?;

            operation_history.extend(history_items);

            operation_history::write(phase_log_path, &operation_history)?;
        }
    }

    Ok(modified)
}

/// find the only tasks for each phase that allow placement changes.
fn build_phase_operation_task_map(
    placement_operation: &PlacementOperation,
    phase_states: &BTreeMap<Reference, PhaseState>,
) -> BTreeMap<Reference, (OperationReference, TaskReference)> {
    let phase_operation_task_map = phase_states
        .iter()
        .filter_map(|(phase_reference, phase_state)| {
            let operation_and_task_references = phase_state
                .operation_states
                .iter()
                .enumerate()
                .find_map(|(op_index, operation_state)| {
                    if !can_modify_operation(phase_state, op_index) {
                        return None;
                    }
                    operation_state
                        .task_states
                        .iter()
                        .enumerate()
                        .find_map(|(task_index, (task_reference, task_state))| {
                            if !task_state.requires_placements() {
                                return None;
                            }

                            if !can_modify_task(operation_state, task_index) {
                                return None;
                            }

                            let can_change = match (&placement_operation, task_state.status()) {
                                (PlacementOperation::Reset, TaskStatus::Complete) => true,
                                (_, TaskStatus::Started) => true,
                                _ => false,
                            };

                            if can_change {
                                Some((operation_state.reference.clone(), task_reference.clone()))
                            } else {
                                None
                            }
                        })
                });
            operation_and_task_references.map(|(operation_reference, task_reference)| {
                (
                    phase_reference.clone(),
                    (operation_reference.clone(), task_reference.clone()),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    phase_operation_task_map
}

#[cfg(test)]
mod build_phase_operation_task_map_tests {
    use std::collections::BTreeMap;

    use indexmap::IndexMap;
    use pnp::reference::Reference;
    use rstest::rstest;

    use crate::phase::{PhaseReference, PhaseState};
    use crate::placement::{PlacementOperation, PlacementStatus};
    use crate::process::{
        LoadPcbsTaskState, ManualSolderingTaskState, OperationReference, OperationState, PlacementTaskState,
        SerializableTaskState, TaskReference,
    };

    fn build_phase_states_1() -> BTreeMap<Reference, PhaseState> {
        BTreeMap::from([(PhaseReference::from_raw_str("phase_1"), PhaseState {
            operation_states: vec![
                OperationState {
                    reference: OperationReference::from_raw_str("operation_1"),
                    task_states: IndexMap::from([(
                        TaskReference::from_raw_str("task_1"),
                        Box::new(LoadPcbsTaskState::default()) as Box<dyn SerializableTaskState>,
                    )]),
                },
                OperationState {
                    reference: OperationReference::from_raw_str("operation_2"),
                    task_states: IndexMap::from([
                        (
                            TaskReference::from_raw_str("task_2"),
                            Box::new(PlacementTaskState::default()) as Box<dyn SerializableTaskState>,
                        ),
                        (
                            TaskReference::from_raw_str("task_3"),
                            Box::new(ManualSolderingTaskState::default()) as Box<dyn SerializableTaskState>,
                        ),
                    ]),
                },
            ],
        })])
    }

    #[test]
    pub fn test_build_phase_operation_task_map_reset_when_completed() {
        // given
        let placement_operation = PlacementOperation::Reset;
        let mut phase_states = build_phase_states_1();

        // and complete the first operation
        let phase1_reference = phase_states
            .iter()
            .next()
            .unwrap()
            .0
            .clone();
        let phase1_state = phase_states
            .get_mut(&phase1_reference)
            .unwrap();
        phase1_state.operation_states[0].task_states[0].set_completed();
        // and complete the second
        let placements_task = &mut phase1_state.operation_states[1].task_states[0];
        if let Some(stuff) = placements_task.placements_state_mut() {
            stuff.set_total_placements(1);
            stuff.on_placement_status_change(&PlacementStatus::Pending, &PlacementStatus::Placed);
        }

        // and
        let expected_result = BTreeMap::from([(
            PhaseReference::from_raw_str("phase_1"),
            (
                OperationReference::from_raw_str("operation_2"),
                TaskReference::from_raw_str("task_2"),
            ),
        )]);

        // when
        let result = super::build_phase_operation_task_map(&placement_operation, &phase_states);

        // then
        assert_eq!(result, expected_result);
    }

    #[rstest]
    #[case(PlacementOperation::Place)]
    #[case(PlacementOperation::Skip)]
    #[case(PlacementOperation::Reset)]
    pub fn test_build_phase_operation_task_map(#[case] placement_operation: PlacementOperation) {
        // given
        let mut phase_states = build_phase_states_1();

        // and complete the first operation
        let phase1_reference = phase_states
            .iter()
            .next()
            .unwrap()
            .0
            .clone();
        let phase1_state = phase_states
            .get_mut(&phase1_reference)
            .unwrap();
        phase1_state.operation_states[0].task_states[0].set_completed();
        // and start the second
        phase1_state.operation_states[1].task_states[0].set_started();

        // and
        let expected_result = BTreeMap::from([(
            PhaseReference::from_raw_str("phase_1"),
            (
                OperationReference::from_raw_str("operation_2"),
                TaskReference::from_raw_str("task_2"),
            ),
        )]);

        // when
        let result = super::build_phase_operation_task_map(&placement_operation, &phase_states);

        // then
        assert_eq!(result, expected_result);
    }

    #[rstest]
    #[case(PlacementOperation::Place)]
    #[case(PlacementOperation::Skip)]
    #[case(PlacementOperation::Reset)]
    pub fn test_build_phase_operation_task_map_when_first_operation_not_started(
        #[case] placement_operation: PlacementOperation,
    ) {
        // given
        let phase_states = build_phase_states_1();

        // and
        let expected_result = BTreeMap::new();

        // when
        let result = super::build_phase_operation_task_map(&placement_operation, &phase_states);

        // then
        assert_eq!(result, expected_result);
    }

    #[rstest]
    #[case(PlacementOperation::Place)]
    #[case(PlacementOperation::Skip)]
    #[case(PlacementOperation::Reset)]
    pub fn test_build_phase_operation_task_map_when_second_operation_not_started(
        #[case] placement_operation: PlacementOperation,
    ) {
        // given
        let mut phase_states = build_phase_states_1();

        // and complete the first operation
        let phase1_reference = phase_states
            .iter()
            .next()
            .unwrap()
            .0
            .clone();
        let phase1_state = phase_states
            .get_mut(&phase1_reference)
            .unwrap();
        phase1_state.operation_states[0].task_states[0].set_completed();

        // and
        let expected_result = BTreeMap::new();

        // when
        let result = super::build_phase_operation_task_map(&placement_operation, &phase_states);

        // then
        assert_eq!(result, expected_result);
    }
}

/// Sometimes it's necessary to refresh the phase operation states
///
/// e.g.
/// 1) adding placements may make a complete phase incomplete
/// 2) removing placements may make a phase complete
pub fn refresh_phase_operation_states(project: &mut Project) -> bool {
    let mut modified = false;

    for (phase_reference, phase_state) in project.phase_states.iter_mut() {
        trace!("reference: {:?}, phase_state: {:?}", phase_reference, phase_state);

        for operation_state in phase_state.operation_states.iter_mut() {
            trace!(
                "operation: {:?}, operation_state: {:?}",
                operation_state.reference,
                operation_state
            );

            // FUTURE optimize this if there's ever more then one task that needs placements

            for (task_reference, task_state) in operation_state.task_states.iter_mut() {
                trace!("task: {:?}, task_state: {:?}", task_reference, task_state);

                if task_state.requires_placements() {
                    let original_task_state = dyn_clone::clone(task_state);

                    {
                        let placement_api = task_state
                            .placements_state_mut()
                            .unwrap();
                        placement_api.reset();

                        let phase_placements = project.placements
                            .iter()
                            .filter(|(_object_path, placement_state)|
                                matches!(&placement_state.phase, Some(candidate_phase_reference) if candidate_phase_reference.eq(phase_reference))
                            ).collect::<Vec<_>>();

                        let total = phase_placements.len();
                        placement_api.set_total_placements(total);

                        for (_object_path, placement) in phase_placements {
                            placement_api
                                .on_placement_status_change(&PlacementStatus::Pending, &placement.operation_status);
                        }

                        debug!("summary: {:?}", placement_api.summary());
                    }

                    trace!(
                        "Refreshing placement task state complete.  before: {:?}, after: {:?}",
                        original_task_state,
                        task_state
                    );

                    let state_updated = task_state != &original_task_state;

                    info!(
                        "Refreshed placement task state. phase: {}, operation: {}, task: {}, status: {}, updated: {}",
                        phase_reference,
                        operation_state.reference,
                        task_reference,
                        task_state.status(),
                        state_updated
                    );

                    modified |= state_updated;
                }
            }
        }
    }
    modified
}

#[derive(Error, Debug)]
pub enum PartStateError {
    #[error("No part state found. manufacturer: {}, mpn: {}", part.manufacturer, part.mpn)]
    NoPartStateFound { part: Part },
}

#[cfg(test)]
mod apply_phase_operation_task_action_tests {
    use indexmap::IndexMap;
    use pnp::reference::Reference;
    use rstest::rstest;

    use crate::phase::PhaseState;
    use crate::process;
    use crate::process::TaskAction;
    use crate::process::{OperationState, SerializableTaskState, TaskKind, TaskReference, TaskStatus};
    use crate::project::{Project, TaskActionError};

    #[rstest]
    #[case(
        TaskAction::Start,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Pending),
                ])
            ])
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Start,
        ("p1", "o1", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                    ("core::place_components", TaskStatus::Pending),
                ])
            ]),
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Start,
        ("p2", "o2", "core::manual_soldering"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                ]),
                ("o2", vec![
                    // Cannot create place_components task with completed state, requires placements to be placed.
                    //("core::place_components", TaskStatus::Complete),
                    // Instead use a test-only task.
                    ("core::test_task", TaskStatus::Complete),
                ]),
                ("o3", vec![
                    ("core::automated_soldering", TaskStatus::Complete),
                ]),
            ]),
            ("p2", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                ]),
                ("o2", vec![
                    // Cannot create place_components task with completed state, requires placements to be placed.
                    //("core::place_components", TaskStatus::Complete),
                    // Instead use a test-only task.
                    ("core::test_task", TaskStatus::Complete),
                    ("core::manual_soldering", TaskStatus::Pending),
                ])
            ])
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Complete,
        ("p1", "o1", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                    ("core::place_components", TaskStatus::Started),
                ])
            ])
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Abandon,
        ("p1", "o1", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete),
                    ("core::place_components", TaskStatus::Started),
                ])
            ])
        ],
        Ok(true)
    )]
    #[case(
        TaskAction::Start,
        ("p1", "o2", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Started)
                ]),
                ("o2", vec![
                    ("core::place_components", TaskStatus::Pending)
                ]),
            ])
        ],
        Err(TaskActionError::PrecedingOperationNotComplete)
    )]
    #[case(
        TaskAction::Start,
        ("p1", "o1", "core::place_components"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Pending),
                    ("core::place_components", TaskStatus::Pending),
                ])
            ])
        ],
        Err(TaskActionError::PrecedingTaskNotComplete)
    )]
    #[case(
        TaskAction::Start,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Started)
                ])
            ])
        ],
        Err(TaskActionError::TaskAlreadyStarted)
    )]
    #[case(
        TaskAction::Complete,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Complete)
                ])
            ])
        ],
        Err(TaskActionError::TaskAlreadyComplete)
    )]
    #[case(
        TaskAction::Abandon,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Abandoned)
                ])
            ])
        ],
        Err(TaskActionError::TaskAlreadyAbandoned)
    )]
    #[case(
        TaskAction::Complete,
        ("p1", "o1", "core::load_pcbs"),
        vec![
            ("p1", vec![
                ("o1", vec![
                    ("core::load_pcbs", TaskStatus::Pending)
                ])
            ])
        ],
        Err(TaskActionError::TaskNotStarted)
    )]
    pub fn can_apply_action(
        #[case] action: TaskAction,
        #[case] references: (&str, &str, &str),
        #[case] phase_operation_task_status_map: Vec<(&str, Vec<(&str, Vec<(&str, TaskStatus)>)>)>,
        #[case] expected_result: Result<bool, TaskActionError>,
    ) {
        let (phase_reference, operation_reference, task_reference) = {
            (
                Reference::from_raw_str(references.0),
                Reference::from_raw_str(references.1),
                TaskReference::from_raw_str(references.2),
            )
        };

        let mut project = Project::default();
        for (phase_reference, operation_task_status_map) in phase_operation_task_status_map.iter() {
            let operation_states = operation_task_status_map
                .iter()
                .map(|(operation_reference, task_status_map)| {
                    let task_states = task_status_map
                        .iter()
                        .map(|(task_reference, task_status)| {
                            let kind = TaskKind::for_legacy_task(&TaskReference::from_raw_str(task_reference))
                                .unwrap_or(TaskKind::Generic);
                            let mut task_state = process::make_task_state(kind);
                            match task_status {
                                // Default state is pending
                                TaskStatus::Pending => {}

                                TaskStatus::Started => task_state.set_started(),
                                TaskStatus::Complete => task_state.set_completed(),
                                TaskStatus::Abandoned => task_state.set_abandoned(),
                            }

                            (TaskReference::from_raw_str(task_reference), task_state)
                        })
                        .collect::<Vec<(TaskReference, Box<dyn SerializableTaskState>)>>();
                    let operation_state = OperationState {
                        reference: Reference::from_raw_str(operation_reference),
                        task_states: IndexMap::from_iter(task_states),
                    };
                    operation_state
                })
                .collect::<Vec<OperationState>>();

            let phase_state = PhaseState {
                operation_states,
            };
            let _ = project
                .phase_states
                .insert(Reference::from_raw_str(phase_reference), phase_state);
        }

        // when
        let result = super::can_apply_action(
            &mut project,
            &phase_reference,
            &operation_reference,
            &task_reference,
            &action,
        );

        // then
        let result = result.map(|_task_state| true);

        assert_eq!(result, expected_result);
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum TaskActionError {
    #[error("Phase cannot be started.")]
    PhaseCannotBeStarted,
    #[error("Preceding operation not complete.")]
    PrecedingOperationNotComplete,
    #[error("Preceding task not complete.")]
    PrecedingTaskNotComplete,
    #[error("Task already started.")]
    TaskAlreadyStarted,
    #[error("Task already complete.")]
    TaskAlreadyComplete,
    #[error("Task already abandoned.")]
    TaskAlreadyAbandoned,
    #[error("Task not started.")]
    TaskNotStarted,
}

/// Safety: assumes all references are valid.
fn can_apply_action<'p>(
    project: &'p mut Project,
    phase_reference: &Reference,
    operation_reference: &OperationReference,
    task_reference: &TaskReference,
    task_action: &TaskAction,
) -> Result<&'p mut Box<dyn SerializableTaskState>, TaskActionError> {
    let can_start_phase = project.can_start_phase(phase_reference);

    let phase_state = project
        .phase_states
        .get_mut(phase_reference)
        .unwrap();

    let mut is_first_operation = true;

    let operation_state = phase_state
        .operation_states
        .iter_mut()
        .try_fold(None, |mut acc, operation_state| {
            if acc.is_some() {
                // already found the operation state
                return Ok(acc);
            }

            if operation_state
                .reference
                .eq(operation_reference)
            {
                acc = Some(operation_state);
                return Ok(acc);
            }

            is_first_operation = false;

            //
            // check overall-state of preceding operation
            //
            let preceding_operation_status = operation_state.status();
            if preceding_operation_status != OperationStatus::Complete {
                return Err(TaskActionError::PrecedingOperationNotComplete);
            }

            Ok(acc)
        })?
        .unwrap();

    let mut is_first_task = true;

    let task_state = operation_state
        .task_states
        .iter_mut()
        .try_fold(None, |mut acc, (candidate_task_reference, task_state)| {
            if acc.is_some() {
                return Ok(acc);
            }

            if task_reference.eq(candidate_task_reference) {
                //
                // check the state of this task
                //
                match (task_action, task_state.status()) {
                    (TaskAction::Start, TaskStatus::Pending) => {
                        if is_first_operation && is_first_task && !can_start_phase {
                            return Err(TaskActionError::PhaseCannotBeStarted);
                        }
                    }
                    (TaskAction::Start, TaskStatus::Started) => return Err(TaskActionError::TaskAlreadyStarted),
                    (TaskAction::Complete, TaskStatus::Complete) => return Err(TaskActionError::TaskAlreadyComplete),
                    (TaskAction::Abandon, TaskStatus::Abandoned) => return Err(TaskActionError::TaskAlreadyAbandoned),

                    // 'start' with wrong state
                    (TaskAction::Start, TaskStatus::Abandoned) => return Err(TaskActionError::TaskAlreadyStarted),
                    (TaskAction::Start, TaskStatus::Complete) => return Err(TaskActionError::TaskAlreadyComplete),

                    // 'complete' with wrong state
                    (TaskAction::Complete, TaskStatus::Abandoned) => return Err(TaskActionError::TaskAlreadyAbandoned),
                    (TaskAction::Complete, TaskStatus::Pending) => return Err(TaskActionError::TaskNotStarted),

                    // 'abandon' with wrong state
                    (TaskAction::Abandon, TaskStatus::Pending) => return Err(TaskActionError::TaskNotStarted),
                    _ => {}
                }
                acc = Some(task_state);
            } else {
                is_first_task = false;

                //
                // check the state of the preceding task
                //
                let preceding_task_status = task_state.status();
                if preceding_task_status != TaskStatus::Complete {
                    return Err(TaskActionError::PrecedingTaskNotComplete);
                }
            }

            Ok(acc)
        })?
        .unwrap();

    Ok(task_state)
}

pub fn apply_phase_operation_task_action(
    project: &mut Project,
    directory: &Path,
    phase_reference: &Reference,
    operation_reference: OperationReference,
    task_reference: TaskReference,
    action: TaskAction,
) -> anyhow::Result<bool> {
    let mut modified = false;

    let phase_state = project
        .phase_states
        .get_mut(phase_reference)
        .ok_or(PhaseError::UnknownPhase(phase_reference.clone()))?;

    // in case of an error, we need to help the user by giving them a list of the possible operation references
    let possible_operation_references = phase_state
        .operation_states
        .iter()
        .map(|state| state.reference.clone())
        .collect::<Vec<_>>();

    let operation_state = phase_state
        .operation_states
        .iter_mut()
        .find(|state| state.reference.eq(&operation_reference));

    // If we didn't find the operation we were looking, bail.
    let operation_state = operation_state.ok_or(PhaseError::InvalidOperationForPhase(
        phase_reference.clone(),
        operation_reference.clone(),
        possible_operation_references,
    ))?;

    // in case of an error, we need to help the user by giving them a list of the possible task references
    let possible_task_references = operation_state
        .task_states
        .iter()
        .map(|(reference, _state)| reference.clone())
        .collect::<Vec<_>>();

    let task_ref_and_state = operation_state
        .task_states
        .iter_mut()
        .find(|(&ref reference, _state)| reference.eq(&task_reference));

    // If we didn't find the task we were looking, bail.
    let _task_ref_and_state = task_ref_and_state.ok_or(PhaseError::InvalidTaskForOperation(
        phase_reference.clone(),
        operation_reference.clone(),
        task_reference.clone(),
        possible_task_references,
    ))?;

    // make sure the operation's CAN be changed.
    // reasons why it might not be possible include:
    // 1) trying to change a task where preceding tasks or operations are not in the correct state
    // 2) trying to complete AutomatedPnp/ManuallySolderComponents when not all components have been placed (or skipped)
    // 3) some other task-defined reason.
    // 4) applying tasks to phases when the preceding phase is not complete.

    let task_state = can_apply_action(project, phase_reference, &operation_reference, &task_reference, &action)?;

    match action {
        TaskAction::Start => {
            info!(
                "Marking task as started. phase: {}, operation: {}, task: {}",
                phase_reference, operation_reference, task_reference
            );
            task_state.set_started()
        }
        TaskAction::Complete => {
            info!(
                "Marking task as completed. phase: {}, operation: {}, task: {}",
                phase_reference, operation_reference, task_reference
            );
            task_state.set_completed()
        }
        TaskAction::Abandon => {
            info!(
                "Marking task as abandoned. phase: {}, operation: {}, task: {}",
                phase_reference, operation_reference, task_reference
            );
            task_state.set_abandoned()
        }
    }

    let new_status = task_state.status();

    let mut task_history_items: Vec<(&TaskReference, Box<dyn OperationHistoryKind>)> = Vec::new();

    match project.find_task_kind(phase_reference, &operation_reference, &task_reference) {
        Some(kind) => task_history_items.push((
            &task_reference,
            Box::new(TaskOperationHistoryKind {
                kind,
                status: new_status,
            }) as Box<dyn OperationHistoryKind>,
        )),
        None => warn!(
            "Unable to build history, the task isn't in the phase's process. phase: {}, operation: {}, task: {}",
            phase_reference, operation_reference, task_reference
        ),
    }

    if !task_history_items.is_empty() {
        modified = true;

        for (task_reference, task_history) in task_history_items.into_iter() {
            let now = OffsetDateTime::now_utc();

            let history_item = OperationHistoryItem {
                date_time: now,
                phase: phase_reference.clone(),
                operation_reference: operation_reference.clone(),
                task_reference: task_reference.clone(),
                task_history,
                extra: Default::default(),
            };

            let mut phase_log_path = PathBuf::from(directory);
            phase_log_path.push(format!("{}_log.json", phase_reference));

            let mut operation_history: Vec<OperationHistoryItem> = operation_history::read_or_default(&phase_log_path)?;

            operation_history.push(history_item);

            operation_history::write(phase_log_path, &operation_history)?;
        }
    }

    Ok(modified)
}

pub fn update_placement_orderings(
    project: &mut Project,
    reference: &Reference,
    placement_orderings: &Vec<PlacementSortingItem>,
) -> anyhow::Result<bool> {
    let phase = project
        .phases
        .get_mut(reference)
        .ok_or(PhaseError::UnknownPhase(reference.clone()))?;

    let modified = if phase
        .placement_orderings
        .eq(placement_orderings)
    {
        false
    } else {
        phase
            .placement_orderings
            .clone_from(placement_orderings);

        info!(
            "Phase placement orderings set. phase: '{}', orderings: [{}]",
            reference,
            placement_orderings
                .iter()
                .map(|item| {
                    format!(
                        "{}:{}",
                        item.mode
                            .to_string()
                            .to_shouty_snake_case(),
                        item.sort_order
                            .to_string()
                            .to_shouty_snake_case()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
        true
    };

    Ok(modified)
}

pub fn reset_operations(project: &mut Project) -> anyhow::Result<()> {
    reset_placement_operations(project);
    reset_phase_operations(project);

    refresh_phase_operation_states(project);

    Ok(())
}

fn reset_placement_operations(project: &mut Project) {
    for (_object_path, placement_state) in project.placements.iter_mut() {
        placement_state.operation_status = PlacementStatus::Pending;
    }

    info!("Placement operations reset.");
}

fn reset_phase_operations(project: &mut Project) {
    for (reference, phase_state) in project.phase_states.iter_mut() {
        phase_state.reset();
        info!("Phase operations reset. phase: {}", reference);
    }
}

pub fn find_phase_parts(
    project: &Project,
    phase_reference: &Reference,
    manufacturer_pattern: Regex,
    mpn_pattern: Regex,
) -> BTreeSet<Part> {
    project
        .placements
        .iter()
        .filter_map(|(_object_path, placement_state)| match &placement_state.phase {
            Some(candidate_phase) if candidate_phase.eq(phase_reference) => {
                if manufacturer_pattern.is_match(
                    &placement_state
                        .placement
                        .part
                        .manufacturer,
                ) && mpn_pattern.is_match(&placement_state.placement.part.mpn)
                {
                    Some(placement_state.placement.part.clone())
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
}

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("Unable to load placements, cause: {0}")]
    UnableToLoadPlacements(anyhow::Error),
    #[error("Unable to build unit positions. check pcb configuration. {0}")]
    UnableToBuildUnitPositions(String),
    #[error("Unable to assign phase to placements, cause: {0}")]
    UnableToAssignPhaseToPlacements(#[from] AssignmentError),
}

#[cfg(test)]
mod project_process_tests {
    use std::path::PathBuf;

    use pnp::part::Part;
    use pnp::pcb::PcbSide;
    use pnp::reference::Reference;

    use crate::file::FileReference;
    use crate::part::PartState;
    use crate::process::{ProcessDefinition, ProcessError, ProcessPresetFactory, TaskAction, TaskReference};
    use crate::project::{apply_phase_operation_task_action, Project};

    fn project_with_manual_process() -> Project {
        let mut project = Project::default();
        project
            .add_process(
                FileReference::Relative(PathBuf::from("manual.process.json")),
                ProcessPresetFactory::by_preset_name("manual").unwrap(),
            )
            .unwrap();
        project
    }

    fn renamed(definition: &ProcessDefinition, reference: &str) -> ProcessDefinition {
        let mut definition = definition.clone();
        definition.reference = Reference::from_raw_str(reference);
        definition
    }

    /// `PROCDEF-4`
    #[test]
    fn a_project_has_no_processes_until_they_are_added() {
        assert!(Project::default().processes.is_empty());
    }

    /// `PROCDEF-5`
    #[test]
    fn a_process_file_or_reference_can_only_be_added_once() {
        let mut project = project_with_manual_process();
        let manual = ProcessPresetFactory::by_preset_name("manual").unwrap();

        let same_file = project.add_process(
            FileReference::Relative(PathBuf::from("manual.process.json")),
            renamed(&manual, "other"),
        );
        assert!(matches!(same_file, Err(ProcessError::DuplicateProcessFile { .. })));

        let same_reference = project.add_process(FileReference::Relative(PathBuf::from("other.process.json")), manual);
        assert!(matches!(
            same_reference,
            Err(ProcessError::DuplicateProcessReference { .. })
        ));
    }

    /// `PROCDEF-6`
    #[test]
    fn renaming_a_process_renames_it_in_its_phases_and_parts() {
        let mut project = project_with_manual_process();
        let manual = project.processes[0].definition.clone();
        let part = Part::new("MFR1".to_string(), "MPN1".to_string());
        project
            .part_states
            .insert(part.clone(), PartState {
                applicable_processes: [manual.reference.clone()].into(),
            });
        project
            .update_phase(
                Reference::from_raw_str("top_1"),
                manual.reference.clone(),
                "load_out.csv".to_string(),
                PcbSide::Top,
            )
            .unwrap();

        let result = project.replace_process(&manual.reference, renamed(&manual, "hand"));

        assert_eq!(result.unwrap(), true);
        let hand = Reference::from_raw_str("hand");
        assert_eq!(project.phases[&Reference::from_raw_str("top_1")].process, hand);
        assert_eq!(project.part_states[&part].applicable_processes, [hand].into());
    }

    /// `PROCDEF-6`
    #[test]
    // HACK: core-io-hack - starting a task writes the phase's operation history, which the test removes.
    #[allow(clippy::disallowed_methods)]
    fn a_process_cannot_be_changed_once_a_phase_using_it_has_started() {
        let mut project = project_with_manual_process();
        let manual = project.processes[0].definition.clone();
        let phase = Reference::from_raw_str("top_1");
        project
            .update_phase(
                phase.clone(),
                manual.reference.clone(),
                "load_out.csv".to_string(),
                PcbSide::Top,
            )
            .unwrap();
        let directory = std::env::temp_dir();
        apply_phase_operation_task_action(
            &mut project,
            &directory,
            &phase,
            Reference::from_raw_str("load_pcbs"),
            TaskReference::from_raw_str("load_pcbs"),
            TaskAction::Start,
        )
        .unwrap();
        let _ = std::fs::remove_file(directory.join("top_1_log.json"));

        let result = project.replace_process(&manual.reference, renamed(&manual, "hand"));

        assert!(matches!(result, Err(ProcessError::ProcessInProgress { .. })));
        assert_eq!(project.processes[0].definition, manual);
    }

    /// `PROCDEF-8`
    #[test]
    fn a_project_from_before_processes_had_files_can_be_read() {
        let json = r#"{
            "name": "job1",
            "processes": [
                {
                    "reference": "manual",
                    "operations": [
                        { "reference": "load_pcbs", "tasks": ["core::load_pcbs"] },
                        { "reference": "manually_solder_components", "tasks": ["core::place_components", "core::manual_soldering"] }
                    ],
                    "rules": []
                }
            ]
        }"#;

        let project: Project = serde_json::from_str(json).unwrap();

        assert_eq!(project.processes_without_files(), vec![0]);
        let process = &project.processes[0].definition;
        assert_eq!(
            process.operations[1].tasks[0].reference,
            TaskReference::from_raw_str("core::place_components")
        );
        assert_eq!(process.validate(), Ok(()));
    }
}
