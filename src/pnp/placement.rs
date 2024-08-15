use rust_decimal::Decimal;
use crate::planning::pcb::PcbSide;
use crate::pnp::part::Part;

/// Uses right-handed cartesian coordinate system
/// See https://en.wikipedia.org/wiki/Cartesian_coordinate_system
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct Placement {
    pub ref_des: String,
    pub part: Part,
    pub place: bool,
    pub pcb_side: PcbSide,
    
    /// Positive = Right
    pub x: Decimal,
    /// Positive = Up
    pub y: Decimal,
    /// Positive values indicate anti-clockwise rotation
    /// Range is >-180 to +180.
    pub rotation: Decimal,
}
