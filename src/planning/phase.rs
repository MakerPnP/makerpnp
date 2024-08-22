use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use indexmap::IndexSet;
use thiserror::Error;
use crate::stores::load_out::LoadOutSource;
use crate::planning::reference::Reference;
use crate::planning::pcb::PcbSide;
use crate::planning::placement::PlacementSortingItem;
use crate::planning::process::{Process, ProcessName, ProcessOperationKind, ProcessOperationState};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phase {
    pub reference: Reference,
    pub process: ProcessName,

    pub load_out: LoadOutSource,
    
    // TODO consider adding PCB unit + SIDE assignments to the phase instead of just a single side
    pub pcb_side: PcbSide,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub placement_orderings: Vec<PlacementSortingItem>
}

#[derive(Error, Debug)]
pub enum PhaseError {
    #[error("Unknown phase. phase: '{0:}'")]
    UnknownPhase(Reference),
    
    #[error("Invalid operation for phase. phase: '{0:}', operation: {1:?}")]
    InvalidOperationForPhase(Reference, ProcessOperationKind),
}

pub struct PhaseOrderings<'a>(pub &'a IndexSet<Reference>);

impl<'a> Display for PhaseOrderings<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "['{}']", self.0.iter().map(Reference::to_string).collect::<Vec<String>>().join("', '"))
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct PhaseState {
    pub operation_state: BTreeMap<ProcessOperationKind, ProcessOperationState>
}

impl PhaseState {
    pub fn from_process(process: &Process) -> Self {

        let mut operation_state = BTreeMap::new();
        
        for process_kind in process.operations.iter() {
            operation_state.insert(process_kind.clone(), ProcessOperationState::default());
        }
        
        Self {
            operation_state,
        }
    }
}
