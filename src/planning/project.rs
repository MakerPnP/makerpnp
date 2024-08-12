use std::collections::btree_map::Entry;
use tracing::info;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use std::collections::BTreeMap;
use crate::stores::load_out;
use crate::stores::load_out::LoadOutSource;
use crate::planning::design::DesignVariant;
use crate::planning::reference::Reference;
use crate::planning::part::PartState;
use crate::planning::pcb::{Pcb, PcbSide};
use crate::planning::phase::Phase;
use crate::planning::placement::PlacementState;
use crate::planning::process::Process;
use crate::pnp::object_path::{ObjectPath, UnitPath};
use crate::pnp::part::Part;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Project {
    pub name: String,

    pub processes: Vec<Process>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub pcbs: Vec<Pcb>,

    // TODO consider using ObjectPath instead of UnitPath here?
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub unit_assignments: BTreeMap<UnitPath, DesignVariant>,

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

        load_out::ensure_load_out(&load_out)?;
        
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

impl Default for Project {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_string(),
            processes: vec![Process::new("pnp"), Process::new("manual")],
            pcbs: vec![],
            unit_assignments: Default::default(),
            part_states: Default::default(),
            phases: Default::default(),
            placements: Default::default(),
        }
    }
}