use std::str::FromStr;
use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessName(pub String);

#[derive(Debug, Error)]
#[error("Process name error")] 
pub struct ProcessNameError;

impl FromStr for ProcessName {
    type Err = ProcessNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ProcessName(s.to_string()))
    }
}

impl Display for ProcessName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Process {
    pub name: ProcessName,
    pub operations: Vec<ProcessOperationKind>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessOperationKind {
    LoadPcbs,
    AutomatedPnp,
    ReflowComponents,
    ManuallySolderComponents,
}

impl Process {
    pub fn has_operation(&self, operation: &ProcessOperationKind) -> bool {
        self.operations.contains(operation)
    }
}

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Unused process. processes: {:?}, process: '{}'", processes, process)]
    UnusedProcessError { processes: Vec<Process>, process: String }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq)]
pub struct ProcessOperationState {
    pub status: ProcessOperationStatus,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<ProcessOperationExtraState>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum ProcessOperationStatus {
    Pending,
    Incomplete,
    Complete
}

impl Default for ProcessOperationStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum ProcessOperationExtraState {
    PlacementOperation { placements_state: PlacementsState },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq)]
pub struct PlacementsState {
    pub placed: usize,
    pub total: usize,
}

impl PlacementsState {
    pub fn are_all_placements_placed(&self) -> bool {
        self.placed == self.total
    }
}

pub enum ProcessOperationSetItem {
    Completed
}