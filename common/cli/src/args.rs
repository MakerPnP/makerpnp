use clap::ValueEnum;
use eda::EdaTool;
use planning::actions::{AddOrRemoveAction, SetOrClearAction};
use planning::placement::{PlacementOperation, PlacementSortingMode};
use planning::process::{TaskAction, TaskKind};
use pnp::pcb::PcbSide;
use util::sorting::SortOrder;

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
    Area,
    FeederReference,
    Height,
    Part,
    Pcb,
    PcbUnit,
    PcbUnitXY,
    PcbUnitYX,
    RefDes,
    // FUTURE add other modes, such as COST, PART, AREA, HEIGHT, REFDES, ANGLE, DESIGN_X, DESIGN_Y, PANEL_X, PANEL_Y, DESCRIPTION
    //        HEIGHT, AREA, REFDES are the most immediately useful
}

impl PlacementSortingModeArg {
    pub fn to_placement_sorting_mode(&self) -> PlacementSortingMode {
        match self {
            PlacementSortingModeArg::Area => PlacementSortingMode::Area,
            PlacementSortingModeArg::FeederReference => PlacementSortingMode::FeederReference,
            PlacementSortingModeArg::Height => PlacementSortingMode::Height,
            PlacementSortingModeArg::Part => PlacementSortingMode::Part,
            PlacementSortingModeArg::Pcb => PlacementSortingMode::Pcb,
            PlacementSortingModeArg::PcbUnit => PlacementSortingMode::PcbUnit,
            PlacementSortingModeArg::PcbUnitXY => PlacementSortingMode::PcbUnitXY,
            PlacementSortingModeArg::PcbUnitYX => PlacementSortingMode::PcbUnitYX,
            PlacementSortingModeArg::RefDes => PlacementSortingMode::RefDes,
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
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

#[derive(ValueEnum, Debug, Clone)]
pub enum AddOrRemoveOperationArg {
    Add,
    Remove,
}

impl From<AddOrRemoveOperationArg> for AddOrRemoveAction {
    fn from(value: AddOrRemoveOperationArg) -> Self {
        match value {
            AddOrRemoveOperationArg::Add => Self::Add,
            AddOrRemoveOperationArg::Remove => Self::Remove,
        }
    }
}

#[derive(ValueEnum, Debug, Clone)]
pub enum SetOrClearOperationArg {
    Set,
    Clear,
}

impl From<SetOrClearOperationArg> for SetOrClearAction {
    fn from(value: SetOrClearOperationArg) -> Self {
        match value {
            SetOrClearOperationArg::Set => Self::Set,
            SetOrClearOperationArg::Clear => Self::Clear,
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
    #[value(name("easyeda"))]
    EasyEda,
}

impl EdaToolArg {
    pub fn build(&self) -> EdaTool {
        match self {
            EdaToolArg::DipTrace => EdaTool::DipTrace,
            EdaToolArg::KiCad => EdaTool::KiCad,
            EdaToolArg::EasyEda => EdaTool::EasyEda,
        }
    }
}

#[derive(Clone, Debug)]
#[derive(ValueEnum)]
pub enum PlacementOperationArg {
    #[value(name("placed"))]
    Placed,
    #[value(name("skipped"))]
    Skipped,
    #[value(name("reset"))]
    Reset,
}

impl From<PlacementOperationArg> for PlacementOperation {
    fn from(value: PlacementOperationArg) -> Self {
        match value {
            PlacementOperationArg::Placed => Self::Place,
            PlacementOperationArg::Skipped => Self::Skip,
            PlacementOperationArg::Reset => Self::Reset,
        }
    }
}

#[derive(Clone, Debug)]
#[derive(ValueEnum)]
pub enum TaskActionArg {
    #[value(name("start"))]
    Start,
    #[value(name("complete"))]
    Complete,
    #[value(name("abandon"))]
    Abandon,
}

impl From<TaskActionArg> for TaskAction {
    fn from(value: TaskActionArg) -> Self {
        match value {
            TaskActionArg::Start => TaskAction::Start,
            TaskActionArg::Complete => TaskAction::Complete,
            TaskActionArg::Abandon => TaskAction::Abandon,
        }
    }
}

#[derive(Clone, Debug)]
#[derive(ValueEnum)]
pub enum TaskKindArg {
    /// Started, and then completed or abandoned, by the user, e.g. applying glue or wave soldering.
    #[value(name("generic"))]
    Generic,
    /// As 'generic', and the project's report lists the PCBs to load.
    #[value(name("load_pcbs"))]
    LoadPcbs,
    /// Places the phase's components, complete once every placement has been placed or skipped.  At most one per process.
    #[value(name("place_components"))]
    PlaceComponents,
}

impl From<TaskKindArg> for TaskKind {
    fn from(value: TaskKindArg) -> Self {
        match value {
            TaskKindArg::Generic => TaskKind::Generic,
            TaskKindArg::LoadPcbs => TaskKind::LoadPcbs,
            TaskKindArg::PlaceComponents => TaskKind::PlaceComponents,
        }
    }
}
