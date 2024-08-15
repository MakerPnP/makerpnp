use std::str::FromStr;
use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Process(String);

impl Process {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }
    
    pub fn is_pnp(&self) -> bool {
        self.0.to_lowercase().eq("pnp")
    }
}

impl FromStr for Process {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Process(s.to_string()))
    }
}

impl Display for Process {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Unused process. processes: {:?}, process: '{}'", processes, process)]
    UnusedProcessError { processes: Vec<Process>, process: Process }
}

pub fn assert_process(process: &Process, processes: &[Process]) -> Result<(), ProcessError> {
    if !processes.contains(&process) {
        Err(ProcessError::UnusedProcessError { processes: Vec::from(processes), process: process.clone() })
    } else {
        Ok(())
    }
}
