use std::fmt::{Display, Formatter};

use indexmap::{IndexMap, IndexSet};
use pnp::pcb::PcbSide;
use pnp::reference::Reference;
use thiserror::Error;

use crate::placement::{PlacementOperation, PlacementSortingItem};
use crate::process::{
    can_modify_operation, can_modify_task, make_task_state, OperationReference, OperationState, OperationStatus,
    ProcessDefinition, ProcessReference, TaskReference, TaskStatus,
};

pub type PhaseReference = Reference;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phase {
    pub reference: PhaseReference,
    pub process: ProcessReference,

    pub load_out_source: String,

    // TODO consider adding PCB unit + SIDE assignments to the phase instead of just a single side
    pub pcb_side: PcbSide,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub placement_orderings: Vec<PlacementSortingItem>,
}

#[derive(Error, Debug)]
pub enum PhaseError {
    #[error("Unknown phase. phase: '{0:}'")]
    UnknownPhase(Reference),

    #[error("Invalid operation for phase. phase: '{}', operation: '{}', possible_operations: {:?}", .0, .1, .2.iter().map(|reference|reference.to_string()).collect::<Vec<_>>())]
    InvalidOperationForPhase(Reference, OperationReference, Vec<OperationReference>),
    #[error("Invalid task for operation. phase: '{}', operation: '{}', task: '{}', possible_tasks: {:?}", .0, .1, .2, .3.iter().map(|reference|reference.to_string()).collect::<Vec<_>>())]
    InvalidTaskForOperation(Reference, OperationReference, TaskReference, Vec<TaskReference>),
    // #[error("Preceding operation for phase incomplete. phase: '{0:}', preceding_operation: {1:?}")]
    // PrecedingOperationIncomplete(Reference, OperationReference),
    #[error("Phase in use. phase: '{0:}'")]
    PhaseInUse(Reference),
    #[error("Unknown process. process: '{0:}'")]
    UnknownProcess(ProcessReference),
}

pub struct PhaseOrderings<'a>(pub &'a IndexSet<Reference>);

impl<'a> Display for PhaseOrderings<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "['{}']",
            self.0
                .iter()
                .map(Reference::to_string)
                .collect::<Vec<String>>()
                .join("', '")
        )
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct PhaseState {
    // the order of operations must be preserved.
    pub operation_states: Vec<OperationState>,
}

impl PhaseState {
    // Safety: all process must be valid
    pub fn from_process(process: &ProcessDefinition) -> Self {
        let operation_states = process
            .operations
            .iter()
            .map(|process_operation| {
                let task_states = process_operation
                    .tasks
                    .iter()
                    .map(|task| (task.reference.clone(), make_task_state(task.kind)))
                    .collect::<IndexMap<_, _>>();

                OperationState {
                    reference: process_operation.reference.clone(),
                    task_states,
                }
            })
            .collect::<Vec<_>>();

        Self {
            operation_states,
        }
    }

    pub fn reset(&mut self) {
        for state in self.operation_states.iter_mut() {
            for (_task_reference, task_state) in state.task_states.iter_mut() {
                task_state.reset()
            }
        }
    }

    pub fn is_pending(&self) -> bool {
        self.operation_states
            .iter()
            .all(|os| os.status() == OperationStatus::Pending)
    }

    pub fn is_complete(&self) -> bool {
        self.operation_states
            .iter()
            .all(|os| os.status() == OperationStatus::Complete)
    }

    pub fn is_abandoned(&self) -> bool {
        self.operation_states
            .iter()
            .any(|os| os.status() == OperationStatus::Abandoned)
    }

    pub fn status(&self) -> PhaseStatus {
        if self.is_pending() {
            PhaseStatus::Pending
        } else if self.is_complete() {
            PhaseStatus::Complete
        } else if self.is_abandoned() {
            PhaseStatus::Abandoned
        } else {
            PhaseStatus::Incomplete
        }
    }

    /// Whether `operation` can be recorded for this phase's placements, and if so, which task records it.
    ///
    /// The same rules as recording a placements operation: the task must have been started, or be complete for a reset.
    pub fn can_modify_placements(
        &self,
        operation: &PlacementOperation,
    ) -> Result<(&OperationReference, &TaskReference), PlacementsStateError> {
        self.operation_states
            .iter()
            .enumerate()
            .find_map(|(op_index, operation_state)| {
                operation_state
                    .task_states
                    .iter()
                    .enumerate()
                    .find_map(|(task_index, (task_reference, task_state))| {
                        if !task_state.requires_placements() {
                            return None;
                        }

                        if !can_modify_operation(self, op_index) {
                            return Some(Err(PlacementsStateError::InvalidOperationState));
                        }

                        if !can_modify_task(operation_state, task_index) {
                            return Some(Err(PlacementsStateError::InvalidTaskState));
                        }

                        let can_change = match (operation, task_state.status()) {
                            (PlacementOperation::Reset, TaskStatus::Complete) => true,
                            (_, TaskStatus::Started) => true,
                            _ => false,
                        };
                        if !can_change {
                            return Some(Err(PlacementsStateError::InvalidTaskStatus));
                        }

                        Some(Ok((&operation_state.reference, task_reference)))
                    })
            })
            .unwrap_or(Err(PlacementsStateError::NoPlacementsTask))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlacementsStateError {
    InvalidOperationState,
    InvalidTaskState,
    /// The task hasn't been started, or for a reset, isn't complete either.
    InvalidTaskStatus,
    NoPlacementsTask,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum PhaseStatus {
    Pending,
    Incomplete,
    Complete,
    Abandoned,
}

#[cfg(test)]
mod can_modify_placements_tests {
    use indexmap::IndexMap;
    use rstest::rstest;

    use super::{PhaseState, PlacementsStateError};
    use crate::placement::{PlacementOperation, PlacementStatus};
    use crate::process::{
        OperationReference, OperationState, PlacementTaskState, PlacementsTaskState, SerializableTaskState,
        TaskReference, TaskState, TaskStatus, TestTaskState,
    };

    fn phase_state(load_status: TaskStatus, placements_status: TaskStatus) -> PhaseState {
        let mut load = TestTaskState::default();
        match load_status {
            TaskStatus::Pending => {}
            TaskStatus::Started => load.set_started(),
            TaskStatus::Complete => {
                load.set_started();
                load.set_completed();
            }
            TaskStatus::Abandoned => {
                load.set_started();
                load.set_abandoned();
            }
        }

        let mut placements = PlacementTaskState::default();
        match placements_status {
            TaskStatus::Pending => {}
            TaskStatus::Started => placements.set_started(),
            TaskStatus::Complete => {
                placements.set_total_placements(1);
                placements.on_placement_status_change(&PlacementStatus::Pending, &PlacementStatus::Placed);
            }
            TaskStatus::Abandoned => {
                placements.set_started();
                placements.set_abandoned();
            }
        }
        assert_eq!(placements.status(), placements_status);

        let operation = |reference: &str, task: &str, state: Box<dyn SerializableTaskState>| OperationState {
            reference: OperationReference::from_raw_str(reference),
            task_states: IndexMap::from([(TaskReference::from_raw_str(task), state)]),
        };

        PhaseState {
            operation_states: vec![
                operation("load", "core::test_task", Box::new(load)),
                operation("place", "core::place_components", Box::new(placements)),
            ],
        }
    }

    #[rstest]
    #[case(
        TaskStatus::Pending,
        PlacementOperation::Place,
        Err(PlacementsStateError::InvalidTaskStatus)
    )]
    #[case(
        TaskStatus::Pending,
        PlacementOperation::Skip,
        Err(PlacementsStateError::InvalidTaskStatus)
    )]
    #[case(
        TaskStatus::Pending,
        PlacementOperation::Reset,
        Err(PlacementsStateError::InvalidTaskStatus)
    )]
    #[case(TaskStatus::Started, PlacementOperation::Place, Ok(()))]
    #[case(TaskStatus::Started, PlacementOperation::Skip, Ok(()))]
    #[case(TaskStatus::Started, PlacementOperation::Reset, Ok(()))]
    #[case(
        TaskStatus::Complete,
        PlacementOperation::Place,
        Err(PlacementsStateError::InvalidTaskStatus)
    )]
    #[case(
        TaskStatus::Complete,
        PlacementOperation::Skip,
        Err(PlacementsStateError::InvalidTaskStatus)
    )]
    #[case(TaskStatus::Complete, PlacementOperation::Reset, Ok(()))]
    #[case(
        TaskStatus::Abandoned,
        PlacementOperation::Place,
        Err(PlacementsStateError::InvalidTaskStatus)
    )]
    #[case(
        TaskStatus::Abandoned,
        PlacementOperation::Reset,
        Err(PlacementsStateError::InvalidTaskStatus)
    )]
    fn task_status(
        #[case] placements_status: TaskStatus,
        #[case] operation: PlacementOperation,
        #[case] expected_result: Result<(), PlacementsStateError>,
    ) {
        // given
        let phase_state = phase_state(TaskStatus::Complete, placements_status);

        // when
        let result = phase_state
            .can_modify_placements(&operation)
            .map(|(operation_reference, task_reference)| {
                assert_eq!(operation_reference, &OperationReference::from_raw_str("place"));
                assert_eq!(task_reference, &TaskReference::from_raw_str("core::place_components"));
            });

        // then
        assert_eq!(result, expected_result);
    }

    #[test]
    fn preceding_operations_must_be_complete() {
        // given
        let phase_state = phase_state(TaskStatus::Started, TaskStatus::Pending);

        // when
        let result = phase_state.can_modify_placements(&PlacementOperation::Place);

        // then
        assert_eq!(result, Err(PlacementsStateError::InvalidOperationState));
    }
}
