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
    pub is_pnp: bool,
}

impl Process {
    pub fn is_pnp(&self) -> bool {
        self.is_pnp
    }
}

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Unused process. processes: {:?}, process: '{}'", processes, process)]
    UnusedProcessError { processes: Vec<Process>, process: String }
}
