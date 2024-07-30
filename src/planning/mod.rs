use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use serde_with::serde_as;
use tracing::info;
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
    pub part_states: BTreeMap<Part, PartState>,

    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub phases: BTreeMap<Reference, Phase>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[derive(PartialEq, Eq)]
pub struct PartState {
    pub process: ProcessAssignment,
    pub load_out: LoadOutAssignment,
}

impl Default for PartState {
    fn default() -> Self {
        Self {
            process: ProcessAssignment::Unassigned,
            load_out: LoadOutAssignment::Unassigned,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialOrd, Ord)]
#[derive(PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Process {
    Pnp
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[derive(PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessAssignment {
    Unassigned,
    Assigned(Process),
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[derive(PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoadOutAssignment {
    Unassigned,
    Assigned(LoadOutName),
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
                info!("Unit assignment added. unit: {}, design_variant: {}", unit_path, design_variant )
            }
            Entry::Occupied(mut entry) => {
                let old_value = entry.insert(design_variant.clone());
                info!("Unit assignment updated. unit: {}, old: {}, new: {}", unit_path, old_value, design_variant )
            }
        }

        Ok(())
    }

    pub fn update_phase(&mut self, reference: Reference, process: Process, load_out: Option<LoadOutName>) -> anyhow::Result<()> {

        match self.phases.entry(reference.clone()) {
            Entry::Vacant(entry) => {
                let phase = Phase { reference: reference.clone(), process: process.clone(), load_out: load_out.clone() };
                entry.insert(phase);
                info!("Created phase. reference: '{}', process: {:?}, load_out: {:?}", reference, process, load_out);
            }
            Entry::Occupied(mut entry) => {
                let existing_phase = entry.get_mut();
                let old_phase = existing_phase.clone();

                existing_phase.process = process;
                existing_phase.load_out = load_out;

                info!("Updated phase. old: {:?}, new: {:?}", old_phase, existing_phase);
            }
        }

        Ok(())
    }
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_string(),
            unit_assignments: Default::default(),
            processes: vec![Process::Pnp],
            part_states: Default::default(),
            phases: Default::default(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phase {
    reference: Reference,
    process: Process,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    load_out: Option<LoadOutName>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct DesignVariant {
    pub design_name: DesignName,
    pub variant_name: VariantName,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadOutName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct DesignName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct VariantName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitPath(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reference(String);

impl FromStr for LoadOutName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(LoadOutName(s.to_string()))
    }
}

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

impl Display for LoadOutName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
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
