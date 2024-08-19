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
