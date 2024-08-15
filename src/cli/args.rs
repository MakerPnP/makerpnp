use clap::ValueEnum;
use crate::eda::EdaTool;
use crate::planning::pcb::{PcbKind, PcbSide};
use crate::util::sorting::SortOrder;
use crate::planning::placement::{PlacementOperation, PlacementSortingMode};

/// Args decouple of CLI arg handling requirements from the internal data structures

#[derive(Debug, Clone)]
#[derive(ValueEnum)]
#[value(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SortOrderArg {
    Asc,
    Desc,
}

impl SortOrderArg {
    pub fn to_sort_order(&self) -> SortOrder {
        match self {
            SortOrderArg::Asc => SortOrder::Asc,
            SortOrderArg::Desc => SortOrder::Desc,
        }
    }
}

#[derive(Debug, Clone)]
#[derive(ValueEnum)]
#[value(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlacementSortingModeArg {
    FeederReference,
    PcbUnit,

    // FUTURE add other modes, such as COST, PART, AREA, HEIGHT, REFDES, ANGLE, DESIGN_X, DESIGN_Y, PANEL_X, PANEL_Y, DESCRIPTION
}

impl PlacementSortingModeArg {
    pub fn to_placement_sorting_mode(&self) -> PlacementSortingMode {
        match self {
            PlacementSortingModeArg::FeederReference => PlacementSortingMode::FeederReference,
            PlacementSortingModeArg::PcbUnit => PlacementSortingMode::PcbUnit,
        }
    }
}

#[derive(ValueEnum, Clone)]
#[value(rename_all = "lower")]
pub enum PcbSideArg {
    Top,
    Bottom,
}

impl From<PcbSideArg> for PcbSide {
    fn from(value: PcbSideArg) -> Self {
        match value {
            PcbSideArg::Top => Self::Top,
            PcbSideArg::Bottom => Self::Bottom,
        }
    }
}

#[derive(ValueEnum, Clone)]
#[value(rename_all = "lower")]
pub enum PcbKindArg {
    Single,
    Panel,
}

impl From<PcbKindArg> for PcbKind {
    fn from(value: PcbKindArg) -> Self {
        match value {
            PcbKindArg::Single => Self::Single,
            PcbKindArg::Panel => Self::Panel,
        }
    }
}

#[derive(Clone)]
#[derive(ValueEnum)]
pub enum EdaToolArg {
    #[value(name("diptrace"))]
    DipTrace,
    #[value(name("kicad"))]
    KiCad,
}

impl EdaToolArg {
    pub fn build(&self) -> EdaTool {
        match self {
            EdaToolArg::DipTrace => EdaTool::DipTrace,
            EdaToolArg::KiCad => EdaTool::KiCad,
        }
    }
}

#[derive(Clone)]
#[derive(ValueEnum)]
pub enum PlacementOperationArg {
    #[value(name("placed"))]
    Placed,
}

impl From<PlacementOperationArg> for PlacementOperation {
    fn from(value: PlacementOperationArg) -> Self {
        match value {
            PlacementOperationArg::Placed => Self::Placed,
        }
    }
}
