use std::fmt::{Display, Formatter};
use std::str::FromStr;
use thiserror::Error;
use crate::planning::variant::VariantName;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DesignVariant {
    pub design_name: DesignName,
    pub variant_name: VariantName,
}

impl Display for DesignVariant {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.design_name, self.variant_name)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DesignName(String);

impl FromStr for DesignName {
    type Err = DesignNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DesignName(s.to_string()))
    }
}

impl Display for DesignName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Debug, Error)]
#[error("Design name error")]
pub struct DesignNameError;
