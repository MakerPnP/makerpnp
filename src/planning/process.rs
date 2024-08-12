use std::str::FromStr;
use std::fmt::{Display, Formatter};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Process(String);

impl Process {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
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