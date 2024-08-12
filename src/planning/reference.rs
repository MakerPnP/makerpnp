use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reference(String);

impl FromStr for Reference {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Reference(s.to_string()))
    }
}

impl Display for Reference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}