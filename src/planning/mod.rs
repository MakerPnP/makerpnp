use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all(deserialize = "snake_case"))]
pub struct Project {
    pub name: String,
    pub unit_assignments: Vec<UnitAssignment>,
}

impl Project {
    pub fn add_assignment(&mut self, unit_assignment: UnitAssignment) {
        // TODO check to see if assignment already exists
        self.unit_assignments.push(unit_assignment)
    }
}

impl Project {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_string(),
            unit_assignments: vec![],
        }
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct UnitAssignment {
    pub unit_path: UnitPath,
    pub design_name: DesignName,
    pub variant_name: VariantName,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct DesignName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct VariantName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct UnitPath(String);

impl FromStr for DesignName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DesignName(s.to_string()))
    }
}

impl FromStr for VariantName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(VariantName(s.to_string()))
    }
}

impl FromStr for UnitPath {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(UnitPath(s.to_string()))
    }
}

impl Display for DesignName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Display for VariantName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Display for UnitPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}
