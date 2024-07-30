use std::collections::{BTreeMap};
use std::collections::btree_map::Entry;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use serde_with::serde_as;
use tracing::trace;
use crate::pnp::part::Part;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Project {
    pub name: String,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub unit_assignments: BTreeMap<UnitPath, DesignVariant>,
    pub processes: Vec<Process>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub process_part_assignments: BTreeMap<Part, ProcessAssignment>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub phases: Vec<Phase>,
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
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }


    pub fn update_assignment(&mut self, unit_path: UnitPath, design_variant: DesignVariant) -> anyhow::Result<()> {
        match self.unit_assignments.entry(unit_path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(design_variant.clone());
                trace!("Unit assignment added. unit: {}, design_variant: {}", unit_path, design_variant )
            }
            Entry::Occupied(mut entry) => { 
                let old_value = entry.insert(design_variant.clone());
                trace!("Unit assignment updated. unit: {}, old: {}, new: {}", unit_path, old_value, design_variant )
            }
        }         
        
        Ok(())
    }

    pub fn add_phase(&mut self, reference: Reference, process: Process) -> anyhow::Result<()> {
        let phase = Phase { reference, process };

        // TODO check to see if phase already exists
        self.phases.push(phase);
        
        Ok(())
    }
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_string(),
            unit_assignments: Default::default(),
            processes: vec![Process::Pnp],
            process_part_assignments: Default::default(),
            phases: vec![],
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Phase {
    reference: Reference,
    process: Process,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct DesignVariant {
    pub design_name: DesignName,
    pub variant_name: VariantName,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct DesignName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct VariantName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitPath(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct Reference(String);

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

impl FromStr for Reference {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Reference(s.to_string()))
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

impl Display for Reference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Display for DesignVariant {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.design_name, self.variant_name)
    }
    
}
