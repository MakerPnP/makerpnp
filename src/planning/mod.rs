use std::collections::{BTreeMap, BTreeSet};
use std::collections::btree_map::Entry;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use thiserror::Error;
use tracing::info;
use crate::pnp::object_path::ObjectPath;
use crate::pnp::part::Part;
use crate::pnp::placement::Placement;

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
    
    #[serde_as(as = "Vec<(DisplayFromStr, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub placements: BTreeMap<ObjectPath, PlacementState>
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
#[derive(PartialEq, Eq)]
pub struct PartState {
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub applicable_processes: BTreeSet<Process>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct PlacementState {
    pub unit_path: UnitPath,
    pub placement: Placement,
    pub placed: bool,
    pub status: PlacementStatus,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub phase: Option<Reference>
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum PlacementStatus {
    Known,
    Unknown,
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
                info!("Unit assignment added. unit: '{}', design_variant: {}", unit_path, design_variant )
            }
            Entry::Occupied(mut entry) => {
                let old_value = entry.insert(design_variant.clone());
                info!("Unit assignment updated. unit: '{}', old: {}, new: {}", unit_path, old_value, design_variant )
            }
        }

        Ok(())
    }

    pub fn update_phase(&mut self, reference: Reference, process: Process, load_out: LoadOutSource, pcb_side: PcbSide) -> anyhow::Result<()> {

        ensure_load_out(&load_out)?;
        
        match self.phases.entry(reference.clone()) {
            Entry::Vacant(entry) => {
                let phase = Phase { reference: reference.clone(), process: process.clone(), load_out: load_out.clone(), pcb_side: pcb_side.clone(), sort_orderings: vec![] };
                entry.insert(phase);
                info!("Created phase. reference: '{}', process: {}, load_out: {:?}", reference, process, load_out);
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

fn ensure_load_out(load_out_source: &LoadOutSource) -> anyhow::Result<()> {
    let load_out_path_buf = PathBuf::from(load_out_source.to_string());
    let load_out_path = load_out_path_buf.as_path();
    if !load_out_path.exists() {
        File::create(&load_out_path)?;    
        info!("Created load-out. source: '{}'", load_out_source);
    }
    
    Ok(())
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_string(),
            unit_assignments: Default::default(),
            processes: vec![Process("pnp".to_string()), Process("manual".to_string())],
            part_states: Default::default(),
            phases: Default::default(),
            placements: Default::default(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phase {
    pub reference: Reference,
    pub process: Process,

    pub load_out: LoadOutSource,
    
    // TODO consider adding PCB unit + SIDE assignments to the phase instead of just a single side
    pub pcb_side: PcbSide,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub sort_orderings: Vec<PlacementSortingItem>
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DesignVariant {
    pub design_name: DesignName,
    pub variant_name: VariantName,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum PcbSide {
    Top,
    Bottom,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadOutSource(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DesignName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VariantName(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitPath(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reference(String);

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Process(String);

impl FromStr for LoadOutSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(LoadOutSource(s.to_string()))
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

impl FromStr for Process {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Process(s.to_string()))
    }
}

impl Display for LoadOutSource {
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

impl Display for Process {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Display for DesignVariant {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.design_name, self.variant_name)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SortOrder {
    Asc,
    Desc,
}

impl Display for SortOrder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asc=> write!(f, "Asc"),
            Self::Desc=> write!(f, "Desc"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlacementSortingMode {
    FeederReference,
    PcbUnit,

    // FUTURE add other modes, such as COST, PART, AREA, HEIGHT, REFDES, ANGLE, DESIGN_X, DESIGN_Y, PANEL_X, PANEL_Y, DESCRIPTION
}

impl Display for PlacementSortingMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FeederReference => write!(f, "FeederReference"),
            Self::PcbUnit => write!(f, "PcbUnit"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlacementSortingItem {
    pub mode: PlacementSortingMode,
    pub sort_order: SortOrder
}

#[derive(Error, Debug)]
pub enum PlacementSortingError {
    #[error("Invalid placement sorting path. value: '{0:}'")]
    Invalid(String)
}