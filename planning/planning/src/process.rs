//! PCBs are assembled using one or more processes.
//! Each assembly project has one or more phases.
//! Each phase uses a process.
//! Each process has one or more operations.
//! Each operation can be pending, incomplete, complete or abandoned.
//! Some operations just have a simple status, others require per-operation actions to be performed before they are
//! complete
//! Each operation can have one or more tasks.
//! Each task can be pending, incomplete, complete or abandoned.
//!
//! The user defines the processes, their operations and their tasks.  Each task has a reference chosen by the user, and
//! a [`TaskKind`], which is all the planner needs to know about what the task does.
//!
//! Phases can be abandoned or skipped.
//! Operations can be abandoned or skipped.
//!
//! Later operations and phases cannot be actioned unless preceding phases and actions are completed/skipped/abandoned.

use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::ops::ControlFlow;

use dyn_clone::DynClone;
use dyn_eq::DynEq;
use indexmap::IndexMap;
use pnp::reference::Reference;
use thiserror::Error;
use util::dynamic::as_any::AsAny;

use crate::file::FileReference;
use crate::phase::PhaseState;
use crate::placement::PlacementStatus;

/// e.g. `manual` or `pnp`
pub type ProcessReference = Reference;

/// The /definition/ of a process.
///
/// A process is defined in its own file, read and written by the stores service (`STORE-10`).  A project keeps a copy of the
/// definition of each process it uses, see `ProjectProcess`.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct ProcessDefinition {
    pub reference: ProcessReference,

    /// examples:
    /// for `manual` = `["load_pcbs", "manually_solder_components"]`
    /// for `pnp` = `["load_pcbs", "automated_pnp", "reflow_oven_soldering"]`
    ///
    pub operations: Vec<OperationDefinition>,

    /// examples: `["core::..."]`
    pub rules: Vec<ProcessRuleReference>,
}

/// A user defined (or pre-configured) process operation reference
/// e.g. "place-components"
pub type OperationReference = Reference;

/// The /definition/ of a process operation
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct OperationDefinition {
    /// e.g. "manually_solder_components"
    pub reference: OperationReference,

    /// e.g. `place_components` then `manual_soldering`, done in that order.
    /// @see [`OperationState`]
    pub tasks: Vec<TaskDefinition>,
}

/// A task reference chosen by the user, e.g. "apply_glue".
///
/// Projects from before tasks had kinds use the references of the tasks that were built into the planner, e.g.
/// "core::load_pcbs", see [`TaskKind::for_legacy_task`].
pub type TaskReference = Reference;

/// a namespaced rule reference. e.g. "core::unique_feeder_references"
pub type ProcessRuleReference = Reference;

/// The rules a process can have.
pub const PROCESS_RULES: [&str; 1] = ["core::unique_feeder_references"];

/// The /definition/ of a task.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
#[serde(try_from = "TaskDefinitionRepr")]
pub struct TaskDefinition {
    /// e.g. "apply_glue", unique within its operation.
    pub reference: TaskReference,
    pub kind: TaskKind,
}

impl TaskDefinition {
    pub fn new(reference: TaskReference, kind: TaskKind) -> Self {
        Self {
            reference,
            kind,
        }
    }
}

/// What the planner needs to know about what a task does.
///
/// Anything else a task does, e.g. applying solder paste or glue, curing glue, or wave soldering, is up to the user.
/// Such a task is [`TaskKind::Generic`].
#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord
)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// Started, and then completed or abandoned, by the user.
    Generic,
    /// As [`TaskKind::Generic`], and the project's report lists the PCBs to load.
    LoadPcbs,
    /// Places the phase's components.  It's complete once every placement has been placed or skipped.
    ///
    /// A process can have at most one.
    PlaceComponents,
}

impl TaskKind {
    pub const ALL: [TaskKind; 3] = [TaskKind::Generic, TaskKind::LoadPcbs, TaskKind::PlaceComponents];

    /// The kind of each task that was built into the planner before tasks had kinds.
    pub fn for_legacy_task(reference: &TaskReference) -> Option<TaskKind> {
        match reference.0.as_str() {
            "core::load_pcbs" => Some(TaskKind::LoadPcbs),
            "core::place_components" => Some(TaskKind::PlaceComponents),
            "core::automated_soldering" | "core::manual_soldering" => Some(TaskKind::Generic),
            _ => None,
        }
    }

    /// The kind's name in files and on the command line, e.g. `place_components`.
    pub fn name(&self) -> &'static str {
        match self {
            TaskKind::Generic => "generic",
            TaskKind::LoadPcbs => "load_pcbs",
            TaskKind::PlaceComponents => "place_components",
        }
    }
}

impl Display for TaskKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Unknown task kind. kind: '{0}', kinds: generic, load_pcbs, place_components")]
pub struct UnknownTaskKindError(String);

impl std::str::FromStr for TaskKind {
    type Err = UnknownTaskKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TaskKind::ALL
            .into_iter()
            .find(|kind| kind.name().eq(s))
            .ok_or_else(|| UnknownTaskKindError(s.to_string()))
    }
}

/// A task as stored in a file: a [`TaskDefinition`], or, in projects from before tasks had kinds, the reference of a
/// task that was built into the planner.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum TaskDefinitionRepr {
    Legacy(TaskReference),
    Current { reference: TaskReference, kind: TaskKind },
}

impl TryFrom<TaskDefinitionRepr> for TaskDefinition {
    type Error = String;

    fn try_from(value: TaskDefinitionRepr) -> Result<Self, Self::Error> {
        match value {
            TaskDefinitionRepr::Current {
                reference,
                kind,
            } => Ok(TaskDefinition {
                reference,
                kind,
            }),
            TaskDefinitionRepr::Legacy(reference) => match TaskKind::for_legacy_task(&reference) {
                Some(kind) => Ok(TaskDefinition {
                    reference,
                    kind,
                }),
                None => Err(format!("Unknown task. task: '{}'", reference)),
            },
        }
    }
}

impl ProcessDefinition {
    /// A process with no operations.
    pub fn new(reference: ProcessReference) -> Self {
        Self {
            reference,
            operations: vec![],
            rules: vec![],
        }
    }

    pub fn has_rule(&self, rule: &ProcessRuleReference) -> bool {
        self.rules.contains(rule)
    }

    pub fn find_task(&self, operation: &OperationReference, task: &TaskReference) -> Option<&TaskDefinition> {
        self.operations
            .iter()
            .find(|candidate| candidate.reference.eq(operation))
            .and_then(|operation| {
                operation
                    .tasks
                    .iter()
                    .find(|candidate| candidate.reference.eq(task))
            })
    }

    /// Checks that a phase's state can be built from the definition.
    pub fn validate(&self) -> Result<(), ProcessDefinitionError> {
        let mut operations = HashSet::new();
        let mut place_components_tasks = 0;

        for operation in self.operations.iter() {
            if !operations.insert(&operation.reference) {
                return Err(ProcessDefinitionError::DuplicateOperation {
                    process: self.reference.clone(),
                    operation: operation.reference.clone(),
                });
            }

            let mut tasks = HashSet::new();
            for task in operation.tasks.iter() {
                if !tasks.insert(&task.reference) {
                    return Err(ProcessDefinitionError::DuplicateTask {
                        process: self.reference.clone(),
                        operation: operation.reference.clone(),
                        task: task.reference.clone(),
                    });
                }
                if task.kind == TaskKind::PlaceComponents {
                    place_components_tasks += 1;
                }
            }
        }

        if place_components_tasks > 1 {
            return Err(ProcessDefinitionError::MoreThanOnePlaceComponentsTask {
                process: self.reference.clone(),
            });
        }

        Ok(())
    }

    /// Makes one change to the definition, as the CLI does.
    ///
    /// The definition is left unchanged if the change would make it invalid, see [`Self::validate`].
    ///
    /// Returns whether the definition changed.
    pub fn apply_edit(&mut self, edit: ProcessEdit) -> Result<bool, ProcessDefinitionError> {
        let mut candidate = self.clone();

        match edit {
            ProcessEdit::SetReference {
                reference,
            } => {
                candidate.reference = reference;
            }
            ProcessEdit::AddOperation {
                operation,
            } => {
                candidate
                    .operations
                    .push(OperationDefinition {
                        reference: operation,
                        tasks: vec![],
                    });
            }
            ProcessEdit::RemoveOperation {
                operation,
            } => {
                let index = candidate.operation_index(&operation)?;
                candidate.operations.remove(index);
            }
            ProcessEdit::AddTask {
                operation,
                task,
                kind,
            } => {
                let index = candidate.operation_index(&operation)?;
                candidate.operations[index]
                    .tasks
                    .push(TaskDefinition::new(task, kind));
            }
            ProcessEdit::RemoveTask {
                operation,
                task,
            } => {
                let index = candidate.operation_index(&operation)?;
                let tasks = &mut candidate.operations[index].tasks;
                let task_index = tasks
                    .iter()
                    .position(|candidate| candidate.reference.eq(&task))
                    .ok_or_else(|| ProcessDefinitionError::UnknownTask {
                        process: self.reference.clone(),
                        operation: operation.clone(),
                        task: task.clone(),
                    })?;
                tasks.remove(task_index);
            }
            ProcessEdit::AddRule {
                rule,
            } => {
                if !candidate.rules.contains(&rule) {
                    candidate.rules.push(rule);
                }
            }
            ProcessEdit::RemoveRule {
                rule,
            } => {
                candidate
                    .rules
                    .retain(|candidate| !candidate.eq(&rule));
            }
        }

        candidate.validate()?;

        let modified = candidate.ne(self);
        *self = candidate;

        Ok(modified)
    }

    fn operation_index(&self, operation: &OperationReference) -> Result<usize, ProcessDefinitionError> {
        self.operations
            .iter()
            .position(|candidate| candidate.reference.eq(operation))
            .ok_or_else(|| ProcessDefinitionError::UnknownOperation {
                process: self.reference.clone(),
                operation: operation.clone(),
            })
    }
}

/// One change to a process's definition, see [`ProcessDefinition::apply_edit`].
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum ProcessEdit {
    SetReference {
        reference: ProcessReference,
    },
    /// Adds the operation after the others.
    AddOperation {
        operation: OperationReference,
    },
    RemoveOperation {
        operation: OperationReference,
    },
    /// Adds the task after the operation's other tasks.
    AddTask {
        operation: OperationReference,
        task: TaskReference,
        kind: TaskKind,
    },
    RemoveTask {
        operation: OperationReference,
        task: TaskReference,
    },
    AddRule {
        rule: ProcessRuleReference,
    },
    RemoveRule {
        rule: ProcessRuleReference,
    },
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ProcessDefinitionError {
    #[error("Duplicate operation. process: '{process}', operation: '{operation}'")]
    DuplicateOperation {
        process: ProcessReference,
        operation: OperationReference,
    },
    #[error("Duplicate task. process: '{process}', operation: '{operation}', task: '{task}'")]
    DuplicateTask {
        process: ProcessReference,
        operation: OperationReference,
        task: TaskReference,
    },
    #[error("A process can have at most one task that places components. process: '{process}'")]
    MoreThanOnePlaceComponentsTask { process: ProcessReference },
    #[error("Unknown operation. process: '{process}', operation: '{operation}'")]
    UnknownOperation {
        process: ProcessReference,
        operation: OperationReference,
    },
    #[error("Unknown task. process: '{process}', operation: '{operation}', task: '{task}'")]
    UnknownTask {
        process: ProcessReference,
        operation: OperationReference,
        task: TaskReference,
    },
}

/// Processes a new process can be made from, so the user doesn't have to start from nothing.
pub struct ProcessPresetFactory {}

#[derive(Error, Debug)]
pub enum ProcessPresetFactoryError {
    #[error("Unknown process preset.  preset: {}", preset)]
    UnknownPreset { preset: String },
}

impl ProcessPresetFactory {
    pub fn available_presets() -> Vec<String> {
        vec!["pnp".to_string(), "manual".to_string()]
    }

    /// The process's reference is the preset's name.
    pub fn by_preset_name(name: &str) -> Result<ProcessDefinition, ProcessPresetFactoryError> {
        let task = |reference: &str, kind: TaskKind| TaskDefinition::new(TaskReference::from_raw_str(reference), kind);

        match name {
            "pnp" => Ok(ProcessDefinition {
                reference: ProcessReference::from_raw_str("pnp"),
                operations: vec![
                    OperationDefinition {
                        reference: Reference::from_raw_str("load_pcbs"),
                        tasks: vec![task("load_pcbs", TaskKind::LoadPcbs)],
                    },
                    OperationDefinition {
                        reference: Reference::from_raw_str("automated_pnp"),
                        tasks: vec![task("place_components", TaskKind::PlaceComponents)],
                    },
                    OperationDefinition {
                        reference: Reference::from_raw_str("reflow_oven_soldering"),
                        tasks: vec![task("automated_soldering", TaskKind::Generic)],
                    },
                ],
                rules: vec![ProcessRuleReference::from_raw_str("core::unique_feeder_references")],
            }),
            "manual" => Ok(ProcessDefinition {
                reference: ProcessReference::from_raw_str("manual"),
                operations: vec![
                    OperationDefinition {
                        reference: Reference::from_raw_str("load_pcbs"),
                        tasks: vec![task("load_pcbs", TaskKind::LoadPcbs)],
                    },
                    OperationDefinition {
                        reference: Reference::from_raw_str("manually_solder_components"),
                        tasks: vec![
                            task("place_components", TaskKind::PlaceComponents),
                            task("manual_soldering", TaskKind::Generic),
                        ],
                    },
                ],
                rules: vec![],
            }),
            preset => Err(ProcessPresetFactoryError::UnknownPreset {
                preset: preset.to_string(),
            }),
        }
    }
}

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Undefined process. processes: {:?}, process: '{}'", processes, process)]
    UndefinedProcessError {
        processes: Vec<ProcessReference>,
        process: String,
    },
    #[error("Process in use. process: '{}'", process_reference)]
    ProcessInUse { process_reference: ProcessReference },
    #[error("Process in progress. process: '{}'", process_reference)]
    ProcessInProgress { process_reference: ProcessReference },
    #[error("Duplicate process reference. process: '{}'", process_reference)]
    DuplicateProcessReference { process_reference: ProcessReference },
    #[error("Process file already added. file: {}", process_file)]
    DuplicateProcessFile { process_file: FileReference },
    #[error("Unknown pre-set process. available-presets: {:?}, pre-set: {}", presets, preset)]
    UnknownPreset { presets: Vec<String>, preset: String },
    #[error("Invalid process. cause: {0}")]
    InvalidProcess(ProcessDefinitionError),
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Clone)]
pub struct OperationState {
    pub reference: OperationReference,
    pub task_states: IndexMap<TaskReference, Box<dyn SerializableTaskState>>,
}

impl OperationState {
    /// Determine the operation status based on the tasks
    pub fn status(&self) -> OperationStatus {
        let result = self
            .task_states
            .iter()
            .rev()
            .try_fold(None, |status, (_task_reference, task_state)| {
                //trace!("acc: {:?}, task_status: {:?}", status, task_state.status());
                match (status, task_state.status()) {
                    (None, TaskStatus::Abandoned) => ControlFlow::Break(OperationStatus::Abandoned),
                    (None, TaskStatus::Complete) => ControlFlow::Break(OperationStatus::Complete),
                    (None, TaskStatus::Started) => ControlFlow::Break(OperationStatus::Started),
                    (None, TaskStatus::Pending) => ControlFlow::Continue(Some(OperationStatus::Pending)),
                    (Some(OperationStatus::Pending), TaskStatus::Abandoned) => {
                        ControlFlow::Break(OperationStatus::Abandoned)
                    }
                    (Some(OperationStatus::Pending), TaskStatus::Started) => {
                        ControlFlow::Break(OperationStatus::Started)
                    }
                    (Some(OperationStatus::Pending), TaskStatus::Complete) => {
                        ControlFlow::Break(OperationStatus::Started)
                    }
                    (Some(OperationStatus::Pending), TaskStatus::Pending) => {
                        ControlFlow::Continue(Some(OperationStatus::Pending))
                    }
                    (_, _) => unreachable!(),
                }
            });
        //trace!("result: {:?}", result);

        match result {
            // no tasks
            ControlFlow::Continue(None) => OperationStatus::Pending,

            // some tasks
            ControlFlow::Continue(Some(value)) => value,
            ControlFlow::Break(value) => value,
        }
    }
}

#[cfg(test)]
mod operation_state_tests {
    use indexmap::IndexMap;
    use pnp::reference::Reference;
    use rstest::rstest;

    use crate::process::{
        OperationState, OperationStatus, SerializableTaskState, TaskReference, TaskStatus, TestTaskState,
    };

    #[rstest]
    #[case(vec![TaskStatus::Pending], OperationStatus::Pending)]
    #[case(vec![TaskStatus::Complete], OperationStatus::Complete)]
    #[case(vec![TaskStatus::Abandoned], OperationStatus::Abandoned)]
    #[case(vec![TaskStatus::Started], OperationStatus::Started)]
    #[case(vec![TaskStatus::Pending, TaskStatus::Pending], OperationStatus::Pending)]
    #[case(vec![TaskStatus::Started, TaskStatus::Pending], OperationStatus::Started)]
    #[case(vec![TaskStatus::Complete, TaskStatus::Pending], OperationStatus::Started)]
    #[case(vec![TaskStatus::Complete, TaskStatus::Started], OperationStatus::Started)]
    #[case(vec![TaskStatus::Complete, TaskStatus::Complete], OperationStatus::Complete)]
    #[case(vec![TaskStatus::Complete, TaskStatus::Abandoned], OperationStatus::Abandoned)]
    #[case(vec![TaskStatus::Pending, TaskStatus::Pending, TaskStatus::Pending], OperationStatus::Pending)]
    #[case(vec![TaskStatus::Complete, TaskStatus::Started, TaskStatus::Pending], OperationStatus::Started)]
    #[case(vec![TaskStatus::Complete, TaskStatus::Complete, TaskStatus::Pending], OperationStatus::Started)]
    #[case(vec![TaskStatus::Complete, TaskStatus::Abandoned, TaskStatus::Pending], OperationStatus::Abandoned)]
    #[case(vec![], OperationStatus::Pending)]
    fn status(#[case] task_statuses: Vec<TaskStatus>, #[case] expected_result: OperationStatus) {
        // given
        let task_states = task_statuses
            .into_iter()
            .enumerate()
            .map(|(index, status)| {
                (
                    TaskReference::from_raw(index.to_string()),
                    Box::new(TestTaskState {
                        status,
                    }) as Box<dyn SerializableTaskState>,
                )
            })
            .collect::<IndexMap<_, _>>();

        let operation_state = OperationState {
            reference: Reference::from_raw_str("test"),
            task_states,
        };

        // when
        let result = operation_state.status();

        assert_eq!(result, expected_result);
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Clone)]
pub enum OperationStatus {
    Pending,
    Started,
    Complete,
    Abandoned,
}

#[typetag::serde(tag = "type")]
pub trait SerializableTaskState: TaskState + DynEq + DynClone + AsAny + Send + Sync + Debug {}
dyn_eq::eq_trait_object!(SerializableTaskState);
dyn_clone::clone_trait_object!(SerializableTaskState);

pub trait TaskState {
    fn status(&self) -> TaskStatus;
    fn reset(&mut self);

    fn is_complete(&self) -> bool {
        matches!(self.status(), TaskStatus::Complete)
    }

    fn can_complete(&self) -> bool;

    fn is_pending(&self) -> bool {
        matches!(self.status(), TaskStatus::Pending)
    }

    fn set_started(&mut self);

    /// Will panic if not completable.
    ///
    /// See [`Self::can_complete`]
    fn set_completed(&mut self);

    fn set_abandoned(&mut self);

    /// Allows callers to access this operation's placements api
    fn placements_state(&self) -> Option<&dyn PlacementsTaskState> {
        None::<&dyn PlacementsTaskState>
    }
    fn placements_state_mut(&mut self) -> Option<&mut dyn PlacementsTaskState> {
        None::<&mut dyn PlacementsTaskState>
    }

    fn requires_placements(&self) -> bool {
        self.placements_state().is_some()
    }
}

pub trait PlacementsTaskState: AsAny {
    fn reset(&mut self);

    fn on_placement_status_change(&mut self, old_status: &PlacementStatus, new_status: &PlacementStatus);

    fn set_total_placements(&mut self, total: usize);

    fn summary(&self) -> PlacementTaskSummary;
}

#[derive(Clone, Debug)]
pub struct PlacementTaskSummary {
    pub placed: usize,
    pub skipped: usize,
    pub total: usize,
}

/// Allowed transitions
///
/// 1) Pending -> Started
/// 2) Started -> Complete | Abandoned
/// 3) * -> Pending (reset)
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Started,
    Complete,
    Abandoned,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl Display for TaskStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => f.write_str("Pending"),
            TaskStatus::Started => f.write_str("Started"),
            TaskStatus::Complete => f.write_str("Complete"),
            TaskStatus::Abandoned => f.write_str("Abandoned"),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
pub struct PlacementTaskState {
    pub placed: usize,
    pub skipped: usize,
    pub total: usize,

    // FUTURE consider using some struct that wraps the status to prevent disallowed state changes
    status: TaskStatus,
}

#[typetag::serde(name = "core::placement_task_state")]
impl SerializableTaskState for PlacementTaskState {}

impl PlacementTaskState {
    pub fn are_all_placements_placed(&self) -> bool {
        (self.placed + self.skipped) == self.total
    }

    fn refresh_status(&mut self) {
        // it is invalid to have a total == 0 and any items placed or skipped
        // ensure total is set BEFORE placed or skipped.
        assert!(!(self.total == 0 && (self.placed > 0 || self.skipped > 0)));

        let should_check = match self.status {
            TaskStatus::Pending => true,
            TaskStatus::Started => true,
            TaskStatus::Complete => true,
            TaskStatus::Abandoned => false,
        };

        if should_check {
            if self.total > 0 {
                if self.are_all_placements_placed() {
                    self.status = TaskStatus::Complete;
                } else if self.placed > 0 || self.skipped > 0 {
                    self.status = TaskStatus::Started;
                }
            } else {
                self.status = TaskStatus::Pending;
            }
        }
    }
}

impl TaskState for PlacementTaskState {
    fn status(&self) -> TaskStatus {
        self.status.clone()
    }

    fn reset(&mut self) {
        *self = Self::default()
    }

    /// This task can only be completed via other apis
    fn can_complete(&self) -> bool {
        false
    }

    fn set_started(&mut self) {
        self.status = TaskStatus::Started;
    }

    /// See [`Self::can_complete`]
    fn set_completed(&mut self) {
        panic!("task cannot be completed this way");
    }

    fn set_abandoned(&mut self) {
        self.status = TaskStatus::Abandoned;
    }

    fn placements_state(&self) -> Option<&dyn PlacementsTaskState> {
        Some(self)
    }

    fn placements_state_mut(&mut self) -> Option<&mut dyn PlacementsTaskState> {
        Some(self)
    }
}

impl PlacementsTaskState for PlacementTaskState {
    fn reset(&mut self) {
        self.placed = 0;
        self.skipped = 0;
        self.total = 0;
    }

    fn on_placement_status_change(&mut self, old_status: &PlacementStatus, new_status: &PlacementStatus) {
        match old_status {
            PlacementStatus::Placed => self.placed -= 1,
            PlacementStatus::Skipped => self.skipped -= 1,
            PlacementStatus::Pending => {}
        }
        match new_status {
            PlacementStatus::Placed => self.placed += 1,
            PlacementStatus::Skipped => self.skipped += 1,
            PlacementStatus::Pending => {}
        }
        self.refresh_status();
    }

    fn set_total_placements(&mut self, total: usize) {
        self.total = total;
        self.refresh_status();
    }

    fn summary(&self) -> PlacementTaskSummary {
        PlacementTaskSummary {
            placed: self.placed,
            skipped: self.skipped,
            total: self.total,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum TaskAction {
    Start,
    Complete,
    Abandon,
}

macro_rules! generic_task_impl {
    ( $name:ident, $key:literal ) => {
        #[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
        pub struct $name {
            status: TaskStatus,
        }

        #[typetag::serde(name = $key)]
        impl SerializableTaskState for $name {}

        impl TaskState for $name {
            fn status(&self) -> TaskStatus {
                self.status.clone()
            }

            fn reset(&mut self) {
                *self = Self::default()
            }

            fn can_complete(&self) -> bool {
                true
            }

            fn set_started(&mut self) {
                self.status = TaskStatus::Started;
            }

            fn set_completed(&mut self) {
                self.status = TaskStatus::Complete;
            }

            fn set_abandoned(&mut self) {
                self.status = TaskStatus::Abandoned;
            }
        }
    };
}

generic_task_impl!(GenericTaskState, "core::generic_task_state");

// The states of the tasks that were built into the planner before tasks had kinds.  They're only read from projects, and
// behave the same as a `GenericTaskState`.
generic_task_impl!(LoadPcbsTaskState, "core::load_pcbs_task_state");
generic_task_impl!(AutomatedSolderingTaskState, "core::automated_soldering_task_state");
generic_task_impl!(ManualSolderingTaskState, "core::manual_soldering_task_state");
#[cfg(test)]
generic_task_impl!(TestTaskState, "core::test_task_state");

/// The state of a task of the given kind, before it has started.
pub fn make_task_state(kind: TaskKind) -> Box<dyn SerializableTaskState> {
    match kind {
        TaskKind::Generic | TaskKind::LoadPcbs => Box::new(GenericTaskState::default()),
        TaskKind::PlaceComponents => Box::new(PlacementTaskState::default()),
    }
}

/// Checks if an operation can be modified:
/// - All preceding operations must be complete
/// - All following operations must be pending
pub fn can_modify_operation(phase_state: &PhaseState, op_index: usize) -> bool {
    // Check that all preceding operations are complete
    let all_preceding_ops_complete = phase_state
        .operation_states
        .iter()
        .take(op_index)
        .all(|op| matches!(op.status(), OperationStatus::Complete));

    // Check that all following operations are pending
    let all_following_ops_pending = phase_state
        .operation_states
        .iter()
        .skip(op_index + 1)
        .all(|op| matches!(op.status(), OperationStatus::Pending));

    all_preceding_ops_complete && all_following_ops_pending
}

/// Checks if a task can be modified:
/// - All preceding tasks must be complete
/// - All following tasks must be pending
pub fn can_modify_task(operation_state: &OperationState, task_index: usize) -> bool {
    // Check that all preceding tasks are complete
    let all_preceding_tasks_complete = operation_state
        .task_states
        .iter()
        .take(task_index)
        .all(|(_, task)| matches!(task.status(), TaskStatus::Complete));

    // Check that all following tasks are pending
    let all_following_tasks_pending = operation_state
        .task_states
        .iter()
        .skip(task_index + 1)
        .all(|(_, task)| matches!(task.status(), TaskStatus::Pending));

    all_preceding_tasks_complete && all_following_tasks_pending
}

#[cfg(test)]
mod process_definition_tests {
    use pnp::reference::Reference;

    use super::{
        OperationDefinition, ProcessDefinition, ProcessDefinitionError, ProcessEdit, ProcessPresetFactory,
        TaskDefinition, TaskKind,
    };

    fn task(reference: &str, kind: TaskKind) -> TaskDefinition {
        TaskDefinition::new(Reference::from_raw_str(reference), kind)
    }

    fn process(operations: Vec<(&str, Vec<TaskDefinition>)>) -> ProcessDefinition {
        ProcessDefinition {
            reference: Reference::from_raw_str("glue_and_wave"),
            operations: operations
                .into_iter()
                .map(|(reference, tasks)| OperationDefinition {
                    reference: Reference::from_raw_str(reference),
                    tasks,
                })
                .collect(),
            rules: vec![],
        }
    }

    /// `PROCDEF-2`
    #[test]
    fn a_task_is_stored_with_its_kind() {
        let definition = task("apply_glue", TaskKind::Generic);

        let json = serde_json::to_string(&definition).unwrap();

        assert_eq!(json, r#"{"reference":"apply_glue","kind":"generic"}"#);
        assert_eq!(serde_json::from_str::<TaskDefinition>(&json).unwrap(), definition);
    }

    /// `PROCDEF-8`
    #[test]
    fn a_legacy_task_is_read_with_the_kind_of_the_task_built_into_the_planner() {
        let tasks: Vec<TaskDefinition> = serde_json::from_str(
            r#"["core::load_pcbs", "core::place_components", "core::automated_soldering", "core::manual_soldering"]"#,
        )
        .unwrap();

        assert_eq!(tasks, vec![
            task("core::load_pcbs", TaskKind::LoadPcbs),
            task("core::place_components", TaskKind::PlaceComponents),
            task("core::automated_soldering", TaskKind::Generic),
            task("core::manual_soldering", TaskKind::Generic),
        ]);
    }

    /// `PROCDEF-8`
    #[test]
    fn an_unknown_legacy_task_is_an_error() {
        let result = serde_json::from_str::<TaskDefinition>(r#""core::unknown""#);

        assert!(result.is_err());
    }

    /// `PROCDEF-4`
    #[test]
    fn the_presets_are_valid() {
        for preset in ProcessPresetFactory::available_presets() {
            let definition = ProcessPresetFactory::by_preset_name(&preset).unwrap();
            assert_eq!(definition.validate(), Ok(()), "preset: {}", preset);
        }
    }

    /// `PROCDEF-2`
    #[test]
    fn a_process_can_attach_components_without_placing_them_by_machine_or_reflow() {
        // glue, place, cure, then wave solder
        let definition = process(vec![
            ("load", vec![task("load_pcbs", TaskKind::LoadPcbs)]),
            ("glue", vec![task("apply_glue", TaskKind::Generic)]),
            ("place", vec![task("place_components", TaskKind::PlaceComponents)]),
            ("cure", vec![task("cure_glue", TaskKind::Generic)]),
            ("wave", vec![task("wave_solder", TaskKind::Generic)]),
        ]);

        assert_eq!(definition.validate(), Ok(()));
    }

    /// `PROCDEF-3`
    #[test]
    fn a_process_can_have_only_one_task_that_places_components() {
        let definition = process(vec![
            ("place_top", vec![task("place", TaskKind::PlaceComponents)]),
            ("place_again", vec![task("place", TaskKind::PlaceComponents)]),
        ]);

        assert_eq!(
            definition.validate(),
            Err(ProcessDefinitionError::MoreThanOnePlaceComponentsTask {
                process: Reference::from_raw_str("glue_and_wave"),
            })
        );
    }

    /// `PROCDEF-3`
    #[test]
    fn operations_and_tasks_within_an_operation_must_be_unique() {
        let duplicate_operation = process(vec![("glue", vec![]), ("glue", vec![])]);
        assert!(matches!(
            duplicate_operation.validate(),
            Err(ProcessDefinitionError::DuplicateOperation { .. })
        ));

        let duplicate_task = process(vec![("glue", vec![
            task("apply", TaskKind::Generic),
            task("apply", TaskKind::Generic),
        ])]);
        assert!(matches!(
            duplicate_task.validate(),
            Err(ProcessDefinitionError::DuplicateTask { .. })
        ));

        // the same task in different operations is fine
        let same_task_in_two_operations = process(vec![
            ("glue_top", vec![task("apply", TaskKind::Generic)]),
            ("glue_bottom", vec![task("apply", TaskKind::Generic)]),
        ]);
        assert_eq!(same_task_in_two_operations.validate(), Ok(()));
    }

    /// `PROCDEF-1`
    #[test]
    fn edits_build_a_process() {
        let mut definition = ProcessDefinition::new(Reference::from_raw_str("glue_and_wave"));

        let edits = vec![
            ProcessEdit::AddOperation {
                operation: Reference::from_raw_str("glue"),
            },
            ProcessEdit::AddTask {
                operation: Reference::from_raw_str("glue"),
                task: Reference::from_raw_str("apply_glue"),
                kind: TaskKind::Generic,
            },
            ProcessEdit::AddTask {
                operation: Reference::from_raw_str("glue"),
                task: Reference::from_raw_str("place_components"),
                kind: TaskKind::PlaceComponents,
            },
        ];
        for edit in edits {
            assert_eq!(definition.apply_edit(edit), Ok(true));
        }

        assert_eq!(
            definition,
            process(vec![("glue", vec![
                task("apply_glue", TaskKind::Generic),
                task("place_components", TaskKind::PlaceComponents)
            ])])
        );
    }

    /// `PROCDEF-3`
    #[test]
    fn an_edit_that_would_make_the_process_invalid_changes_nothing() {
        let original = process(vec![("place", vec![task("place", TaskKind::PlaceComponents)])]);
        let mut definition = original.clone();

        let result = definition.apply_edit(ProcessEdit::AddTask {
            operation: Reference::from_raw_str("place"),
            task: Reference::from_raw_str("place_again"),
            kind: TaskKind::PlaceComponents,
        });

        assert!(result.is_err());
        assert_eq!(definition, original);
    }

    /// `PROCDEF-3`
    #[test]
    fn editing_an_unknown_operation_is_an_error() {
        let mut definition = process(vec![]);

        let result = definition.apply_edit(ProcessEdit::RemoveOperation {
            operation: Reference::from_raw_str("glue"),
        });

        assert!(matches!(result, Err(ProcessDefinitionError::UnknownOperation { .. })));
    }
}
