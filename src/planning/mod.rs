use std::collections::{BTreeMap};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use serde_with::serde_as;
use crate::pnp::part::Part;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Project {
    pub name: String,
    pub unit_assignments: Vec<UnitAssignment>,
    pub processes: Vec<Process>,

    #[serde_as(as = "Vec<(_, _)>")]
    pub process_part_assignments: BTreeMap<Part, ProcessAssignment>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[derive(Hash, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Process {
    Pnp
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[derive(Hash, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
//#[serde(try_from = "String")]
pub enum ProcessAssignment {
    Unassigned,
    Assigned(Process),
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
            processes: vec![Process::Pnp],
            process_part_assignments: Default::default(),
        }
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct UnitAssignment {
    pub unit_path: UnitPath,
    pub design_name: DesignName,
    pub variant_name: VariantName,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct DesignName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct VariantName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
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
