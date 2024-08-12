use clap::ValueEnum;
use crate::util::sorting::SortOrder;
use crate::planning::placement::PlacementSortingMode;

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