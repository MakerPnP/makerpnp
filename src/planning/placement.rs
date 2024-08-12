use thiserror::Error;
use std::fmt::{Display, Formatter};
use crate::util::sorting::SortOrder;
use crate::planning::reference::Reference;
use crate::pnp::object_path::UnitPath;
use crate::pnp::placement::Placement;

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

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum PlacementStatus {
    Known,
    Unknown,
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