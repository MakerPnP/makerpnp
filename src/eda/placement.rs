use rust_decimal::Decimal;
use crate::planning::PcbSide;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaPlacementField {
    pub name: String,
    // FUTURE if there's a requirement to store other EDA specific data types other than String, perhaps implement an enum named EdaPlacementValue.
    pub value: String,
}

impl EdaPlacementField {
    pub fn new(name: String, value: String) -> Self {
        Self {
            name,
            value,
        }
    }
}

/// Uses right-handed cartesian coordinate system
/// See https://en.wikipedia.org/wiki/Cartesian_coordinate_system
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaPlacement {
    pub ref_des: String,
    pub place: bool,
    pub fields: Vec<EdaPlacementField>,
    pub pcb_side: PcbSide,
    
    /// Positive = Right
    pub x: Decimal,
    /// Positive = Up
    pub y: Decimal,
    /// Positive values indicate anti-clockwise rotation
    /// Range is >-180 to +180.
    pub rotation: Decimal,
}

impl Default for EdaPlacement {
    fn default() -> Self {
        Self {
            ref_des: "".to_string(),
            place: false,
            fields: vec![],
            pcb_side: PcbSide::Top,
            x: Default::default(),
            y: Default::default(),
            rotation: Default::default(),
        }
    }
}