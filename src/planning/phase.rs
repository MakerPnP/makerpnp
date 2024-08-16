use thiserror::Error;
use crate::stores::load_out::LoadOutSource;
use crate::planning::reference::Reference;
use crate::planning::pcb::PcbSide;
use crate::planning::placement::PlacementSortingItem;
use crate::planning::process::Process;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phase {
    pub reference: Reference,
    pub process: Process,

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
    UnknownPhase(Reference)
}
