use thiserror::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::collections::BTreeMap;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use tracing::trace;
use crate::planning::design::DesignVariant;
use crate::util::sorting::SortOrder;
use crate::planning::reference::Reference;
use crate::pnp::object_path::ObjectPath;
use crate::pnp::part::Part;
use crate::pnp::placement::Placement;
use crate::stores::placements::PlacementRecord;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct PlacementState {

    #[serde_as(as = "DisplayFromStr")]
    pub unit_path: ObjectPath,
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

pub fn load_placements(placements_path: PathBuf) -> Result<Vec<Placement>, csv::Error>{
    let mut csv_reader = csv::ReaderBuilder::new().from_path(placements_path)?;

    let records = csv_reader.deserialize().into_iter()
        .inspect(|record| {
            trace!("{:?}", record);
        })
        .filter_map(|record: Result<PlacementRecord, csv::Error> | {
            // TODO report errors
            match record {
                Ok(record) => Some(record.as_placement()),
                _ => None
            }
        })
        .collect();

    Ok(records)
}

pub fn load_all_placements(unique_design_variants: &[DesignVariant], path: &PathBuf) -> anyhow::Result<BTreeMap<DesignVariant, Vec<Placement>>> {
    let mut all_placements: BTreeMap<DesignVariant, Vec<Placement>> = Default::default();

    for design_variant in unique_design_variants {
        let DesignVariant { design_name: design, variant_name: variant } = design_variant;

        let mut placements_path = PathBuf::from(path);
        placements_path.push(format!("{}_{}_placements.csv", design, variant));

        let placements = load_placements(placements_path)?;
        let _ = all_placements.insert(design_variant.clone(), placements);
    }

    Ok(all_placements)
}

pub fn build_unique_parts(design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>) -> Vec<Part> {

    let mut unique_parts: Vec<Part> = vec![];
    for placements in design_variant_placement_map.values() {

        for record in placements {
            if !unique_parts.contains(&record.part) {
                unique_parts.push(record.part.clone());
            }
        }
    }

    unique_parts
}

pub enum PlacementOperation {
    Placed
}