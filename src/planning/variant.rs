use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VariantName(String);

impl FromStr for VariantName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(VariantName(s.to_string()))
    }
}

impl Display for VariantName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}