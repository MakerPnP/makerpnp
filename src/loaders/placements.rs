use rust_decimal::Decimal;
use crate::planning::PcbSide;
use crate::pnp::part::Part;
use crate::pnp::placement::Placement;

/// See `EdaPlacement` for details of co-ordinate system
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlacementRecord {
    pub ref_des: String,
    pub manufacturer: String,
    pub mpn: String,
    pub place: bool,
    pub pcb_side: PlacementRecordPcbSide,
    pub x: Decimal,
    pub y: Decimal,
    pub rotation: Decimal,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub enum PlacementRecordPcbSide {
    Top,
    Bottom,
}

impl From<&PlacementRecordPcbSide> for PcbSide {
    fn from(value: &PlacementRecordPcbSide) -> Self {
        match value {
            PlacementRecordPcbSide::Top => PcbSide::Top,
            PlacementRecordPcbSide::Bottom => PcbSide::Bottom,
        }
    }
}

impl From<&PcbSide> for PlacementRecordPcbSide {
    fn from(value: &PcbSide) -> Self {
        match value {
            PcbSide::Top => PlacementRecordPcbSide::Top,
            PcbSide::Bottom => PlacementRecordPcbSide::Bottom,
        }
    }
}

impl PlacementRecord {
    pub fn as_placement(&self) -> Placement {
        Placement {
            ref_des: self.ref_des.clone(),
            part: Part { manufacturer: self.manufacturer.clone(), mpn: self.mpn.clone() },
            place: self.place,
            pcb_side: PcbSide::from(&self.pcb_side),
            x: self.x,
            y: self.y,
            rotation: self.rotation,
        }
    }
}