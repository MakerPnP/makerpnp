use clap::ValueEnum;
use eda::EdaTool;
use pnp::pcb::{PcbKind, PcbSide};
use util::sorting::SortOrder;
use planning::placement::{PlacementOperation, PlacementSortingMode};
use planning::process::{ProcessOperationKind, ProcessOperationSetItem};

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

#[derive(Clone)]
#[derive(ValueEnum)]
pub enum ProcessOperationArg {
    #[value(name("loadpcbs"))]
    LoadPcbs,
    #[value(name("automatedpnp"))]
    AutomatedPnp,
    #[value(name("reflowcomponents"))]
    ReflowComponents,
    #[value(name("manuallysoldercomponents"))]
    ManuallySolderComponents,
}

impl From<ProcessOperationArg> for ProcessOperationKind {
    fn from(value: ProcessOperationArg) -> Self {
        match value {
            ProcessOperationArg::LoadPcbs => ProcessOperationKind::LoadPcbs,
            ProcessOperationArg::AutomatedPnp => ProcessOperationKind::AutomatedPnp,
            ProcessOperationArg::ReflowComponents => ProcessOperationKind::ReflowComponents,
            ProcessOperationArg::ManuallySolderComponents => ProcessOperationKind::ManuallySolderComponents,
        }
    }
}

#[cfg(test)]
mod from_process_operation_arg_for_process_operation_kind_tests {
    use rstest::rstest;
    use super::ProcessOperationArg;
    use planning::process::ProcessOperationKind;

    #[rstest]
    #[case(ProcessOperationArg::LoadPcbs, ProcessOperationKind::LoadPcbs)]
    #[case(ProcessOperationArg::AutomatedPnp, ProcessOperationKind::AutomatedPnp)]
    #[case(ProcessOperationArg::ReflowComponents, ProcessOperationKind::ReflowComponents)]
    #[case(ProcessOperationArg::ManuallySolderComponents, ProcessOperationKind::ManuallySolderComponents)]
    pub fn from(#[case] arg: ProcessOperationArg, #[case] expected_kind: ProcessOperationKind) {
        // expect 
        assert_eq!(ProcessOperationKind::from(arg), expected_kind)
    }
}

#[derive(Clone)]
#[derive(ValueEnum)]
pub enum ProcessOperationSetArg {
    #[value(name("completed"))]
    Completed,
}

impl From<ProcessOperationSetArg> for ProcessOperationSetItem {
    fn from(value: ProcessOperationSetArg) -> Self {
        match value {
            ProcessOperationSetArg::Completed => ProcessOperationSetItem::Completed
        }
    }
}