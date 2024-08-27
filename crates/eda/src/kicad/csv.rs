use rust_decimal::Decimal;
use thiserror::Error;
use pnp::pcb::PcbSide;
use crate::placement::{EdaPlacement, EdaPlacementField};

#[derive(Error, Debug)]
pub enum KiCadPlacementRecordError {
    #[error("Unknown")]
    Unknown
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct KiCadPlacementRecord {
    #[serde(rename(deserialize = "ref"))]
    ref_des: String,
    package: String,
    val: String,
    side: KiCadPcbSide,
    x: Decimal,
    y: Decimal,
    /// Positive values indicate anti-clockwise rotation
    /// Range is >-180 to +180.
    /// No rounding.
    /// Values are truncated to 3 decimal places in the UI.
    rotation: Decimal,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all(deserialize = "lowercase"))]
enum KiCadPcbSide {
    Top,
    Bottom,
}

impl From<&KiCadPcbSide> for PcbSide {
    fn from(value: &KiCadPcbSide) -> Self {
        match value {
            KiCadPcbSide::Top => PcbSide::Top,
            KiCadPcbSide::Bottom => PcbSide::Bottom,
        }
    }
}

impl KiCadPlacementRecord {
    pub fn build_eda_placement(&self) -> Result<EdaPlacement, KiCadPlacementRecordError> {
        Ok(EdaPlacement {
            ref_des: self.ref_des.to_string(),
            place: true,
            fields: vec![
                EdaPlacementField { name: "package".to_string(), value: self.package.to_string() },
                EdaPlacementField { name: "val".to_string(), value: self.val.to_string() },
            ],
            pcb_side: PcbSide::from(&self.side),
            x: self.x,
            y: self.y,
            // TODO normalize rotation in case kicad uses values outside it's expected range.
            rotation: self.rotation,
        })

        // _ => Err(KiCadPlacementRecordError::Unknown)
    }
}
